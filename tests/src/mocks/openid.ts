/* eslint-disable @typescript-eslint/no-explicit-any */

import { Certificates, MockServer, TypedRequest, TypedResponse } from '$lib/mocks/mock_server';
import '$lib/string_utils';
import bodyParser from 'body-parser';
import express from 'express';
import { body, validationResult } from 'express-validator';
import { JWK, JWKObject, JWSAlgorithms, JWT } from 'ts-jose';

// new key set can be generated at https://mkjwk.org/ quite easily
// (RSA, size:2048 (smaller is rejected by the jose module as of specification), Use:Signature, Alg:RS256, ID:Sha-1 )
/* spell-checker: disable */
const JWKS: JWKObject = {
    p: '21pzZgFcZqxR3CXwJ4uaXhAZHPHCi2MdNwe6MFUr8i85ehj9-za1qlnW1Jb5XmusJQhu-iFMPhlR0h51n5rM_O_XRVBSp9uu-yh-cAYNwYFxMbtlkXvCnRhpAwKimNehokJ2YyRpLlW6Kn47dd3JjxYH3DRBBSPohQnHNzozARU',
    kty: 'RSA',
    q: 'xW3XRPacjFnGXt6x1RbFV48wIGfeYEAKrFPbcQRL2uY1pq2htGDmso8umEK7lIFUFonqBJKR3dw8t3NuQN8P9rZSGdXVhQ52DKnKvLAQT4IKoyXOGdOuugBbRh57VEpTw8fMfyzdJwccLmWSTPtVj_0GCa6T6oZCDCDuEnPJfPk',
    d: 'cHehvcojcKjS6pkdmCjHsWJGHiOunw0PHSArkvEKTZIekw_nekfYYKw7BPt4ZH6NeD9A-s0v_y0lwvQ7_OPtj1BUlicgPnOIfvzEaYdCr2Qx9XYWyqHKJANZ9FGUAFxFzVI1xnKB6sUC1zt3PiiJZXsq3-LL5ke6OGA3G6g2e0a8I67bQbbZd_TOe8Jh0N5IUyfnkv8jYiC5waNjZSVY9_DZE2rSZ-CmIhypUTTUfXhgNxciZGMMB3mtzMG3vR_kUv-VooXqsWgecUu9Af97maSBwoC2MessJ7VvvR553ZeYkfoCsRs8k1au2O3qLW6TON6QVZr1D602nQ0murgUIQ',
    e: 'AQAB',
    use: 'sig',
    qi: 'sX4jokfUgFeUBjTBQA7mFZ6Hg8dcIidDcSa11heUb9TZt24oR-c3wsWT11cOdT6-wjEL9b-H0UZd3iC8YjwNBu6cHwQJb9sJ3-ZLSRSQJ0HuAozhMuB4n-7Oewzb63AHgwuBSb_gwxWl0X-KYERYxK7vtu38PnHFjxWCeyqtYJc',
    dp: 'gV5rSPHsiTGAZhKJ_Qi81lUwOn3re0HNbTNFgFP7Qy7O-0_aG1s88Wdi6KbSE_n04TKEIUmaKdXNB9unC6bE1zitAdhJp25NWRuc1nz7h_DLzcT0NkWDlhtbc8cOFo62aXhBUl-bGRS-Y2lnsDBKO_WGVT0MS_fNnwkRUWUlx7E',
    alg: 'RS256',
    dq: 'nycH1Vk0I9QvHMVK-CtuFEKimk0BL_gQYpELIlVDTQgtkdsAsyc2chUIi8en7XRANBcjZmI9YmsrKvvLklH_TXP2RUti3-sjcNvjSi5oR5_eMVzFg35oqRqmeaUS6IUud3H2QUMKWG7b4e8RfCtT80oWdvGb3gAy-BIHuSpL8Ak',
    n: 'qSq4xK-7D9wEIgfo1athchJvLZMn0oWh8lRXL8zwED4FtMX4nxqLGU8oir8E__Pic3sOn9ZS-bnRMlXJkIS0uZT1zBIoU6RQIfe2ScI6AaZ6QTTK5Viu10wy4S4wXdIyIInVSgnWcccrkWnrewxyj1pcZFzgzT1ZRD8BZ0roOxLefrCN0WOODABI4zTY-L5q0X5JpBk0jC1wk6YofQZYtEO4XU-wvHZIugKnjSsAvyRgcWZq1niH2_8tdnXrnvDlTnC6IZzRBjLrVW7nHu1KtiDAnwL3NRrsnW0wu1fjQCG_YUNCFRkIHwpnq5X8Zn7gsnvdTBAosJn9urnqmJ85bQ'
};
/* spell-checker: enable */

interface ServerConfig {
    tls?: Certificates;
    url: string;
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

        app.get('/openid/.well-known/openid-configuration', (_req: TypedRequest<any, any>, res: TypedResponse<any>) => {
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

        app.get('/openid/jwks', (_req: TypedRequest<any, any>, res: TypedResponse<any>) => {
            res.status(200).json({ keys: [JWKS] });
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

            const key = await JWK.fromObject(JWKS);
            const idToken = await JWT.sign(payload, key, {
                alg: JWKS.alg as JWSAlgorithms,
                issuer: issuer,
                audience: audience
                //expiresIn: '1h',
                //kid: JWKS.kid
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
