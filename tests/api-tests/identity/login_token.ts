import { expect, test } from '$fixtures/setup';
import { TestUser } from '$lib/api/test_user';
import { UserInfo } from '$lib/api/user_api';
import { getPageProblem, getPageRedirectUrl } from '$lib/api/utils';

test.describe('Login with access cookie for new user', () => {
    test('Login with (captcha: NO, token: NO, rememberMe: INVALID) shall fail and redirect to the default error page', async ({
        api
    }) => {
        const response = await api.auth
            .loginWithTokenRequest(null, null, null, null, null, undefined)
            .withParams({ rememberMe: 'invalid' });
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            'https://local-scytta.com:4443/error?type=auth-input-error&status=400'
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-input-error',
                status: 400,
                extension: null,
                sensitive: expect.objectContaining({
                    type: 'input-query-format',
                    detail: 'Failed to deserialize query string: rememberMe: provided string was not `true` or `false`'
                })
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Login with (captcha: NO, token: NO, redirectMe: NO) shall fail and redirect to the login page', async ({
        api
    }) => {
        const response = await api.auth.loginWithTokenRequest(null, null, null, null, null, undefined);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.loginUrl);
        expect(getPageProblem(text)).toBeNull();

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Login with (captcha: NO, token: NO, rememberMe: false) shall fail and redirect to the login page', async ({
        api
    }) => {
        const response = await api.auth.loginWithTokenRequest(null, null, null, null, false, undefined);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.loginUrl);
        expect(getPageProblem(text)).toBeNull();

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Login with (captcha: NO, token: NO, rememberMe: true) shall fail and redirect to the default error page', async ({
        api
    }) => {
        const response = await api.auth.loginWithTokenRequest(null, null, null, null, true, undefined);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.errorUrl + '?type=auth-error&status=400');
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-error',
                status: 400,
                extension: null,
                sensitive: expect.objectContaining({
                    type: 'captcha-not-provided'
                })
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Login with (captcha: INVALID, token: NO, rememberMe: true) shall fail and redirect to the default error page', async ({
        api
    }) => {
        const response = await api.auth.loginWithTokenRequest(null, null, null, null, true, 'invalid');
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.errorUrl + '?type=auth-error&status=400');
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-error',
                status: 400,
                extension: null,
                sensitive: expect.objectContaining({
                    type: 'captcha-failed-validation'
                })
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Login with (captcha: YES, token: NO, rememberMe: true) shall succeed and register a new guest user', async ({
        api
    }) => {
        const response = await api.auth.loginWithTokenRequest(null, null, null, null, true, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();
        expect(cookies.tid).toBeValidTID();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(cookies.sid.value)).toBeGuestUser();
    });
});

test.describe('Login with access cookie for returning user', () => {
    let testUser: TestUser = undefined!;
    let userInfo: Omit<UserInfo, 'sessionLength'> = undefined!;

    test.beforeEach(async ({ api }) => {
        testUser = await api.testUsers.createGuest();
        expect(testUser.sid).toBeDefined();
        expect(testUser.tid).toBeDefined();

        const { sessionLength, ...partialUserInfo } = testUser.userInfo!;
        userInfo = partialUserInfo;
    });

    test('Login with (token: NULL, session: VALID, rememberMe: true) shall fail', async ({ api }) => {
        const response = await api.auth.loginWithTokenRequest(null, testUser.sid, null, null, true, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=auth-logout-required&status=400'
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-logout-required',
                status: 400,
                extension: null,
                sensitive: null
            })
        );

        const newCookies = response.cookies();
        expect(newCookies.tid).toBeClearCookie();
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be the same session').toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(testUser.sid)).toEqual(expect.objectContaining(userInfo));
    });

    test('Login with (token: VALID, session: VALID, rememberMe: true) shall fail', async ({ api }) => {
        const response = await api.auth.loginWithTokenRequest(testUser.tid!, testUser.sid, null, null, true, undefined);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=auth-logout-required&status=400'
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-logout-required',
                status: 400,
                extension: null,
                sensitive: null
            })
        );

        const newCookies = response.cookies();
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'it shall be the same token').toEqual(testUser.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be the same session').toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(testUser.sid)).toEqual(expect.objectContaining(userInfo));
    });

    test('Login with (token: VALID, session: NULL, rememberMe: NULL) shall succeed', async ({ api }) => {
        const response = await api.auth.loginWithTokenRequest(testUser.tid!, null, null, null, null, undefined);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const newCookies = response.cookies();
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'token shall be rotated').not.toEqual(testUser.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(newCookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });

    test('Login with (token: VALID, session: NULL, rememberMe: false) shall succeed', async ({ api }) => {
        const response = await api.auth.loginWithTokenRequest(testUser.tid!, null, null, null, false, undefined);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const newCookies = response.cookies();
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'token shall be rotated').not.toEqual(testUser.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(newCookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });

    test('Login with (token: VALID, session: NULL, rememberMe: true) shall succeed and login the user', async ({
        api
    }) => {
        const response = await api.auth.loginWithTokenRequest(testUser.tid!, null, null, null, true, undefined);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const newCookies = response.cookies();
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'token shall be rotated').not.toEqual(testUser.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(newCookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });
});

test.describe('Login edge cases', () => {
    test('Getting user info shall succeed only if fingerprint is not altered', async ({ api }) => {
        const extraHeaders = {
            'user-agent': 'agent',
            'cf-region': 'region',
            'cf-ipcity': 'city',
            'cf-ipcountry': 'country'
        };

        const user = await api.testUsers.createGuest({}, extraHeaders);

        // altering non-fingerprint value has no effect
        expect(
            await api.user.getUserInfo(user.sid, {
                ...extraHeaders,
                'cf-region': 'new-region',
                'cf-ipcity': 'new-city',
                'cf-ipcountry': 'new-country'
            })
        ).toBeGuestUser();

        // altering fingerprint value invalidates the session
        for (const mod of [{ 'user-agent': 'new-agent' }]) {
            const response = await api.user.getUserInfoRequest(user.sid).withHeaders({ ...extraHeaders, ...mod });
            expect(response).toHaveStatus(401);
        }
    });

    test('Query token shall have the highest precedence', async ({ api }) => {
        const userCookie = await api.testUsers.createGuest();
        const userQuery = await api.testUsers.createGuest();
        const tokenQuery = await api.token.createSAToken(userQuery.sid, 120, false);
        const userHeader = await api.testUsers.createGuest();
        const tokenHeader = await api.token.createPersistentToken(userHeader.sid, 120, false);

        const response = await api.auth.loginWithTokenRequest(
            userCookie.tid!,
            null,
            tokenQuery.token,
            tokenHeader.token,
            false,
            null
        );
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();
        const userLoggedIn = await api.user.getUserInfo(cookies.sid.value);
        expect(userLoggedIn.userId).not.toEqual(userCookie.userId);
        expect(userLoggedIn.userId).toEqual(userQuery.userId);
        expect(userLoggedIn.userId).not.toEqual(userHeader.userId);
    });

    test('Header token shall have the 2nd highest precedence', async ({ api }) => {
        const userCookie = await api.testUsers.createGuest();
        const userHeader = await api.testUsers.createGuest();
        const tokenHeader = await api.token.createPersistentToken(userHeader.sid, 120, false);

        const response = await api.auth.loginWithTokenRequest(
            userCookie.tid!,
            null,
            null,
            tokenHeader.token,
            false,
            null
        );
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();
        const userLoggedIn = await api.user.getUserInfo(cookies.sid.value);
        expect(userLoggedIn.userId).not.toEqual(userCookie.userId);
        expect(userLoggedIn.userId).toEqual(userHeader.userId);
    });
});
