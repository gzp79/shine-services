/* eslint-disable @typescript-eslint/no-explicit-any */
import { Certificates, MockServer } from '$lib/mocks/mock_server';
import '$lib/string_utils';
import bodyParser from 'body-parser';
import express from 'express';
import { Request, RequestHandler, Response } from 'express-serve-static-core';
import { body, validationResult } from 'express-validator';
import { JWK, JWKObject, JWSAlgorithms, JWT } from 'ts-jose';
import { CERTIFICATES, DEFAULT_URL, JWKS } from './mock_constants';
import { getAuthorizeHtml } from './utils';

interface ServerConfig {
    url?: string;
    jwks?: JWKObject;
    tls?: Certificates;
}

export default class Server extends MockServer {
    protected readonly _jwks: JWKObject;

    constructor(config?: ServerConfig) {
        const url = new URL('openid', config?.url ?? DEFAULT_URL);
        url.port = '8091';
        super('openid', url, config?.tls ?? CERTIFICATES);

        this.log(`url: ${url}`);
        this._jwks = config?.jwks ?? JWKS;
    }

    protected init() {
        let app = this.app!;
        app = app.use(bodyParser.json() as any as RequestHandler);
        app = app.use(express.urlencoded({ extended: true }) as any as RequestHandler);

        app.get('/openid/.well-known/openid-configuration', (_req: Request, res: Response) => {
            res.status(200).json({
                issuer: this.baseUrl,
                jwks_uri: this.baseUrl + '/jwks',
                authorization_endpoint: this.baseUrl + '/authorize',
                token_endpoint: this.baseUrl + '/token',
                userinfo_endpoint: this.baseUrl + '/userinfo',
                response_types_supported: ['id_token'],
                subject_types_supported: ['public'],
                id_token_signing_alg_values_supported: ['RS256']
            });
        });

        app.get('/openid/jwks', (_req: Request, res: Response) => {
            res.status(200).json({ keys: [JWKS] });
        });

        const validate = [
            body('code').isString().notEmpty(),
            body('grant_type').isString().notEmpty(),
            body('redirect_uri').isString().notEmpty(),
            body('code_verifier').isString().notEmpty()
        ] as any as RequestHandler;
        app.post('/openid/token', validate, async (req: Request, res: Response) => {
            if (!req.is('application/x-www-form-urlencoded')) {
                this.log('Unexpected content type');
                throw new Error('Unexpected content type');
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

            const issuer = this.baseUrl.toString();
            const audience = 'someClientId';

            const payload = {
                sub: user.id,
                iss: issuer,
                aud: audience,
                exp: Math.floor(Date.now() / 1000) + 3600, // Set expiration to 1 hour from now
                iat: Math.floor(Date.now() / 1000), // Issued at time
                nonce: user.nonce,
                nickname: user.name,
                email: user.email,
                ...user
            };

            const key = await JWK.fromObject(this._jwks);
            const idToken = await JWT.sign(payload, key, {
                alg: this._jwks.alg as JWSAlgorithms,
                issuer: issuer,
                audience: audience,
                //expiresIn: '1h',
                kid: this._jwks.kid
            });

            this.log(`id-token: ${idToken}`);

            res.status(200).json({
                access_token: idToken,
                id_token: idToken,
                token_type: 'Bearer'
            });
        });

        app.get('/openid/authorize', async (req: Request, res: Response) => {
            const authParams = req.query as Record<string, string>;
            const htmlContent = getAuthorizeHtml(authParams);
            res.status(200).send(htmlContent);
        });
    }
}
