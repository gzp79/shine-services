import request from '$lib/request';
import config from '../test.config';

describe('Sanity check', () => {
    it('Invalid api call shall fail with 404', async () => {
        const response = await request.get(config.getUrlFor('/info/404')).send();
        expect(response.statusCode).toEqual(404);
    });

    it('Health check shall pass', async () => {
        const response = await request.get(config.getUrlFor('/info/ready')).send();
        expect(response.statusCode).toEqual(200);
    });

    it('Registered providers shall be returned', async () => {
        const response = await request.get(config.getUrlFor('/identity/api/auth/providers')).send();
        expect(response.statusCode).toEqual(200);
        expect(response.body.providers).toEqual(expect.arrayContaining(['oauth2_flow', 'openid_flow']));
    });
});
