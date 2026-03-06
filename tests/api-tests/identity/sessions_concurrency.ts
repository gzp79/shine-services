import { expect, test } from '$fixtures/setup';
import OAuth2MockServer from '$lib/mocks/oauth2';

test.describe('Session concurrency tests', { tag: '@concurrency' }, () => {
    let mockOAuth2: OAuth2MockServer;

    test.beforeEach(async () => {
        mockOAuth2 = new OAuth2MockServer();
        await mockOAuth2.start();
    });

    test.afterEach(async () => {
        await mockOAuth2.stop();
    });

    test('Concurrent logins from same token shall create separate sessions', async ({ api }) => {
        const user = await api.testUsers.createLinked(mockOAuth2, { rememberMe: true });
        const tid = user.tid!;

        // Concurrent logins with same TID
        const [session1, session2] = await Promise.all([
            api.auth.loginWithToken(tid, null),
            api.auth.loginWithToken(tid, null)
        ]);

        // Both should succeed with different SIDs
        expect(session1.sid).toBeDefined();
        expect(session2.sid).toBeDefined();
        expect(session1.sid).not.toEqual(session2.sid);

        // Both sessions should be valid
        const sessions = await api.session.getSessions(session1.sid);
        expect(sessions.length).toBeGreaterThanOrEqual(2);
    });

    test('Logout during active API request shall handle gracefully', async ({ api }) => {
        const user = await api.testUsers.createLinked(mockOAuth2);

        // Start a request, then logout before it completes
        const infoPromise = api.user.getUserInfoRequest(user.sid, 'full');
        const logoutPromise = api.auth.logoutRequest(user.sid, null, false);

        const [infoResponse, logoutResponse] = await Promise.all([infoPromise, logoutPromise]);

        // Logout should succeed
        expect(logoutResponse).toHaveStatus(200);

        // Info request may succeed or fail depending on timing
        // Either outcome is acceptable, as long as it doesn't crash
        expect([200, 401]).toContain(infoResponse.status());
    });

    test('Session refresh during logout shall handle race condition', async ({ api }) => {
        const user = await api.testUsers.createLinked(mockOAuth2);

        // Race: refresh (getUserInfo) vs logout
        const [refreshResponse, logoutResponse] = await Promise.all([
            api.user.getUserInfoRequest(user.sid, 'fast'),
            api.auth.logoutRequest(user.sid, null, false)
        ]);

        // Logout should always succeed
        expect(logoutResponse).toHaveStatus(200);

        // Refresh may succeed or fail
        expect([200, 401]).toContain(refreshResponse.status());

        // After both complete, session should be invalid
        const finalCheck = await api.user.getUserInfoRequest(user.sid, 'fast');
        expect(finalCheck).toHaveStatus(401);
    });

    test('Simultaneous session creation from multiple IPs shall all succeed', async ({ api }) => {
        const user = await api.testUsers.createLinked(mockOAuth2, { rememberMe: true });

        // Simulate logins from different locations
        const [s1, s2, s3] = await Promise.all([
            api.auth.loginWithToken(user.tid!, false, { 'cf-region': 'us-east' }),
            api.auth.loginWithToken(user.tid!, false, { 'cf-region': 'eu-west' }),
            api.auth.loginWithToken(user.tid!, false, { 'cf-region': 'ap-south' })
        ]);

        // All should succeed
        expect(s1.sid).toBeDefined();
        expect(s2.sid).toBeDefined();
        expect(s3.sid).toBeDefined();

        // All SIDs should be different
        const sids = [s1.sid, s2.sid, s3.sid];
        expect(new Set(sids).size).toEqual(3);

        // Should see all 3 sessions
        const sessions = await api.session.getSessions(s1.sid);
        expect(sessions.length).toBeGreaterThanOrEqual(3);
    });
});
