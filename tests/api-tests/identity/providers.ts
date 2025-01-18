import { expect, test } from '@fixtures/service-fixture';
import { ProvidersSchema } from '$lib/api/auth_api';

test.describe('External login providers', () => {
    test('Registered providers shall be returned', async ({ api }) => {
        const response = await api.auth.getProvidersRequest().send();
        expect(response).toHaveStatus(200);
        const responseBody = await response.parse(ProvidersSchema);
        expect(responseBody.providers).toEqual(expect.arrayContaining(['oauth2_flow', 'openid_flow']));
    });
});
