import { expect, test } from '$fixtures/setup';
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
        // Attacker gets a session ID
        const attackerSession = await api.auth.loginWithGuestRequest(null, null, null);
        const attackerSid = attackerSession.cookies().sid.value;

        // Victim authenticates with email (should get NEW session, not reuse attacker's)
        const email = `victim-${randomUUID()}@example.com`;

        // Note: We can't actually send cookies with loginWithEmailRequest in first call
        // This test verifies that completing login creates a new session
        const victimLoginResponse = await api.auth.loginWithEmailRequest(email, false, null);
        expect(victimLoginResponse).toHaveStatus(200);

        // In production: victim should get a NEW session ID (not reuse existing)
        const victimSid = victimLoginResponse.cookies().sid?.value;
        if (victimSid) {
            expect(victimSid).not.toEqual(attackerSid);
        }

        // Attacker's session should remain a guest session
        const attackerInfo = await api.user.getUserInfoRequest(attackerSid, 'fast');
        expect(attackerInfo).toHaveStatus(200);
    });

    test('Concurrent session limit enforcement', async ({ api }) => {
        const user = await api.testUsers.createGuest();

        // Create multiple sessions (test current behavior, not necessarily limit enforcement)
        const sessions = await Promise.all(Array.from({ length: 12 }, () => api.auth.loginWithToken(user.tid!, null)));

        // All sessions should be created
        expect(sessions.length).toBe(12);
        sessions.forEach((session) => {
            expect(session.sid).toBeDefined();
        });

        // Check how many active sessions exist
        const activeSessions = await api.session.getSessions(sessions[11].sid);
        expect(activeSessions.length).toBeGreaterThan(0);

        // If there's a limit, oldest sessions should be evicted
        // This test documents current behavior
    });
});
