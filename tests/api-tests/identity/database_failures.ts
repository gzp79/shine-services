import { expect, test } from '$fixtures/setup';
import { Toxiproxy } from 'toxiproxy-node-client';

test.describe('Database failure tests', { tag: '@infrastructure' }, () => {
    let toxiproxy: Toxiproxy;

    test.beforeAll(async () => {
        toxiproxy = new Toxiproxy('http://localhost:8474');
    });

    test.afterEach(async () => {
        await toxiproxy.reset();
    });

    test('Database connection failure shall return 503 and recover', async ({ api }) => {
        const user = await api.testUsers.createGuest();

        // Take PostgreSQL down
        const postgresProxy = await toxiproxy.get('postgres');
        await postgresProxy.update({ enabled: false, listen: postgresProxy.listen, upstream: postgresProxy.upstream });

        // Should fail with 503
        const failResponse = await api.token.getTokensRequest(user.sid);
        expect(failResponse).toHaveStatus(503);

        // Restore
        await postgresProxy.update({ enabled: true, listen: postgresProxy.listen, upstream: postgresProxy.upstream });

        // Should recover
        const recovery = await api.token.getTokensRequest(user.sid);
        expect(recovery).toHaveStatus(200);
    });

    test('Database timeout shall not hang requests', async ({ api }) => {
        const user = await api.testUsers.createGuest();

        // Inject 10-second latency
        const postgresProxy = await toxiproxy.get('postgres');
        await postgresProxy.addToxic({
            name: 'slow',
            type: 'latency',
            stream: 'downstream',
            toxicity: 1.0,
            attributes: { latency: 10000, jitter: 0 }
        });

        // Should timeout before hanging (< 10s)
        const start = Date.now();
        const response = await api.token.getTokensRequest(user.sid);
        const duration = Date.now() - start;

        expect(duration).toBeLessThan(10000);
        expect(response.status()).toBe(503);
    });

    test('Connection pool exhaustion shall queue gracefully', async ({ api }) => {
        const user = await api.testUsers.createGuest();

        // Fire many concurrent requests
        const requests = Array.from({ length: 50 }, () => api.token.getTokensRequest(user.sid));
        const results = await Promise.all(requests);

        // All should complete without hanging
        results.forEach((r) => {
            expect([200, 503]).toContain(r.status());
        });
        //console.log('Failed requests due to pool exhaustion:', results.filter((r) => r.status() === 503).length);
    });
});
