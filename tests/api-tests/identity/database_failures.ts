import { expect, test } from '$fixtures/setup';
import { waitForCondition } from '$lib/utils';
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
        const recovery = await waitForCondition(async () => await api.token.getTokensRequest(user.sid), {
            timeout: 5000,
            errorMessage: 'Service did not recover from database failure'
        });
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

        // Should timeout (not instant) but before the 10s latency
        expect(duration).toBeGreaterThan(3000); // Should take time (not instant fail)
        expect(duration).toBeLessThan(8000); // Should timeout before 10s latency
        expect(response).toHaveStatus(503); // Should fail gracefully
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

        // At least 80% should succeed with reasonable pool size
        const successCount = results.filter((r) => r.status() === 200).length;
        expect(successCount).toBeGreaterThan(40); // 80% success rate minimum
    });

    test('Deadlock during concurrent role updates shall retry', async ({ api }) => {
        const admin1 = await api.testUsers.createGuest({ roles: ['SuperAdmin'] });
        const admin2 = await api.testUsers.createGuest({ roles: ['SuperAdmin'] });
        const user1 = await api.testUsers.createGuest();
        const user2 = await api.testUsers.createGuest();

        // Create high contention scenario
        const operations = [
            api.user.addRole(admin1.sid, false, user1.userId, 'Role1'),
            api.user.addRole(admin2.sid, false, user2.userId, 'Role2'),
            api.user.addRole(admin1.sid, false, user2.userId, 'Role1'),
            api.user.addRole(admin2.sid, false, user1.userId, 'Role2')
        ];

        // All operations should eventually succeed (with retries)
        const results = await Promise.allSettled(operations);
        const failures = results.filter((r) => r.status === 'rejected');

        // Service should handle deadlocks gracefully (may have temp failures but retry)
        expect(failures.length).toBeLessThan(operations.length / 2); // At least 50% success rate
    });
});
