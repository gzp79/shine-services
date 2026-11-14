import { expect, test } from '$fixtures/setup';
import { joinURL, sharedServices } from '$lib/utils';

for (const serviceName of sharedServices) {
    test(`Service shall use the test environment (${serviceName})`, async ({ serviceUrl, api }) => {
        const url = joinURL(serviceUrl(serviceName), '/info/version');
        const response = await api.client.get(url);
        expect(response).toHaveStatus(200);
        expect(
            await response.json(),
            `${serviceName} at ${url} must be started with the test configuration, not local or prod`
        ).toEqual(expect.objectContaining({ environment: 'test' }));
    });
}
