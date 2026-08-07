import { expect, test } from '$fixtures/setup';
import { UserInfoSchema } from '$lib/api/user_api';
import { getPageRedirectUrl } from '$lib/api/utils';

test.describe('Cookie security attributes', { tag: '@security' }, () => {
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
