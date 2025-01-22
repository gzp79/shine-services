import { expect, test } from '$fixtures/setup';
import { MockServer } from '$lib/mocks/mock_server';
import OAuth2MockServer from '$lib/mocks/oauth2';

test.describe('Sessions', () => {
    // assume server is not off more than a few seconds and the test is fast enough
    const now = new Date().getTime();
    const createdRange = [new Date(now - 60 * 1000), new Date(now + 60 * 1000)];

    let mock!: MockServer;
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
        const response = await api.session.getSessionsRequest(null).send();
        expect(response).toHaveStatus(401);
    });

    test('Session shall keep the site info', async ({ api }) => {
        const extraHeaders = {
            'user-agent': 'agent',
            'cf-ipcountry': 'country',
            'cf-region': 'region',
            'cf-ipcity': 'city'
        };

        const user = await api.testUsers.createGuest({}, extraHeaders);

        const sessions = await api.session.getSessions(user.sid, extraHeaders);
        expect(sessions).toHaveLength(1);
        const session = sessions[0];
        expect(session.agent).toEqual('agent');
        expect(session.country).toEqual('country');
        expect(session.region).toEqual('region');
        expect(session.city).toEqual('city');
        expect(session.createdAt).toBeAfter(createdRange[0]);
        expect(session.createdAt).toBeBefore(createdRange[1]);
    });

    test('Multiple login shall create multiple session and logout from a session shall invalidate the connected session', async ({
        api
    }) => {
        const user = await api.testUsers.createGuest({}, { 'cf-region': 'r1' });

        // initial session for a new user
        let sessions = await api.session.getSessions(user.sid);
        expect(sessions).toHaveLength(1);
        const session = sessions[0];
        expect(session.region).toEqual('r1');
        expect(session.createdAt).toBeAfter(createdRange[0]);
        expect(session.createdAt).toBeBefore(createdRange[1]);

        // login from a new region, but keep agent intact, to bypass fingerprint check
        const userCookies2 = await api.auth.loginWithToken(user.tid!, false, { 'cf-region': 'r2' });
        const sid2 = userCookies2.sid;

        sessions = await api.session.getSessions(user.sid);
        expect(sessions).toHaveLength(2);
        for (const session of sessions) {
            expect(session.createdAt).toBeAfter(createdRange[0]);
            expect(session.createdAt).toBeBefore(createdRange[1]);
        }
        expect(sessions.map((s) => s.region).sort()).toEqual(['r1', 'r2']);

        sessions = await api.session.getSessions(sid2);
        expect(sessions).toHaveLength(2);
        for (const session of sessions) {
            expect(session.createdAt).toBeAfter(createdRange[0]);
            expect(session.createdAt).toBeBefore(createdRange[1]);
        }
        expect(sessions.map((s) => s.region).sort()).toEqual(['r1', 'r2']);

        //logout from the second session
        await api.auth.logout(sid2, null, false);
        sessions = await api.session.getSessions(user.sid);
        expect(sessions).toHaveLength(1);
        for (const session of sessions) {
            expect(session.createdAt).toBeAfter(createdRange[0]);
            expect(session.createdAt).toBeBefore(createdRange[1]);
        }
        expect(sessions.map((s) => s.region).sort()).toEqual(['r1']);
    });

    test('Multiple login shall create multiple session and logout with terminateAll shall invalidate all of them', async ({
        api
    }) => {
        const mock = await startMock();
        const user = await api.testUsers.createLinked(mock, { rememberMe: true }, { 'cf-region': 'r1' });

        // login a few more times
        await api.auth.loginWithOAuth2(mock, user.externalUser!, false, { 'cf-region': 'r2' });
        await api.auth.loginWithOAuth2(mock, user.externalUser!, false, { 'cf-region': 'r3' });

        let sessions = await api.session.getSessions(user.sid);
        expect(sessions).toHaveLength(3);
        expect(sessions.map((s) => s.region).sort()).toEqual(['r1', 'r2', 'r3']);

        //logout from all the session
        await api.auth.logout(user.sid, null, true);

        //login again and check sessions
        const newUserCookies = await api.auth.loginWithOAuth2(mock, user.externalUser!, false, {
            'cf-region': 'r4'
        });
        sessions = await api.session.getSessions(newUserCookies.sid);
        expect(sessions).toHaveLength(1);
        expect(sessions.map((s) => s.region).sort()).toEqual(['r4']);
    });
});
