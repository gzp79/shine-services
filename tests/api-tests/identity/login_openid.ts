import { expect, test } from '$fixtures/setup';
import { ExternalUser } from '$lib/api/external_user';
import { getPageProblem, getPageRedirectUrl } from '$lib/api/utils';
import { MockServer } from '$lib/mocks/mock_server';
import OpenIdMockServer from '$lib/mocks/openid';
import { generateRandomString } from '$lib/string_utils';
import { createUrl, parseSignedCookie } from '$lib/utils';
import { randomUUID } from 'crypto';
import os from 'os';

test.describe('Check OpenId auth', () => {
    let mock: MockServer | undefined;

    const startMock = async (start = true): Promise<OpenIdMockServer> => {
        if (!mock) {
            mock = new OpenIdMockServer();
            if (start) {
                await mock.start();
            }
        }
        return mock as OpenIdMockServer;
    };

    test.afterEach(async () => {
        await mock?.stop();
        mock = undefined;
    });

    test('Auth with (parameters: NULL, session: NULL, external: NULL) shall fail', async ({ homeUrl, api }) => {
        await startMock();
        const response = await api.auth.authorizeWithOpenIdRequest(null, null, null, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(createUrl(`${homeUrl}/error`, { errorType: 'auth-error' }));
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-error',
                status: 400,
                extension: null,
                sensitive: expect.objectContaining({
                    type: 'external-missing-cookie'
                })
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Auth with (parameters: VALID, session: NULL, external: NULL) shall fail', async ({ homeUrl, api }) => {
        const mock = await startMock();
        const start = await api.auth.startLoginWithOpenId(mock, null);

        const response = await api.auth.authorizeWithOpenIdRequest(
            null,
            null,
            start.authParams.state,
            ExternalUser.newRandomUser('openid_flow').toCode({ nonce: start.authParams.nonce })
        );
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(createUrl(`${homeUrl}/error`, { errorType: 'auth-error' }));
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-error',
                status: 400,
                extension: null,
                sensitive: expect.objectContaining({
                    type: 'external-missing-cookie'
                })
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Auth with (parameters: NULL, session: NULL, external: VALID) shall fail', async ({ api, homeUrl }) => {
        const mock = await startMock();
        const start = await api.auth.startLoginWithOpenId(mock, null);

        const response = await api.auth.authorizeWithOpenIdRequest(null, start.eid, null, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(`${homeUrl}/error`, {
                errorType: 'auth-input-error'
            })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-input-error',
                status: 400,
                extension: null,
                sensitive: expect.objectContaining({
                    type: 'input-query-format',
                    detail: 'Failed to deserialize query string: missing field `code`'
                })
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Auth with (parameters: INVALID state, session: NULL, external: VALID) shall fail', async ({ api }) => {
        const mock = await startMock();
        const start = await api.auth.startLoginWithOpenId(mock, null);

        const response = await api.auth.authorizeWithOpenIdRequest(
            null,
            start.eid,
            'invalid',
            ExternalUser.newRandomUser('openid_flow').toCode({ nonce: start.authParams.nonce })
        );
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, { errorType: 'auth-error' })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-error',
                status: 400,
                extension: null,
                sensitive: expect.objectContaining({
                    type: 'external-invalid-csrf'
                })
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Auth with (parameters: INVALID code, session: NULL, external: VALID) shall fail', async ({ api }) => {
        const mock = await startMock();
        const start = await api.auth.startLoginWithOpenId(mock, null);

        const response = await api.auth.authorizeWithOpenIdRequest(null, start.eid, start.authParams.state, 'invalid');
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, { errorType: 'auth-internal-error' })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-internal-error',
                status: 500,
                extension: null,
                sensitive: expect.objectContaining({
                    type: 'external-exchange-failed',
                    sensitive: expect.stringContaining('server returned empty error response')
                })
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Auth with (parameters: INVALID nonce, session: NULL, external: VALID) shall fail', async ({ api }) => {
        const mock = await startMock();
        const start = await api.auth.startLoginWithOpenId(mock, null);

        const response = await api.auth.authorizeWithOpenIdRequest(
            null,
            start.eid,
            start.authParams.state,
            ExternalUser.newRandomUser('openid_flow').toCode({ nonce: 'invalid' })
        );
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, { errorType: 'auth-internal-error' })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-internal-error',
                status: 500,
                extension: null,
                sensitive: expect.objectContaining({
                    type: 'external-info-failed',
                    sensitive: expect.stringContaining('nonce mismatch')
                })
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Auth with failing 3rd party token service shall fail', async ({ api }) => {
        // mock is intentionally not started
        const mock = await startMock(false);
        const start = await api.auth.startLoginWithOpenId(mock, null);

        const response = await api.auth.authorizeWithOpenIdRequest(
            null,
            start.eid,
            start.authParams.state,
            ExternalUser.newRandomUser('openid_flow').toCode({ nonce: start.authParams.nonce })
        );
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, { errorType: 'auth-internal-error' })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-internal-error',
                status: 500,
                extension: null,
                sensitive: expect.objectContaining({
                    type: 'external-exchange-failed',
                    sensitive:
                        os.platform() === 'win32'
                            ? expect.stringContaining(
                                  'No connection could be made because the target machine actively refused it.'
                              )
                            : expect.stringContaining('Connection refused')
                })
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });
});

test.describe('Login with OpenId', () => {
    let mock!: OpenIdMockServer;

    test.beforeEach(async () => {
        mock = new OpenIdMockServer();
        await mock.start();
    });

    test.afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
    });

    test('Login without captcha shall fail and redirect to the default error page', async ({ api }) => {
        const response = await api.auth.loginWithOpenIdRequest(null, null, null, undefined);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, { errorType: 'auth-error' })
        );
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

    test('Login with wrong captcha shall fail and redirect to the default error page', async ({ api }) => {
        const response = await api.auth.loginWithOpenIdRequest(null, null, null, 'invalid');
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, { errorType: 'auth-error' })
        );
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

    test('Start login with (token: NULL, session: VALID) shall succeed and clear the current session', async ({
        api
    }) => {
        const { sid } = await api.auth.loginAsGuestUser();

        const response = await api.auth.loginWithOpenIdRequest(null, sid, null, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toStartWith(mock!.getUrlFor('authorize'));
        expect(getPageProblem(text)).toBeNull();

        const authCookies = response.cookies();
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeClearCookie();
        expect(authCookies.eid).toBeValidEID();

        const infoResponse = await api.user.getUserInfoRequest(sid, 'full');
        expect(infoResponse).toHaveStatus(401);
        expect(await infoResponse.parseProblem()).toEqual(
            expect.objectContaining({
                type: 'unauthorized',
                status: 401,
                extension: null,
                sensitive: 'sessionExpired'
            })
        );
    });

    test('Start login with (token: NULL, session: EXPIRED) shall succeed', async ({ api }) => {
        const { sid } = await api.auth.loginAsGuestUser();
        await api.auth.logout(sid, null, false);

        const response = await api.auth.loginWithOpenIdRequest(null, sid, null, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toStartWith(mock!.getUrlFor('authorize'));
        expect(getPageProblem(text)).toBeNull();

        const authCookies = response.cookies();
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeClearCookie();
        expect(authCookies.eid).toBeValidEID();
    });

    test('Start login with (token: VALID, session: NULL) shall succeed', async ({ api }) => {
        const { tid } = await api.auth.loginAsGuestUser();

        const response = await api.auth.loginWithOpenIdRequest(tid, null, null, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toStartWith(mock.getUrlFor('authorize'));
        expect(getPageProblem(text)).toBeNull();

        const authCookies = response.cookies();
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeClearCookie();
        expect(authCookies.eid).toBeValidEID();
    });

    test('Start login with (token: VALID, session: VALID) shall succeed and a new session is created ', async ({
        api
    }) => {
        const { tid, sid } = await api.auth.loginAsGuestUser();

        const response = await api.auth.loginWithOpenIdRequest(tid, sid, null, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toStartWith(mock!.getUrlFor('authorize'));
        expect(getPageProblem(text)).toBeNull();

        const authCookies = response.cookies();
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeClearCookie();
        expect(authCookies.eid).toBeValidEID();

        const infoResponse = await api.user.getUserInfoRequest(sid, 'full');
        expect(infoResponse).toHaveStatus(401);
        expect(await infoResponse.parseProblem()).toEqual(
            expect.objectContaining({
                type: 'unauthorized',
                status: 401,
                extension: null,
                sensitive: 'sessionExpired'
            })
        );
    });

    test('Login with (token: NULL, session: NULL, rememberMe: false) shall succeed and register a new user', async ({
        api
    }) => {
        const user = ExternalUser.newRandomUser('openid_flow');

        const cookies = await api.auth.loginWithOpenId(mock, user, false);
        expect(parseSignedCookie(cookies.tid).key).toBeUndefined();
        expect(parseSignedCookie(cookies.sid).key).toBeDefined();
        expect(parseSignedCookie(cookies.eid).key).toBeUndefined();

        expect((await api.user.getUserInfo(cookies.sid, 'fast')).name).toEqual(user.name);
        expect((await api.user.getUserInfo(cookies.sid, 'full')).name).toEqual(user.name);
    });

    test('Login with (token cookie: NULL, session: NULL, rememberMe: true) shall succeed and register a new user', async ({
        api
    }) => {
        const user = ExternalUser.newRandomUser('openid_flow');

        const cookies = await api.auth.loginWithOpenId(mock, user, true);
        expect(parseSignedCookie(cookies.tid).key).toBeDefined();
        expect(parseSignedCookie(cookies.sid).key).toBeDefined();
        expect(parseSignedCookie(cookies.eid).key).toBeUndefined();

        expect((await api.user.getUserInfo(cookies.sid, 'fast')).name).toEqual(user.name);
        expect((await api.user.getUserInfo(cookies.sid, 'full')).name).toEqual(user.name);
    });

    test('Login with occupied email shall fail', async ({ api }) => {
        const user = await api.testUsers.createLinked(mock, { email: generateRandomString(5) + '@example.com' });
        const newUser = new ExternalUser('openid_flow', randomUUID(), randomUUID(), user.externalUser!.email);

        const start = await api.auth.startLoginWithOpenId(mock, false);
        const response = await api.auth.authorizeWithOpenIdRequest(
            start.sid,
            start.eid,
            start.authParams.state,
            newUser.toCode({ nonce: start.authParams.nonce })
        );
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, { errorType: 'auth-register-email-conflict' })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-register-email-conflict',
                status: 409,
                extension: null,
                sensitive: null
            })
        );
    });

    test('Login with invalid email from external user shall have no email', async ({ api }) => {
        const user = await api.testUsers.createLinked(mock, { email: 'invalid' });
        expect(user.userInfo!.details!.email).toBeUndefined();
    });

    test('Login with the same external user shall succeed', async ({ api }) => {
        const user = await api.testUsers.createLinked(mock);

        const newUserCookies = await api.auth.loginWithOpenId(mock, user.externalUser!, null);
        expect(newUserCookies.sid, 'It shall be a new session').not.toEqual(user.sid);
        expect((await api.user.getUserInfo(newUserCookies.sid, 'fast')).userId).toEqual(user.userId);
        expect((await api.user.getUserInfo(newUserCookies.sid, 'full')).userId).toEqual(user.userId);
    });

    test('Login with the returned token shall be a success', async ({ api }) => {
        const user = await api.testUsers.createLinked(mock, { rememberMe: true });

        const newUserCookies = await api.auth.loginWithToken(user.tid!, null);
        expect(newUserCookies.sid, 'It shall be a new session').not.toEqual(user.sid);
        expect((await api.user.getUserInfo(newUserCookies.sid, 'fast')).userId).toEqual(user.userId);
        expect((await api.user.getUserInfo(newUserCookies.sid, 'full')).userId).toEqual(user.userId);
    });

    test('Login with long name shall be truncated', async ({ api }) => {
        const user = new ExternalUser(
            'openid_flow',
            randomUUID(),
            randomUUID() + 'make_sure_this_is_long_enough_to_be_truncated',
            generateRandomString(5) + '@example.com'
        );

        const cookies = await api.auth.loginWithOpenId(mock, user, false);
        expect((await api.user.getUserInfo(cookies.sid, 'fast')).name).toEqual(user.name.substring(0, 20));
        expect((await api.user.getUserInfo(cookies.sid, 'full')).name).toEqual(user.name.substring(0, 20));
    });

    test('Login with invalid redirect url shall fail', async ({ api }) => {
        const response = await api.auth
            .loginWithOpenIdRequest(null, null, false, null)
            .withParams({ redirectUrl: 'https://danger.com' });
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, {
                errorType: 'auth-input-error',
                redirectUrl: null
            })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-input-error',
                status: 400,
                extension: null,
                sensitive: expect.objectContaining({
                    type: 'input-validation',
                    detail: 'Input validation failed',
                    extension: expect.objectContaining({
                        redirectUrl: [
                            expect.objectContaining({
                                code: 'invalid-redirect-url',
                                message: 'Redirect URL is not allowed'
                            })
                        ]
                    })
                })
            })
        );
    });

    test('Login with invalid error url shall fail', async ({ api, homeUrl }) => {
        const response = await api.auth
            .loginWithOpenIdRequest(null, null, false, null)
            .withParams({ errorUrl: 'https://danger.com' });
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(`${homeUrl}/error`, {
                errorType: 'auth-input-error',
                redirectUrl: null
            })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-input-error',
                status: 400,
                extension: null,
                sensitive: expect.objectContaining({
                    type: 'input-validation',
                    detail: 'Input validation failed',
                    extension: expect.objectContaining({
                        errorUrl: [
                            expect.objectContaining({
                                code: 'invalid-redirect-url',
                                message: 'Redirect URL is not allowed'
                            })
                        ]
                    })
                })
            })
        );
    });
});

test.describe('Link to OpenId account', () => {
    let mock!: OpenIdMockServer;

    test.beforeEach(async () => {
        mock = new OpenIdMockServer();
        await mock.start();
    });

    test.afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
    });

    test('Linking without a session shall fail', async ({ api }) => {
        const response = await api.auth.linkWithOpenIdRequest(null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, { errorType: 'auth-login-required' })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-login-required',
                status: 401,
                extension: null,
                sensitive: null
            })
        );
    });

    test('Linking guest shall succeed', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        expect(user.userInfo!.isLinked).toBeFalsy();

        const externalUser = new ExternalUser(
            'openid_flow',
            randomUUID(),
            randomUUID(),
            generateRandomString(5) + '@example.com'
        );
        const start = await api.auth.startLinkWithOpenId(mock, user.sid);
        const response = await api.auth.authorizeWithOpenIdRequest(
            start.sid,
            start.eid,
            start.authParams.state,
            externalUser.toCode({ nonce: start.authParams.nonce })
        );
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);
        expect(getPageProblem(text)).toBeNull();

        expect((await api.user.getUserInfo(user.sid, 'fast')).isLinked).toBeTruthy();
        expect((await api.user.getUserInfo(user.sid, 'full')).isLinked).toBeTruthy();

        user.externalUser = externalUser;
        await user.refreshUserInfo();
        expect(user.userInfo!.isLinked).toBeTruthy();
        expect(user.userInfo!.details?.email).toBeUndefined(); // linking does not set an email for the user
    });

    test('Linking with occupied external user shall fail', async ({ api }) => {
        const user = await api.testUsers.createLinked(mock, { email: generateRandomString(5) + '@example.com' });

        const start = await api.auth.startLinkWithOpenId(mock, user.sid);
        const response = await api.auth.authorizeWithOpenIdRequest(
            start.sid,
            start.eid,
            start.authParams.state,
            user.externalUser!.toCode({ nonce: start.authParams.nonce })
        );
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, { errorType: 'auth-register-external-id-conflict' })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-register-external-id-conflict',
                status: 409,
                extension: null,
                sensitive: null
            })
        );
    });
});
