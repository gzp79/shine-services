import { expect, test } from '$fixtures/setup';
import { joinURL } from '$lib/utils';

test.describe('Sanity check', () => {
    test('Invalid api call shall fail with 404', async ({ identityUrl, api }) => {
        const url = joinURL(identityUrl, '/info/404');
        const response = await api.client.get(url);
        expect(response).toHaveStatus(404);
    });

    test('Health check shall pass', async ({ identityUrl, api }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const response = await api.client.get(url);
        expect(response).toHaveStatus(200);
    });
});

test.describe('Service status', () => {
    test('Unauthenticated status request shall be rejected', async ({ identityUrl, api }) => {
        const url = joinURL(identityUrl, '/info/status');
        const response = await api.client.get(url);
        expect(response).toHaveStatus(401);
    });

    test('Non-admin status request shall be rejected', async ({ identityUrl, api }) => {
        const user = await api.testUsers.createGuest();
        const url = joinURL(identityUrl, '/info/status');
        const response = await api.client.get(url).withCookies({ sid: user.sid });
        expect(response).toHaveStatus(403);
    });

    test('Admin status request shall return service status', async ({ identityUrl, api }) => {
        const admin = await api.testUsers.createGuest({ roles: ['SuperAdmin'] });
        const url = joinURL(identityUrl, '/info/status');
        const response = await api.client.get(url).withCookies({ sid: admin.sid });
        expect(response).toHaveStatus(200);

        const status = await response.json();

        expect(status).toEqual(
            expect.objectContaining({
                uptime: expect.objectContaining({
                    startTime: expect.any(String),
                    uptimeSeconds: expect.any(Number)
                }),
                http: expect.objectContaining({
                    inFlightRequests: expect.any(Number)
                }),
                postgres: expect.objectContaining({
                    connections: expect.any(Number),
                    idleConnections: expect.any(Number)
                }),
                redis: expect.objectContaining({
                    connections: expect.any(Number),
                    idleConnections: expect.any(Number)
                })
            })
        );
    });
});
