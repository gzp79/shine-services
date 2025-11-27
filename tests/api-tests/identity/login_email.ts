import { Api, expect, test } from '$fixtures/setup';
import { ApiRequest, ApiResponse, ProblemSchema } from '$lib/api/api';
import { getEmailLink, getPageProblem, getPageRedirectUrl } from '$lib/api/utils';
import MockSmtp from '$lib/mocks/mock_smtp';
import OpenIDMockServer from '$lib/mocks/openid';
import { createUrl } from '$lib/utils';
import assert from 'assert';
import { randomUUID } from 'crypto';

async function checkLoginResponse(response: ApiResponse, api: Api): Promise<ApiResponse> {
    const text = await response.text();
    expect(getPageRedirectUrl(text)).toEqual(
        createUrl(api.auth.defaultRedirects.errorUrl, {
            type: 'auth-email-login',
            status: 202,
            redirectUrl: null
        })
    );
    expect(getPageProblem(text)).toEqual(
        expect.objectContaining({
            type: 'auth-email-login',
            status: 202,
            extension: null,
            sensitive: null
        })
    );
    return response;
}

test.describe('Login with email for guest', () => {
    let mock: MockSmtp;

    test.beforeAll(async () => {
        mock = new MockSmtp();
        await mock.start();
    });

    test.afterAll(async () => {
        await mock.stop();
        mock = undefined!;
    });

    test('Creating emailAccess with api shall be rejected', async ({ api }) => {
        const user = await api.testUsers.createGuest();

        const response = await api.token.createTokenRequest(user.sid, 'emailAccess', 20, false);
        expect(response).toHaveStatus(400);

        const error = await response.parse(ProblemSchema);
        expect(error).toEqual(
            expect.objectContaining({
                type: 'input-body-format',
                status: 400,
                detail: expect.stringContaining(
                    'kind: unknown variant `emailAccess`, expected `persistent` or `singleAccess` at line 1'
                )
            })
        );
    });

    test('Login with invalid captcha shall be rejected', async ({ api }) => {
        const targetEmailAddress = `${randomUUID()}@example.com`;

        const response = await api.auth.loginWithEmailRequest(targetEmailAddress, false, 'invalid');
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, {
                type: 'auth-error',
                status: 400,
                redirectUrl: api.auth.defaultRedirects.redirectUrl
            })
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

    for (const lang of ['en', 'hu', undefined]) {
        for (const rememberMe of [true, false, null]) {
            test(`Login (lang: ${lang}, rememberMe: ${rememberMe}) shall create a new user `, async ({
                api,
                appDomain,
                identityUrl
            }) => {
                const targetEmailAddress = `${randomUUID()}@example.com`;

                const mailPromise = mock.waitMail();
                const response = await api.auth
                    .loginWithEmailRequest(targetEmailAddress, rememberMe, null)
                    .withParams(api.auth.defaultRedirects)
                    .withParams(lang ? { lang } : {});
                const mail = await mailPromise;

                await checkLoginResponse(response, api);

                expect(mail).toHaveMailTo(targetEmailAddress);
                expect(mail).toHaveMailFrom(`no-replay@${appDomain}`);
                let content;
                switch (lang) {
                    case 'hu':
                        content = 'Köszönjük, hogy regisztráltál';
                        break;
                    default:
                    case 'en':
                        content = 'Thank you for registering';
                        break;
                }
                expect(mail.text).toContain(content);

                const url = getEmailLink(mail);
                expect(url).toStartWith(`${identityUrl}/auth/token/login?`);

                const loginResponse = await ApiRequest.get(url);
                expect(loginResponse).toHaveStatus(200);

                const loginText = await loginResponse.text();
                expect(getPageRedirectUrl(loginText)).toEqual(api.auth.defaultRedirects.redirectUrl);

                const loginCookies = loginResponse.cookies();
                if (rememberMe) expect(loginCookies.tid).toBeValidTID();
                else expect(loginCookies.tid).toBeClearCookie();
                expect(loginCookies.sid).toBeValidSID();
                expect(loginCookies.eid).toBeClearCookie();

                for (const infoMethod of ['fast', 'full'] as const) {
                    const userInfo = await api.user.getUserInfo(loginCookies.sid.value, infoMethod);
                    expect(userInfo.isEmailConfirmed).toBeTruthy();
                    expect(userInfo.isLinked).toBeFalsy();
                }
            });
        }
    }

    test('Login shall clear current cookies', async ({ api }) => {
        const testUser = await api.testUsers.createGuest();
        const targetEmailAddress = `${randomUUID()}@example.com`;

        const mailPromise = mock.waitMail();
        const response = await api.auth
            .loginWithEmailRequest(targetEmailAddress, false, null)
            .withCookies({ sid: testUser.sid!, tid: testUser.tid! });
        await mailPromise;

        await checkLoginResponse(response, api);

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Login with invalid link-captcha shall be rejected', async ({ api, homeUrl }) => {
        const targetEmailAddress = `${randomUUID()}@example.com`;

        const mailPromise = mock.waitMail();
        const response = await api.auth.loginWithEmailRequest(targetEmailAddress, false, null);
        await checkLoginResponse(response, api);
        const mail = await mailPromise;

        const link = getEmailLink(mail);
        const linkUrl = new URL(link);
        linkUrl.searchParams.set('captcha', 'invalid');

        const loginResponse = await ApiRequest.get(linkUrl.toString());
        expect(loginResponse).toHaveStatus(200);
        const loginText = await loginResponse.text();
        expect(getPageRedirectUrl(loginText)).toEqual(
            createUrl(`${homeUrl}/error`, {
                type: 'auth-token-expired',
                status: 401,
                redirectUrl: api.auth.defaultRedirects.redirectUrl
            })
        );
        expect(getPageProblem(loginText)).toEqual(
            expect.objectContaining({
                type: 'auth-token-expired',
                status: 401,
                extension: null,
                sensitive: 'emailConflict'
            })
        );
    });

    test('Login with invalid link-token shall be rejected', async ({ api, homeUrl }) => {
        const targetEmailAddress = `${randomUUID()}@example.com`;

        const mailPromise = mock.waitMail();
        const response = await api.auth.loginWithEmailRequest(targetEmailAddress, false, null);
        await checkLoginResponse(response, api);
        const mail = await mailPromise;

        const link = getEmailLink(mail);
        const linkUrl = new URL(link);
        linkUrl.searchParams.set('token', 'invalid');

        const loginResponse = await ApiRequest.get(linkUrl.toString());
        expect(loginResponse).toHaveStatus(200);
        const loginText = await loginResponse.text();
        expect(getPageRedirectUrl(loginText)).toEqual(
            createUrl(`${homeUrl}/error`, {
                type: 'auth-token-expired',
                status: 401,
                redirectUrl: api.auth.defaultRedirects.redirectUrl
            })
        );
        expect(getPageProblem(loginText)).toEqual(
            expect.objectContaining({
                type: 'auth-token-expired',
                status: 401,
                extension: null,
                sensitive: 'expiredToken'
            })
        );
    });

    test('Login with token twice shall be rejected', async ({ api, homeUrl }) => {
        const targetEmailAddress = `${randomUUID()}@example.com`;

        const mailPromise = mock.waitMail();
        const response = await api.auth.loginWithEmailRequest(targetEmailAddress, false, null);
        await checkLoginResponse(response, api);
        const mail = await mailPromise;

        const link = getEmailLink(mail);
        const linkUrl = new URL(link);

        await test.step('Login with token 1st time', async () => {
            const loginResponse = await ApiRequest.get(linkUrl.toString());
            expect(loginResponse).toHaveStatus(200);
            const loginText = await loginResponse.text();
            expect(getPageProblem(loginText)).toBeNull();
        });

        await test.step('Login with token 2nd time', async () => {
            const loginResponse = await ApiRequest.get(linkUrl.toString());
            expect(loginResponse).toHaveStatus(200);
            const loginText = await loginResponse.text();
            expect(getPageRedirectUrl(loginText)).toEqual(
                createUrl(`${homeUrl}/error`, {
                    type: 'auth-token-expired',
                    status: 401,
                    redirectUrl: api.auth.defaultRedirects.redirectUrl
                })
            );
            expect(getPageProblem(loginText)).toEqual(
                expect.objectContaining({
                    type: 'auth-token-expired',
                    status: 401,
                    extension: null,
                    sensitive: 'expiredToken'
                })
            );
        });
    });

    test('Login with token as auth header shall be rejected and revoked', async ({ api, homeUrl }) => {
        const targetEmailAddress = `${randomUUID()}@example.com`;

        const mailPromise = mock.waitMail();
        const response = await api.auth.loginWithEmailRequest(targetEmailAddress, false, null);
        await checkLoginResponse(response, api);
        const mail = await mailPromise;

        const link = getEmailLink(mail);
        const linkUrl = new URL(link);

        await test.step('Login with token as auth header', async () => {
            const token = linkUrl.searchParams.get('token')!;
            const loginResponse = await api.auth.loginWithTokenRequest(null, null, null, token, null, null);
            expect(loginResponse).toHaveStatus(200);
            const loginText = await loginResponse.text();
            expect(getPageRedirectUrl(loginText)).toEqual(
                createUrl(api.auth.defaultRedirects.errorUrl, {
                    type: 'auth-token-expired',
                    status: 401,
                    redirectUrl: api.auth.defaultRedirects.redirectUrl
                })
            );
            expect(getPageProblem(loginText)).toEqual(
                expect.objectContaining({
                    type: 'auth-token-expired',
                    status: 401,
                    extension: null,
                    sensitive: 'invalidToken'
                })
            );
        });

        await test.step('Login with token 2nd time', async () => {
            const loginResponse = await ApiRequest.get(linkUrl.toString());
            expect(loginResponse).toHaveStatus(200);
            const loginText = await loginResponse.text();
            expect(getPageRedirectUrl(loginText)).toEqual(
                createUrl(`${homeUrl}/error`, {
                    type: 'auth-token-expired',
                    status: 401,
                    redirectUrl: api.auth.defaultRedirects.redirectUrl
                })
            );
            expect(getPageProblem(loginText)).toEqual(
                expect.objectContaining({
                    type: 'auth-token-expired',
                    status: 401,
                    extension: null,
                    sensitive: 'expiredToken'
                })
            );
        });
    });
});

test.describe('Login with email for returning user', () => {
    let mockSmtp: MockSmtp;
    let mockOIDC: OpenIDMockServer;

    test.beforeAll(async () => {
        mockSmtp = new MockSmtp();
        await mockSmtp.start();
        mockOIDC = new OpenIDMockServer();
        await mockOIDC.start();
    });

    test.afterAll(async () => {
        await mockSmtp.stop();
        mockSmtp = undefined!;
        await mockOIDC.stop();
        mockOIDC = undefined!;
    });

    for (const lang of ['en', 'hu', undefined]) {
        for (const rememberMe of [true, false, null]) {
            test(`Login (lang: ${lang}, rememberMe: ${rememberMe}) shall create a new user `, async ({
                api,
                appDomain,
                identityUrl
            }) => {
                const targetEmailAddress = `${randomUUID()}@example.com`;
                const testUser = await api.testUsers.createLinked(mockOIDC, { email: targetEmailAddress });
                assert(testUser.userInfo);
                expect(testUser.userInfo.isEmailConfirmed).toBeFalsy();

                const mailPromise = mockSmtp.waitMail();
                const response = await api.auth
                    .loginWithEmailRequest(targetEmailAddress, rememberMe, null)
                    .withParams(api.auth.defaultRedirects)
                    .withParams(lang ? { lang } : {});
                const mail = await mailPromise;

                await checkLoginResponse(response, api);

                expect(mail).toHaveMailTo(targetEmailAddress);
                expect(mail).toHaveMailFrom(`no-replay@${appDomain}`);
                let content;
                switch (lang) {
                    case 'hu':
                        content = 'Egy egyszeri bejelentkezési linket kértél';
                        break;
                    default:
                    case 'en':
                        content = 'You requested a one-time login link for';
                        break;
                }
                expect(mail.text).toContain(content);

                const url = getEmailLink(mail);
                expect(url).toStartWith(`${identityUrl}/auth/token/login?`);

                const loginResponse = await ApiRequest.get(url);
                expect(loginResponse).toHaveStatus(200);

                const loginText = await loginResponse.text();
                expect(getPageRedirectUrl(loginText)).toEqual(api.auth.defaultRedirects.redirectUrl);

                const loginCookies = loginResponse.cookies();
                if (rememberMe) expect(loginCookies.tid).toBeValidTID();
                else expect(loginCookies.tid).toBeClearCookie();
                expect(loginCookies.sid).toBeValidSID();
                expect(loginCookies.eid).toBeClearCookie();

                for (const infoMethod of ['fast', 'full'] as const) {
                    const userInfo = await api.user.getUserInfo(loginCookies.sid.value, infoMethod);
                    expect(userInfo.userId).toEqual(testUser.userId);
                    expect(userInfo.isEmailConfirmed).toBeTruthy();
                    expect(userInfo.isLinked).toBeTruthy();
                }
            });
        }
    }

    test('Login with email should confirm the email address', async ({ api }) => {
        const targetEmailAddress = `${randomUUID()}@example.com`;
        const testUser = await api.testUsers.createLinked(mockOIDC, { email: targetEmailAddress });
        assert(testUser.userInfo);
        expect(testUser.userInfo.isEmailConfirmed).toBeFalsy();

        const mailPromise = mockSmtp.waitMail();
        const response = await api.auth
            .loginWithEmailRequest(targetEmailAddress, false, null)
            .withParams(api.auth.defaultRedirects);
        const mail = await mailPromise;

        await checkLoginResponse(response, api);

        const url = getEmailLink(mail);
        const loginResponse = await ApiRequest.get(url);
        expect(loginResponse).toHaveStatus(200);

        for (const infoMethod of ['fast', 'full'] as const) {
            const userInfo = await api.user.getUserInfo(testUser.sid, infoMethod);
            expect(userInfo.userId).toEqual(testUser.userId);
            expect(userInfo.isEmailConfirmed).toBeTruthy();
            expect(userInfo.isLinked).toBeTruthy();
        }
    });

    test('Login with token twice shall be rejected', async ({ api, homeUrl }) => {
        const targetEmailAddress = `${randomUUID()}@example.com`;
        const testUser = await api.testUsers.createLinked(mockOIDC, { email: targetEmailAddress });
        assert(testUser.email);
        expect(testUser.email).toEqual(targetEmailAddress);

        const mailPromise = mockSmtp.waitMail();
        const response = await api.auth.loginWithEmailRequest(testUser.email, false, null);
        await checkLoginResponse(response, api);
        const mail = await mailPromise;

        const link = getEmailLink(mail);
        const linkUrl = new URL(link);

        await test.step('Login with token 1st time', async () => {
            const loginResponse = await ApiRequest.get(linkUrl.toString());
            expect(loginResponse).toHaveStatus(200);
            const loginText = await loginResponse.text();
            expect(getPageProblem(loginText)).toBeNull();
        });

        await test.step('Login with token 2nd time', async () => {
            const loginResponse = await ApiRequest.get(linkUrl.toString());
            expect(loginResponse).toHaveStatus(200);
            const loginText = await loginResponse.text();
            expect(getPageRedirectUrl(loginText)).toEqual(
                createUrl(`${homeUrl}/error`, {
                    type: 'auth-token-expired',
                    status: 401,
                    redirectUrl: api.auth.defaultRedirects.redirectUrl
                })
            );
            expect(getPageProblem(loginText)).toEqual(
                expect.objectContaining({
                    type: 'auth-token-expired',
                    status: 401,
                    extension: null,
                    sensitive: 'expiredToken'
                })
            );
        });
    });

    test('Updating email should invalidate the login link', async ({ api, homeUrl }) => {
        const targetEmailAddress = `${randomUUID()}@example.com`;
        const updatedEmailAddress = `${randomUUID()}@example.com`;

        const testUser = await api.testUsers.createLinked(mockOIDC, { email: targetEmailAddress });
        assert(testUser.email);
        expect(testUser.email).toEqual(targetEmailAddress);

        let linkOld;
        {
            const mailPromise = mockSmtp.waitMail();
            const response = await api.auth.loginWithEmailRequest(testUser.email, false, null);
            await checkLoginResponse(response, api);
            const mail = await mailPromise;

            const link = getEmailLink(mail);
            linkOld = new URL(link);
        }

        await testUser.changeEmail(mockSmtp, updatedEmailAddress);
        assert(testUser.email);
        expect(testUser.email).toEqual(updatedEmailAddress);

        let linkNew;
        {
            const mailPromise = mockSmtp.waitMail();
            const response = await api.auth.loginWithEmailRequest(testUser.email, false, null);
            await checkLoginResponse(response, api);
            const mail = await mailPromise;

            const link = getEmailLink(mail);
            linkNew = new URL(link);
        }

        await test.step('Login with old link shall fail', async () => {
            const loginResponse = await ApiRequest.get(linkOld.toString());
            expect(loginResponse).toHaveStatus(200);
            const loginText = await loginResponse.text();
            expect(getPageRedirectUrl(loginText)).toEqual(
                createUrl(`${homeUrl}/error`, {
                    type: 'auth-token-expired',
                    status: 401,
                    redirectUrl: api.auth.defaultRedirects.redirectUrl
                })
            );
            expect(getPageProblem(loginText)).toEqual(
                expect.objectContaining({
                    type: 'auth-token-expired',
                    status: 401,
                    extension: null,
                    sensitive: 'emailConflict'
                })
            );
        });

        await test.step('Login with old link shall succeed', async () => {
            const loginResponse = await ApiRequest.get(linkNew.toString());
            expect(loginResponse).toHaveStatus(200);
            const loginText = await loginResponse.text();
            expect(getPageProblem(loginText)).toBeNull();
        });
    });
});
