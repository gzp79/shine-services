import { expect, test } from '$fixtures/setup';
import { ProblemSchema } from '$lib/api/api';
import { getPageProblem, getPageRedirectUrl } from '$lib/api/utils';
import { MockServer } from '$lib/mocks/mock_server';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { createUrl, getSHA256Hash, parseSignedCookie } from '$lib/utils';

test.describe('Access token (TID)', () => {
    // assume server is not off more than a few seconds and the test is fast enough
    const now = new Date().getTime();
    const createRange = [new Date(now - 60 * 1000), new Date(now + 60 * 1000)];
    const expireRange = [new Date(now + 13 * 24 * 60 * 60 * 1000), new Date(now + 15 * 24 * 60 * 60 * 1000)];

    let mock: MockServer;
    const startMock = async (): Promise<OAuth2MockServer> => {
        mock = new OAuth2MockServer();
        await mock.start();
        return mock as OAuth2MockServer;
    };

    test.afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
    });

    test('Get token without a session shall fail', async ({ api }) => {
        // initial session for a new user
        const response = await api.token.getTokensRequest(null);
        expect(response).toHaveStatus(401);
    });

    test('Token creation with api shall be rejected', async ({ api }) => {
        const user = await api.testUsers.createGuest();

        const response = await api.token.createTokenRequest(user.sid, 'access', 20, false);
        expect(response).toHaveStatus(400);

        const error = await response.parse(ProblemSchema);
        expect(error).toEqual(
            expect.objectContaining({
                type: 'input-body-format',
                status: 400,
                detail: expect.stringContaining('kind: unknown variant `access`')
            })
        );
    });

    test('Token shall keep the site info', async ({ api }) => {
        const extraHeaders = {
            'user-agent': 'agent',
            'cf-ipcountry': 'country',
            'cf-region': 'region',
            'cf-ipcity': 'city'
        };

        const user = await api.testUsers.createGuest({}, extraHeaders);

        // initial session for a new user
        const tokens = await api.token.getTokens(user.sid, extraHeaders);
        expect(tokens).toHaveLength(1);
        const token = tokens[0];
        expect(token.userId).toEqual(user.userId);
        expect(token.agent).toEqual('agent');
        expect(token.country).toEqual('country');
        expect(token.region).toEqual('region');
        expect(token.city).toEqual('city');
        expect(token.createdAt).toBeAfter(createRange[0]);
        expect(token.createdAt).toBeBefore(createRange[1]);
        expect(token.expireAt).toBeAfter(expireRange[0]);
        expect(token.expireAt).toBeBefore(expireRange[1]);
    });

    test('A failed login with invalid authorization shall not change the current user', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const response = await api.auth
            .loginWithTokenRequest(user.tid!, user.sid!, null, null, false, null)
            .withHeaders({ authorization: 'Basic invalid' }); // only Bearer is supported, thus it is considered invalid
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
                sensitive: 'invalidHeader'
            })
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeValidTID();
        expect(cookies.tid.value, 'Token cookie shall not be changed').toEqual(user.tid);
        expect(cookies.sid).toBeValidSID();
        expect(cookies.sid.value, 'User session shall not be changed').toEqual(user.sid);
        expect(cookies.eid).toBeClearCookie();
    });

    test('Create multiple tokens and logout from a single session shall invalidate that single access token', async ({
        api
    }) => {
        const mock = await startMock();
        const user = await api.testUsers.createLinked(
            mock,
            {
                rememberMe: true
            },
            { 'cf-ipcity': 'r1' }
        );
        const externalUser = user.externalUser!;

        // initial session for a new user
        let tokens = await api.token.getTokens(user.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual(['r1']);

        // login and create a new token
        const userCookies2 = await api.auth.loginWithOAuth2(mock, externalUser, true, { 'cf-ipcity': 'r2' });
        const sid2 = userCookies2.sid;
        const tid2 = userCookies2.tid;
        tokens = await api.token.getTokens(user.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual(['r1', 'r2']);

        const userCookies3 = await api.auth.loginWithOAuth2(mock, externalUser, true, { 'cf-ipcity': 'r3' });
        const sid3 = userCookies3.sid;
        tokens = await api.token.getTokens(user.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual(['r1', 'r2', 'r3']);

        // login but don't create new token
        await api.auth.loginWithOAuth2(mock, externalUser, false, { 'cf-ipcity': 'r4' });
        tokens = await api.token.getTokens(user.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual(['r1', 'r2', 'r3']);

        //logout using the second session
        // Note: to revoke the tid2, we have to also provide it. From sid, the server cannot tell the (tid) remember me token.
        // They are connected only on the client side.
        let logout = await api.auth.logoutRequest(sid2, tid2, false);
        expect(logout.cookies().sid).toBeClearCookie();
        expect(logout.cookies().tid).toBeClearCookie();
        tokens = await api.token.getTokens(user.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual(['r1', 'r3']);

        //logout using the 3rd session, but without the tid3 will not revoke the token.
        // It will be clear from the cookies, thus browser shall forget it.
        logout = await api.auth.logoutRequest(sid3, null, false);
        expect(logout.cookies().sid).toBeClearCookie();
        expect(logout.cookies().tid).toBeClearCookie();
        tokens = await api.token.getTokens(user.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual(['r1', 'r3']);
    });

    test('Create multiple tokens and logout from all site shall invalidate all the access tokens', async ({ api }) => {
        const mock = await startMock();
        const user = await api.testUsers.createLinked(mock, { rememberMe: true }, { 'cf-ipcity': 'r1' });
        const externalUser = user.externalUser!;

        // login a few more times
        await api.auth.loginWithOAuth2(mock, externalUser, true, { 'cf-ipcity': 'r2' });
        await api.auth.loginWithOAuth2(mock, externalUser, true, { 'cf-ipcity': 'r3' });
        let tokens = await api.token.getTokens(user.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual(['r1', 'r2', 'r3']);

        //logout from the all the session
        await api.auth.logout(user.sid, null, true);

        //login again without remember me shall create no tid
        const newUserCookies = await api.auth.loginWithOAuth2(mock, externalUser, false, { 'cf-region': 'r4' });
        tokens = await api.token.getTokens(newUserCookies.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual([]);
    });

    test('Delete token by hash shall revoke the token', async ({ api }) => {
        const mock = await startMock();

        const user = await api.testUsers.createLinked(mock, { rememberMe: true }, { 'cf-ipcity': 'r1' });
        const externalUser = user.externalUser!;
        const cookies2 = await api.auth.loginWithOAuth2(mock, externalUser, true, { 'cf-ipcity': 'r2' });
        const tid2 = cookies2.tid;
        await api.auth.loginWithOAuth2(mock, externalUser, true, { 'cf-ipcity': 'r3' });

        let tokens = await api.token.getTokens(user.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual(['r1', 'r2', 'r3']);

        // find the 2nd token and revoke it
        const tokenId = tokens.find((x) => x.city === 'r2')!.tokenHash;
        let token = await api.token.getToken(user.sid, tokenId);
        expect(token).toBeDefined();
        expect(token!.userId).toEqual(user.userId);
        expect(token!.city).toEqual('r2');
        expect(token!.tokenHash).toEqual(tokenId);

        // revoke
        await api.token.revokeToken(user.sid, tokenId);

        // it shall be gone
        token = await api.token.getToken(user.sid, tokenId);
        expect(token).toBeUndefined();
        tokens = await api.token.getTokens(user.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual(['r1', 'r3']);

        // login shall fail with the revoked token
        const responseLogin = await api.auth.loginWithTokenRequest(tid2, null, null, null, false, null);
        expect(responseLogin).toHaveStatus(200);

        const text = await responseLogin.text();
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
    });

    test('Login with token shall rotate the token', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const c0 = parseSignedCookie(user.tid!);
        expect(c0.rky).toBeNull();
        const tid = user.tid!;

        const newCookies = await api.auth.loginWithToken(tid, null);
        expect(tid).not.toEqual(newCookies.tid);
    });

    test('Token rotation with lost response shall work', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        //active: t0, revoke: -
        const tid0 = user.tid!;
        const c0 = parseSignedCookie(tid0);
        const t0 = c0.key;
        expect(c0.rky).toBeNull();

        await user.rotateTID();
        // active: t1, revoke: t0
        const tid1 = user.tid!;
        const c1 = parseSignedCookie(tid1);
        const t1 = c1.key;
        expect(c1.rky).toEqual(t0);

        // Emulate a few lost responses by using the old token
        const l2 = await api.auth.loginWithToken(tid1, false);
        // active: t2, revoke: t1
        const tid2 = l2.tid!;
        const c2 = parseSignedCookie(tid2);
        const t2 = c2.key;
        expect(c2.rky).toEqual(t1);

        const l3 = await api.auth.loginWithToken(tid1, false);
        // active: t3, revoke: t1
        const tid3 = l3.tid!;
        const c3 = parseSignedCookie(tid3);
        const t3 = c3.key;
        expect(c3.rky).toEqual(t1);

        // Get back to the normal operation
        await user.rotateTID();
        // active: t4, revoke: t1
        const tid4 = user.tid!;
        const c4 = parseSignedCookie(tid4);
        const t4 = c4.key;
        expect(c4.rky).toEqual(c1.key);

        await user.rotateTID();
        // active: t5, revoke: t4
        const tid5 = user.tid!;
        const c5 = parseSignedCookie(tid5);
        const t5 = c5.key;
        expect(c5.rky).toEqual(c4.key);

        // live tokens: t2,t3,t4,t5
        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        const expectedTokens = [t2, t3, t4, t5].map((x) => getSHA256Hash(x)).sort();
        expect(tokens).toEqual(expectedTokens);

        // Token rotated out shall not work (t0)
        const response = await api.auth.loginWithTokenRequest(tid0, null, null, null, null, null);
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
    });

    test('Using token in query shall fail and revoke the token', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const c0 = parseSignedCookie(user.tid!);
        const t0 = c0.key;
        expect(c0.rky).toBeNull();

        let tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens).toEqual([getSHA256Hash(t0)]);

        const response = await api.auth.loginWithTokenRequest(null, null, t0, null, null, null);

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

        tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens).toBeEmptyValue();
    });

    test('Using token in header shall fail and revoke the token', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const c0 = parseSignedCookie(user.tid!);
        const t0 = c0.key;
        expect(c0.rky).toBeNull();

        let tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens).toEqual([getSHA256Hash(t0)]);

        const response = await api.auth.loginWithTokenRequest(null, null, null, t0, null, null);

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

        tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens).toBeEmptyValue();
    });
});
