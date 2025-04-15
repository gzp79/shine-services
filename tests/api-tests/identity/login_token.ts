import { expect, test } from '$fixtures/setup';
import { TestUser } from '$lib/api/test_user';
import { UserInfo } from '$lib/api/user_api';
import { getPageProblem, getPageRedirectUrl } from '$lib/api/utils';
import { createUrl } from '$lib/utils';

test.describe('Login with access cookie', () => {
    let testUser: TestUser = undefined!;
    let userInfo: Omit<UserInfo, 'sessionLength' | 'remainingSessionTime'> = undefined!;

    test.beforeEach(async ({ api }) => {
        testUser = await api.testUsers.createGuest();
        expect(testUser.sid).toBeDefined();
        expect(testUser.tid).toBeDefined();

        const { sessionLength, remainingSessionTime, ...partialUserInfo } = testUser.userInfo!;
        userInfo = partialUserInfo;
    });

    test('Login with (token: NULL, session: NULL, rememberMe: true) shall redirect to the login page', async ({
        api
    }) => {
        const response = await api.auth.loginWithTokenRequest(null, null, null, null, true, 'invalid');
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, {
                type: 'auth-login-required',
                status: 401,
                redirectUrl: api.auth.defaultRedirects.redirectUrl
            })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-login-required',
                status: 401,
                extension: null,
                sensitive: null
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Login with (token: NULL, session: VALID, rememberMe: NULL) shall succeed and create a new session', async ({
        api
    }) => {
        const response = await api.auth.loginWithTokenRequest(null, testUser.sid, null, null, null, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);
        expect(await getPageProblem(text)).toBeNull();

        const newCookies = response.cookies();
        expect(newCookies.tid).toBeClearCookie();
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();

        expect(await api.user.getUserInfoRequest(testUser.sid, 'full')).toHaveStatus(401);
        expect(await api.user.getUserInfo(newCookies.sid.value, 'full')).toEqual(expect.objectContaining(userInfo));
    });

    test('Login with (token: NULL, session: VALID, rememberMe: false) shall succeed and create a new session', async ({
        api
    }) => {
        const response = await api.auth.loginWithTokenRequest(null, testUser.sid, null, null, false, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);
        expect(await getPageProblem(text)).toBeNull();

        const newCookies = response.cookies();
        expect(newCookies.tid).toBeClearCookie();
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();

        expect(await api.user.getUserInfoRequest(testUser.sid, 'full')).toHaveStatus(401);
        expect(await api.user.getUserInfo(newCookies.sid.value, 'full')).toEqual(expect.objectContaining(userInfo));
    });

    test('Login with (token: NULL, session: VALID, rememberMe: true) shall succeed and create a new session', async ({
        api
    }) => {
        const response = await api.auth.loginWithTokenRequest(null, testUser.sid, null, null, true, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);
        expect(await getPageProblem(text)).toBeNull();

        const newCookies = response.cookies();
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'token shall be rotated').not.toEqual(testUser.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();

        expect(await api.user.getUserInfoRequest(testUser.sid, 'full')).toHaveStatus(401);
        expect(await api.user.getUserInfo(newCookies.sid.value, 'full')).toEqual(expect.objectContaining(userInfo));
    });

    test('Login with (token: VALID, session: NULL, rememberMe: NULL) shall succeed and create a new session', async ({
        api
    }) => {
        const response = await api.auth.loginWithTokenRequest(testUser.tid!, null, null, null, null, undefined);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);
        expect(await getPageProblem(text)).toBeNull();

        const newCookies = response.cookies();
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'token shall be rotated').not.toEqual(testUser.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();

        //expect(await api.user.getUserInfoRequest(testUser.sid, 'fast')).toHaveStatus(401) - would fail as the sid change is not known by the server as sid was not sent
        expect(await api.user.getUserInfo(newCookies.sid.value, 'fast')).toEqual(
            expect.objectContaining({ ...userInfo, details: null })
        );

        //expect(await api.user.getUserInfoRequest(testUser.sid, 'full')).toHaveStatus(401) - would fail as the sid change is not known by the server as sid was not sent
        expect(await api.user.getUserInfo(newCookies.sid.value, 'full')).toEqual(expect.objectContaining(userInfo));
    });

    test('Login with (token: VALID, session: NULL, rememberMe: false) shall succeed and create a new session', async ({
        api
    }) => {
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

        //expect(await api.user.getUserInfoRequest(testUser.sid, 'fast')).toHaveStatus(401) - would fail as the sid change is not known by the server as sid was not sent
        expect(await api.user.getUserInfo(newCookies.sid.value, 'fast')).toEqual(
            expect.objectContaining({ ...userInfo, details: null })
        );

        //expect(await api.user.getUserInfoRequest(testUser.sid, 'full')).toHaveStatus(401) - would fail as the sid change is not known by the server as sid was not sent
        expect(await api.user.getUserInfo(newCookies.sid.value, 'full')).toEqual(expect.objectContaining(userInfo));
    });

    test('Login with (token: VALID, session: NULL, rememberMe: true) shall succeed and and create a new session', async ({
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

        //expect(await api.user.getUserInfoRequest(testUser.sid, 'fast')).toHaveStatus(401) - would fail as the sid change is not known by the server as sid was not sent
        expect(await api.user.getUserInfo(newCookies.sid.value, 'fast')).toEqual(
            expect.objectContaining({ ...userInfo, details: null })
        );

        //expect(await api.user.getUserInfoRequest(testUser.sid, 'full')).toHaveStatus(401) - would fail as the sid change is not known by the server as sid was not sent
        expect(await api.user.getUserInfo(newCookies.sid.value, 'full')).toEqual(expect.objectContaining(userInfo));
    });

    test('Login with (token: VALID, session: VALID, rememberMe: true) shall succeed and create a new session', async ({
        api
    }) => {
        const response = await api.auth.loginWithTokenRequest(testUser.tid!, testUser.sid, null, null, true, undefined);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);
        expect(await getPageProblem(text)).toBeNull();

        const newCookies = response.cookies();
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'token shall be rotated').not.toEqual(testUser.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be the a new session').not.toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();

        expect(await api.user.getUserInfoRequest(testUser.sid, 'full')).toHaveStatus(401);
        expect(await api.user.getUserInfo(newCookies.sid.value, 'full')).toEqual(expect.objectContaining(userInfo));
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

        for (const method of ['fast', 'full'] as const) {
            // altering non-fingerprint value has no effect
            expect(
                await api.user.getUserInfo(user.sid, method, {
                    ...extraHeaders,
                    'cf-region': 'new-region',
                    'cf-ipcity': 'new-city',
                    'cf-ipcountry': 'new-country'
                })
            ).toBeGuestUser();

            // altering fingerprint value invalidates the session
            for (const mod of [{ 'user-agent': 'new-agent' }]) {
                const response = await api.user
                    .getUserInfoRequest(user.sid, method)
                    .withHeaders({ ...extraHeaders, ...mod });
                expect(response).toHaveStatus(401);
            }
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

        for (const method of ['fast', 'full'] as const) {
            const userLoggedIn = await api.user.getUserInfo(cookies.sid.value, method);
            expect(userLoggedIn.userId).not.toEqual(userCookie.userId);
            expect(userLoggedIn.userId).toEqual(userQuery.userId);
            expect(userLoggedIn.userId).not.toEqual(userHeader.userId);
        }
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
        const userLoggedIn = await api.user.getUserInfo(cookies.sid.value, 'full');
        expect(userLoggedIn.userId).not.toEqual(userCookie.userId);
        expect(userLoggedIn.userId).toEqual(userHeader.userId);
    });
});
