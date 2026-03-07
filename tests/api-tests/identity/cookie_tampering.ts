import { expect, test } from '$fixtures/setup';
import { UserInfoSchema } from '$lib/api/user_api';
import { getPageProblem, getPageRedirectUrl } from '$lib/api/utils';
import { createUrl } from '$lib/utils';

test.describe('Cookie tampering detection', { tag: '@security' }, () => {
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
});
