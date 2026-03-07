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
