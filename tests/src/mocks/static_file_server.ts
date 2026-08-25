/* eslint-disable @typescript-eslint/no-explicit-any */
import express from 'express';
import { NextFunction, Request, Response } from 'express-serve-static-core';
import { CERTIFICATES } from './mock_constants';
import { Certificates, MockServer } from './mock_server';

export interface ServerConfig {
    url: URL;
    staticFilesPath: string;
    tls?: Certificates;
    /**
     * When set, the server emulates the deployed game/asset bucket: `GET /latest.json`
     * returns `{ version }` synthesized in memory, and the static files are mounted under
     * `/<version>/...`. This lets the local build output (e.g. `client/web/dist`) be served
     * as-is, without assembling a versioned folder and manifest on disk.
     */
    latestVersion?: string;
}

export class StaticFileServer extends MockServer {
    private staticFilesPath: string;
    private latestVersion?: string;

    constructor(name: string, config: ServerConfig) {
        super(name, config.url, config.tls ?? CERTIFICATES);
        this.staticFilesPath = config.staticFilesPath ?? 'dist';
        this.latestVersion = config.latestVersion;

        this.log(`folder: ${this.staticFilesPath}`);
        if (this.latestVersion) {
            this.log(`latest version: ${this.latestVersion}`);
        }
    }

    protected init() {
        const app = this.app!;

        // Set security headers
        app.use(((_req: Request, res: Response, next: NextFunction) => {
            /* eslint-disable @stylistic/quotes */
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
            /* eslint-enable @stylistic/quotes */
            next();
        }) as any);

        // Emulate the deployed bucket: synthesize the manifest and mount the build under
        // a version prefix, so the on-disk build output can be served without repackaging.
        if (this.latestVersion) {
            const version = this.latestVersion;
            app.get('/latest.json', ((_req: Request, res: Response) => {
                res.json({ version });
            }) as any);
            app.use(`/${version}`, express.static(this.staticFilesPath) as any);
        } else {
            // Serve static files
            app.use(express.static(this.staticFilesPath) as any);
        }
    }
}
