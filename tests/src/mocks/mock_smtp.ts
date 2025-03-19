/* eslint-disable @typescript-eslint/no-explicit-any */
import debug from 'debug';
import { ParsedMail, simpleParser } from 'mailparser';
import { Logger, LoggerLevel } from 'nodemailer/lib/shared';
import { SMTPServer } from 'smtp-server';

const log = debug(`test:mock:smtp`);

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

class MockSmtp {
    private server: SMTPServer;
    private isStarted = false;
    private resolveWaitMail?: (mail: ParsedMail) => void;
    private rejectWaitMail?: (reason?: any) => void;
    private onMailReceived?: (mail: ParsedMail) => void;

    constructor(public readonly port: number = 2525) {
        this.server = new SMTPServer({
            logger: new MockSmtpLogger(),

            onData: (stream, _session, callback) => {
                simpleParser(stream, (err, parsed) => {
                    if (err) {
                        log('Error parsing email:', err);
                        this.rejectWaitMail?.(err);
                    } else {
                        this.onMailReceived?.(parsed);
                        this.resolveWaitMail?.(parsed);
                    }
                });
                return callback(null);
            },

            authOptional: true,
            onAuth: (auth, _session, callback) => {
                return callback(null, { user: auth.username });
            },

            disabledCommands: ['STARTTLS'],
            /*secure: true,
            key: CERTIFICATES.key,
            cert: CERTIFICATES.cert,
            rejectUnauthorized: false*/
            closeTimeout: 100
        });
    }

    async start(): Promise<void> {
        if (this.isRunning) {
            log('SMTP Mock Server already running.');
            return;
        }

        await new Promise<void>((resolve) => {
            this.server.listen(this.port, () => {
                this.isStarted = true;
                resolve();
            });
        });

        log(`SMTP Mock Server started.`);
    }

    async stop(): Promise<void> {
        await new Promise<void>((resolve) => {
            log('SMTP Mock stopping...');

            this.server.close(() => {
                this.rejectWaitMail?.(new Error('Mail server stopped.'));
                this.isStarted = false;
                resolve();
            });
        });

        log('SMTP Mock stopped.');
    }

    get isRunning(): boolean {
        return this.isStarted;
    }

    async waitMail(): Promise<ParsedMail> {
        if (!this.isStarted) throw new Error('SMTP Mock Server is not running.');
        log('Waiting for email...');
        return new Promise((resolve, reject) => {
            this.resolveWaitMail = (mail) => {
                resolve(mail);
                this.resolveWaitMail = undefined;
                this.rejectWaitMail = undefined;
            };
            this.rejectWaitMail = (reason?: any) => {
                reject(reason);
                this.resolveWaitMail = undefined;
                this.rejectWaitMail = undefined;
            };
        });
    }

    onMail(callback: (mail: ParsedMail) => void) {
        this.onMailReceived = callback;
    }
}

export default MockSmtp;
