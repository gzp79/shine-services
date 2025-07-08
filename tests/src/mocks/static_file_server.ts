/* eslint-disable @typescript-eslint/no-explicit-any */
import express from 'express';
import { NextFunction, Request, Response } from 'express-serve-static-core';
import { CERTIFICATES } from './mock_constants';
import { Certificates, MockServer } from './mock_server';

export interface ServerConfig {
    url: URL;
    staticFilesPath: string;
    tls?: Certificates;
}

export class StaticFileServer extends MockServer {
    private staticFilesPath: string;

    constructor(name: string, config: ServerConfig) {
        super(name, config.url, config.tls ?? CERTIFICATES);
        this.staticFilesPath = config.staticFilesPath ?? 'dist';

        this.log(`folder: ${this.staticFilesPath}`);
    }

    protected init() {
        const app = this.app!;

        // Set security headers
        app.use(((_req: Request, res: Response, next: NextFunction) => {
            /* eslint-disable @stylistic/ts/quotes */
            /* spell-checker:disable */
            res.setHeader('X-Frame-Options', 'DENY');
            res.setHeader('X-Content-Type-Options', 'nosniff');
            res.setHeader('Referrer-Policy', 'no-referrer');
            res.setHeader('Permissions-Policy', 'document-domain=()');
            res.setHeader(
                'Content-Security-Policy',
                "worker-src 'none'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval' challenges.cloudflare.com static.cloudflareinsights.com; frame-ancestors 'none';"
            );
            /* spell-checker:enable */
            /* eslint-enable @stylistic/ts/quotes */
            next();
        }) as any);

        // Serve static files
        app.use(express.static(this.staticFilesPath) as any);
    }
}
