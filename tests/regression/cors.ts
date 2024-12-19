import request from '$lib/request';
import config from '../test.config';

describe('Sanity check', () => {
    const url = config.getIdentityUrlFor('/info/ready');

    it('Allow origin shall not be present without origin', async () => {
        const url = config.getIdentityUrlFor('/info/ready');
        const response = await request.get(url);
        expect(response).toHaveStatus(200);
        expect(response.headers['access-control-allow-origin']).toBeUndefined();
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });

    it('Allow origin shall be present for missing path too', async () => {
        const url = config.getIdentityUrlFor('/info/404');
        const origin = config.identityUrl;
        const response = await request.get(url).set('Origin', origin);
        expect(response).toHaveStatus(404);
        expect(response.headers['access-control-allow-origin']).toEqual(origin);
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });

    it('Allow origin shall be present with an origin of the service', async () => {
        const origin = config.identityUrl;
        const response = await request.get(url).set('Origin', origin);
        expect(response).toHaveStatus(200);
        expect(response.headers['access-control-allow-origin']).toEqual(origin);
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });

    it('Allow origin shall be present for another subdomain', async () => {
        const origin = 'https://another.sandbox.com:8443';
        const response = await request.get(url).set('Origin', origin);
        expect(response).toHaveStatus(200);
        expect(response.headers['access-control-allow-origin']).toEqual(origin);
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });

    it('Allow origin shall be present for another port', async () => {
        const origin = 'https://cloud.sandbox.com:123';
        const response = await request.get(url).set('Origin', origin);
        expect(response).toHaveStatus(200);
        expect(response.headers['access-control-allow-origin']).toEqual(origin);
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });

    it('Allow origin shall not be present for another protocol', async () => {
        const origin = 'http://cloud.sandbox.com:8443';
        const response = await request.get(url).set('Origin', origin);
        expect(response).toHaveStatus(200);
        expect(response.headers['access-control-allow-origin']).toBeUndefined();
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });

    it('Allow origin shall not be present for another domain', async () => {
        const origin = 'https://example.com';
        const response = await request.get(url).set('Origin', origin);
        expect(response).toHaveStatus(200);
        expect(response.headers['access-control-allow-origin']).toBeUndefined();
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });
});
