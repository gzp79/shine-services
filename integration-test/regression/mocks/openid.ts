import { MockServer, TypedRequest, TypedResponse } from '$lib/karate';
import bodyParser from 'body-parser';
import express from 'express';
import { body, validationResult } from 'express-validator';
import { JWK, JWKS, JWT } from 'ts-jose';

export default class Server extends MockServer {
    constructor() {
        super('openid', 8090);
    }

    protected init() {
        const app = this.app!;
        app.use(bodyParser.json());
        app.use(express.urlencoded({ extended: true }));

        app.get(
            '/openid/.well-known/openid-configuration',
            (req: TypedRequest<any, any>, res: TypedResponse<any>) => {
                const base = this.config.openid_mock_url;
                res.status(200).json({
                    issuer: base,
                    jwks_uri: base + '/jwks',
                    authorization_endpoint: base + '/authorize',
                    token_endpoint: base + '/token',
                    userinfo_endpoint: base + '/userinfo',
                    response_types_supported: ['id_token'],
                    subject_types_supported: ['public'],
                    id_token_signing_alg_values_supported: ['RS256']
                });
            }
        );

        app.get(
            '/openid/jwks',
            (req: TypedRequest<any, any>, res: TypedResponse<any>) => {
                res.status(200).json({ keys: [this.config.openid_jwks] });
            }
        );

        const validate = [
            body('code').isString().notEmpty(),
            body('grant_type').isString().notEmpty(),
            body('redirect_uri').isString().notEmpty(),
            body('code_verifier').isString().notEmpty()
        ];
        app.post(
            '/openid/token',
            validate,
            async (req: TypedRequest<any, any>, res: TypedResponse<any>) => {
                if (!req.is('application/x-www-form-urlencoded')) {
                    this.log(`Unexpected content type`);
                    throw new Error(`Unexpected content type`);
                }

                console.log(req.query);
                console.log(req.body);

                const errors = validationResult(req);
                if (!errors.isEmpty()) {
                    this.log(
                        `Unexpected query parameters: ${JSON.stringify(errors)}`
                    );
                    throw new Error(
                        `Unexpected query parameters: ${JSON.stringify(errors)}`
                    );
                }

                const code: string = req.body.code;
                const user = code.parseAsQueryParams();

                if (!user || !user.id) {
                    res.status(400).end();
                    return;
                }

                const issuer = this.config.openid_mock_url;
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

                const key = await JWK.fromObject(this.config.openid_jwks);
                console.log(`jwk: ${key.key.type}`);
                console.log(`jwk: ${JSON.stringify(key)}`);
                const keys = new JWKS([key]);
                const idToken = await JWT.sign(payload, key, {
                    alg: this.config.openid_jwks.alg,
                    issuer: issuer,
                    audience: audience,
                    //expiresIn: '1h',
                    kid: this.config.openid_jwks.kid
                });

                this.log(`id-token: ${idToken}`);

                // todo create a signed jwk
                res.status(200).json({
                    access_token: idToken,
                    id_token: idToken,
                    token_type: 'Bearer'
                });
            }
        );
    }
}
