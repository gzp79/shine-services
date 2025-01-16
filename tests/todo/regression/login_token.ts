import api from '$lib/api/api';
import { UserInfo } from '$lib/api/user_api';
import { getCookies, getPageRedirectUrl } from '$lib/response_utils';
import { TestUser } from '$lib/test_user';
import config from '../test.config';

describe('Login with token for new user', () => {
    it('Login with (captcha: NO, token: NO, rememberMe: INVALID) shall fail and redirect to the default error page', async () => {
        const response = await api.request
            .loginWithToken(null, null, null, null, null, undefined)
            .query({ rememberMe: 'invalid' });
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            'https://local-scytta.com:8443/error?type=invalidInput&status=400'
        );
        expect(response.text).toContain('Failed to deserialize query string');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Login with (captcha: NO, token: NO, redirectMe: NO) shall fail and redirect to the login page', async () => {
        const response = await api.request.loginWithToken(null, null, null, null, null, undefined);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.loginUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Login with (captcha: NO, token: NO, rememberMe: false) shall fail and redirect to the login page', async () => {
        const response = await api.request.loginWithToken(null, null, null, null, false, undefined);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.loginUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Login with (captcha: NO, token: NO, rememberMe: true) shall fail and redirect to the default error page', async () => {
        const response = await api.request.loginWithToken(null, null, null, null, true, undefined);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=400'
        );
        expect(response.text).toContain('&quot;Captcha&quot;:&quot;missing&quot;');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Login with (captcha: INVALID, token: NO, rememberMe: true) shall fail and redirect to the default error page', async () => {
        const response = await api.request.loginWithToken(null, null, null, null, true, 'invalid');
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=400'
        );
        expect(response.text).toContain('&quot;Captcha&quot;:&quot;invalid-input-response&quot;');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Login with (captcha: YES, token: NO, rememberMe: true) shall succeed and register a new guest user', async () => {
        const response = await api.request.loginWithToken(null, null, null, null, true, null);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeValidTID();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(cookies.sid.value)).toBeGuestUser();
    });

    it('Getting user info shall succeed only if fingerprint is not altered', async () => {
        const extraHeaders = {
            'user-agent': 'agent',
            'cf-region': 'region',
            'cf-ipcity': 'city',
            'cf-ipcountry': 'country'
        };

        const user = await TestUser.createGuest({}, extraHeaders);

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
            const response = await api.request.getUserInfo(user.sid).set({ ...extraHeaders, ...mod });
            expect(response).toHaveStatus(401);
        }
    });
});

describe('Login with token for returning user', () => {
    let testUser: TestUser = undefined!;
    let userInfo: Omit<UserInfo, 'sessionLength'> = undefined!;

    beforeEach(async () => {
        console.log('Register a new user...');
        testUser = await TestUser.createGuest();
        expect(testUser.sid).toBeDefined();
        expect(testUser.tid).toBeDefined();

        const { sessionLength, ...partialUserInfo } = testUser.userInfo!;
        userInfo = partialUserInfo;
    });

    it('Login with (token: NULL, session: VALID, rememberMe: true) shall fail with logout required', async () => {
        const response = await api.request.loginWithToken(null, testUser.sid, null, null, true, null);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
        );
        expect(response.text).toContain('&quot;LogoutRequired&quot;');

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeClearCookie();
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be the same session').toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(testUser.sid)).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with (token: VALID, session: VALID, rememberMe: true) shall fail with logout required', async () => {
        const response = await api.request.loginWithToken(
            testUser.tid!,
            testUser.sid,
            null,
            null,
            true,
            undefined
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
        );
        expect(response.text).toContain('&quot;LogoutRequired&quot;');

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'it shall be the same token').toEqual(testUser.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be the same session').toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(testUser.sid)).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with (token: VALID, session: NULL, rememberMe: NULL) shall succeed and login the user', async () => {
        const response = await api.request.loginWithToken(testUser.tid!, null, null, null, null, undefined);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'token shall be rotated').not.toEqual(testUser.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(newCookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with (token: VALID, session: NULL, rememberMe: false) shall succeed and login the user', async () => {
        const response = await api.request.loginWithToken(testUser.tid!, null, null, null, false, undefined);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'token shall be rotated').not.toEqual(testUser.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(newCookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with (token: VALID, session: NULL, rememberMe: true) shall succeed and login the user', async () => {
        const response = await api.request.loginWithToken(testUser.tid!, null, null, null, true, undefined);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'token shall be rotated').not.toEqual(testUser.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(testUser.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(newCookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });
});
