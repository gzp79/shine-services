/* eslint-disable @typescript-eslint/no-explicit-any */
import debug from 'debug';
import { ParsedMail, simpleParser } from 'mailparser';
import { Logger, LoggerLevel } from 'nodemailer/lib/shared';
import { SMTPServer } from 'smtp-server';

const log = debug('test:mock:smtp');

/* prettier-ignore */
class MockSmtpLogger implements Logger {
    level(_level: LoggerLevel): void {}

    trace(..._params: any[]): void { /*this.log('TRACE', params);*/ }
    debug(..._params: any[]): void { /*this.log('DEBUG',  params);*/ }
    info(...params: any[]): void { this.log('INFO', params); }
    warn(...params: any[]): void { this.log('WARN', params); }
    error(...params: any[]): void { this.log('ERROR', params); }
    fatal(...params: any[]): void { this.log('FATAL', params); }

    private log(level: string, params: any[]): void {
        const [_entry, message, ...args] = params;
        log(`[${level}] ` + message, ...args);
    }
}

interface WaitMailOptions {
    timeout?: number;
    predicate?: (mail: ParsedMail) => boolean;
}

interface WaitEmailsOptions {
    timeout?: number;
    totalTimeout?: number;
}

interface ExpectNoMailOptions {
    timeout?: number;
}

interface PendingWait {
    resolve: (mail: ParsedMail) => void;
    reject: (reason: any) => void;
    predicate?: (mail: ParsedMail) => boolean;
    timeoutId?: NodeJS.Timeout;
}

class MockSmtp {
    private server: SMTPServer = undefined!;
    private isStarted = false;

    // Queue-based promise handling (FIFO)
    private pendingWaits: PendingWait[] = [];

    // Event listeners (multiple callbacks supported)
    private mailListeners: Array<(mail: ParsedMail) => void> = [];

    private static readonly DEFAULT_TIMEOUT = 5000;
    private static readonly DEFAULT_NO_MAIL_TIMEOUT = 500;

    constructor(public readonly port: number = 2525) {}

    private createServer(): SMTPServer {
        return new SMTPServer({
            logger: new MockSmtpLogger(),

            onData: (stream, _session, callback) => {
                simpleParser(stream, (err, parsed) => {
                    if (err) {
                        log('Error parsing email:', err);
                        this.rejectAllPendingWaits(err);
                        return callback(err);
                    }

                    try {
                        this.handleIncomingMail(parsed);
                        return callback(null);
                    } catch (error: any) {
                        log('Error in onMail callback:', error);
                        return callback(new Error(error.message || 'SMTP rejection'));
                    }
                });
            },

            authOptional: true,
            onAuth: (auth, _session, callback) => {
                return callback(null, { user: auth.username });
            },

            disabledCommands: ['STARTTLS'],
            closeTimeout: 100
        });
    }

    async start(): Promise<void> {
        if (this.isRunning) {
            log('SMTP Mock Server already running.');
            return;
        }

        this.server = this.createServer();
        await new Promise<void>((resolve) => {
            this.server.listen(this.port, () => {
                this.isStarted = true;
                resolve();
            });
        });

        log('SMTP Mock Server started.');
    }

    async stop(): Promise<void> {
        await new Promise<void>((resolve) => {
            log('SMTP Mock stopping...');

            this.server.close(() => {
                const waits = [...this.pendingWaits];
                this.pendingWaits = [];
                waits.forEach((wait) => {
                    if (wait.timeoutId) {
                        clearTimeout(wait.timeoutId);
                    }
                    wait.reject(new Error('Mail server stopped.'));
                });
                this.isStarted = false;
                resolve();
            });
        });

        log('SMTP Mock stopped.');
    }

    get isRunning(): boolean {
        return this.isStarted;
    }

    /**
     * Wait for next email (queued if multiple calls)
     * @throws on timeout or if predicate never matches
     */
    async waitMail(opts?: WaitMailOptions): Promise<ParsedMail> {
        if (!this.isStarted) {
            throw new Error('SMTP Mock Server is not running.');
        }

        const timeout = opts?.timeout ?? MockSmtp.DEFAULT_TIMEOUT;
        const predicate = opts?.predicate;

        log(`Waiting for email${predicate ? ' (with predicate)' : ''}...`);

        return new Promise((resolve, reject) => {
            const timeoutId = setTimeout(() => {
                const index = this.pendingWaits.findIndex((w) => w.timeoutId === timeoutId);
                if (index >= 0) {
                    this.pendingWaits.splice(index, 1);
                }
                reject(new Error(`Timeout waiting for email after ${timeout}ms`));
            }, timeout);

            this.pendingWaits.push({
                resolve,
                reject,
                predicate,
                timeoutId
            });
        });
    }

    /**
     * Wait for multiple emails in order
     */
    async waitEmails(count: number, opts?: WaitEmailsOptions): Promise<ParsedMail[]> {
        if (count <= 0) {
            throw new Error('Count must be positive');
        }

        const timeout = opts?.timeout ?? MockSmtp.DEFAULT_TIMEOUT;
        const totalTimeout = opts?.totalTimeout;

        const results: ParsedMail[] = [];
        const startTime = Date.now();

        for (let i = 0; i < count; i++) {
            let remainingTimeout = timeout;

            if (totalTimeout) {
                const elapsed = Date.now() - startTime;
                remainingTimeout = Math.min(timeout, totalTimeout - elapsed);

                if (remainingTimeout <= 0) {
                    throw new Error(`Total timeout exceeded waiting for ${count} emails (got ${i})`);
                }
            }

            const mail = await this.waitMail({ timeout: remainingTimeout });
            results.push(mail);
        }

        return results;
    }

    /**
     * Assert NO email arrives within timeout
     * @throws if any email received
     */
    async expectNoMail(opts?: ExpectNoMailOptions): Promise<void> {
        const timeout = opts?.timeout ?? MockSmtp.DEFAULT_NO_MAIL_TIMEOUT;

        log(`Expecting no email for ${timeout}ms...`);

        return new Promise((resolve, reject) => {
            const timeoutId = setTimeout(() => {
                // Remove from pending waits
                const index = this.pendingWaits.findIndex((w) => w.timeoutId === timeoutId);
                if (index >= 0) {
                    this.pendingWaits.splice(index, 1);
                }
                log('No email received as expected.');
                resolve();
            }, timeout);

            this.pendingWaits.push({
                resolve: (mail) => {
                    clearTimeout(timeoutId);
                    reject(new Error(`Unexpected email received: ${mail.subject}`));
                },
                reject: (reason) => {
                    clearTimeout(timeoutId);
                    reject(reason);
                },
                timeoutId
            });
        });
    }

    /**
     * Listen to all incoming emails
     * Returns cleanup function
     */
    onMail(callback: (mail: ParsedMail) => void): () => void {
        this.mailListeners.push(callback);
        return () => this.offMail(callback);
    }

    /**
     * Fire once then auto-remove
     */
    onceMail(callback: (mail: ParsedMail) => void): () => void {
        const wrapper = (mail: ParsedMail) => {
            this.offMail(wrapper);
            callback(mail);
        };
        this.mailListeners.push(wrapper);
        return () => this.offMail(wrapper);
    }

    /**
     * Remove specific callback
     */
    offMail(callback: (mail: ParsedMail) => void): void {
        const index = this.mailListeners.indexOf(callback);
        if (index >= 0) {
            this.mailListeners.splice(index, 1);
        }
    }

    /**
     * Clear pending waitMail() promises (rejects them)
     */
    clearPendingWaits(): void {
        const waits = [...this.pendingWaits];
        this.pendingWaits = [];

        waits.forEach((wait) => {
            if (wait.timeoutId) {
                clearTimeout(wait.timeoutId);
            }
            wait.reject(new Error('Pending waits cleared'));
        });

        if (waits.length > 0) {
            log(`Cleared ${waits.length} pending waits.`);
        }
    }

    /**
     * Full reset (clear waits + remove all callbacks)
     */
    reset(): void {
        this.clearPendingWaits();
        this.mailListeners = [];
        log('MockSmtp reset.');
    }

    /**
     * Handle incoming email: fire callbacks then resolve pending waits
     */
    private handleIncomingMail(mail: ParsedMail): void {
        log(`Email received: ${mail.subject}`);

        // 1. Fire all callbacks (non-blocking, may throw)
        for (const listener of this.mailListeners) {
            try {
                listener(mail);
            } catch (error: any) {
                log('Error in mail listener:', error);
                // Rethrow to reject SMTP connection
                throw error;
            }
        }

        // 2. Find first matching pending wait (FIFO with predicate)
        const index = this.pendingWaits.findIndex((wait) => !wait.predicate || wait.predicate(mail));

        if (index >= 0) {
            const wait = this.pendingWaits.splice(index, 1)[0];
            if (wait.timeoutId) {
                clearTimeout(wait.timeoutId);
            }
            wait.resolve(mail);
            log('Resolved pending wait.');
        } else {
            log('No matching pending wait for this email.');
        }
    }

    /**
     * Reject all pending waits with error
     */
    private rejectAllPendingWaits(reason: any): void {
        const waits = [...this.pendingWaits];
        this.pendingWaits = [];

        waits.forEach((wait) => {
            if (wait.timeoutId) {
                clearTimeout(wait.timeoutId);
            }
            wait.reject(reason);
        });
    }
}

export default MockSmtp;
