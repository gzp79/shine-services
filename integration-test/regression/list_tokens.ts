import request from 'superagent';
import { ActiveToken, getTokens, logout } from '$lib/auth_utils';
import config from '../test.config';
import { ExternalUser, TestUser } from '$lib/user';
import Oauth2MockServer from '$lib/mocks/oauth2';
import { loginWithOAuth2 } from '$lib/login_utils';
import { MockServer } from '$lib/mock_server';

describe('Active token', () => {
    let mock!: MockServer;

    // assume server is not off more than a few seconds and the test is fast enough
    const now = new Date().getTime();
    const createRange = [new Date(now - 60 * 1000), new Date(now + 60 * 1000)];
    const expireRange = [new Date(now + 13 * 24 * 60 * 60 * 1000), new Date(now + 15 * 24 * 60 * 60 * 1000)];
    const anyToken: ActiveToken = {
        userId: expect.toBeString(),
        kind: "autoRenewal",
        createdAt: expect.toBeBetween(createRange[0], createRange[1]),
        expireAt: expect.toBeBetween(expireRange[0], expireRange[1]),
        isExpired: false,
        agent: '',
        country: null,
        region: null,
        city: null,
    };

    afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
    });

    it('Get token without user should fail', async () => {
        // initial session for a new user
        let response = await request
            .get(config.getUrlFor('identity/api/auth/user/tokens'))
            .send()
            .catch((err) => err.response);
        expect(response.statusCode).toEqual(401);
    });

    it('Token should keep site-info', async () => {
        const extraHeaders = {
            'user-agent': 'agent',
            'cf-country': 'country',
            'cf-region': 'region',
            'cf-city': 'city'
        };

        const user = await TestUser.createGuest({ extraHeaders });

        // initial session for a new user
        expect(await getTokens(user.sid, extraHeaders)).toIncludeSameMembers([
            {
                ...anyToken,
                agent: 'agent',
                country: 'country',
                region: 'region',
                city: 'city'
            }
        ]);
    });

    it('Multiple remember-me login should create multiple tokens', async () => {
        mock = await new Oauth2MockServer({ tls: config.mockTLS }).start();

        const user = await TestUser.createLinked({ rememberMe: true, extraHeaders: { 'cf-city': 'r1' } });

        // initial session for a new user
        expect(await getTokens(user.sid)).toIncludeSameMembers([{ ...anyToken, city: 'r1' }]);
       
        // login and create new token
        const userCookies2 = await loginWithOAuth2(user.externalUser!, true, { 'cf-city': 'r2' });
        const sid2 = userCookies2.sid.value;
        expect(await getTokens(user.sid)).toIncludeSameMembers([
            { ...anyToken, city: 'r1' },
            { ...anyToken, city: 'r2' }
        ]);

        // login but don't create new token
        await loginWithOAuth2(user.externalUser!, false, { 'cf-city': 'r3' });
        expect(await getTokens(user.sid)).toIncludeSameMembers([
            { ...anyToken, city: 'r1' },
            { ...anyToken, city: 'r2' }
        ]);

        //logout from the second session
        await logout(sid2, false);
        expect(await getTokens(user.sid)).toIncludeSameMembers([{ ...anyToken, city: 'r1' }]);
    });

    it('Logout from all session', async () => {
        mock = await new Oauth2MockServer({ tls: config.mockTLS }).start();
        const now = new Date().getTime();
        const dateRange = [new Date(now - 60 * 1000), new Date(now + 60 * 1000)];

        const user = await TestUser.createLinked({ rememberMe: true });

        const anySession = {
            createdAt: expect.toBeBetween(dateRange[0], dateRange[1]),
            agent: '',
            country: null,
            region: null,
            city: null
        };

        // log in a few more times
        await loginWithOAuth2(user.externalUser!);
        await loginWithOAuth2(user.externalUser!);
        expect(await getTokens(user.sid)).toIncludeSameMembers([anySession, anySession, anySession]);

        //logout from the all the session
        await logout(user.sid, true);

        //logo in again and check sessions
        const newUserCookies = await loginWithOAuth2(user.externalUser!, true);
        expect(await getTokens(newUserCookies.sid.value)).toIncludeSameMembers([anySession]);
    });
});
