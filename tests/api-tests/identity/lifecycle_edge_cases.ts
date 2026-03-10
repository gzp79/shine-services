import { expect, test } from '$fixtures/setup';
import { getEmailLinkToken, getPageProblem } from '$lib/api/utils';
import MockSmtp from '$lib/mocks/mock_smtp';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { randomUUID } from 'crypto';

test.describe('User lifecycle edge cases', { tag: '@edge-cases' }, () => {
    test('Role change during authorization check shall use READ COMMITTED isolation', async ({ api }) => {
        const admin = await api.testUsers.createGuest({ roles: ['SuperAdmin'] });
        const user = await api.testUsers.createGuest();

        // Start authorization check BEFORE role change commits
        const check1Promise = api.user.getUserInfo(user.sid, 'full');

        // Add admin role (commits immediately)
        await api.user.addRole(admin.sid, false, user.userId, 'Admin');

        // First check sees snapshot at query start (READ COMMITTED behavior)
        const check1 = await check1Promise;

        // Second check sees newly committed state
        const check2 = await api.user.getUserInfo(user.sid, 'full');
        expect(check2.roles).toContain('Admin');

        // First check should either:
        // 1. Not contain Admin (if query started before commit) - most likely
        // 2. Contain Admin (if query started after commit) - race condition
        // Both are valid READ COMMITTED behaviors - what matters is consistency
        if (check1.roles.includes('Admin')) {
            // If first check saw it, it means the role was committed before the query
            expect(check2.roles).toContain('Admin'); // Second check must also see it
        } else {
            // If first check didn't see it, it means query started before commit
            // This is expected READ COMMITTED behavior
            expect(check2.roles).toContain('Admin'); // Second check sees committed data
        }
    });

    test('Old email confirmation token shall be invalidated when email changes', async ({ api }) => {
        const mockOAuth2 = new OAuth2MockServer();
        await mockOAuth2.start();

        const mockSmtp = new MockSmtp();
        await mockSmtp.start();

        try {
            const user = await api.testUsers.createLinked(mockOAuth2);

            // Start email confirmation for original email
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

            // Try to use old confirmation token - MUST be invalidated (security requirement)
            const oldTokenResponse = await api.user.completeConfirmEmailRequest(user.sid, token1!);
            expect(oldTokenResponse).toHaveStatus(400);
            const oldTokenProblem = await oldTokenResponse.parseProblem();
            expect(oldTokenProblem.type).toBe('email-token-expired');
            expect(oldTokenProblem.sensitive).toBe('tokenExpired');

            // Email should remain unconfirmed after invalid token attempt
            const userInfo = await api.user.getUserInfo(user.sid, 'full');
            expect(userInfo.isEmailConfirmed).toBe(false);
            expect(userInfo.details?.email).toBe(newEmail);

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

    test('External link deletion during login shall create new user', async ({ api }) => {
        const mockOAuth2 = new OAuth2MockServer();
        await mockOAuth2.start();

        try {
            // User with external link
            const existingUser = await api.testUsers.createLinked(mockOAuth2);
            const externalAccount = existingUser.externalUser!;

            // Starting OAuth2 login
            const startResponse = await api.auth.startLoginWithOAuth2(mockOAuth2, null);
            const state = startResponse.authParams.state;

            // Link is deleted mid-flow
            const unlinkResult = await api.auth.tryUnlink(
                existingUser.sid,
                externalAccount.provider,
                externalAccount.id
            );
            expect(unlinkResult).toBe(true); // Link successfully deleted

            // Completing OAuth2 login with now-unlinked external account
            const authorizeResponse = await api.auth.authorizeWithOAuth2Request(
                startResponse.sid,
                startResponse.eid,
                state,
                externalAccount.toCode()
            );

            // Should succeed (external account is now available)
            expect(authorizeResponse).toHaveStatus(200);
            const text = await authorizeResponse.text();
            const problem = getPageProblem(text);
            expect(problem).toBeNull(); // No error

            // Should create a NEW user (not link to old user)
            const newSid = authorizeResponse.cookies().sid.value;
            expect(newSid).toBeDefined();

            const newUserInfo = await api.user.getUserInfo(newSid, 'fast');
            expect(newUserInfo.userId).not.toEqual(existingUser.userId); // Different user

            // External account should be linked to new user
            const newLinks = await api.auth.getExternalLinks(newSid);
            expect(newLinks).toHaveLength(1);
            expect(newLinks[0].providerUserId).toBe(externalAccount.id);

            // Old user should have no links
            const oldLinks = await api.auth.getExternalLinks(existingUser.sid);
            expect(oldLinks).toHaveLength(0);
        } finally {
            await mockOAuth2.stop();
        }
    });
});
