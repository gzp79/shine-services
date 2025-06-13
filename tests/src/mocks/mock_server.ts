/* eslint-disable @typescript-eslint/no-explicit-any */
import cors from 'cors';
import { debug } from 'debug';
import express, { Express } from 'express';
import { NextFunction, Request, RequestHandler, Response } from 'express-serve-static-core';
import http from 'http';
import { Server } from 'http';
import https from 'https';
import { Socket } from 'net';
import { delay, joinURL } from '../utils';

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
            this.server = await https.createServer(this.tls, this.app).listen(this.port);
        } else {
            this.log('TLS disabled');
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
        this.app.use(cors() as any as RequestHandler);

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
