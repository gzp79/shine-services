import request from 'superagent';
import { getPageRedirectUrl } from '$lib/page_utils';
import { UserInfo, getCookies, getUserInfo } from '$lib/auth_utils';
import config from '../test.config';
import { Cookie } from 'tough-cookie';
import { TestUser } from '$lib/user';

describe('Login with token', () => {
    it('Login with (token: NO, rememberMe: INVALID) shall fail and redirect to the default error page', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: 'invalid value' })
            .send()
            .catch((err) => err.response);

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
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query(config.defaultRedirects)
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.loginUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Login with (token: NO, rememberMe: false) shall fail and redirect to the login page', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: false, ...config.defaultRedirects })
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.loginUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Login with (token: NO, rememberMe: true) shall succeed and register a new user', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: true, ...config.defaultRedirects })
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeValidTID();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();
        expect(await getUserInfo(cookies.sid.value)).toBeGuestUser();
    });
});

describe('Login with token for returning user', () => {
    let cookies: Record<string, Cookie> = undefined!;
    let userInfo: Omit<UserInfo, 'sessionLength'> = undefined!;

    beforeEach(async () => {
        console.log('Register a new user...');
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: true, ...config.defaultRedirects })
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        cookies = getCookies(response);
        expect(cookies.tid).toBeValidTID();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        const fullUserInfo = await getUserInfo(cookies.sid.value);
        expect(fullUserInfo).toBeGuestUser();
        const { sessionLength, ...partialUserInfo } = fullUserInfo;
        userInfo = partialUserInfo;
    });

    it('Login with (session: VALID, token: NO) shall fail with logout required', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: true, ...config.defaultRedirects })
            .set('Cookie', [`sid=${cookies.sid.value}`])
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
        );
        expect(response.text).toContain('&quot;LogoutRequired&quot;');

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeClearCookie();
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be the same session').toEqual(cookies.sid.value);
        expect(newCookies.eid).toBeClearCookie();
        expect(await getUserInfo(cookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with (session: VALID, token: VALID) shall fail with logout required', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: true, ...config.defaultRedirects })
            .set('Cookie', [`sid=${cookies.sid.value}`, `tid=${cookies.tid.value}`])
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
        );
        expect(response.text).toContain('&quot;LogoutRequired&quot;');

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'it shall be the same token').toEqual(cookies.tid.value);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be the same session').toEqual(cookies.sid.value);
        expect(newCookies.eid).toBeClearCookie();
        expect(await getUserInfo(cookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with (session: NO, token: VALID, rememberMe: None) shall succeed and login the user', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ ...config.defaultRedirects })
            .set('Cookie', [`tid=${cookies.tid.value}`])
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'it shall be the same token').toEqual(cookies.tid.value);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(cookies.sid.value);
        expect(newCookies.eid).toBeClearCookie();
        expect(await getUserInfo(newCookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with (session: NO, token: VALID, rememberMe: false) shall succeed and login the user', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: false, ...config.defaultRedirects })
            .set('Cookie', [`tid=${cookies.tid.value}`])
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'it shall be the same token').toEqual(cookies.tid.value);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(cookies.sid.value);
        expect(newCookies.eid).toBeClearCookie();
        expect(await getUserInfo(newCookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with (session: NO, token: VALID, rememberMe: true) shall succeed and login the user', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: true, ...config.defaultRedirects })
            .set('Cookie', [`tid=${cookies.tid.value}`])
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeValidTID();
        expect(newCookies.tid.value, 'it shall be the same token').toEqual(cookies.tid.value);
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.sid.value, 'it shall be a new session').not.toEqual(cookies.sid.value);
        expect(newCookies.eid).toBeClearCookie();
        expect(await getUserInfo(newCookies.sid.value)).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with (session: NO, token: VALID, site: altered) shall succeed only if fingerprint is not altered', async () => {
        const site_info = {
            'user-agent': 'agent',
            'cf-region': 'region',
            'cf-ipcity': 'city',
            'cf-ipcountry': 'country'
        };

        const user = await TestUser.createGuest({ extraHeaders: site_info });

        // altering non-fingerprint value has no effect
        expect(
            await getUserInfo(user.sid, {
                ...site_info,
                'cf-region': 'new-region',
                'cf-ipcity': 'new-city',
                'cf-ipcountry': 'new-country'
            })
        ).toBeGuestUser();

        // altering fingerprint value invalidates the session
        for (const mod of [{ 'user-agent': 'new-agent' }]) {
            let response = await request
                .get(config.getUrlFor('identity/api/auth/user/info'))
                .set('Cookie', [`sid=${user.sid}`])
                .set({ ...site_info, ...mod })
                .send()
                .catch((err) => err.response);
            expect(response.statusCode).toEqual(401);
        }
    });
});
