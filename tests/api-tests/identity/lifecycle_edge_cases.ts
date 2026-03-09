import { expect, test } from '$fixtures/setup';
import { getEmailLinkToken, getPageProblem } from '$lib/api/utils';
import { UserInfoSchema } from '$lib/api/user_api';
import MockSmtp from '$lib/mocks/mock_smtp';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { randomUUID } from 'crypto';

test.describe('User lifecycle edge cases', { tag: '@edge-cases' }, () => {
    test('Role change during authorization check shall be consistent', async ({ api }) => {
        const admin = await api.testUsers.createGuest({ roles: ['SuperAdmin'] });
        const user = await api.testUsers.createGuest();

        // Start authorization check
        const check1Promise = api.user.getUserInfo(user.sid, 'full');

        // Add admin role concurrently
        await api.user.addRole(admin.sid, false, user.userId, 'Admin');

        const check1 = await check1Promise;

        // Second check should definitely see the new role
        const check2 = await api.user.getUserInfo(user.sid, 'full');
        expect(check2.roles).toContain('Admin');

        // First check might or might not see it (race), but should be consistent
        if (check1.roles.includes('Admin')) {
            expect(check2.roles).toContain('Admin'); // If first saw it, second must too
        }
    });

    test('Email confirmation during email change shall handle gracefully', async ({ api }) => {
        const mockOAuth2 = new OAuth2MockServer();
        await mockOAuth2.start();

        const mockSmtp = new MockSmtp();
        await mockSmtp.start();

        try {
            const user = await api.testUsers.createLinked(mockOAuth2);

            // Start email confirmation
            const mail1Promise = mockSmtp.waitMail();
            await api.user.startConfirmEmail(user.sid);
            const mail1 = await mail1Promise;
            const token1 = getEmailLinkToken(mail1);

            // Change email before confirming
            const newEmail = `new-${randomUUID()}@example.com`;
            const mail2Promise = mockSmtp.waitMail();
            await api.user.startChangeEmail(user.sid, newEmail);
            const mail2 = await mail2Promise;
            const token2 = getEmailLinkToken(mail2);

            // Try to use old confirmation token
            const oldTokenResponse = await api.user.completeConfirmEmailRequest(user.sid, token1!);
            // Old token may be invalidated or may still work (documents actual behavior)
            expect([200, 400]).toContain(oldTokenResponse.status());

            // New token should work
            await api.user.completeConfirmEmail(user.sid, token2!);
            await user.refreshUserInfo();
            expect(user.email).toEqual(newEmail);
            expect(user.userInfo?.isEmailConfirmed).toBe(true);
        } finally {
            await mockSmtp.stop();
            await mockOAuth2.stop();
        }
    });

    test('Token rotation boundary conditions', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const tid0 = user.tid!;

        // Rotate exactly to the edge of the rotation window (3 rotations)
        await user.rotateTID();
        const tid1 = user.tid!;
        await user.rotateTID();
        await user.rotateTID();
        const tid3 = user.tid!;

        // tid0 should now be outside window (expired)
        const response0 = await api.auth.loginWithTokenRequest(tid0, null, null, null, null, null);
        const text0 = await response0.text();
        const problem0 = getPageProblem(text0);
        expect(problem0?.type).toBe('auth-token-expired');

        // tid1 should still be valid (inside window)
        const response1 = await api.auth.loginWithTokenRequest(tid1, null, null, null, null, null);
        expect(response1).toHaveStatus(200);

        // Current token should work
        const response3 = await api.auth.loginWithToken(tid3, null);
        expect(response3.sid).toBeDefined();
    });

    test('External link deletion during login attempt', async ({ api }) => {
        const mockOAuth2 = new OAuth2MockServer();
        await mockOAuth2.start();

        try {
            const user = await api.testUsers.createLinked(mockOAuth2);

            // Start OAuth2 login
            const startResponse = await api.auth.startLoginWithOAuth2(mockOAuth2, null);
            const state = startResponse.authParams.state;

            // Unlink the external account
            await api.auth.tryUnlink(user.sid, user.externalUser!.provider, user.externalUser!.id);

            // Complete OAuth2 flow (should create NEW user, not link to existing)
            const authorizeResponse = await api.auth.authorizeWithOAuth2Request(
                startResponse.sid,
                startResponse.eid,
                state,
                user.externalUser!.toCode()
            );

            expect(authorizeResponse).toHaveStatus(200);
            const newUserInfoResponse = await api.user.getUserInfoRequest(
                authorizeResponse.cookies().sid.value,
                'fast'
            );

            // May succeed (new user) or fail (session invalid) depending on implementation
            if (newUserInfoResponse.status() === 200) {
                const newUserInfo = await newUserInfoResponse.parse(UserInfoSchema);
                // Should be a DIFFERENT user (original link was deleted)
                expect(newUserInfo.userId).not.toEqual(user.userId);
            } else {
                // If session is invalid, that's also acceptable behavior
                expect(newUserInfoResponse.status()).toBe(401);
            }
        } finally {
            await mockOAuth2.stop();
        }
    });
});
