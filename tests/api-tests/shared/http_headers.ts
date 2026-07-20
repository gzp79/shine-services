import { expect, test } from '$fixtures/setup';
import { joinURL, sharedServices } from '$lib/utils';

for (const serviceName of sharedServices) {
    test.describe(`Security headers (${serviceName})`, { tag: ['@regression'] }, () => {
        test('Response shall include security headers', async ({ serviceUrl, api: { client } }) => {
            const urlBase = serviceUrl(serviceName);

            const url = joinURL(urlBase, '/info/ready');
            const response = await client.get(url);
            expect(response).toHaveStatus(200);
            expect(response).toHaveHeader('strict-transport-security', 'max-age=31536000; includeSubDomains');
            expect(response).toHaveHeader('x-content-type-options', 'nosniff');
            expect(response).toHaveHeader('x-frame-options', 'DENY');
        });

        test('Security headers shall be present on error responses', async ({ serviceUrl, api: { client } }) => {
            const urlBase = serviceUrl(serviceName);

            const url = joinURL(urlBase, '/info/404');
            const response = await client.get(url);
            expect(response).toHaveStatus(404);
            expect(response).toHaveHeader('strict-transport-security', 'max-age=31536000; includeSubDomains');
            expect(response).toHaveHeader('x-content-type-options', 'nosniff');
            expect(response).toHaveHeader('x-frame-options', 'DENY');
        });
    });

    test.describe(`CORS check (${serviceName})`, { tag: ['@regression'] }, () => {
        test('Allow origin shall not be present without origin', async ({ serviceUrl, api: { client } }) => {
            const urlBase = serviceUrl(serviceName);

            const url = joinURL(urlBase, '/info/ready');
            const response = await client.get(url);
            expect(response).toHaveStatus(200);
            expect(response).toHaveHeader('access-control-allow-origin', undefined);
            expect(response).toHaveHeader('access-control-allow-credentials', 'true');
        });

        test('Allow origin shall be present for not-found request', async ({ serviceUrl, api: { client } }) => {
            const urlBase = serviceUrl(serviceName);

            const url = joinURL(urlBase, '/info/404');
            const originUrl = new URL(urlBase);
            const origin = `${originUrl.protocol}//${originUrl.hostname}:${originUrl.port}`;
            const response = await client.get(url).withHeaders({ Origin: origin });
            expect(response).toHaveStatus(404);
            expect(response).toHaveHeader('access-control-allow-origin', origin);
            expect(response).toHaveHeader('access-control-allow-credentials', 'true');
        });

        test('Allow origin shall be present for a successful request', async ({ serviceUrl, api: { client } }) => {
            const urlBase = serviceUrl(serviceName);

            const url = joinURL(urlBase, '/info/ready');
            const originUrl = new URL(urlBase);
            const origin = `${originUrl.protocol}//${originUrl.hostname}:${originUrl.port}`;
            const response = await client.get(url).withHeaders({ Origin: origin });
            expect(response).toHaveStatus(200);
            expect(response).toHaveHeader('access-control-allow-origin', origin);
            expect(response).toHaveHeader('access-control-allow-credentials', 'true');
        });

        test('Allow origin shall be present for any subdomain', async ({ serviceUrl, api: { client } }) => {
            const urlBase = serviceUrl(serviceName);

            const url = joinURL(urlBase, '/info/ready');
            const originUrl = new URL(urlBase);
            const origin = `${originUrl.protocol}//subdom.${originUrl.hostname}:${originUrl.port}`;
            const response = await client.get(url).withHeaders({ Origin: origin });
            expect(response).toHaveStatus(200);
            expect(response).toHaveHeader('access-control-allow-origin', origin);
            expect(response).toHaveHeader('access-control-allow-credentials', 'true');
        });

        test('Allow origin shall be present for another port', async ({ serviceUrl, api: { client } }) => {
            const urlBase = serviceUrl(serviceName);

            const url = joinURL(urlBase, '/info/ready');
            const originUrl = new URL(urlBase);
            const origin = `${originUrl.protocol}//${originUrl.hostname}:123`;
            const response = await client.get(url).withHeaders({ Origin: origin });
            expect(response).toHaveStatus(200);
            expect(response).toHaveHeader('access-control-allow-origin', origin);
            expect(response).toHaveHeader('access-control-allow-credentials', 'true');
        });

        test('Allow origin shall not be present for another protocol', async ({ serviceUrl, api: { client } }) => {
            const urlBase = serviceUrl(serviceName);

            const url = joinURL(urlBase, '/info/ready');
            const originUrl = new URL(urlBase);
            const origin = `file://${originUrl.hostname}:${originUrl.port}`;
            const response = await client.get(url).withHeaders({ Origin: origin });
            expect(response).toHaveStatus(200);
            expect(response).toHaveHeader('access-control-allow-origin', undefined);
            expect(response).toHaveHeader('access-control-allow-credentials', 'true');
        });

        test('Allow origin shall not be present for another domain', async ({ serviceUrl, api: { client } }) => {
            const urlBase = serviceUrl(serviceName);

            const url = joinURL(urlBase, '/info/ready');
            const origin = 'https://example.com';
            const response = await client.get(url).withHeaders({ Origin: origin });
            expect(response).toHaveStatus(200);
            expect(response).toHaveHeader('access-control-allow-origin', undefined);
            expect(response).toHaveHeader('access-control-allow-credentials', 'true');
        });
    });
}
