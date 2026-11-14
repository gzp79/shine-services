import { expect, test } from '$fixtures/setup';
import { waitForCondition } from '$lib/utils';

test.describe('Redis failure tests', { tag: '@infrastructure' }, () => {
    // Injected toxics affect every service, hence these tests must never overlap with others.
    test.describe.configure({ mode: 'serial' });

    test('Redis connection failure shall return 503 and recover', async ({ api, infraOutage }) => {
        const user = await api.testUsers.createGuest();

        // Take Redis down
        const outage = infraOutage('identity');
        await outage.stopRedis();

        // Should fail with 503
        const failResponse = await api.user.getUserInfoRequest(user.sid, 'fast');
        expect(failResponse).toHaveStatus(503);

        // Restore
        await outage.startRedis();

        // Should recover
        await waitForCondition(
            async () => {
                const recovery = await api.user.getUserInfoRequest(user.sid, 'fast');
                if (recovery.status() !== 200) throw new Error('Not recovered yet');
                return recovery;
            },
            {
                timeout: 5000,
                errorMessage: 'Service did not recover from Redis failure'
            }
        );

        const recovery = await api.user.getUserInfo(user.sid, 'fast');
        expect(recovery.userId).toEqual(user.userId);
    });

    test('Redis high latency shall not hang requests', async ({ api, infraOutage }) => {
        const user = await api.testUsers.createGuest();

        // Inject 10-second latency
        await infraOutage('identity').slowRedis(10000);

        // Should timeout before hanging (< 10s)
        const start = Date.now();
        const response = await api.user.getUserInfoRequest(user.sid, 'fast');
        const duration = Date.now() - start;

        // Should timeout (not instant) but before the 10s latency
        expect(duration).toBeGreaterThan(2000); // Should take time (not instant fail)
        expect(duration).toBeLessThan(8000); // Should timeout before 10s latency
        expect(response).toHaveStatus(503); // Should fail gracefully
    });
});
