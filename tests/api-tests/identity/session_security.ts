import { expect, test } from '$fixtures/setup';
import { getEmailLink, getPageProblem, getPageRedirectUrl } from '$lib/api/utils';
import MockSmtp from '$lib/mocks/mock_smtp';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { randomUUID } from 'crypto';

test.describe('Session security', { tag: '@security' }, () => {
    let mockOAuth2: OAuth2MockServer;

    test.beforeEach(async () => {
        mockOAuth2 = new OAuth2MockServer();
        await mockOAuth2.start();
    });

    test.afterEach(async () => {
        await mockOAuth2.stop();
    });

    test('Session fixation attack shall be prevented', async ({ api }) => {
        const mockSmtp = new MockSmtp();
        await mockSmtp.start();

        try {
            // Step 1: Attacker obtains a session ID
            const attackerSession = await api.auth.loginAsGuestUser();
            const attackerSid = attackerSession.sid;
            const attackerUserId = (await api.user.getUserInfo(attackerSid, 'full')).userId;

            // Step 2: Victim starts email login
            const victimEmail = `victim-${randomUUID()}@example.com`;
            const mailPromise = mockSmtp.waitMail();

            // Step 3: CRITICAL - Victim initiates login while having attacker's session cookie
            // This simulates the attack: victim's browser has attacker's session cookie
            const loginInitResponse = await api.auth
                .loginWithEmailRequest(victimEmail, false, null)
                .withCookies({ sid: attackerSid });

            expect(loginInitResponse).toHaveStatus(200);

            // Step 4: Victim clicks email link
            const mail = await mailPromise;
            const loginLink = getEmailLink(mail);
            const loginCompleteResponse = await api.client.get(loginLink).withCookies({ sid: attackerSid }); // Still using attacker's session

            // Step 5: SECURITY CHECK - Victim must get a NEW session, not reuse attacker's
            const victimSid = loginCompleteResponse.cookies().sid.value;
            expect(victimSid).toBeDefined();
            expect(victimSid).not.toEqual(attackerSid); // CRITICAL: New session required

            const text = await loginCompleteResponse.text();
            const problem = getPageProblem(text);
            expect(problem).toBeNull(); // Login should succeed

            // Step 6: Verify system invalidated attacker's session (excellent security)
            const attackerInfoAfter = await api.user.getUserInfoRequest(attackerSid, 'full');
            expect(attackerInfoAfter).toHaveStatus(401); // Attacker's session invalidated!

            // Victim has correct session
            const victimInfo = await api.user.getUserInfo(victimSid, 'full');
            expect(victimInfo.userId).not.toBe(attackerUserId); // Victim is separate user
            expect(victimInfo.details?.email).toBe(victimEmail); // Victim has their email
        } finally {
            await mockSmtp.stop();
        }
    });

    test('System shall support multiple concurrent sessions', async ({ api }) => {
        const user = await api.testUsers.createGuest();

        // User creates multiple concurrent sessions with distinct metadata
        const SESSION_COUNT = 12;
        const sessions = await Promise.all(
            Array.from({ length: SESSION_COUNT }, (_, i) =>
                api.auth.loginWithToken(user.tid!, null, { 'cf-region': `region-${i}` })
            )
        );

        // All sessions should be created successfully
        expect(sessions.length).toBe(SESSION_COUNT);
        sessions.forEach((session) => {
            expect(session.sid).toBeDefined();
            expect(session.sid.length).toBeGreaterThan(50); // Session ID is a signed cookie
        });

        // All sessions should be tracked and retrievable
        const activeSessions = await api.session.getSessions(sessions[SESSION_COUNT - 1].sid);
        expect(activeSessions.length).toBeGreaterThanOrEqual(SESSION_COUNT);

        // All sessions should be functional
        for (const session of sessions) {
            const sessionCheck = await api.user.getUserInfoRequest(session.sid, 'fast');
            expect(sessionCheck).toHaveStatus(200);
        }

        // Session metadata should be preserved
        const regionCounts = activeSessions.filter((s) => s.region?.startsWith('region-')).length;
        expect(regionCounts).toBe(SESSION_COUNT);
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
        const logoutText = await logoutResponse.text();
        expect(getPageRedirectUrl(logoutText)).toEqual(api.auth.defaultRedirects.redirectUrl);

        // Race between logout and getUserInfo:
        // - If getUserInfo reads session before logout deletes: 200
        // - If logout completes first: 401
        // Both outcomes are correct depending on timing
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
        const logoutText = await logoutResponse.text();
        expect(getPageRedirectUrl(logoutText)).toEqual(api.auth.defaultRedirects.redirectUrl);

        // Race between refresh and logout:
        // - If refresh reads session before logout: 200
        // - If logout completes first: 401
        // Both outcomes are correct depending on timing
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
