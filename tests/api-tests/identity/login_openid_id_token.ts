import { expect, test } from '$fixtures/setup';
import { ExternalUser } from '$lib/api/external_user';
import { getPageRedirectUrl } from '$lib/api/utils';
import OpenIdMockServer from '$lib/mocks/openid';
import { parseSignedCookie } from '$lib/utils';

test.describe('Login with OpenId id_token', () => {
    let mock!: OpenIdMockServer;

    test.beforeEach(async () => {
        mock = new OpenIdMockServer();
        await mock.start();
    });

    test.afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
    });

    test('Login with id_token shall succeed and register a new user', async ({ api }) => {
        const user = ExternalUser.newRandomUser('openid_flow');
        const idToken = await mock.getIdToken(user);

        const response = await api.auth.loginWithOpenIdIdToken('openid_flow', idToken);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();
        expect(parseSignedCookie(cookies.sid).key).toBeDefined();

        expect((await api.user.getUserInfo(cookies.sid, 'fast')).name).toEqual(user.name);
    });
});
