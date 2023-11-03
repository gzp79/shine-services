import request from 'superagent';
import { getPageRedirectUrl } from '$lib/page_utils';
import { UserInfo, getCookies, getUserInfo } from '$lib/auth_utils';
import config from '../test.config';
import { Cookie } from 'tough-cookie';

describe('Validate (interactive) token flow', () => {
    it('Login with invalid input should redirect to the default error page', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: 'invalid value' })
            .send();

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

    it('Login without token and redirectMe should redirect user to the login page', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query(config.defaultRedirects)
            .send();

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.loginUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Login without token and with false rememberMe should redirect user to the login page', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: false, ...config.defaultRedirects })
            .send();

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.loginUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Login without token with true rememberMe should register a new user', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: true, ...config.defaultRedirects })
            .send();

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeValidTID();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();
        expect(await getUserInfo(cookies.sid.value)).toBeGuestUser();
    });
});

describe('(Interactive) token flow', () => {
    let cookies: Record<string, Cookie> = undefined!;
    let userInfo: Omit<UserInfo, 'sessionLength'> = undefined!;

    beforeEach(async () => {
        console.log('Register a new user...');
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: true, ...config.defaultRedirects })
            .send();

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

    it('Login with a session shall be an error', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: true, ...config.defaultRedirects })
            .set('Cookie', [`sid=${cookies.sid.value}`])

            .send();

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

    it('Login with a session and a token is an error', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: true, ...config.defaultRedirects })
            .set('Cookie', [`sid=${cookies.sid.value}`, `tid=${cookies.tid.value}`])
            .send();

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

    it('Login with the token and without rememberMe shall be a success', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ ...config.defaultRedirects })
            .set('Cookie', [`tid=${cookies.tid.value}`])

            .send();

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

    it('Login with the token but altered agent shall be a failure', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ ...config.defaultRedirects })
            .set('Cookie', [`tid=${cookies.tid.value}`])
            .set('User-Agent', 'integration-test')
            .send();

        expect(response.statusCode).toEqual(200);
        console.log(response.text);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=400'
        );
        expect(response.text).toContain('&quot;InvalidToken&quot;');

        const newCookies = getCookies(response);
        expect(newCookies.tid).toBeClearCookie();
        expect(newCookies.sid).toBeClearCookie();
        expect(newCookies.eid).toBeClearCookie();
    });

    it('Login with the token and with false rememberMe shall be a success', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: false, ...config.defaultRedirects })
            .set('Cookie', [`tid=${cookies.tid.value}`])
            .send();

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

    it('Login with the token and with rememberMe shall be a success', async () => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query({ rememberMe: true, ...config.defaultRedirects })
            .set('Cookie', [`tid=${cookies.tid.value}`])
            .send();

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
});
