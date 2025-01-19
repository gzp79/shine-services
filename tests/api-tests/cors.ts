import { expect, test } from '$fixtures/setup';
import { ApiRequest } from '$lib/api/api';
import { joinURL } from '$lib/utils';

test.describe('CORS check', { tag: ['@regression'] }, () => {
    test('Allow origin shall not be present without origin', async ({ identityUrl }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const response = await ApiRequest.get(url).send();
        expect(response).toHaveStatus(200);
        expect(response).toHaveHeader('access-control-allow-origin', undefined);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });

    test('Allow origin shall be present for not-found request', async ({ identityUrl }) => {
        const url = joinURL(identityUrl, '/info/404');
        const originUrl = new URL(identityUrl);
        const origin = `${originUrl.protocol}//${originUrl.hostname}:${originUrl.port}`;
        const response = await ApiRequest.get(url).withHeaders({ Origin: origin }).send();
        expect(response).toHaveStatus(404);
        expect(response).toHaveHeader('access-control-allow-origin', origin);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });

    test('Allow origin shall be present for a successful request', async ({ identityUrl }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const originUrl = new URL(identityUrl);
        const origin = `${originUrl.protocol}//${originUrl.hostname}:${originUrl.port}`;
        const response = await ApiRequest.get(url).withHeaders({ Origin: origin }).send();
        expect(response).toHaveStatus(200);
        expect(response).toHaveHeader('access-control-allow-origin', origin);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });

    test('Allow origin shall not be present for a subdomain', async ({ identityUrl }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const originUrl = new URL(identityUrl);
        const origin = `${originUrl.protocol}//subdom.${originUrl.hostname}:${originUrl.port}`;
        const response = await ApiRequest.get(url).withHeaders({ Origin: origin }).send();
        expect(response).toHaveStatus(200);
        expect(response).toHaveHeader('access-control-allow-origin', undefined);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });

    test('Allow origin shall be present for another port', async ({ identityUrl }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const originUrl = new URL(identityUrl);
        const origin = `${originUrl.protocol}//${originUrl.hostname}:123`;
        console.log('origin', origin);
        const response = await ApiRequest.get(url).withHeaders({ Origin: origin }).send();
        expect(response).toHaveStatus(200);
        expect(response).toHaveHeader('access-control-allow-origin', origin);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });

    test('Allow origin shall not be present for another protocol', async ({ identityUrl }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const originUrl = new URL(identityUrl);
        const origin = `file://${originUrl.hostname}:${originUrl.port}`;
        const response = await ApiRequest.get(url).withHeaders({ Origin: origin }).send();
        expect(response).toHaveStatus(200);
        expect(response).toHaveHeader('access-control-allow-origin', undefined);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });

    test('Allow origin shall not be present for another domain', async ({ identityUrl }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const origin = 'https://example.com';
        const response = await ApiRequest.get(url).withHeaders({ Origin: origin }).send();
        expect(response).toHaveStatus(200);
        expect(response).toHaveHeader('access-control-allow-origin', undefined);
        expect(response).toHaveHeader('access-control-allow-credentials', 'true');
    });
});
