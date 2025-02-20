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
    private _isRunning = false;
    private mailReceived: Promise<ParsedMail>;
    private resolveMailReceived?: (mail: ParsedMail) => void;
    private rejectMailReceived?: (reason?: any) => void;

    constructor(public readonly port: number = 2525) {
        this.server = new SMTPServer({
            logger: new MockSmtpLogger(),

            onData: (stream, _session, callback) => {
                simpleParser(stream, (err, parsed) => {
                    if (err) {
                        log('Error parsing email:', err);
                        this.rejectMailReceived?.(err);
                    } else {
                        this.resolveMailReceived?.(parsed);
                    }
                    this.mailReceived = new Promise((resolve, reject) => {
                        this.resolveMailReceived = resolve;
                        this.rejectMailReceived = reject;
                    });
                });
                return callback(null);
            },

            authOptional: true,
            onAuth: (auth, _session, callback) => {
                return callback(null, { user: auth.username });
            },

            disabledCommands: ['STARTTLS']
            /*secure: true,
            key: CERTIFICATES.key,
            cert: CERTIFICATES.cert,
            rejectUnauthorized: false*/
        });

        this.mailReceived = new Promise((resolve, reject) => {
            this.resolveMailReceived = resolve;
            this.rejectMailReceived = reject;
        });
    }

    async start(): Promise<void> {
        if (this._isRunning) {
            log('SMTP Mock Server already running.');
            return;
        }

        await new Promise<void>((resolve) => {
            this.server.listen(this.port, () => {
                this._isRunning = true;
                resolve();
            });
        });

        log(`SMTP Mock Server started.`);
    }

    async stop(): Promise<void> {
        await new Promise<void>((resolve) => {
            log('SMTP Mock stopping...');

            this.server.close(() => {
                this.rejectMailReceived?.(new Error('Mail server stopped.'));
                this.rejectMailReceived = undefined;
                this.resolveMailReceived = undefined;
                this._isRunning = false;
                resolve();
            });
        });

        log('SMTP Mock stopped.');
    }

    get isRunning(): boolean {
        return this._isRunning;
    }

    async waitMail(): Promise<ParsedMail> {
        if (!this._isRunning) throw new Error('SMTP Mock Server is not running.');
        log('Waiting for email...');
        const mail = await this.mailReceived;
        return mail;
    }
}

export default MockSmtp;
