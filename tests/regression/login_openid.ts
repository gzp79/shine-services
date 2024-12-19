import { randomUUID } from 'crypto';
import os from 'os';
import api from '$lib/api/api';
import { ExternalUser } from '$lib/api/external_user';
import { MockServer } from '$lib/mock_server';
import OpenIdMockServer from '$lib/mocks/openid';
import { getCookies, getPageRedirectUrl } from '$lib/response_utils';
import { generateRandomString } from '$lib/string_utils';
import { TestUser } from '$lib/test_user';
import { parseSignedCookie } from '$lib/utils';
import config from '../test.config';

describe('Check OpenId auth', () => {
    let mock: MockServer | undefined;

    const startMock = async (start = true): Promise<OpenIdMockServer> => {
        if (!mock) {
            mock = new OpenIdMockServer({
                tls: config.mockTLS,
                url: config.mockUrl,
                jwks: config.openidJWKS
            });
            if (start) {
                await mock.start();
            }
        }
        return mock as OpenIdMockServer;
    };

    afterEach(async () => {
        await mock?.stop();
        mock = undefined;
    });

    it('Auth with (parameters: NULL, session: NULL, external: NULL) shall fail', async () => {
        await startMock();
        const response = await api.request.authorizeWithOpenId(null, null, null, null);

        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            'https://web.sandbox.com:8443/error?type=authError&status=400'
        );
        expect(response.text).toContain('&quot;MissingExternalLoginCookie&quot;');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth with (parameters: VALID, session: NULL, external: NULL) shall fail', async () => {
        const mock = await startMock();
        const start = await api.auth.startLoginWithOpenId(mock, null);

        const response = await api.request.authorizeWithOpenId(
            null,
            null,
            start.authParams.state,
            ExternalUser.newRandomUser().toCode({ nonce: start.authParams.nonce })
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            'https://web.sandbox.com:8443/error?type=authError&status=400'
        );
        expect(response.text).toContain('&quot;MissingExternalLoginCookie&quot;');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth with (parameters: NULL, session: NULL, external: VALID) shall fail', async () => {
        const mock = await startMock();
        const start = await api.auth.startLoginWithOpenId(mock, null);

        const response = await api.request.authorizeWithOpenId(null, start.eid, null, null);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=invalidInput&status=400'
        );
        expect(response.text).toContain('Failed to deserialize query string');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth with (parameters: INVALID state, session: NULL, external: VALID) shall fail', async () => {
        const mock = await startMock();
        const start = await api.auth.startLoginWithOpenId(mock, null);

        const response = await api.request.authorizeWithOpenId(
            null,
            start.eid,
            'invalid',
            ExternalUser.newRandomUser().toCode({ nonce: start.authParams.nonce })
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=400'
        );
        expect(response.text).toContain('&quot;InvalidCSRF&quot;');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth with (parameters: INVALID code, session: NULL, external: VALID) shall fail', async () => {
        const mock = await startMock();
        const start = await api.auth.startLoginWithOpenId(mock, null);

        const response = await api.request.authorizeWithOpenId(
            null,
            start.eid,
            start.authParams.state,
            'invalid'
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=500'
        );
        expect(response.text).toContain('server returned empty error response');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth with (parameters: INVALID nonce, session: NULL, external: VALID) shall fail', async () => {
        const mock = await startMock();
        const start = await api.auth.startLoginWithOpenId(mock, null);

        const response = await api.request.authorizeWithOpenId(
            null,
            start.eid,
            start.authParams.state,
            ExternalUser.newRandomUser().toCode({ nonce: 'invalid' })
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=400'
        );
        expect(response.text).toContain('&quot;InvalidNonce(\\&quot;');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth with failing 3rd party token service shall fail', async () => {
        // mock is intentionally not started
        const mock = await startMock(false);
        const start = await api.auth.startLoginWithOpenId(mock, null);

        const response = await api.request.authorizeWithOpenId(
            null,
            start.eid,
            start.authParams.state,
            ExternalUser.newRandomUser().toCode({ nonce: start.authParams.nonce })
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=500'
        );
        if (os.platform() === 'win32') {
            expect(response.text).toContain(
                'No connection could be made because the target machine actively refused it.'
            );
        } else {
            expect(response.text).toContain('Connection refused');
        }

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });
});

describe('Login with OpenId', () => {
    let mock!: OpenIdMockServer;

    beforeEach(async () => {
        mock = new OpenIdMockServer({
            tls: config.mockTLS,
            url: config.mockUrl,
            jwks: config.openidJWKS
        });
        await mock.start();
    });

    afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
    });

    it('Login without captcha shall fail and redirect to the default error page', async () => {
        const response = await api.request.loginWithOpenId(null, null, null, undefined);
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

    it('Login with wrong captcha shall fail and redirect to the default error page', async () => {
        const response = await api.request.loginWithOpenId(null, null, null, 'invalid');
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

    it('Start login with (token: NULL, session: VALID) shall fail', async () => {
        const { sid } = await api.auth.loginAsGuestUser();

        const response = await api.request.loginWithOpenId(null, sid, null, null);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
        );
        expect(response.text).toContain('&quot;LogoutRequired&quot;');

        const authCookies = getCookies(response);
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeValidSID();
        expect(authCookies.sid.value).toEqual(sid);
        expect(authCookies.eid).toBeClearCookie();
    });

    it('Start login with (token: NULL, session: EXPIRED) shall succeed', async () => {
        const { sid } = await api.auth.loginAsGuestUser();
        await api.auth.logout(sid, false);

        const response = await api.request.loginWithOpenId(null, sid, null, null);
        expect(response).toHaveStatus(200);
        const redirectUrl = getPageRedirectUrl(response.text);
        expect(redirectUrl).toStartWith(mock!.getUrlFor('authorize'));

        const authCookies = getCookies(response);
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeClearCookie();
        expect(authCookies.eid).toBeValidEID();
    });

    it('Start login with (token: VALID, session: NULL) shall succeed', async () => {
        const { tid } = await api.auth.loginAsGuestUser();

        const response = await api.request.loginWithOpenId(tid, null, null, null);
        expect(response).toHaveStatus(200);
        const redirectUrl = getPageRedirectUrl(response.text);
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const authCookies = getCookies(response);
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeClearCookie();
        expect(authCookies.eid).toBeValidEID();
    });

    it('Start login with (token: VALID, session: VALID) shall succeed', async () => {
        const { tid, sid } = await api.auth.loginAsGuestUser();

        const response = await api.request.loginWithOpenId(tid, sid, null, null);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
        );

        const authCookies = getCookies(response);
        expect(authCookies.tid).toBeValidTID();
        expect(authCookies.tid.value).toEqual(tid);
        expect(authCookies.sid).toBeValidSID();
        expect(authCookies.sid.value).toEqual(sid);
        expect(authCookies.eid).toBeClearCookie();
    });

    it('Login with (token: NULL, session: NULL, rememberMe: false) shall succeed and register a new user', async () => {
        const user = ExternalUser.newRandomUser();

        const cookies = await api.auth.loginWithOpenId(mock, user, false);
        expect(parseSignedCookie(cookies.tid).key).toBeUndefined();
        expect(parseSignedCookie(cookies.sid).key).toBeString();
        expect(parseSignedCookie(cookies.eid).key).toBeUndefined();
        const userInfo = await api.user.getUserInfo(cookies.sid);
        expect(userInfo.name).toEqual(user.name);
    });

    it('Login with (token cookie: NULL, session: NULL, rememberMe: true) shall succeed and register a new user', async () => {
        const user = ExternalUser.newRandomUser();

        const cookies = await api.auth.loginWithOpenId(mock, user, true);
        expect(parseSignedCookie(cookies.tid).key).toBeString();
        expect(parseSignedCookie(cookies.sid).key).toBeString();
        expect(parseSignedCookie(cookies.eid).key).toBeUndefined();
        const userInfo = await api.user.getUserInfo(cookies.sid);
        expect(userInfo.name).toEqual(user.name);
    });

    it('Login with occupied email shall fail', async () => {
        const user = await TestUser.createLinked(mock, { email: generateRandomString(5) + '@example.com' });
        const newUser = new ExternalUser(randomUUID(), randomUUID(), user.externalUser!.email);

        const start = await api.auth.startLoginWithOpenId(mock, false);
        const response = await api.request.authorizeWithOpenId(
            start.sid,
            start.eid,
            start.authParams.state,
            newUser.toCode({ nonce: start.authParams.nonce })
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=emailAlreadyUsed&status=409'
        );
    });

    it('Login with the same external user shall succeed', async () => {
        const user = await TestUser.createLinked(mock);
        const newUserCookies = await api.auth.loginWithOpenId(mock, user.externalUser!, null);
        expect(newUserCookies.sid, 'It shall be a new session').not.toEqual(user.sid);
        expect((await api.user.getUserInfo(newUserCookies.sid)).userId).toEqual(user.userId);
    });

    it('Login with the returned token shall be a success', async () => {
        const user = await TestUser.createLinked(mock, { rememberMe: true });
        const newUserCookies = await api.auth.loginWithToken(user.tid!, null);
        expect(newUserCookies.sid, 'It shall be a new session').not.toEqual(user.sid);
        expect((await api.user.getUserInfo(newUserCookies.sid)).userId).toEqual(user.userId);
    });
});

describe('Link to OpenId account', () => {
    let mock!: OpenIdMockServer;

    beforeEach(async () => {
        mock = new OpenIdMockServer({
            tls: config.mockTLS,
            url: config.mockUrl,
            jwks: config.openidJWKS
        });
        await mock.start();
    });

    afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
    });

    it('Linking without a session shall fail', async () => {
        const response = await api.request.linkWithOpenId(null);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=loginRequired&status=401'
        );
    });

    it('Linking guest shall succeed', async () => {
        const user = await TestUser.createGuest();
        expect(user.userInfo!.isLinked).toBeFalse();

        const externalUser = new ExternalUser(
            randomUUID(),
            randomUUID(),
            generateRandomString(5) + '@example.com'
        );
        const start = await api.auth.startLinkWithOpenId(mock, user.sid);
        const response = await api.request.authorizeWithOpenId(
            start.sid,
            start.eid,
            start.authParams.state,
            externalUser.toCode({ nonce: start.authParams.nonce })
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

        user.externalUser = externalUser;
        await user.refreshUserInfo();
        expect(user.userInfo!.isLinked).toBeTrue();
    });

    it('Linking with occupied email shall succeed', async () => {
        const user = await TestUser.createLinked(mock, { email: generateRandomString(5) + '@example.com' });
        const newUser = new ExternalUser(randomUUID(), randomUUID(), user.externalUser!.email);

        const start = await api.auth.startLinkWithOpenId(mock, user.sid);
        const response = await api.request.authorizeWithOpenId(
            start.sid,
            start.eid,
            start.authParams.state,
            newUser.toCode({ nonce: start.authParams.nonce })
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);
    });

    it('Linking with occupied external user shall fail', async () => {
        const user = await TestUser.createLinked(mock, { email: generateRandomString(5) + '@example.com' });

        const start = await api.auth.startLinkWithOpenId(mock, user.sid);
        const response = await api.request.authorizeWithOpenId(
            start.sid,
            start.eid,
            start.authParams.state,
            user.externalUser!.toCode({ nonce: start.authParams.nonce })
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=providerAlreadyUsed&status=409'
        );
    });
});
