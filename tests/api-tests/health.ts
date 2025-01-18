import { expect, test } from '@fixtures/service-fixture';
import { ApiRequest } from '$lib/api/api';
import { joinURL } from '$lib/utils';

test.describe('Sanity check', () => {
    test('Invalid api call shall fail with 404', async ({ identityUrl }) => {
        const url = joinURL(identityUrl, '/info/404');
        const response = await ApiRequest.get(url).send();
        expect(response).toHaveStatus(404);
    });

    test('Health check shall pass', async ({ identityUrl }) => {
        const url = joinURL(identityUrl, '/info/ready');
        const response = await ApiRequest.get(url).send();
        expect(response).toHaveStatus(200);
    });
});
