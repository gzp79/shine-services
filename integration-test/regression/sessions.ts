import request from 'superagent';
import { ActiveSession, getSessions, logout } from '$lib/auth_utils';
import config from '../test.config';
import Oauth2MockServer from '$lib/mocks/oauth2';
import { loginWithOAuth2, loginWithToken } from '$lib/login_utils';
import { MockServer } from '$lib/mock_server';
import { TestUser } from '$lib/user';

describe('Sessions', () => {
    let mock!: MockServer;

    // assume server is not off more than a few seconds and the test is fast enough
    const now = new Date().getTime();
    const dateRange = [new Date(now - 60 * 1000), new Date(now + 60 * 1000)];
    const anySession: ActiveSession = {
        userId: expect.toBeString(),
        createdAt: expect.toBeBetween(dateRange[0], dateRange[1]),
        agent: '',
        country: null,
        region: null,
        city: null
    };

    afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
    });

    it('Get session without user should fail', async () => {
        // initial session for a new user
        let response = await request
            .get(config.getUrlFor('identity/api/auth/user/sessions'))
            .send()
            .catch((err) => err.response);
        expect(response.statusCode).toEqual(401);
    });

    it('Session should keep site-info', async () => {
        const extraHeaders = {
            'user-agent': 'agent',
            'cf-ipcountry': 'country',
            'cf-region': 'region',
            'cf-ipcity': 'city'
        };

        const user = await TestUser.createGuest({ extraHeaders });

        // initial session for a new user
        expect(await getSessions(user.sid, extraHeaders)).toIncludeSameMembers([
            {
                ...anySession,
                agent: 'agent',
                country: 'country',
                region: 'region',
                city: 'city'
            }
        ]);
    });

    it('Multiple login should create multiple sessions', async () => {
        const user = await TestUser.createGuest({ extraHeaders: { 'cf-region': 'r1' } });

        // initial session for a new user
        expect(await getSessions(user.sid)).toIncludeSameMembers([{ ...anySession, region: 'r1' }]);

        // login from a new country (agent is not altered, to bypass fingerprint check)
        const userCookies2 = await loginWithToken(user.tid!, { 'cf-region': 'r2' });
        const sid2 = userCookies2.sid.value;
        expect(await getSessions(sid2)).toIncludeSameMembers([
            { ...anySession, region: 'r1' },
            { ...anySession, region: 'r2' }
        ]);

        //logout from the second session
        await logout(sid2, false);
        expect(await getSessions(user.sid)).toIncludeSameMembers([{ ...anySession, region: 'r1' }]);
    });

    it('Logout from all session', async () => {
        mock = await new Oauth2MockServer({ tls: config.mockTLS }).start();

        const user = await TestUser.createLinked({
            rememberMe: true,
            extraHeaders: { 'cf-region': 'r1' }
        });

        // login a few more times
        await loginWithOAuth2(user.externalUser!, false, { 'cf-region': 'r2' });
        await loginWithOAuth2(user.externalUser!, false, { 'cf-region': 'r3' });
        expect(await getSessions(user.sid)).toIncludeSameMembers([
            { ...anySession, region: 'r1' },
            { ...anySession, region: 'r2' },
            { ...anySession, region: 'r3' }
        ]);

        //logout from the all the session
        await logout(user.sid, true);

        //login again and check sessions
        const newUserCookies = await loginWithOAuth2(user.externalUser!, false, { 'cf-region': 'r4' });
        expect(await getSessions(newUserCookies.sid.value)).toIncludeSameMembers([
            { ...anySession, region: 'r4' }
        ]);
    });
});
