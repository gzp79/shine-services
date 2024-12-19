import bodyParser from 'body-parser';
import express from 'express';
import { body, validationResult } from 'express-validator';
import { Certificates, MockServer, TypedRequest, TypedResponse } from '$lib/mock_server';
import '$lib/string_utils';

export interface ServerConfig {
    tls?: Certificates;
    url: string;
}

export default class Server extends MockServer {
    constructor(config: ServerConfig) {
        const url = new URL('oauth2', config.url);
        url.port = '8090';
        super('openid', url, config.tls);
    }

    protected init() {
        const app = this.app!;
        app.use(bodyParser.json());
        app.use(express.urlencoded({ extended: true }));

        const validate = [
            body('code').isString().notEmpty(),
            body('grant_type').isString().notEmpty(),
            body('redirect_uri').isString().notEmpty(),
            body('code_verifier').isString().notEmpty()
        ];

        app.post('/oauth2/token', validate, async (req: TypedRequest<any, any>, res: TypedResponse<any>) => {
            if (!req.is('application/x-www-form-urlencoded')) {
                this.log(`Unexpected content type`);
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

        app.get('/oauth2/users', async (req: TypedRequest<any, any>, res: TypedResponse<any>) => {
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
