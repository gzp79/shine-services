import { expect, test } from '$fixtures/setup';
import { ExternalUser } from '$lib/api/external_user';
import { getPageRedirectUrl } from '$lib/api/utils';
import { MockServer } from '$lib/mocks/mock_server';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { generateRandomString } from '$lib/string_utils';
import { parseSignedCookie } from '$lib/utils';
import { randomUUID } from 'crypto';
import os from 'os';

test.describe('Check OAuth2 auth', () => {
    let mock: MockServer | undefined;

    const startMock = async (start = true): Promise<OAuth2MockServer> => {
        if (!mock) {
            mock = new OAuth2MockServer();
            if (start) {
                await mock.start();
            }
        }
        return mock as OAuth2MockServer;
    };

    test.afterEach(async () => {
        await mock?.stop();
        mock = undefined;
    });

    test('Auth with (parameters: NULL, session: NULL, external: NULL) shall fail', async ({ api }) => {
        await startMock();
        const response = await api.auth.authorizeWithOAuth2Request(null, null, null, null).send();

        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            'https://local-scytta.com:4443/error?type=authError&status=400'
        );
        expect(await response.text()).toContain('&quot;MissingExternalLoginCookie&quot;');

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Auth with (parameters: VALID, session: NULL, external: NULL) shall fail', async ({ api }) => {
        const mock = await startMock();
        const { authParams } = await api.auth.startLoginWithOAuth2(mock, null);

        const response = await api.auth
            .authorizeWithOAuth2Request(null, null, authParams.state, ExternalUser.newRandomUser().toCode())
            .send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            'https://local-scytta.com:4443/error?type=authError&status=400'
        );
        expect(await response.text()).toContain('&quot;MissingExternalLoginCookie&quot;');

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Auth with (parameters: NULL, session: NULL, external: VALID) shall fail', async ({ api }) => {
        const mock = await startMock();
        const { eid } = await api.auth.startLoginWithOAuth2(mock, null);

        const response = await api.auth.authorizeWithOAuth2Request(null, eid, null, null).send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=invalidInput&status=400'
        );
        expect(await response.text()).toContain('Failed to deserialize query string');

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Auth with (parameters: INVALID state, session: NULL, external: VALID) shall fail', async ({ api }) => {
        const mock = await startMock();
        const { eid } = await api.auth.startLoginWithOAuth2(mock, null);

        const response = await api.auth
            .authorizeWithOAuth2Request(null, eid, 'invalid', ExternalUser.newRandomUser().toCode())
            .send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=authError&status=400'
        );
        expect(await response.text()).toContain('&quot;InvalidCSRF&quot;');

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Auth with (parameters: INVALID code, session: NULL, external: VALID) shall fail', async ({ api }) => {
        const mock = await startMock();
        const { authParams, eid } = await api.auth.startLoginWithOAuth2(mock, null);

        const response = await api.auth.authorizeWithOAuth2Request(null, eid, authParams.state, 'invalid').send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=authError&status=500'
        );
        expect(await response.text()).toContain('server returned empty error response');

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Auth with failing 3rd party token service shall fail', async ({ api }) => {
        // mock is intentionally not started
        const mock = await startMock(false);
        const { authParams, eid } = await api.auth.startLoginWithOAuth2(mock, null);

        const response = await api.auth
            .authorizeWithOAuth2Request(null, eid, authParams.state, ExternalUser.newRandomUser().toCode())
            .send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=authError&status=500'
        );
        if (os.platform() === 'win32') {
            expect(await response.text()).toContain(
                'No connection could be made because the target machine actively refused it.'
            );
        } else {
            expect(await response.text()).toContain('Connection refused');
        }

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });
});

test.describe('Login with OAuth2', () => {
    let mock!: OAuth2MockServer;

    test.beforeEach(async () => {
        mock = new OAuth2MockServer();
        await mock.start();
    });

    test.afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
    });

    test('Login without captcha shall fail and redirect to the default error page', async ({ api }) => {
        const response = await api.auth.loginWithOAuth2Request(null, null, null, undefined).send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=authError&status=400'
        );
        expect(await response.text()).toContain('&quot;Captcha&quot;:&quot;missing&quot;');

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Login with wrong captcha shall fail and redirect to the default error page', async ({ api }) => {
        const response = await api.auth.loginWithOAuth2Request(null, null, null, 'invalid').send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=authError&status=400'
        );
        expect(await response.text()).toContain('&quot;Captcha&quot;:&quot;invalid-input-response&quot;');

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Start login with (token: NULL, session: VALID) shall fail', async ({ api }) => {
        const { sid } = await api.auth.loginAsGuestUser();

        const response = await api.auth.loginWithOAuth2Request(null, sid, null, null).send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
        );
        expect(await response.text()).toContain('&quot;LogoutRequired&quot;');

        const authCookies = response.cookies();
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeValidSID();
        expect(authCookies.sid.value).toEqual(sid);
        expect(authCookies.eid).toBeClearCookie();
    });

    test('Start login with (token: NULL, session: EXPIRED) shall succeed', async ({ api }) => {
        const { sid } = await api.auth.loginAsGuestUser();
        await api.auth.logout(sid, null, false);

        const response = await api.auth.loginWithOAuth2Request(null, sid, null, null).send();
        expect(response).toHaveStatus(200);
        const redirectUrl = getPageRedirectUrl(await response.text());
        expect(redirectUrl).toStartWith(mock!.getUrlFor('authorize'));

        const authCookies = response.cookies();
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeClearCookie();
        expect(authCookies.eid).toBeValidEID();
    });

    test('Start login with (token: VALID, session: NULL) shall succeed', async ({ api }) => {
        const { tid } = await api.auth.loginAsGuestUser();

        const response = await api.auth.loginWithOAuth2Request(tid, null, null, null).send();
        expect(response).toHaveStatus(200);
        const redirectUrl = getPageRedirectUrl(await response.text());
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const authCookies = response.cookies();
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeClearCookie();
        expect(authCookies.eid).toBeValidEID();
    });

    test('Start login with (token: VALID, session: VALID) shall succeed', async ({ api }) => {
        const { tid, sid } = await api.auth.loginAsGuestUser();

        const response = await api.auth.loginWithOAuth2Request(tid, sid, null, null).send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
        );

        const authCookies = response.cookies();
        expect(authCookies.tid).toBeValidTID();
        expect(authCookies.tid.value).toEqual(tid);
        expect(authCookies.sid).toBeValidSID();
        expect(authCookies.sid.value).toEqual(sid);
        expect(authCookies.eid).toBeClearCookie();
    });

    test('Login with (token: NULL, session: NULL, rememberMe: false) shall succeed and register a new user', async ({
        api
    }) => {
        const user = ExternalUser.newRandomUser();

        const cookies = await api.auth.loginWithOAuth2(mock, user, false);
        expect(parseSignedCookie(cookies.tid).key).toBeUndefined();
        expect(parseSignedCookie(cookies.sid).key).toBeString();
        expect(parseSignedCookie(cookies.eid).key).toBeUndefined();
        const userInfo = await api.user.getUserInfo(cookies.sid);
        expect(userInfo.name).toEqual(user.name);
    });

    test('Login with (token cookie: NULL, session: NULL, rememberMe: true) shall succeed and register a new user', async ({
        api
    }) => {
        const user = ExternalUser.newRandomUser();

        const cookies = await api.auth.loginWithOAuth2(mock, user, true);
        expect(parseSignedCookie(cookies.tid).key).toBeString();
        expect(parseSignedCookie(cookies.sid).key).toBeString();
        expect(parseSignedCookie(cookies.eid).key).toBeUndefined();
        const userInfo = await api.user.getUserInfo(cookies.sid);
        expect(userInfo.name).toEqual(user.name);
    });

    test('Login with occupied email shall fail', async ({ api }) => {
        const user = await api.testUsers.createLinked(mock, { email: generateRandomString(5) + '@example.com' });
        const newUser = new ExternalUser(randomUUID(), randomUUID(), user.externalUser!.email);

        const start = await api.auth.startLoginWithOAuth2(mock, false);
        const response = await api.auth
            .authorizeWithOAuth2Request(start.sid, start.eid, start.authParams.state, newUser.toCode())
            .send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=emailAlreadyUsed&status=409'
        );
    });

    test('Login with the same external user shall succeed', async ({ api }) => {
        const user = await api.testUsers.createLinked(mock);
        const newUserCookies = await api.auth.loginWithOAuth2(mock, user.externalUser!, null);
        expect(newUserCookies.sid, 'It shall be a new session').not.toEqual(user.sid);
        expect((await api.user.getUserInfo(newUserCookies.sid)).userId).toEqual(user.userId);
    });

    test('Login with the returned token shall be a success', async ({ api }) => {
        const user = await api.testUsers.createLinked(mock, { rememberMe: true });
        const newUserCookies = await api.auth.loginWithToken(user.tid!, null);
        expect(newUserCookies.sid, 'It shall be a new session').not.toEqual(user.sid);
        expect((await api.user.getUserInfo(newUserCookies.sid)).userId).toEqual(user.userId);
    });
});

test.describe('Link to OAuth2 account', () => {
    let mock!: OAuth2MockServer;

    test.beforeEach(async () => {
        mock = new OAuth2MockServer();
        await mock.start();
    });

    test.afterEach(async () => {
        await mock.stop();
        mock = undefined!;
    });

    test('Linking without a session shall fail', async ({ api }) => {
        const response = await api.auth.linkWithOAuth2Request(null).send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=loginRequired&status=401'
        );
    });

    test('Linking guest shall succeed', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        expect(user.userInfo!.isLinked).toBeFalsy();

        const externalUser = new ExternalUser(randomUUID(), randomUUID(), generateRandomString(5) + '@example.com');
        const start = await api.auth.startLinkWithOAuth2(mock, user.sid);
        const response = await api.auth
            .authorizeWithOAuth2Request(start.sid, start.eid, start.authParams.state, externalUser.toCode())
            .send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(api.auth.defaultRedirects.redirectUrl);

        user.externalUser = externalUser;
        await user.refreshUserInfo();
        expect(user.userInfo!.isLinked).toBeTruthy();
    });

    test('Linking with occupied email shall succeed', async ({ api }) => {
        const user = await api.testUsers.createLinked(mock, { email: generateRandomString(5) + '@example.com' });
        const newUser = new ExternalUser(randomUUID(), randomUUID(), user.externalUser!.email);

        const start = await api.auth.startLinkWithOAuth2(mock, user.sid);
        const response = await api.auth
            .authorizeWithOAuth2Request(start.sid, start.eid, start.authParams.state, newUser.toCode())
            .send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(api.auth.defaultRedirects.redirectUrl);
    });

    test('Linking with occupied external user shall fail', async ({ api }) => {
        const user = await api.testUsers.createLinked(mock, { email: generateRandomString(5) + '@example.com' });

        const start = await api.auth.startLinkWithOAuth2(mock, user.sid);
        const response = await api.auth
            .authorizeWithOAuth2Request(start.sid, start.eid, start.authParams.state, user.externalUser!.toCode())
            .send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=providerAlreadyUsed&status=409'
        );
    });
});
