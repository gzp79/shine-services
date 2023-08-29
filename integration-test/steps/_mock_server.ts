import express, { Express } from 'express';
import { Send, Request, Response, Query } from 'express-serve-static-core';
import { Server, createServer } from 'http';

export type MockServerLogger = (message: string, mime?: string) => void;

// TypedRequest, TypedResponse based on https://plainenglish.io/blog/typed-express-request-and-response-with-typescript

export interface TypedRequest<T extends Query, U> extends Request {
    body: U;
    query: T;
}

export interface TypedResponse<ResBody> extends Response {
    json: Send<ResBody, this>;
}

export class MockServer {
    logger?: MockServerLogger;
    app: Express = undefined!;
    server: Server = undefined!;

    constructor(
        public readonly name: string,
        public readonly port: number
    ) {}

    public get isRunning(): boolean {
        return this.app !== undefined;
    }

    protected log(message: string) {
        if (this.logger) {
            this.logger(`[${this.name}] ${message}`);
        } else {
            console.log(`[${this.name}] ${message}`);
        }
    }

    public async start(logger?: MockServerLogger) {
        if (this.isRunning) {
            throw new Error('Server has already been started');
        }

        this.logger = logger;
        this.log('Starting application');
        this.app = express();
        this.log('Init server');
        this.initCommon();
        this.init();
        this.log('Start listening...');
        this.server = await createServer(this.app).listen(this.port);
        this.log('Server started.');
    }

    private initCommon() {
        this.app.use((err: any, _req: any, _res: any, _next: any) => {
            this.log(err.stack);
            throw err;
        });

        this.app.use(async (req: TypedRequest<any,any>, res: TypedResponse<any>, next) => {
            this.log(`[${req.method}] ${req.url} ...`);
            await next();
            this.log(`[${req.method}] ${req.url} completed.`);
        });
    }

    /// Override in extends to implement the logic
    protected init() {}

    public async stop() {
        this.log('Stopping server.');
        await this.server?.close();
        this.server = undefined!;
        this.app = undefined!;
        this.log('Stopping stopped.');
    }
}
