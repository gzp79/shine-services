import { expect, test } from '$fixtures/setup';
import { SessionMint } from '$lib/mocks/session_mint';
import { joinURL, sharedServices, waitForCondition } from '$lib/utils';

type StatusJson = Record<string, unknown>;

for (const envName of ['test', 'local', 'prod']) {
    for (const serviceName of sharedServices) {
        test(`Service environment check - ${serviceName} requires ${envName}`, async ({ serviceTestInfo }) => {
            const env = serviceTestInfo(serviceName);
            await env.runIf([envName]);
            expect(env.current()).toBe(envName);
        });
    }
}

type PoolStatus = {
    connections: number;
    idleConnections: number;
    healthy: boolean;
    listener: { connected: boolean; channels: number };
};

const expectPoolStatus = (value: unknown): void => {
    expect(value).toEqual(
        expect.objectContaining({
            connections: expect.any(Number),
            idleConnections: expect.any(Number),
            healthy: expect.any(Boolean),
            listener: { connected: expect.any(Boolean), channels: expect.any(Number) }
        })
    );
};

const optionalProviderValidators: Record<string, (value: unknown) => void> = {
    postgres: expectPoolStatus,
    redis: expectPoolStatus
};

// Reported by every service: the pool of the shared current-user (session) check.
const sessionRedisValidator = expectPoolStatus;

const poolProviders = ['postgres', 'redis', 'sessionRedis'];

function expectStatusPayload(status: unknown): void {
    expect(status).toEqual(
        expect.objectContaining({
            environment: expect.stringMatching(/^(test|local|prod)$/),
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
    expect(statusObj).toHaveProperty('sessionRedis');
    sessionRedisValidator(statusObj.sessionRedis);
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

        test('Listener of a pool with channels shall be connected', async ({ serviceUrl, api }) => {
            const url = joinURL(serviceUrl(serviceName), '/info/status');
            const admin = await mint.createUserSession({ roles: ['SuperAdmin'] });

            // A listener without channels stays idle; once it has some it has to hold its connection. It may be
            // reconnecting after an outage of an earlier test.
            await waitForCondition(
                async () => {
                    const response = await api.client.get(url).withCookies({ sid: admin.sessionCookie });
                    expect(response).toHaveStatus(200);
                    const status = (await response.json()) as Record<string, PoolStatus | undefined>;
                    for (const name of poolProviders) {
                        const listener = status[name]?.listener;
                        if (listener && listener.channels > 0) {
                            expect(listener.connected, `${name} listener has ${listener.channels} channels`).toBe(true);
                        }
                    }
                    return true;
                },
                { timeout: 15_000, interval: 500 }
            );
        });
    });
}
