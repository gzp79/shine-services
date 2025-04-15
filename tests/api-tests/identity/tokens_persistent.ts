import { expect, test } from '$fixtures/setup';
import { ProblemSchema } from '$lib/api/api';
import { TestUser } from '$lib/api/test_user';
import { getPageProblem, getPageRedirectUrl } from '$lib/api/utils';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { createUrl } from '$lib/utils';

test.describe('Persistent token', () => {
    let mock: OAuth2MockServer = undefined!;
    let user: TestUser = undefined!;

    test.beforeEach(async ({ api }) => {
        mock = new OAuth2MockServer();
        await mock.start();
        user = await api.testUsers.createLinked(mock);
    });

    test.afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
        user = undefined!;
    });

    test('Creating a persistent token without session shall fail', async ({ api }) => {
        const response = await api.token.createTokenRequest(null, 'persistent', 20, false);
        expect(response).toHaveStatus(401);
    });

    test('Token creation with a too long duration shall be rejected', async ({ api }) => {
        const response = await api.token.createTokenRequest(user.sid, 'persistent', 31536001, false);
        expect(response).toHaveStatus(400);

        const error = await response.parse(ProblemSchema);
        expect(error).toEqual(
            expect.objectContaining({
                type: 'input-validation',
                status: 400,
                extension: expect.objectContaining({
                    time_to_live: [expect.objectContaining({ code: 'range' })]
                })
            })
        );
    });

    test('A successful login with a persistent token shall change the current user', async ({ api }) => {
        const token = await api.token.createPersistentToken(user.sid, 120, false);

        const response = await api.auth.loginWithTokenRequest(null, null, null, token.token, false, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();
    });

    test('A failed login with an header token shall clear the current user', async ({ api }) => {
        const response = await api.auth.loginWithTokenRequest(null, null, null, 'invalid', false, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, {
                type: 'auth-token-expired',
                status: 401,
                redirectUrl: api.auth.defaultRedirects.redirectUrl
            })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-token-expired',
                status: 401,
                extension: null,
                sensitive: 'expiredToken'
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('Using token in query shall fail and revoke the token', async ({ api }) => {
        const token = await api.token.createPersistentToken(user.sid, 120, false);
        expect((await api.token.getTokens(user.sid)).map((x) => x.tokenHash)).toEqual([token.tokenHash]);

        const response = await api.auth.loginWithTokenRequest(null, null, token.token, null, null, null);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, {
                type: 'auth-token-expired',
                status: 401,
                redirectUrl: api.auth.defaultRedirects.redirectUrl
            })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-token-expired',
                status: 401,
                extension: null,
                sensitive: 'invalidToken'
            })
        );

        // the user is cleared
        // note: session is not revoked only b/c the invalid login did not get the session
        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();

        // token is revoked
        expect(await api.token.getTokens(user.sid)).toBeEmptyValue();
    });

    test('Login with client-bound token with altered fingerprint shall fail and revoke token', async ({ api }) => {
        const token = await api.token.createPersistentToken(user.sid, 120, true);
        expect((await api.token.getTokens(user.sid)).map((x) => x.tokenHash)).toEqual([token.tokenHash]);

        const response = await api.auth
            .loginWithTokenRequest(null, null, null, token.token, false, null)
            .withHeaders({ 'user-agent': 'agent2' });
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, {
                type: 'auth-token-expired',
                status: 401,
                redirectUrl: api.auth.defaultRedirects.redirectUrl
            })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-token-expired',
                status: 401,
                extension: null,
                sensitive: 'expiredToken'
            })
        );

        // token is revoked
        expect(await api.token.getTokens(user.sid)).toBeEmptyValue();
    });

    test('Using persistent token multiple times shall work', async ({ api }) => {
        const now = new Date().getTime();
        const token = await api.token.createPersistentToken(user.sid, 120, false);
        expect(token.expireAt).toBeAfter(new Date(now + 120 * 1000));
        expect(token.expireAt).toBeBefore(new Date(now + 130 * 1000));

        for (let i = 0; i < 3; i++) {
            const response1 = await api.auth.loginWithTokenRequest(null, null, null, token.token, false, null);
            expect(response1).toHaveStatus(200);
            const sid1 = response1.cookies().sid.value;
            const user1 = await api.user.getUserInfo(sid1, 'full');
            expect(user1.userId, 'It shall be the same user').toEqual(user.userId);

            const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
            expect(tokens).toEqual([token.tokenHash]);
        }
    });

    test('The persistent token revoke shall work', async ({ api }) => {
        const now = new Date().getTime();
        const token = await api.token.createPersistentToken(user.sid, 120, false);
        expect(token.expireAt).toBeAfter(new Date(now + 120 * 1000));
        expect(token.expireAt).toBeBefore(new Date(now + 130 * 1000));

        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens).toEqual([token.tokenHash]);

        let response = await api.token.revokeTokenRequest(user.sid, token.tokenHash);
        expect(response).toHaveStatus(200);

        const tokens1 = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens1, 'Token shall be removed').toEqual([]);

        response = await api.auth.loginWithTokenRequest(null, null, null, token.token, false, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, {
                type: 'auth-token-expired',
                status: 401,
                redirectUrl: api.auth.defaultRedirects.redirectUrl
            })
        );
    });
});
