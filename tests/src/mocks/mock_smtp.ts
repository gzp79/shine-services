import debug from 'debug';
import { ParsedMail, simpleParser } from 'mailparser';
import { SMTPServer } from 'smtp-server';

const log = debug(`test:mock:smtp`);

class MockSmtp {
    private server: SMTPServer;
    private checkMail?: (mail: ParsedMail) => void;

    constructor(public readonly port: number = 2525) {
        this.server = new SMTPServer({
            onData: (stream, _session, callback) => {
                simpleParser(stream, (err, parsed) => {
                    if (err) {
                        log('Error parsing email:', err);
                    } else {
                        log('Email received:', parsed);
                        this.checkMail?.(parsed);
                    }
                });
                return callback(null);
            },

            authOptional: true,
            onAuth: (auth, _session, callback) => {
                log('Auth:', auth);
                return callback(null, { user: auth.username });
            },

            disabledCommands: ['STARTTLS']
            /*secure: true,
            key: CERTIFICATES.key,
            cert: CERTIFICATES.cert,
            rejectUnauthorized: false*/
        });
    }

    async start(checkMail: (mail: ParsedMail) => void): Promise<void> {
        if (this.checkMail !== undefined) {
            throw new Error('SMTP Mock Server has already been started');
        }
        this.checkMail = checkMail;
        await new Promise<void>((resolve) => {
            this.server.listen(this.port, () => {
                this.checkMail = checkMail;
                log(`SMTP Mock Listening on port ${this.port}.`);
                resolve();
            });
        });

        log(`SMTP Mock Server started.`);
    }

    async stop(): Promise<void> {
        return new Promise((resolve) => {
            this.server.close(() => {
                this.checkMail = undefined;
                log('SMTP Mock Server stopped.');
                resolve();
            });
        });
    }
}

export default MockSmtp;
