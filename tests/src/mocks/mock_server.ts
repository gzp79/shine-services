/* eslint-disable @typescript-eslint/no-explicit-any */
import cors from 'cors';
import { debug } from 'debug';
import express, { Express } from 'express';
import { NextFunction, Request, RequestHandler, Response } from 'express-serve-static-core';
import http from 'http';
import { Server } from 'http';
import https from 'https';
import { Socket } from 'net';
import { joinURL } from '../utils';

export interface Certificates {
    cert: string;
    key: string;
}

export class MockServer {
    protected app: Express = undefined!;
    protected server: Server = undefined!;
    protected connections: Socket[] = [];
    protected port: string;
    protected readonly log: debug.Debugger;

    constructor(
        public readonly name: string,
        public readonly baseUrl: URL,
        public readonly tls?: Certificates
    ) {
        if (this.baseUrl.port === '' || this.baseUrl.port === '0' || this.baseUrl.port === undefined) {
            throw new Error(`Port is not defined in the base use (${this.baseUrl})`);
        }
        this.port = this.baseUrl.port;
        this.log = debug(`test:mock:${this.name}`);
    }

    public get isRunning(): boolean {
        return this.app !== undefined;
    }

    public get readyUrl(): string {
        return `${this.baseUrl.protocol}//${this.baseUrl.host}/ready`;
    }

    public getUrlFor(path: string): string {
        return joinURL(this.baseUrl, path);
    }

    public async start() {
        if (this.isRunning) {
            throw new Error('Server has already been started');
        }

        this.log('Starting application');
        this.app = express();
        this.log('Init server');
        this.initCommon();
        this.init();
        this.log(`Start listening at port ${this.port} with baseurl ${this.baseUrl} ...`);
        if (this.tls) {
            this.log('TLS enabled');
            this.server = await https.createServer(this.tls, this.app).listen(parseInt(this.port), '0.0.0.0');
        } else {
            this.log('TLS disabled');
            this.server = await http.createServer(this.app).listen(parseInt(this.port), '0.0.0.0');
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
    }

    public async stop() {
        if (!this.isRunning) {
            return;
        }

        this.log('Stopping server ...');

        this.log(`Destroying open(${this.connections.length}) connections ...`);
        for (const connection of this.connections) {
            connection.destroy();
        }
        this.connections = [];

        await new Promise<void>((resolve, reject) => {
            this.server.close((err) => {
                if (err) reject(err);
                else resolve();
            });
        });

        this.server = undefined!;
        this.app = undefined!;
        this.log('Server stopped.');
    }

    private initCommon() {
        this.app.use(cors() as any as RequestHandler);

        this.app.get('/ready', (_req: Request, res: Response) => {
            res.status(200).send('Ok');
        });

        this.app.use((err: any, _req: any, _res: any, _next: any) => {
            this.log(err.stack);
            throw err;
        });

        this.app.use(async (req: Request, _res: Response, next?: NextFunction) => {
            this.log(`[${req.method}] ${req.url} ...`);
            await next?.();
            this.log(`[${req.method}] ${req.url} completed.`);
        });
    }

    /// Override in extends to implement the logic
    protected init() {}
}
