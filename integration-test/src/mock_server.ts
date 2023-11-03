import https from 'https';
import http from 'http';
import express, { Express } from 'express';
import { Send, Request, Response, Query } from 'express-serve-static-core';
import { Server } from 'http';
import { Socket } from 'net';
import { delay } from './utils';

export interface TypedRequest<T extends Query, U> extends Request {
    body: U;
    query: T;
}

export interface TypedResponse<ResBody> extends Response {
    json: Send<ResBody, this>;
}

export interface Certificates {
    cert: string;
    key: string;
}

export class MockServer {
    app: Express = undefined!;
    server: Server = undefined!;
    connections: Socket[] = [];

    constructor(
        public readonly name: string,
        public readonly port: number,
        public readonly tls?: Certificates
    ) {}

    public get isRunning(): boolean {
        return this.app !== undefined;
    }

    protected log(message: string) {
        console.log(`[${this.name}] ${message}`);
    }

    public async start(): Promise<MockServer> {
        if (this.isRunning) {
            throw new Error('Server has already been started');
        }

        this.log('Starting application');
        this.app = express();
        this.log('Init server');
        this.initCommon();
        this.init();
        this.log('Start listening...');
        if (this.tls) {
            this.server = await https.createServer(this.tls, this.app).listen(this.port);
        } else {
            this.server = await http.createServer(this.app).listen(this.port);
        }

        // keep track of the open connections
        this.server.on('connection', (connection) => {
            this.connections.push(connection);
            connection.on('close', () => {
                // Remove closed connection from the array
                const index = this.connections.indexOf(connection);
                if (index !== -1) {
                    this.connections.splice(index, 1);
                }
            });
        });

        this.log('Server started.');

        return this;
    }

    public async stop() {
        this.log('Stopping server ...');
        await this.server?.close();
        await delay(100);
        this.log(`Closing open(${this.connections.length}) connections ...`);
        for (const connection of this.connections) {
            await connection.end();
        }
        this.server = undefined!;
        this.app = undefined!;
        await delay(100);
        this.log('Server stopped.');
    }

    private initCommon() {
        this.app.use((err: any, _req: any, _res: any, _next: any) => {
            this.log(err.stack);
            throw err;
        });

        this.app.use(async (req: TypedRequest<any, any>, res: TypedResponse<any>, next) => {
            this.log(`[${req.method}] ${req.url} ...`);
            await next();
            this.log(`[${req.method}] ${req.url} completed.`);
        });
    }

    /// Override in extends to implement the logic
    protected init() {}
}
