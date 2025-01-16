import { joinURL } from '$lib/utils';
import { test, expect } from '@fixtures/service-fixture';

test.describe('CORS check', { tag: ['@regression'] }, () => {
    test('Allow origin shall not be present without origin', async ({ request, identityUrl }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const response = await request.get(url);
        expect(response).toHaveStatus(200);
        expect(response).toHaveHeader('access-control-allow-origin', undefined);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });

    test('Allow origin shall be present for not-found request', async ({ request, identityUrl }) => {
        const url = joinURL(identityUrl, '/info/404');
        const originUrl = new URL(identityUrl);
        const origin = `${originUrl.protocol}//${originUrl.hostname}:${originUrl.port}`;
        const response = await request.get(url, { headers: { Origin: origin } });
        expect(response).toHaveStatus(404);
        expect(response).toHaveHeader('access-control-allow-origin', origin);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });

    test('Allow origin shall be present for a successful request', async ({ request, identityUrl }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const originUrl = new URL(identityUrl);
        const origin = `${originUrl.protocol}//${originUrl.hostname}:${originUrl.port}`;
        const response = await request.get(url, { headers: { Origin: origin } });
        expect(response).toHaveStatus(200);
        expect(response).toHaveHeader('access-control-allow-origin', origin);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });

    test('Allow origin shall not be present for a subdomain', async ({ request, identityUrl }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const originUrl = new URL(identityUrl);
        const origin = `${originUrl.protocol}//subdom.${originUrl.hostname}:${originUrl.port}`;
        const response = await request.get(url, { headers: { Origin: origin } });
        expect(response).toHaveStatus(200);
        expect(response).toHaveHeader('access-control-allow-origin', undefined);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });

    test('Allow origin shall be present for another port', async ({ request, identityUrl }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const originUrl = new URL(identityUrl);
        const origin = `${originUrl.protocol}//${originUrl.hostname}:123`;
        console.log('origin', origin);
        const response = await request.get(url, { headers: { Origin: origin } });
        expect(response).toHaveStatus(200);
        expect(response).toHaveHeader('access-control-allow-origin', origin);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });

    test('Allow origin shall not be present for another protocol', async ({ request, identityUrl }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const originUrl = new URL(identityUrl);
        const origin = `file://${originUrl.hostname}:${originUrl.port}`;
        const response = await request.get(url, { headers: { Origin: origin } });
        expect(response).toHaveStatus(200);
        expect(response).toHaveHeader('access-control-allow-origin', undefined);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });

    test('Allow origin shall not be present for another domain', async ({ request, identityUrl }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const origin = 'https://example.com';
        const response = await request.get(url, { headers: { Origin: origin } });
        expect(response).toHaveStatus(200);
        expect(response).toHaveHeader('access-control-allow-origin', undefined);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });
});
