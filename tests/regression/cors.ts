import request from '$lib/request';
import config from '../test.config';

describe('Sanity check', () => {
    const originUrl = new URL(config.serviceUrl);

    it('Allow origin shall not be present without origin', async () => {
        const url = config.getIdentityUrlFor('/info/ready');
        const response = await request.get(url);
        expect(response).toHaveStatus(200);
        expect(response.headers['access-control-allow-origin']).toBeUndefined();
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });

    it('Allow origin shall be present for not-found request', async () => {
        const url = config.getIdentityUrlFor('/info/404');
        const origin = `${originUrl.protocol}//${originUrl.hostname}:${originUrl.port}`;
        const response = await request.get(url).set('Origin', origin);
        expect(response).toHaveStatus(404);
        expect(response.headers['access-control-allow-origin']).toEqual(origin);
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });

    it('Allow origin shall be present for a successful request', async () => {
        const url = config.getIdentityUrlFor('/info/ready');
        const origin = `${originUrl.protocol}//${originUrl.hostname}:${originUrl.port}`;
        const response = await request.get(url).set('Origin', origin);
        expect(response).toHaveStatus(200);
        expect(response.headers['access-control-allow-origin']).toEqual(origin);
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });

    it('Allow origin shall be present for a subdomain', async () => {
        const url = config.getIdentityUrlFor('/info/ready');
        const origin = `${originUrl.protocol}//subdom.${originUrl.hostname}:${originUrl.port}`;
        const response = await request.get(url).set('Origin', origin);
        expect(response).toHaveStatus(200);
        expect(response.headers['access-control-allow-origin']).toEqual(origin);
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });

    it('Allow origin shall be present for another port', async () => {
        const url = config.getIdentityUrlFor('/info/ready');
        const origin = `${originUrl.protocol}//${originUrl.hostname}:123`;
        console.log('origin', origin);
        const response = await request.get(url).set('Origin', origin);
        expect(response).toHaveStatus(200);
        expect(response.headers['access-control-allow-origin']).toEqual(origin);
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });

    it('Allow origin shall not be present for another protocol', async () => {
        const url = config.getIdentityUrlFor('/info/ready');
        const origin = `file://${originUrl.hostname}:${originUrl.port}`;
        const response = await request.get(url).set('Origin', origin);
        expect(response).toHaveStatus(200);
        expect(response.headers['access-control-allow-origin']).toBeUndefined();
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });

    it('Allow origin shall not be present for another domain', async () => {
        const url = config.getIdentityUrlFor('/info/ready');
        const origin = 'https://example.com';
        const response = await request.get(url).set('Origin', origin);
        expect(response).toHaveStatus(200);
        expect(response.headers['access-control-allow-origin']).toBeUndefined();
        expect(response.headers['access-control-allow-credentials']).toEqual('true');
    });
});
