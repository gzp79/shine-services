import api from '$lib/api/api';
import { UserInfo } from '$lib/api/user_api';
import { getCookies, getPageRedirectUrl } from '$lib/response_utils';
import { TestUser } from '$lib/test_user';
import config from '../test.config';

describe('Login with token for new user', () => {
    it('Login with (token: NO, rememberMe: INVALID) shall fail and redirect to the default error page', async () => {
        const response = await api.request.loginWithToken(null, null, null).query({ rememberMe: 'invalid' });
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            'https://web.sandbox.com:8080/error?type=invalidInput&status=400'
        );
        expect(response.text).toContain('Failed to deserialize query string');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Login with (token: NO, redirectMe: NO) shall fail and redirect to the login page', async () => {
        const response = await api.request.loginWithToken(null, null, null);
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.loginUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Login with (token: NO, rememberMe: false) shall fail and redirect to the login page', async () => {
        const response = await api.request.loginWithToken(null, null, false);
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.loginUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Login with (token: NONE, rememberMe: true) shall succeed and register a new user', async () => {
        const response = await api.request.loginWithToken(null, null, true);
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeValidTID();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(cookies.sid.value)).toBeGuestUser();
    });

    it('Login with (token: VALID, site: altered) shall succeed only if fingerprint is not altered', async () => {
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
            expect(response.statusCode).toEqual(401);
        }
    });
});

describe('Login with token for returning user', () => {
    let userCookies: Record<string, string> = undefined!;
    let userInfo: Omit<UserInfo, 'sessionLength'> = undefined!;

    beforeEach(async () => {
        console.log('Register a new user...');
        const response = await api.request.loginWithToken(null, null, true);
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeValidTID();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();
        userCookies = {
            sid: cookies.sid?.value,
            tid: cookies.tid?.value,
            eid: cookies.eid?.value
        };

        const fullUserInfo = await api.user.getUserInfo(cookies.sid.value);
        expect(fullUserInfo).toBeGuestUser();
        const { sessionLength, ...partialUserInfo } = fullUserInfo;
        userInfo = partialUserInfo;
    });

    it('Login with (token: NULL, session: VALID, rememberMe: true) shall fail with logout required', async () => {
        const response = await api.request.loginWithToken(null, userCookies.sid, true);
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
        );
        expect(response.text).toContain('&quot;LogoutRequired&quot;');

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeClearCookie();
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be the same session').toEqual(userCookies.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(userCookies.sid)).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with (token: VALID, session: VALID, rememberMe: true) shall fail with logout required', async () => {
        const response = await api.request.loginWithToken(userCookies.tid, userCookies.sid, true);
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
        );
        expect(response.text).toContain('&quot;LogoutRequired&quot;');

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'it shall be the same token').toEqual(userCookies.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be the same session').toEqual(userCookies.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(userCookies.sid)).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with (token: VALID, session: NULL, rememberMe: NULL) shall succeed and login the user', async () => {
        const response = await api.request.loginWithToken(userCookies.tid, null, null);
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'token shall be rotated').not.toEqual(userCookies.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(userCookies.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(newCookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with (token: VALID, session: NULL, rememberMe: false) shall succeed and login the user', async () => {
        const response = await api.request.loginWithToken(userCookies.tid, null, false);
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'token shall be rotated').not.toEqual(userCookies.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(userCookies.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(newCookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with (token: VALID, session: NULL, rememberMe: true) shall succeed and login the user', async () => {
        const response = await api.request.loginWithToken(userCookies.tid, null, true);
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'token shall be rotated').not.toEqual(userCookies.tid);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(userCookies.sid);
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(newCookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });
});
