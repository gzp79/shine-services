import bodyParser from 'body-parser';
import express from 'express';
import { body, validationResult } from 'express-validator';
import { JWK, JWT } from 'ts-jose';
import { Certificates, MockServer, TypedRequest, TypedResponse } from '$lib/mock_server';
import '$lib/string_utils';

interface ServerConfig {
    tls?: Certificates;
    url: string;
    jwks: any;
}

export default class Server extends MockServer {
    public readonly config: ServerConfig;

    constructor(config: ServerConfig) {
        const url = new URL('openid', config.url);
        url.port = '8091';
        super('openid', url, config.tls);
        this.config = config;
    }

    protected init() {
        const app = this.app!;
        app.use(bodyParser.json());
        app.use(express.urlencoded({ extended: true }));

        app.get(
            '/openid/.well-known/openid-configuration',
            (req: TypedRequest<any, any>, res: TypedResponse<any>) => {
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
            }
        );

        app.get('/openid/jwks', (req: TypedRequest<any, any>, res: TypedResponse<any>) => {
            res.status(200).json({ keys: [this.config.jwks] });
        });

        const validate = [
            body('code').isString().notEmpty(),
            body('grant_type').isString().notEmpty(),
            body('redirect_uri').isString().notEmpty(),
            body('code_verifier').isString().notEmpty()
        ];
        app.post('/openid/token', validate, async (req: TypedRequest<any, any>, res: TypedResponse<any>) => {
            if (!req.is('application/x-www-form-urlencoded')) {
                this.log(`Unexpected content type`);
                throw new Error(`Unexpected content type`);
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
                email: user.email
            };

            const key = await JWK.fromObject(this.config.jwks);
            const idToken = await JWT.sign(payload, key, {
                alg: this.config.jwks.alg,
                issuer: issuer,
                audience: audience,
                //expiresIn: '1h',
                kid: this.config.jwks.kid
            });

            this.log(`id-token: ${idToken}`);

            res.status(200).json({
                access_token: idToken,
                id_token: idToken,
                token_type: 'Bearer'
            });
        });
    }
}
