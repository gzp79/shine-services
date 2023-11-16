import request from 'superagent';
import config from '../test.config';
//import requestLogger from 'superagent-logger';

describe('Sanity check', () => {
    it('Health check shall pass', async () => {
        const response = await request
            .get(config.getUrlFor('/info/ready'))
            .send()
            .catch((err) => err.response);
        expect(response.statusCode).toEqual(200);
    });

    it('Registered providers shall be returned', async () => {
        const response = await request
            .get(config.getUrlFor('/identity/api/auth/providers'))
            .send()
            .catch((err) => err.response);
        expect(response.statusCode).toEqual(200);
        expect(response.body.providers).toEqual(expect.arrayContaining(['oauth2_flow', 'openid_flow']));
    });
});
