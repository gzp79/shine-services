import { expect, test } from '$fixtures/setup';
import { Toxiproxy } from 'toxiproxy-node-client';

test.describe('Redis failure tests', { tag: '@infrastructure' }, () => {
    let toxiproxy: Toxiproxy;

    test.beforeAll(async () => {
        toxiproxy = new Toxiproxy('http://localhost:8474');
    });

    test.afterEach(async () => {
        await toxiproxy.reset();
    });

    test('Redis connection failure shall return 503 and recover', async ({ api }) => {
        const user = await api.testUsers.createGuest();

        // Take Redis down
        const redisProxy = await toxiproxy.get('redis');
        await redisProxy.update({ enabled: false, listen: redisProxy.listen, upstream: redisProxy.upstream });

        // Should fail with 503
        const failResponse = await api.user.getUserInfoRequest(user.sid, 'fast');
        expect(failResponse.status()).toBe(503);

        // Restore
        await redisProxy.update({ enabled: true, listen: redisProxy.listen, upstream: redisProxy.upstream });
        await new Promise((resolve) => setTimeout(resolve, 1000));

        // Should recover
        const recovery = await api.user.getUserInfo(user.sid, 'fast');
        expect(recovery.userId).toEqual(user.userId);
    });

    test('Redis high latency shall not hang requests', async ({ api }) => {
        const user = await api.testUsers.createGuest();

        // Inject 10-second latency
        const redisProxy = await toxiproxy.get('redis');
        await redisProxy.addToxic({
            name: 'slow',
            type: 'latency',
            stream: 'downstream',
            toxicity: 1.0,
            attributes: { latency: 10000, jitter: 0 }
        });

        // Should timeout before hanging (< 10s)
        const start = Date.now();
        const response = await api.user.getUserInfoRequest(user.sid, 'fast');
        const duration = Date.now() - start;

        expect(duration).toBeLessThan(10000);
        expect(response.status()).toBe(503);
    });
});
