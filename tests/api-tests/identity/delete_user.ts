import { expect, test } from '$fixtures/setup';
import { getPageProblem, getPageRedirectUrl } from '$lib/api/utils';
import { createUrl } from '$lib/utils';

test.describe('Delete user', () => {
    test.describe('validation errors', () => {
        test('Delete without a session shall fail with login required', async ({ api }) => {
            const response = await api.auth.deleteUserRequest(null, null);
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

            const cookies = response.cookies();
            expect(cookies.sid).toBeClearCookie();
        });

        test('Delete with missing confirmation shall fail with not-confirmed', async ({ api }) => {
            const user = await api.testUsers.createGuest();

            const response = await api.auth.deleteUserRequest(user.sid, null);
            expect(response).toHaveStatus(200);

            const text = await response.text();
            expect(getPageRedirectUrl(text)).toEqual(
                createUrl(api.auth.defaultRedirects.errorUrl, { errorType: 'auth-not-confirmed' })
            );
            expect(getPageProblem(text)).toEqual(
                expect.objectContaining({
                    type: 'auth-not-confirmed',
                    status: 409,
                    extension: null,
                    sensitive: null
                })
            );
        });

        test('Delete with wrong confirmation shall fail with not-confirmed', async ({ api }) => {
            const user = await api.testUsers.createGuest();

            const response = await api.auth.deleteUserRequest(user.sid, 'definitely-not-my-name');
            expect(response).toHaveStatus(200);

            const text = await response.text();
            expect(getPageRedirectUrl(text)).toEqual(
                createUrl(api.auth.defaultRedirects.errorUrl, { errorType: 'auth-not-confirmed' })
            );
            expect(getPageProblem(text)).toEqual(
                expect.objectContaining({
                    type: 'auth-not-confirmed',
                    status: 409,
                    extension: null,
                    sensitive: null
                })
            );
        });
    });

    test.describe('successful deletion', () => {
        test('Delete with correct confirmation shall redirect and clear session cookie', async ({ api }) => {
            const user = await api.testUsers.createGuest();

            const response = await api.auth.deleteUserRequest(user.sid, user.name);
            expect(response).toHaveStatus(200);

            const text = await response.text();
            expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);
            expect(getPageProblem(text)).toBeNull();

            const cookies = response.cookies();
            expect(cookies.sid).toBeClearCookie();
        });

        test('After deletion, existing session shall be invalidated', async ({ api }) => {
            const user = await api.testUsers.createGuest();
            const oldSid = user.sid;

            await api.auth.deleteUserRequest(user.sid, user.name);

            const infoResponse = await api.user.getUserInfoRequest(oldSid, null);
            expect(infoResponse).toHaveStatus(401);
        });

        test('After deletion, token login shall fail with session expired', async ({ api }) => {
            const user = await api.testUsers.createGuest();
            const oldTid = user.tid!;

            await api.auth.deleteUserRequest(user.sid, user.name);

            const response = await api.auth.loginWithTokenRequest(oldTid, null, null, null, null, null);
            expect(response).toHaveStatus(200);

            const text = await response.text();
            expect(getPageRedirectUrl(text)).toEqual(
                createUrl(api.auth.defaultRedirects.errorUrl, { errorType: 'auth-token-expired' })
            );
            expect(getPageProblem(text)).toEqual(
                expect.objectContaining({
                    type: 'auth-token-expired',
                    status: 401
                })
            );

            const cookies = response.cookies();
            expect(cookies.sid).toBeClearCookie();
        });
    });
});
