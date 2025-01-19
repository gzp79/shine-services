import { expect, test } from '$fixtures/setup';
import { TestUser } from '$lib/api/test_user';
import { ActiveToken } from '$lib/api/token_api';
import { MockServer } from '$lib/mocks/mock_server';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { delay, getSHA256Hash, parseSignedCookie } from '$lib/utils';

test.describe('Remember me token (TID)', () => {
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
        let response = await api.token.getTokensRequest(null).send();
        expect(response).toHaveStatus(401);
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

    test('Multiple login with rememberMe shall create multiple tokens and logout from a session shall invalidate the connected token', async ({
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
        const tid3 = userCookies3.tid;
        tokens = await api.token.getTokens(user.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual(['r1', 'r2', 'r3']);

        // login but don't create new token
        await api.auth.loginWithOAuth2(mock, externalUser, false, { 'cf-ipcity': 'r4' });
        tokens = await api.token.getTokens(user.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual(['r1', 'r2', 'r3']);

        //logout using the second session
        // Note: to revoke the tid2, we have to also provide it. From sid, the server cannot tell the (tid) remember me token.
        // They are connected only on the client side.
        let logout = await api.auth.logoutRequest(sid2, tid2, false).send();
        expect(logout.cookies().sid).toBeClearCookie();
        expect(logout.cookies().tid).toBeClearCookie();
        tokens = await api.token.getTokens(user.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual(['r1', 'r3']);

        //logout using the 3rd session, but without the tid3 will not revoke the token.
        // It will be clear from the cookies, thus browser shall forget it.
        logout = await api.auth.logoutRequest(sid3, null, false).send();
        expect(logout.cookies().sid).toBeClearCookie();
        expect(logout.cookies().tid).toBeClearCookie();
        tokens = await api.token.getTokens(user.sid);
        expect(tokens.map((t) => t.city).sort()).toEqual(['r1', 'r3']);
    });

    test('Multiple login with rememberMe shall create multiple tokens and logout with terminateAll shall invalidate all of them', async ({
        api
    }) => {
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
        /*const cookies2 = await api.auth.loginWithOAuth2(mock, externalUser, true, { 'cf-ipcity': 'r2' });
        const tid2 = cookies2.tid;
        await api.auth.loginWithOAuth2(mock, externalUser, true, { 'cf-ipcity': 'r3' });

        let tokens = await api.token.getTokens(user.sid);
        expect(tokens).toIncludeSameMembers([
            { ...anyToken, city: 'r1' },
            { ...anyToken, city: 'r2' },
            { ...anyToken, city: 'r3' }
        ]);

        // find the 2nd token and revoke it
        const tokenId = tokens.find((x) => x.city === 'r2')!.tokenHash;
        let responseGet = await api.request.getToken(user.sid, tokenId);
        expect(responseGet).toHaveStatus(200);
        expect(responseGet.body.userId).toEqual(user.userId);
        expect(responseGet.body.city).toEqual('r2');
        expect(responseGet.body.tokenHash).toEqual(tokenId);

        // revoke
        let responseDelete = await api.request.revokeToken(user.sid, tokenId);
        expect(responseDelete).toHaveStatus(200);

        // it shall be gone
        let responseGet2 = await api.request.getToken(user.sid, tokenId);
        expect(responseGet2).toHaveStatus(404);
        expect(await api.token.getTokens(user.sid)).toIncludeSameMembers([
            { ...anyToken, city: 'r1' },
            { ...anyToken, city: 'r3' }
        ]);

        // login shall fail with the revoked token
        const responseLogin = await api.request.loginWithToken(tid2, null, null, null, false, null);
        expect(responseLogin).toHaveStatus(200);
        expect(getPageRedirectUrl(responseLogin.text)).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );*/
    });

    test('Login with token shall rotate the token', async ({ api }) => {
        /*
        const user = await api.testUsers.createGuest();
        
        todo: loginWith token creates a new tid        
        */
    });

    test('Token rotation with lost response shall work', async ({ api }) => {
        /*
        const user = await api.testUsers.createGuest();
        const c0 = parseSignedCookie(user.tid!); //active: t1, revoke: -
        expect(c0.rky).toBeNull();
        const tid = user.tid!;

        await user.rotateTID();
        const c1 = parseSignedCookie(user.tid!); // active: t2, revoke: t1
        expect(c1.rky).toEqual(c0.key);

        // Emulate a few lost responses by using the
        const l2 = await api.auth.loginWithToken(user.tid!, false);
        const c2 = parseSignedCookie(l2.tid); // active: t3, revoke: t2
        expect(c2.rky).toEqual(c1.key);

        const l3 = await api.auth.loginWithToken(user.tid!, false);
        const c3 = parseSignedCookie(l3.tid); // active: t4, revoke: t2
        expect(c3.rky).toEqual(c1.key);

        // Get back to the normal operation
        await user.rotateTID();
        const c4 = parseSignedCookie(user.tid!); // active: t5, revoke: t2
        expect(c4.rky).toEqual(c1.key);

        await user.rotateTID();
        const c5 = parseSignedCookie(user.tid!); // active: t6, revoke: t5
        expect(c5.rky).toEqual(c4.key);

        // Token rotated out shall not work
        const request = await api.request.loginWithToken(tid, null, null, null, null, null);
        expect(request).toHaveStatus(200);
        expect(getPageRedirectUrl(request.text)).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );

        // live tokens: t3,t4,t5,t6
        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash);
        const expectedTokens = [c2.key, c3.key, c4.key, c5.key].map((x) => getSHA256Hash(x));
        expect(tokens).toIncludeSameMembers(expectedTokens);
        */
    });

    test('Query token shall have the highest precedence', async ({ api }) => {
        /*
        const userCookie = await api.testUsers.createGuest();
        const userQuery = await api.testUsers.createGuest();
        const tokenQuery = await api.token.createSAToken(userQuery.sid, 120, false);
        const userHeader = await api.testUsers.createGuest();
        const tokenHeader = await api.token.createPersistentToken(userHeader.sid, 120, false);

        const response = await api.request.loginWithToken(
            userCookie.tid!,
            null,
            tokenQuery.token,
            tokenHeader.token,
            false,
            null
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(api.auth.defaultRedirects.redirectUrl);
        var cookies = response.cookies();
        const userLoggedIn = await api.user.getUserInfo(cookies.sid.value);
        expect(userLoggedIn.userId).not.toEqual(userCookie.userId);
        expect(userLoggedIn.userId).toEqual(userQuery.userId);
        expect(userLoggedIn.userId).not.toEqual(userHeader.userId);
        */
    });

    test('Header token shall have the 2nd highest precedence', async ({ api }) => {
        /*
        const userCookie = await api.testUsers.createGuest();
        const userHeader = await api.testUsers.createGuest();
        const tokenHeader = await api.token.createPersistentToken(userHeader.sid, 120, false);

        const response = await api.request.loginWithToken(
            userCookie.tid!,
            null,
            null,
            tokenHeader.token,
            false,
            null
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(api.auth.defaultRedirects.redirectUrl);
        var cookies = response.cookies();
        const userLoggedIn = await api.user.getUserInfo(cookies.sid.value);
        expect(userLoggedIn.userId).not.toEqual(userCookie.userId);
        expect(userLoggedIn.userId).toEqual(userHeader.userId);
        */
    });
});
