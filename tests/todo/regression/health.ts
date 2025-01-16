import request from '$lib/request';
import config from '../test.config';

describe('Sanity check', () => {
    it('Invalid api call shall fail with 404', async () => {
        const response = await request.get(config.getIdentityUrlFor('/info/404'));
        expect(response).toHaveStatus(404);
    });

    it('Health check shall pass', async () => {
        const response = await request.get(config.getIdentityUrlFor('/info/ready'));
        expect(response).toHaveStatus(200);
    });

    it('Registered providers shall be returned', async () => {
        const response = await request.get(config.getIdentityUrlFor('/api/auth/providers'));
        expect(response).toHaveStatus(200);
        expect(response.body.providers).toEqual(expect.arrayContaining(['oauth2_flow', 'openid_flow']));
    });
});
