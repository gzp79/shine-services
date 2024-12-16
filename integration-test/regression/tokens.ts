import api, { TokenKind } from '$lib/api/api';
import { ActiveToken } from '$lib/api/token_api';
import { MockServer } from '$lib/mock_server';
import OAuth2MockServer from '$lib/mocks/oauth2';
import request from '$lib/request';
import { getCookies, getPageRedirectUrl } from '$lib/response_utils';
import { TestUser } from '$lib/test_user';
import { delay, getSHA256Hash, parseSignedCookie } from '$lib/utils';
import config from '../test.config';

describe('Tokens', () => {
    // assume server is not off more than a few seconds and the test is fast enough
    const now = new Date().getTime();
    const createRange = [new Date(now - 60 * 1000), new Date(now + 60 * 1000)];
    const expireRange = [new Date(now + 13 * 24 * 60 * 60 * 1000), new Date(now + 15 * 24 * 60 * 60 * 1000)];
    const anyToken: ActiveToken = {
        userId: expect.toBeString(),
        tokenHash: expect.toBeString(),
        kind: 'access',
        createdAt: expect.toBeBetween(createRange[0], createRange[1]),
        expireAt: expect.toBeBetween(expireRange[0], expireRange[1]),
        isExpired: false,
        agent: '',
        country: null,
        region: null,
        city: null
    };

    let mock: MockServer;
    const startMock = async (): Promise<OAuth2MockServer> => {
        mock = new OAuth2MockServer({ tls: config.mockTLS, url: config.mockUrl });
        await mock.start();
        return mock as OAuth2MockServer;
    };

    afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
    });

    it('Get token without a session shall fail', async () => {
        // initial session for a new user
        let response = await api.request.getTokens(null);
        expect(response).toHaveStatus(401);
    });

    it('Token shall keep the site info', async () => {
        const extraHeaders = {
            'user-agent': 'agent',
            'cf-ipcountry': 'country',
            'cf-region': 'region',
            'cf-ipcity': 'city'
        };

        const user = await TestUser.createGuest({}, extraHeaders);

        // initial session for a new user
        expect(await api.token.getTokens(user.sid, extraHeaders)).toIncludeSameMembers([
            {
                ...anyToken,
                agent: 'agent',
                country: 'country',
                region: 'region',
                city: 'city'
            }
        ]);
    });

    it('Multiple login with rememberMe shall create multiple tokens and logout from a session shall invalidate the connected token', async () => {
        const mock = await startMock();
        const user = await TestUser.createLinked(
            mock,
            {
                rememberMe: true
            },
            { 'cf-ipcity': 'r1' }
        );
        const externalUser = user.externalUser!;

        // initial session for a new user
        expect(await api.token.getTokens(user.sid)).toIncludeSameMembers([{ ...anyToken, city: 'r1' }]);

        // login and create new token
        const userCookies2 = await api.auth.loginWithOAuth2(mock, externalUser, true, { 'cf-ipcity': 'r2' });
        const sid2 = userCookies2.sid;
        const tid2 = userCookies2.tid;
        expect(await api.token.getTokens(user.sid)).toIncludeSameMembers([
            { ...anyToken, city: 'r1' },
            { ...anyToken, city: 'r2' }
        ]);

        // login but don't create new token
        await api.auth.loginWithOAuth2(mock, externalUser, false, { 'cf-ipcity': 'r3' });
        expect(await api.token.getTokens(user.sid)).toIncludeSameMembers([
            { ...anyToken, city: 'r1' },
            { ...anyToken, city: 'r2' }
        ]);

        //logout from the second session with tid.
        // Some notes:
        // - without tid the token would not be deleted as sessions and tokens are not linked
        let response = await request
            .get(config.getUrlFor(`/auth/logout`))
            .set('Cookie', [`sid=${sid2}`, `tid=${tid2}`]);
        expect(response).toHaveStatus(200);
        expect(await api.token.getTokens(user.sid)).toIncludeSameMembers([{ ...anyToken, city: 'r1' }]);
    });

    it('Multiple login with rememberMe shall create multiple tokens and logout with terminateAll shall invalidate all of them', async () => {
        const mock = await startMock();
        const user = await TestUser.createLinked(
            mock,
            {
                rememberMe: true
            },
            { 'cf-ipcity': 'r1' }
        );
        const externalUser = user.externalUser!;

        // login a few more times
        await api.auth.loginWithOAuth2(mock, externalUser, true, { 'cf-ipcity': 'r2' });
        await api.auth.loginWithOAuth2(mock, externalUser, true, { 'cf-ipcity': 'r3' });
        expect(await api.token.getTokens(user.sid)).toIncludeSameMembers([
            { ...anyToken, city: 'r1' },
            { ...anyToken, city: 'r2' },
            { ...anyToken, city: 'r3' }
        ]);

        //logout from the all the session
        await api.auth.logout(user.sid, true);

        //login again and check if no token is present
        const newUserCookies = await api.auth.loginWithOAuth2(mock, externalUser, false, {
            'cf-region': 'r4'
        });
        expect(await api.token.getTokens(newUserCookies.sid)).toBeEmpty();
    });

    it('Delete token by hash shall revoke the token', async () => {
        const mock = await startMock();
        const user = await TestUser.createLinked(
            mock,
            {
                rememberMe: true
            },
            { 'cf-ipcity': 'r1' }
        );
        const externalUser = user.externalUser!;
        const cookies2 = await api.auth.loginWithOAuth2(mock, externalUser, true, { 'cf-ipcity': 'r2' });
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
            config.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );
    });

    it('Token rotation with lost response shall work', async () => {
        const user = await TestUser.createGuest();
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
            config.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );

        // live tokens: t3,t4,t5,t6
        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash);
        const expectedTokens = [c2.key, c3.key, c4.key, c5.key].map((x) => getSHA256Hash(x));
        expect(tokens).toIncludeSameMembers(expectedTokens);
    });

    it('Query token shall have the highest precedence', async () => {
        const userCookie = await TestUser.createGuest();
        const userQuery = await TestUser.createGuest();
        const tokenQuery = await api.token.createSAToken(userQuery.sid, 120, false);
        const userHeader = await TestUser.createGuest();
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
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);
        var cookies = getCookies(response);
        const userLoggedIn = await api.user.getUserInfo(cookies.sid.value);
        expect(userLoggedIn.userId).not.toEqual(userCookie.userId);
        expect(userLoggedIn.userId).toEqual(userQuery.userId);
        expect(userLoggedIn.userId).not.toEqual(userHeader.userId);
    });

    it('Header token shall have the 2nd highest precedence', async () => {
        const userCookie = await TestUser.createGuest();
        const userHeader = await TestUser.createGuest();
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
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);
        var cookies = getCookies(response);
        const userLoggedIn = await api.user.getUserInfo(cookies.sid.value);
        expect(userLoggedIn.userId).not.toEqual(userCookie.userId);
        expect(userLoggedIn.userId).toEqual(userHeader.userId);
    });
});

describe('Created token', () => {
    let mock: OAuth2MockServer = undefined!;
    let user: TestUser = undefined!;

    beforeEach(async () => {
        mock = new OAuth2MockServer({ tls: config.mockTLS, url: config.mockUrl });
        await mock.start();
        user = await TestUser.createLinked(mock);
    });

    afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
        user = undefined!;
    });

    it.each(['singleAccess', 'persistent'] as TokenKind[])(
        'Creating %s token without session shall fail',
        async (kind: TokenKind) => {
            const response = await api.request.createToken(null, kind, 20, false);
            expect(response).toHaveStatus(401);
        }
    );

    class InputValidation {
        constructor(
            public kind: TokenKind,
            public duration: number,
            public bindToSite: boolean,
            public expectedError: any
        ) {}
    }
    const inputValidationCases: InputValidation[] = [
        new InputValidation('singleAccess', 20000, false, {
            time_to_live: [expect.objectContaining({ code: 'range', message: null })]
        }),
        new InputValidation('persistent', 31536001, false, {
            time_to_live: [expect.objectContaining({ code: 'range', message: null })]
        }),
        new InputValidation('access', 200, false, {
            kind: [expect.objectContaining({ code: 'oneOf', message: 'Access tokens are not allowed' })]
        })
    ];

    it.each(inputValidationCases)(
        'Token creation with(kind: $kind, duration:$duration, bindToSite: $bindToSite) shall be rejected with $expectedError',
        async (test) => {
            const response = await api.request.createToken(
                user.sid,
                test.kind,
                test.duration,
                test.bindToSite
            );
            expect(response).toHaveStatus(400);
            expect(response.body.type).toEqual('validation_error');
            expect(response.body.extension).toEqual(test.expectedError);
        }
    );
});

describe('Single access token', () => {
    let mock: OAuth2MockServer = undefined!;
    let user: TestUser = undefined!;

    beforeEach(async () => {
        mock = new OAuth2MockServer({ tls: config.mockTLS, url: config.mockUrl });
        await mock.start();
        user = await TestUser.createLinked(mock);
    });

    afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
        user = undefined!;
    });

    it('A failed login with a single access token shall clear the current user', async () => {
        const response = await api.request.loginWithToken(null, null, 'invalid', null, false, null);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('A successful login with a single access token shall change the current user', async () => {
        const token = await api.token.createSAToken(user.sid, 120, false);

        const response = await api.request.loginWithToken(null, null, token.token, null, false, null);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);
        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();
    });

    it('The single access token with client binding shall respect client fingerprint and revoke token on mismatch', async () => {
        const token = await api.token.createSAToken(user.sid, 120, true);

        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash);
        expect(tokens).toIncludeSameMembers([token.tokenHash]);

        const response = await api.request
            .loginWithToken(null, null, token.token, null, false, null)
            .set({ 'user-agent': 'agent2' });
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=400'
        );

        // token is revoked
        expect(await api.token.getTokens(user.sid)).toBeEmpty();
    });

    it(
        'The single access token shall expire after the specified time',
        async () => {
            const now = new Date().getTime();
            const ttl = 10;
            const token = await api.token.createSAToken(user.sid, ttl, false);
            expect(token.expireAt).toBeAfter(new Date(now + ttl * 1000));
            expect(token.expireAt).toBeBefore(new Date(now + (ttl + 5) * 1000));

            console.log(`Waiting for the token to expire (${ttl} second)...`);
            await delay(ttl * 1000);
            const response = await api.request.loginWithToken(null, null, token.token, null, false, null);
            expect(response).toHaveStatus(200);
            expect(getPageRedirectUrl(response.text)).toEqual(
                config.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
            );
        },
        35 * 1000
    );

    it('The single access token shall be used only once', async () => {
        const now = new Date().getTime();
        const token = await api.token.createSAToken(user.sid, 120, false);
        expect(token.expireAt).toBeAfter(new Date(now + 120 * 1000));
        expect(token.expireAt).toBeBefore(new Date(now + 130 * 1000));

        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash);
        expect(tokens).toIncludeSameMembers([token.tokenHash]);

        const response1 = await api.request.loginWithToken(null, null, token.token, null, false, null);
        expect(response1).toHaveStatus(200);
        const sid1 = getCookies(response1).sid.value;
        const user1 = await api.user.getUserInfo(sid1);
        expect(user1.userId, 'It shall be the same use').toEqual(user.userId);

        const tokens1 = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash);
        expect(tokens1, 'Token shall be removed').toIncludeSameMembers([]);

        const response2 = await api.request.loginWithToken(null, null, token.token, null, false, null);
        expect(response2).toHaveStatus(200);
        expect(getPageRedirectUrl(response2.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );
    });

    it('The single access token revoke shall work', async () => {
        const now = new Date().getTime();
        const token = await api.token.createSAToken(user.sid, 120, false);
        expect(token.expireAt).toBeAfter(new Date(now + 120 * 1000));
        expect(token.expireAt).toBeBefore(new Date(now + 130 * 1000));

        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash);
        expect(tokens).toIncludeSameMembers([token.tokenHash]);

        let responseDelete = await api.request.revokeToken(user.sid, token.tokenHash);
        expect(responseDelete).toHaveStatus(200);

        const tokens1 = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash);
        expect(tokens1, 'Token shall be removed').toIncludeSameMembers([]);

        const response2 = await api.request.loginWithToken(null, null, token.token, null, false, null);
        expect(response2).toHaveStatus(200);
        expect(getPageRedirectUrl(response2.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );
    });
});

describe('Persistent token', () => {
    let mock: OAuth2MockServer = undefined!;
    let user: TestUser = undefined!;

    beforeEach(async () => {
        mock = new OAuth2MockServer({ tls: config.mockTLS, url: config.mockUrl });
        await mock.start();
        user = await TestUser.createLinked(mock);
    });

    afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
        user = undefined!;
    });

    it('A failed login with invalid authorization shall not change the current user', async () => {
        const user = await TestUser.createGuest();
        const response = await api.request
            .loginWithToken(user.tid!, user.sid!, null, null, false, null)
            .set({ Authorization: `Basic invalid` });
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=400'
        );

        const cookies = getCookies(response);
        expect(cookies.tid).toBeValidTID();
        expect(cookies.tid.value, 'Token cookie shall not be changed').toEqual(user.tid);
        expect(cookies.sid).toBeValidSID();
        expect(cookies.sid.value, 'User session shall not be changed').toEqual(user.sid);
        expect(cookies.eid).toBeClearCookie();
    });

    it('A failed login with a persistent token shall clear the current user', async () => {
        const response = await api.request.loginWithToken(null, null, null, 'invalid', false, null);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('A successful login with a persistent token shall change the current user', async () => {
        const token = await api.token.createPersistentToken(user.sid, 120, false);

        const response = await api.request.loginWithToken(null, null, null, token.token, false, null);
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);
        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();
    });

    it('The persistent token with client binding shall respect client fingerprint and revoke token on mismatch', async () => {
        const token = await api.token.createPersistentToken(user.sid, 120, true);

        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash);
        expect(tokens).toIncludeSameMembers([token.tokenHash]);

        const response = await api.request
            .loginWithToken(null, null, null, token.token, false, null)
            .set({ 'user-agent': 'agent2' });
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=400'
        );

        // token is revoked
        expect(await api.token.getTokens(user.sid)).toBeEmpty();
    });

    it('The persistent token should be available for use at all times.', async () => {
        const now = new Date().getTime();
        const token = await api.token.createPersistentToken(user.sid, 120, false);
        expect(token.expireAt).toBeAfter(new Date(now + 120 * 1000));
        expect(token.expireAt).toBeBefore(new Date(now + 130 * 1000));

        for (let i = 0; i < 3; i++) {
            const response1 = await api.request.loginWithToken(null, null, null, token.token, false, null);
            expect(response1).toHaveStatus(200);
            const sid1 = getCookies(response1).sid.value;
            const user1 = await api.user.getUserInfo(sid1);
            expect(user1.userId, 'It shall be the same use').toEqual(user.userId);

            const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash);
            expect(tokens).toIncludeSameMembers([token.tokenHash]);
        }
    });

    it('The persistent token revoke shall work', async () => {
        const now = new Date().getTime();
        const token = await api.token.createPersistentToken(user.sid, 120, false);
        expect(token.expireAt).toBeAfter(new Date(now + 120 * 1000));
        expect(token.expireAt).toBeBefore(new Date(now + 130 * 1000));

        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash);
        expect(tokens).toIncludeSameMembers([token.tokenHash]);

        let responseDelete = await api.request.revokeToken(user.sid, token.tokenHash);
        expect(responseDelete).toHaveStatus(200);

        const tokens1 = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash);
        expect(tokens1, 'Token shall be removed').toIncludeSameMembers([]);

        const response2 = await api.request.loginWithToken(null, null, null, token.token, false, null);
        expect(response2).toHaveStatus(200);
        expect(getPageRedirectUrl(response2.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );
    });
});
