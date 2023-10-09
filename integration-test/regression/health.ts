import * as request from 'superagent';
import config from '../test.config';
//import requestLogger from 'superagent-logger';

describe('Sanity check', () => {
    it('Health check', async () => {
        const response = await request
            .get(config.getUrlFor('/info/ready'))
            //.use(requestLogger)
            .send();
        expect(response.statusCode).toBe(200);
    });

    it('Registered providers', async () => {
        const response = await request
            .get(config.getUrlFor('/identity/api/auth/providers'))
            //.use(requestLogger)
            .send();
        expect(response.statusCode).toBe(200);
        expect(response.body.providers).toEqual(
            expect.arrayContaining(['oauth2_flow', 'openid_flow'])
        );
    });
});
