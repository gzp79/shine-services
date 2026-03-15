import { expect, test } from '$fixtures/setup';
import { getPageProblem, getPageRedirectUrl } from '$lib/api/utils';
import { parseSignedCookie } from '$lib/utils';
import { createHash } from 'crypto';

test.describe('Token concurrency tests', { tag: '@concurrency' }, () => {
    test('Concurrent token rotation with same token shall produce unique tokens', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const tid0 = user.tid!;

        // Concurrent logins with same TID (triggers rotation)
        const [login1, login2] = await Promise.all([
            api.auth.loginWithToken(tid0, null),
            api.auth.loginWithToken(tid0, null)
        ]);

        const tid1 = login1.tid;
        const tid2 = login2.tid;

        // Both rotations should succeed with new tokens
        expect(tid1).toBeDefined();
        expect(tid2).toBeDefined();

        // New tokens should be different from original
        expect(tid1).not.toEqual(tid0);
        expect(tid2).not.toEqual(tid0);

        // Both new tokens should work
        const test1 = await api.auth.loginWithToken(tid1, null);
        const test2 = await api.auth.loginWithToken(tid2, null);
        expect(test1.sid).toBeDefined();
        expect(test2.sid).toBeDefined();
    });

    test('Token revocation during login attempt shall handle race gracefully', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const tid = user.tid!;
        const parsed = parseSignedCookie(tid);
        const tokenHash = createHash('sha256').update(parsed.key).digest('hex');

        // Race: login vs revoke
        const [loginResponse, revokeResponse] = await Promise.all([
            api.auth.loginWithTokenRequest(tid, null, null, null, null, null),
            api.token.revokeTokenRequest(user.sid, tokenHash)
        ]);

        // Revoke should succeed
        expect(revokeResponse).toHaveStatus(200);

        // Race between login and revoke:
        // - If login completes before revoke: successful session (no problem in HTML)
        // - If revoke completes first: auth-token-expired problem in HTML
        // Both outcomes are correct depending on timing
        expect(loginResponse).toHaveStatus(200);
        const loginText = await loginResponse.text();
        const loginProblem = getPageProblem(loginText);
        if (loginProblem) {
            expect(loginProblem.type).toBe('auth-token-expired');
        }

        // After both complete, token should be invalid
        const finalLogin = await api.auth.loginWithTokenRequest(tid, null, null, null, null, null);
        expect(finalLogin).toHaveStatus(200);
        const finalText = await finalLogin.text();
        const problem = getPageProblem(finalText);
        expect(problem?.type).toBe('auth-token-expired');
    });

    test('Concurrent token revocations shall be idempotent', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const tid = user.tid!;
        const parsed = parseSignedCookie(tid);
        const tokenHash = createHash('sha256').update(parsed.key).digest('hex');

        // Concurrent revocations
        const [revoke1, revoke2, revoke3] = await Promise.all([
            api.token.revokeTokenRequest(user.sid, tokenHash),
            api.token.revokeTokenRequest(user.sid, tokenHash),
            api.token.revokeTokenRequest(user.sid, tokenHash)
        ]);

        // At least one should succeed
        const statuses = [revoke1.status(), revoke2.status(), revoke3.status()];
        expect(statuses).toContain(200);

        // Token should be revoked
        const loginResponse = await api.auth.loginWithTokenRequest(tid, null, null, null, null, null);
        const text = await loginResponse.text();
        const problem2 = getPageProblem(text);
        expect(problem2?.type).toBe('auth-token-expired');
    });

    test('Token rotation and logout race shall resolve without errors', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const tid = user.tid!;

        // Race: rotate token (login) vs logout
        const [loginResponse, logoutResponse] = await Promise.all([
            api.auth.loginWithTokenRequest(tid, null, null, null, null, null),
            api.auth.logoutRequest(user.sid, tid, false)
        ]);

        // Both should complete without error
        expect([200]).toContain(loginResponse.status());
        expect(logoutResponse).toHaveStatus(200);
        const logoutText = await logoutResponse.text();
        expect(getPageRedirectUrl(logoutText)).toEqual(api.auth.defaultRedirects.redirectUrl);

        // Original token should be invalid
        const finalLogin = await api.auth.loginWithTokenRequest(tid, null, null, null, null, null);
        const text = await finalLogin.text();
        const problem2 = getPageProblem(text);
        expect(problem2?.type).toBe('auth-token-expired');
    });
});
