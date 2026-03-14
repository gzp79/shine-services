import { expect, test } from '$fixtures/setup';
import { UserInfoSchema } from '$lib/api/user_api';
import { getPageProblem, getPageRedirectUrl } from '$lib/api/utils';
import { createUrl } from '$lib/utils';

test.describe('Cookie security attributes', { tag: '@security' }, () => {
    test('Session cookies shall have HttpOnly flag', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        expect(response).toHaveStatus(200);
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();
        expect(cookies.sid.httpOnly).toBe(true);
        if (cookies.tid.value) {
            expect(cookies.tid.httpOnly).toBe(true);
        }
    });

    test('Session cookies shall have Secure flag on HTTPS', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        expect(response).toHaveStatus(200);
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();

        // Test environment uses HTTPS (https://cloud.local.scytta.com:8443)
        // Service running on HTTPS should always set Secure flag
        expect(cookies.sid.secure).toBe(true);
        if (cookies.tid.value) {
            expect(cookies.tid.secure).toBe(true);
        }
    });

    test('Session cookies shall have SameSite attribute', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        expect(response).toHaveStatus(200);
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();

        // Should be Lax or Strict for CSRF protection
        expect(['Lax', 'Strict', 'lax', 'strict']).toContain(cookies.sid.sameSite);
    });

    test('Session cookies shall have correct Domain attribute', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        expect(response).toHaveStatus(200);
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();

        // Domain should match service domain or be undefined (defaults to origin)
        if (cookies.sid.domain) {
            expect(cookies.sid.domain).toContain('scytta.com');
        }
    });

    test('Session cookies shall have correct Path attribute', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        expect(response).toHaveStatus(200);
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();

        // Path should be / or /identity
        expect(['/', '/identity']).toContain(cookies.sid.path);
    });

    test('Token cookie Max-Age shall match configured TTL for rememberMe', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        expect(response).toHaveStatus(200);
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();

        if (cookies.tid?.value) {
            // RememberMe token should have long TTL (14 days = 1209600 seconds)
            // Calculate from either Max-Age or Expires header
            let ttlSeconds: number;

            if (cookies.tid.expires) {
                ttlSeconds = Math.floor((cookies.tid.expires.getTime() - Date.now()) / 1000);
            } else {
                throw new Error('Token cookie has neither Max-Age nor Expires');
            }

            expect(ttlSeconds).toBeGreaterThan(1000000); // At least ~11 days
            expect(ttlSeconds).toBeLessThan(1500000); // At most ~17 days
        }
    });

    test('Cookies shall not be present on non-auth endpoints', async ({ api, identityUrl }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        const sid = response.cookies().sid.value;

        // Health check should not return cookies
        const healthResponse = await api.client.get(`${identityUrl}/info/ready`).withCookies({ sid });

        const healthCookies = healthResponse.cookies();
        // Should not set new cookies
        expect(healthCookies.sid).toBeUndefined();
        expect(healthCookies.tid).toBeUndefined();
    });

    test('Modified cookie value shall be rejected', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        expect(response).toHaveStatus(200);
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);
        const sid = response.cookies().sid.value;

        // Tamper with cookie value (modify signature)
        const tamperedSid = sid.slice(0, -10) + 'TAMPERED00';

        // Try to use tampered cookie on API endpoint
        // API endpoints must return 401 for invalid/tampered cookies
        const infoResponse = await api.user.getUserInfoRequest(tamperedSid, 'fast');
        expect(infoResponse).toHaveStatus(401);

        const problem = await infoResponse.parseProblem();
        expect(problem.type).toBe('unauthorized');
        expect(problem.detail).toBe('Missing session info');
        expect(problem.sensitive).toBe('unauthenticated');
    });

    test('Old rotated token after rotation window shall be rejected', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const tid0 = user.tid!;

        // Rotate token multiple times
        await user.rotateTID();
        await user.rotateTID();
        await user.rotateTID();

        // Old token should now be outside rotation window
        const response = await api.auth.loginWithTokenRequest(tid0, null, null, null, null, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(
            createUrl(api.auth.defaultRedirects.errorUrl, { errorType: 'auth-token-expired' })
        );
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-token-expired',
                status: 401,
                sensitive: 'expiredToken'
            })
        );
    });

    test('Cookie from different user shall not grant access', async ({ api }) => {
        const user1 = await api.testUsers.createGuest();
        const user2 = await api.testUsers.createGuest();

        // Try to use user1's session for user2's data
        const response = await api.user.getUserInfoRequest(user1.sid, 'fast');
        expect(response).toHaveStatus(200);
        const info1 = await response.parse(UserInfoSchema);

        // User IDs should match their own sessions
        expect(info1.userId).toEqual(user1.userId);
        expect(info1.userId).not.toEqual(user2.userId);

        // Using wrong session doesn't give access to other user's data
        // (this is more about authorization, but validates cookie binding)
        const user2Info = await api.user.getUserInfo(user2.sid, 'fast');
        expect(user2Info.userId).toEqual(user2.userId);
        expect(user2Info.userId).not.toEqual(user1.userId);
    });

    test('Multiple cookies with same name shall be handled consistently', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const attacker = await api.testUsers.createGuest();

        // Send request with both user and attacker cookies
        const response = await api.user.getUserInfoRequest(`${user.sid}; sid=${attacker.sid}`, 'fast');

        // Should authenticate with one of the valid cookies
        expect(response).toHaveStatus(200);
        const info = await response.parse(UserInfoSchema);

        // Should match one of the provided users (not an error state)
        expect([user.userId, attacker.userId]).toContain(info.userId);
    });
});
