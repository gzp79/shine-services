import { expect, test } from '$fixtures/setup';
import { getPageProblem, getPageRedirectUrl } from '$lib/api/utils';

test.describe('Login and register guest', () => {
    test('Login with (tid: NULL, sid: NULL, captcha: NULL) shall fail with missing captcha', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, undefined);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.errorUrl + '?type=auth-error&status=400');
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-error',
                status: 400,
                extension: null,
                sensitive: expect.objectContaining({
                    type: 'captcha-not-provided',
                    detail: expect.stringContaining('Missing captcha token')
                })
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Login with (tid: NULL, sid: NULL, captcha: INVALID) shall fail with captcha validation', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, 'invalid');
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

    test('Login with (tid: VALID, sid: VALID, captcha: NULL) shall fail with missing captcha', async ({ api }) => {
        const testUser = await api.testUsers.createGuest();

        const response = await api.auth.loginWithGuestRequest(testUser.tid!, testUser.sid, undefined);
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
        expect(cookies.tid).toBeValidTID();
        expect(cookies.tid.value).toEqual(testUser.tid);
        expect(cookies.sid).toBeValidSID();
        expect(cookies.sid.value).toEqual(testUser.sid);
        expect(cookies.eid).toBeClearCookie();

        expect(await api.user.getUserInfo(testUser.sid)).toBeGuestUser();
    });

    for (const [tid, sid] of [
        [false, false],
        [false, true],
        [true, false],
        [true, true]
    ]) {
        test(`Login with (tid: ${tid ? 'VALID' : 'NULL'}, sid: ${sid ? 'VALID' : 'NULL'}, captcha: VALID) shall register a new user and switch session`, async ({
            api
        }) => {
            const testUser = await api.testUsers.createGuest();

            const response = await api.auth.loginWithGuestRequest(
                tid ? testUser.tid! : null,
                sid ? testUser.sid : null,
                null
            );
            expect(response).toHaveStatus(200);

            const text = await response.text();
            expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

            const cookies = response.cookies();
            expect(cookies.tid).toBeValidTID();
            expect(cookies.sid).toBeValidSID();
            expect(cookies.eid).toBeClearCookie();

            const userInfo = await api.user.getUserInfo(cookies.sid.value);
            expect(userInfo).toBeGuestUser();
            expect(userInfo.userId).not.toEqual(testUser.userId);
        });
    }
});
