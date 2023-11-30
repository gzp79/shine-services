import request from '$lib/request';
import config from '../test.config';
import { TestUser } from '$lib/test_user';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { MockServer } from '$lib/mock_server';
import { getSHA256Hash, parseSignedCookie } from '$lib/utils';
import api from '$lib/api/api';
import { ActiveToken } from '$lib/api/token_api';

describe('Tokens', () => {
    // assume server is not off more than a few seconds and the test is fast enough
    const now = new Date().getTime();
    const createRange = [new Date(now - 60 * 1000), new Date(now + 60 * 1000)];
    const expireRange = [new Date(now + 13 * 24 * 60 * 60 * 1000), new Date(now + 15 * 24 * 60 * 60 * 1000)];
    const anyToken: ActiveToken = {
        userId: expect.toBeString(),
        tokenFingerprint: expect.toBeString(),
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
        let response = await api.raw.getTokens(null);
        expect(response.statusCode).toEqual(401);
    });

    it('Token shall keep the site info', async () => {
        const extraHeaders = {
            'user-agent': 'agent',
            'cf-ipcountry': 'country',
            'cf-region': 'region',
            'cf-ipcity': 'city'
        };

        const user = await TestUser.createGuest({ extraHeaders });

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
        const user = await TestUser.createLinked(mock, {
            rememberMe: true,
            extraHeaders: { 'cf-ipcity': 'r1' }
        });

        // initial session for a new user
        expect(await api.token.getTokens(user.sid)).toIncludeSameMembers([{ ...anyToken, city: 'r1' }]);

        // login and create new token
        const userCookies2 = await api.auth.loginWithOAuth2(mock, user.externalUser!, true, { 'cf-ipcity': 'r2' });
        const sid2 = userCookies2.sid.value;
        const tid2 = userCookies2.tid.value;
        expect(await api.token.getTokens(user.sid)).toIncludeSameMembers([
            { ...anyToken, city: 'r1' },
            { ...anyToken, city: 'r2' }
        ]);

        // login but don't create new token
        await api.auth.loginWithOAuth2(mock, user.externalUser!, false, { 'cf-ipcity': 'r3' });
        expect(await api.token.getTokens(user.sid)).toIncludeSameMembers([
            { ...anyToken, city: 'r1' },
            { ...anyToken, city: 'r2' }
        ]);

        //logout from the second session with tid.
        // Some notes:
        // - without tid the token would not be deleted as sessions and tokens are not linked
        let response = await request
            .get(config.getUrlFor(`/identity/auth/logout`))
            .set('Cookie', [`sid=${sid2}`, `tid=${tid2}`])
            .send();
        expect(response.statusCode).toEqual(200);
        expect(await api.token.getTokens(user.sid)).toIncludeSameMembers([{ ...anyToken, city: 'r1' }]);
    });

    it('Multiple login with rememberMe shall create multiple tokens and logout with terminateAll shall invalidate all of them', async () => {
        const mock = await startMock();
        const user = await TestUser.createLinked(mock, {
            rememberMe: true,
            extraHeaders: { 'cf-ipcity': 'r1' }
        });

        // login a few more times
        await api.auth.loginWithOAuth2(mock, user.externalUser!, true, { 'cf-ipcity': 'r2' });
        await api.auth.loginWithOAuth2(mock, user.externalUser!, true, { 'cf-ipcity': 'r3' });
        expect(await api.token.getTokens(user.sid)).toIncludeSameMembers([
            { ...anyToken, city: 'r1' },
            { ...anyToken, city: 'r2' },
            { ...anyToken, city: 'r3' }
        ]);

        //logout from the all the session
        await api.auth.logout(user.sid, true);

        //login again and check if no token is present
        const newUserCookies = await api.auth.loginWithOAuth2(mock, user.externalUser!, false, { 'cf-region': 'r4' });
        expect(await api.token.getTokens(newUserCookies.sid.value)).toBeEmpty();
    });
/*
    it('Delete token by hash shall revoke the token', async () => {
        const mock = await startMock();
        const user = await TestUser.createLinked(mock, {
            rememberMe: true,
            extraHeaders: { 'cf-ipcity': 'r1' }
        });
        const cookies2 = await api.auth.loginWithOAuth2(mock, user.externalUser!, true, { 'cf-ipcity': 'r2' });
        const tid2 = cookies2.tid.value;
        await api.auth.loginWithOAuth2(mock, user.externalUser!, true, { 'cf-ipcity': 'r3' });

        let tokens = await api.token.getTokens(user.sid);
        expect(tokens).toIncludeSameMembers([
            { ...anyToken, city: 'r1' },
            { ...anyToken, city: 'r2' },
            { ...anyToken, city: 'r3' }
        ]);

        // find the 2nd token and revoke it
        const tokenId = tokens.find((x) => x.city === 'r2')!.tokenFingerprint;
        let responseGet = await api.raw.getToken(user.sid, tokenId);
        expect(responseGet.statusCode).toEqual(200);
        expect(responseGet.body.userId).toEqual(user.userId);
        expect(responseGet.body.city).toEqual('r2');
        expect(responseGet.body.tokenFingerprint).toEqual(tokenId);

        // revoke
        let responseDelete = await request
            .delete(config.getUrlFor('identity/api/auth/user/tokens/' + tokenId))
            .set('Cookie', user.getSessionCookie())
            .send();
        expect(responseDelete.statusCode).toEqual(200);

        // it shall be gone
        let responseGet2 = await request
            .get(config.getUrlFor('identity/api/auth/user/tokens/' + tokenId))
            .set('Cookie', user.getSessionCookie())
            .send();
        expect(responseGet2.statusCode).toEqual(404);
        expect(await getTokens(user.sid)).toIncludeSameMembers([
            { ...anyToken, city: 'r1' },
            { ...anyToken, city: 'r3' }
        ]);

        // login shall fail with the revoked token
        const responseLogin = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query(config.defaultRedirects)
            .set('Cookie', [`tid=${tid2}`])
            .send();
        expect(responseLogin.statusCode).toEqual(200);
        expect(getPageRedirectUrl(responseLogin.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=sessionExpired&status=401'
        );
    });

    it('Token rotation with lost response shall work', async () => {
        const user = await TestUser.createGuest();
        const c0 = parseSignedCookie(user.tid!); //active: t1, revoke: -
        console.log('c0', c0);
        expect(c0.rt).toBeNull();
        const tid = user.tid!;

        await user.rotateTID();
        const c1 = parseSignedCookie(user.tid!); // active: t2, revoke: t1
        console.log('c1', c1);
        expect(c1.rt).toEqual(c0.t);

        // Emulate a few lost responses by using the
        const l2 = await loginWithToken(user.tid!);
        const c2 = parseSignedCookie(l2.tid.value); // active: t3, revoke: t2
        console.log('c2', c2);
        expect(c2.rt).toEqual(c1.t);

        const l3 = await loginWithToken(user.tid!);
        const c3 = parseSignedCookie(l3.tid.value); // active: t4, revoke: t2
        console.log('c3', c3);
        expect(c3.rt).toEqual(c1.t);

        // Get back to the normal operation
        await user.rotateTID();
        const c4 = parseSignedCookie(user.tid!); // active: t5, revoke: t2
        console.log('c4', c4);
        expect(c4.rt).toEqual(c1.t);

        await user.rotateTID();
        const c5 = parseSignedCookie(user.tid!); // active: t6, revoke: t5
        console.log('c5', c5);
        expect(c5.rt).toEqual(c4.t);

        // Token rotated out shall not work
        const request = await requestLoginWithToken(tid);
        expect(request.statusCode).toEqual(200);
        expect(getPageRedirectUrl(request.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=sessionExpired&status=401'
        );

        // live tokens: t3,t4,t5,t6
        const tokens = (await getTokens(user.sid)).map((x) => x.tokenFingerprint);
        const expectedTokens = [c2.t, c3.t, c4.t, c5.t].map((x) => getSHA256Hash(x));
        expect(tokens).toIncludeSameMembers(expectedTokens);
    });
    */
});
/*
describe('Single access token', () => {
    const now = new Date().getTime();

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

    it('Too long time to live shall be rejected with bad request', async () => {
        expect(await getTokens(user.sid)).toBeEmpty();

        //const request = requestSAToken(user.sid, 20);
    });
});
*/