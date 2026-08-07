import { expect, test } from '$fixtures/setup';
import { SessionMint } from '$lib/mocks/session_mint';
import { joinURL, sharedServices } from '$lib/utils';

type StatusJson = Record<string, unknown>;

const optionalProviderValidators: Record<string, (value: unknown) => void> = {
    postgres: (value) => {
        expect(value).toEqual(
            expect.objectContaining({
                connections: expect.any(Number),
                idleConnections: expect.any(Number)
            })
        );
    },
    redis: (value) => {
        expect(value).toEqual(
            expect.objectContaining({
                connections: expect.any(Number),
                idleConnections: expect.any(Number)
            })
        );
    }
};

function expectStatusPayload(status: unknown): void {
    expect(status).toEqual(
        expect.objectContaining({
            uptime: expect.objectContaining({
                startTime: expect.any(String),
                uptimeSeconds: expect.any(Number)
            }),
            http: expect.objectContaining({
                inFlightRequests: expect.any(Number)
            })
        })
    );

    const statusObj = status as StatusJson;
    for (const [providerName, validator] of Object.entries(optionalProviderValidators)) {
        if (providerName in statusObj) {
            validator(statusObj[providerName]);
        }
    }
}

for (const serviceName of sharedServices) {
    test.describe(`Sanity check (${serviceName})`, () => {
        test('Invalid api call shall fail with 404', async ({ serviceUrl, api }) => {
            const urlBase = serviceUrl(serviceName);

            const url = joinURL(urlBase, '/info/404');
            const response = await api.client.get(url);
            expect(response).toHaveStatus(404);
        });

        test('Health check shall pass', async ({ serviceUrl, api }) => {
            const urlBase = serviceUrl(serviceName);

            const url = joinURL(urlBase, '/info/ready');
            const response = await api.client.get(url);
            expect(response).toHaveStatus(200);
        });
    });
}

for (const serviceName of sharedServices) {
    test.describe(`Service status (${serviceName})`, () => {
        let mint: SessionMint;

        test.beforeEach(async () => {
            mint = await SessionMint.fromServerConfig();
        });

        test.afterEach(async () => {
            await mint.teardownCreatedSessions();
        });

        test('Unauthenticated status request shall be rejected', async ({ serviceUrl, api }) => {
            const urlBase = serviceUrl(serviceName);
            const url = joinURL(urlBase, '/info/status');
            const response = await api.client.get(url);
            expect(response).toHaveStatus(401);
        });

        test('Non-admin status request shall be rejected', async ({ serviceUrl, api }) => {
            const urlBase = serviceUrl(serviceName);
            const user = await mint.createUserSession();
            const url = joinURL(urlBase, '/info/status');
            const response = await api.client.get(url).withCookies({ sid: user.sessionCookie });
            expect(response).toHaveStatus(403);
        });

        test('Admin status request shall return service status', async ({ serviceUrl, api }) => {
            const urlBase = serviceUrl(serviceName);
            const admin = await mint.createUserSession({ roles: ['SuperAdmin'] });
            const url = joinURL(urlBase, '/info/status');
            const response = await api.client.get(url).withCookies({ sid: admin.sessionCookie });
            expect(response).toHaveStatus(200);

            const status = await response.json();
            expectStatusPayload(status);
        });
    });
}
