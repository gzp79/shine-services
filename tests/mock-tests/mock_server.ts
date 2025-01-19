import { expect, test } from '$fixtures/setup';
import { ApiRequest } from '$lib/api/api';
import OpenIdMockServer from '$lib/mocks/openid';

test.describe('OpenId mock server', () => {
    test('Test mock server', async () => {
        const mock = new OpenIdMockServer();
        await mock.start();
        const response = await ApiRequest.get(mock!.getUrlFor('/.well-known/openid-configuration')).send();
        expect(response).toHaveStatus(200);
        await mock?.stop();
        expect(mock.isRunning).toBeFalsy();
    });
});
