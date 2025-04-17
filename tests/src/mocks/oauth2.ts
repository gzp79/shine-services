/* eslint-disable @typescript-eslint/no-explicit-any */
import { Certificates, MockServer } from '$lib/mocks/mock_server';
import '$lib/string_utils';
import bodyParser from 'body-parser';
import express from 'express';
import { Request, RequestHandler, Response } from 'express-serve-static-core';
import { body, validationResult } from 'express-validator';
import { CERTIFICATES, DEFAULT_URL } from './mock_constants';
import { getAuthorizeHtml } from './utils';

export interface ServerConfig {
    url: string;
    tls?: Certificates;
}

export default class Server extends MockServer {
    constructor(config?: ServerConfig) {
        const url = new URL('oauth2', config?.url ?? DEFAULT_URL);
        url.port = '8090';
        super('oauth2', url, config?.tls ?? CERTIFICATES);

        this.log(`url: ${url}`);
    }

    protected init() {
        const app = this.app!;
        app.use(bodyParser.json() as any as RequestHandler);
        app.use(express.urlencoded({ extended: true }) as any as RequestHandler);

        const validate = [
            body('code').isString().notEmpty(),
            body('grant_type').isString().notEmpty(),
            body('redirect_uri').isString().notEmpty(),
            body('code_verifier').isString().notEmpty()
        ] as any as RequestHandler;

        app.post('/oauth2/token', validate, async (req: Request, res: Response) => {
            if (!req.is('application/x-www-form-urlencoded')) {
                this.log('Unexpected content type');
                res.status(201).json({});
                return;
                //throw new Error(`Unexpected content type`);
            }

            const errors = validationResult(req);
            if (!errors.isEmpty()) {
                this.log(`Unexpected query parameters: ${JSON.stringify(errors)}`);
                throw new Error(`Unexpected query parameters: ${JSON.stringify(errors)}`);
            }

            const code: string = req.body.code;
            const user = code.parseAsQueryParams();
            if (!user || !user.id) {
                res.status(400).end();
                return;
            }

            res.status(200).json({
                access_token: code,
                token_type: 'Bearer'
            });
        });

        app.get('/oauth2/authorize', async (req: Request, res: Response) => {
            const authParams = req.query as Record<string, string>;
            const htmlContent = getAuthorizeHtml(authParams);
            res.status(200).send(htmlContent);
        });

        app.get('/oauth2/users', async (req: Request, res: Response) => {
            const code = req.headers.authorization?.split(' ')[1] ?? '';
            const user = (code as string).parseAsQueryParams();
            this.log(`user: ${JSON.stringify(user)}`);

            if (!user || !user.id) {
                res.status(400);
            } else {
                res.status(200).json(user);
            }
        });
    }
}
