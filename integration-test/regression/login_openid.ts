import request from 'superagent';
import os from 'os';
import { getPageRedirectUrl } from '$lib/page_utils';
import { getCookies, getUserInfo, logout } from '$lib/auth_utils';
import config from '../test.config';
import { MockServer } from '$lib/mock_server';
import OpenIdMockServer from '$lib/mocks/openid';
import { ExternalUser, TestUser } from '$lib/user';
import {
    createGuestUser,
    loginWithOpenId,
    loginWithToken,
    requestLinkWithOpenId,
    requestLoginWithOpenId,
    startLoginWithOpenId
} from '$lib/login_utils';
import { generateRandomString } from '$lib/string_utils';
import { randomUUID } from 'crypto';

describe('Check OpenId auth', () => {
    let mock: MockServer | undefined;

    const startMock = async (): Promise<OpenIdMockServer> => {
        if (!mock) {
            mock = new OpenIdMockServer({
                tls: config.mockTLS,
                url: config.mockUrl,
                jwks: config.openidJWKS
            });
            await mock.start();
        }
        return mock as OpenIdMockServer;
    };

    afterEach(async () => {
        await mock?.stop();
        mock = undefined;
    });

    it('Auth with (parameters: NO, cookie: NO) shall fail', async () => {
        await startMock();
        const response = await request
            .get(config.getUrlFor('identity/auth/openid_flow/auth'))
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            'https://web.sandbox.com:8080/error?type=authError&status=400'
        );
        expect(response.text).toContain('&quot;MissingExternalLogin&quot;');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth with (parameters: VALID, cookie: NO) shall fail', async () => {
        const mock = await startMock();
        const { authParams } = await startLoginWithOpenId(mock);
        const response = await request
            .get(config.getUrlFor('identity/auth/openid_flow/auth'))
            .query({
                code: ExternalUser.newRandomUser().toCode({ nonce: authParams.nonce }),
                state: authParams.state
            })
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            'https://web.sandbox.com:8080/error?type=authError&status=400'
        );
        expect(response.text).toContain('&quot;MissingExternalLogin&quot;');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth with (parameters: NO, cookie: VALID) shall fail', async () => {
        const mock = await startMock();
        const { authParams, eid } = await startLoginWithOpenId(mock);
        const response = await request
            .get(config.getUrlFor('identity/auth/openid_flow/auth'))
            .set('Cookie', [`eid=${eid.value}`])
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=invalidInput&status=400'
        );
        expect(response.text).toContain('Failed to deserialize query string');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth with (parameters: INVALID state, cookie: VALID) shall fail', async () => {
        const mock = await startMock();
        const { authParams, eid } = await startLoginWithOpenId(mock);
        const response = await request
            .get(config.getUrlFor('identity/auth/openid_flow/auth'))
            .query({
                code: ExternalUser.newRandomUser().toCode({ nonce: authParams.nonce }),
                state: 'invalid'
            })
            .set('Cookie', [`eid=${eid.value}`])
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=400'
        );
        expect(response.text).toContain('&quot;InvalidCSRF&quot;');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth with (parameters: INVALID code, cookie: VALID) shall fail', async () => {
        const mock = await startMock();
        const { authParams, eid } = await startLoginWithOpenId(mock);
        const response = await request
            .get(config.getUrlFor('identity/auth/openid_flow/auth'))
            .query({
                code: 'invalid',
                state: authParams.state
            })
            .set('Cookie', [`eid=${eid.value}`])
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=500'
        );
        expect(response.text).toContain('Server returned empty error response');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth with failing 3rd party token service shall fail', async () => {
        // mock is intentionally not started
        const mock = new OpenIdMockServer({
            tls: config.mockTLS,
            url: config.mockUrl,
            jwks: config.openidJWKS
        });
        const { authParams, eid } = await startLoginWithOpenId(mock);
        const response = await request
            .get(config.getUrlFor('identity/auth/openid_flow/auth'))
            .query({
                code: ExternalUser.newRandomUser().toCode({ nonce: authParams.nonce }),
                state: authParams.state
            })
            .set('Cookie', [`eid=${eid.value}`])
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
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

    it('Login with (token cookie: NO, session: VALID) shall fail', async () => {
        const { sid } = await createGuestUser();

        const response = await request
            .get(config.getUrlFor('identity/auth/openid_flow/login'))
            .query({ ...config.defaultRedirects })
            .set('Cookie', [`sid=${sid.value}`])
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
        );
        expect(response.text).toContain('&quot;LogoutRequired&quot;');

        const authCookies = getCookies(response);
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeValidSID();
        expect(authCookies.sid.value).toEqual(sid.value);
        expect(authCookies.eid).toBeClearCookie();
    });

    it('Login with (token cookie: NO, session: EXPIRED) shall succeed', async () => {
        const { sid } = await createGuestUser();
        await logout(sid.value, false);

        const response = await request
            .get(config.getUrlFor('identity/auth/openid_flow/login'))
            .query({ ...config.defaultRedirects })
            .set('Cookie', [`sid=${sid.value}`])
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        const redirectUrl = getPageRedirectUrl(response.text);
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const authCookies = getCookies(response);
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeClearCookie();
        expect(authCookies.eid).toBeValidEID();
    });

    it('Login with (token cookie: VALID, session: VALID) shall succeed', async () => {
        const { tid } = await createGuestUser();

        const response = await request
            .get(config.getUrlFor('identity/auth/openid_flow/login'))
            .query({ ...config.defaultRedirects })
            .set('Cookie', [`tid=${tid.value}`])
            .send()
            .catch((err) => err.response);

        expect(response.statusCode).toEqual(200);
        const redirectUrl = getPageRedirectUrl(response.text);
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const authCookies = getCookies(response);
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeClearCookie();
        expect(authCookies.eid).toBeValidEID();
    });

    it('Login with (token cookie: NO, session: NO, rememberMe: false) shall succeed and register a new user', async () => {
        const user = ExternalUser.newRandomUser();
        const cookies = await loginWithOpenId(mock, user, false);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        const userInfo = await getUserInfo(cookies.sid.value);
        expect(userInfo.name).toEqual(user.name);
    });

    it('Login with (token cookie: NO, session: NO, rememberMe: true) shall succeed and register a new user', async () => {
        const user = ExternalUser.newRandomUser();
        const cookies = await loginWithOpenId(mock, user, true);
        expect(cookies.tid).toBeValidTID();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        const userInfo = await getUserInfo(cookies.sid.value);
        expect(userInfo.name).toEqual(user.name);
    });

    it('Login with occupied email shall fail', async () => {
        const user = await TestUser.createLinked(mock, { email: generateRandomString(5) + '@example.com' });

        const response = await requestLoginWithOpenId(
            mock,
            new ExternalUser(randomUUID(), randomUUID(), user.externalUser!.email)
        );
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=emailAlreadyUsed&status=409'
        );
    });

    it('Login with the same external user shall succeed', async () => {
        const user = await TestUser.createLinked(mock);

        const newUserCookies = await loginWithOpenId(mock, user.externalUser!);
        expect(newUserCookies.sid.value, 'It shall be a new session').not.toEqual(user.sid);
        expect((await getUserInfo(newUserCookies.sid.value)).userId).toEqual(user.userId);
    });

    it('Login with the returned token shall be a success', async () => {
        const user = await TestUser.createLinked(mock, { rememberMe: true });

        const newUserCookies = await loginWithToken(user.tid!);
        expect(newUserCookies.sid.value, 'It shall be a new session').not.toEqual(user.sid);
        expect((await getUserInfo(newUserCookies.sid.value)).userId).toEqual(user.userId);
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
        const response = await request
            .get(config.getUrlFor('identity/auth/openid_flow/link'))
            .query({ ...config.defaultRedirects })
            .send()
            .catch((err) => err.response);
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=loginRequired&status=401'
        );
    });

    it('Linking with occupied email shall succeed', async () => {
        const user = await TestUser.createLinked(mock, { email: generateRandomString(5) + '@example.com' });
        const response = await requestLinkWithOpenId(
            mock,
            user.sid,
            new ExternalUser(randomUUID(), randomUUID(), user.externalUser!.email)
        );
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);
    });

    it('Linking with occupied external user shall fail', async () => {
        const user = await TestUser.createLinked(mock, { email: generateRandomString(5) + '@example.com' });
        const response = await requestLinkWithOpenId(mock, user.sid, user.externalUser!);
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=providerAlreadyUsed&status=409'
        );
    });
});
