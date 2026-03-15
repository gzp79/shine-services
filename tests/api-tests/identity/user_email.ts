import { expect, test } from '$fixtures/setup';
import { ProblemSchema } from '$lib/api/api';
import { getEmailLink, getEmailLinkToken } from '$lib/api/utils';
import MockSmtp from '$lib/mocks/mock_smtp';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { randomUUID } from 'crypto';

async function ensureSmtpStarted(mock: MockSmtp | undefined): Promise<MockSmtp> {
    if (mock) return mock;
    const m = new MockSmtp();
    await m.start();
    return m;
}

test.describe('Email confirmation', () => {
    let mockAuth: OAuth2MockServer = undefined!;
    let mockEmail: MockSmtp = undefined!;

    const startMockEmail = async () => (mockEmail = await ensureSmtpStarted(mockEmail));

    test.beforeAll(async () => {
        mockAuth = new OAuth2MockServer();
        await mockAuth.start();
    });

    test.afterEach(async () => {
        await mockEmail?.stop();
        mockEmail = undefined!;
    });

    test.afterAll(async () => {
        await mockAuth?.stop();
    });

    test('Requesting email confirmation without session shall fail', async ({ api }) => {
        const response = await api.user.startConfirmEmailRequest(null);
        expect(response).toHaveStatus(401);
        expect(await response.parseProblem()).toEqual(
            expect.objectContaining({
                type: 'unauthorized',
                status: 401,
                sensitive: 'unauthenticated'
            })
        );
    });

    test('Requesting email confirmation without email shall fail', async ({ api }) => {
        const smtp = await startMockEmail();
        smtp.onMail((_mail) => {
            expect(true, 'No message should arrive').toBe(false);
        });

        const user = await api.testUsers.createGuest();
        const response = await api.user.startConfirmEmailRequest(user.sid);
        expect(response).toHaveStatus(412);
        expect(await response.parseProblem()).toEqual(
            expect.objectContaining({
                type: 'email-missing-email',
                status: 412
            })
        );
    });

    test('Requesting email confirmation with email address shall succeed', async ({ linkUrl, api }) => {
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });

        const smtp = await startMockEmail();
        const mailPromise = smtp.waitMail();
        await api.user.startConfirmEmailRequest(user.sid);
        const mail = await mailPromise;
        expect(mail).toHaveMailTo(email);
        expect(mail).toContainMailBody(`<p>Hello ${user.name},</p>`);
        expect(mail).toContainMailBody('Confirm Email Address');
        expect(mail).toContainMailBody('<p>Best regards,<br/>Scytta</p>');

        const confirmUrl = getEmailLink(mail);
        expect(confirmUrl).toStartWith(linkUrl);
        const confirmParams = new URL(confirmUrl).searchParams;
        expect(confirmParams.get('token')).toBeString();
    });

    test('Requesting email confirmation with invalid lang shall fail', async ({ api }) => {
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });
        const response = await api.user.startConfirmEmailRequest(user.sid, 'invalid');
        expect(response).toHaveStatus(400);
        expect(await response.parseProblem()).toEqual(
            expect.objectContaining({
                type: 'input-query-format',
                status: 400,
                detail: expect.stringContaining('lang: unknown variant `invalid`, expected `en` or `hu`')
            })
        );
    });

    test('Requesting email confirmation with 3rd party error shall fail', async ({ api }) => {
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });
        const response = await api.user.startConfirmEmailRequest(user.sid);
        expect(response).toHaveStatus(500);
    });

    test('Completing email confirmation with invalid token shall fail', async ({ api }) => {
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });
        const response = await api.user.completeConfirmEmailRequest(user.sid, 'invalid');
        expect(response).toHaveStatus(400);

        const error = await response.parse(ProblemSchema);
        expect(error).toEqual(
            expect.objectContaining({
                type: 'email-token-expired',
                status: 400
            })
        );
    });

    for (const lang of ['en', 'hu', undefined]) {
        test(`Confirming email with lang ${lang} shall work`, async ({ api }) => {
            const user = await api.testUsers.createLinked(mockAuth);
            const { sessionLength, remainingSessionTime, ...userInfo } = user.userInfo!;

            const smtp = await startMockEmail();
            const mailPromise = smtp.waitMail();
            await api.user.startConfirmEmail(user.sid, lang);
            const mail = await mailPromise;

            const token = getEmailLinkToken(mail);
            expect(token).toBeString();
            await api.user.completeConfirmEmail(user.sid, token!);

            expect(await api.user.getUserInfo(user.sid, 'fast')).toEqual(
                expect.objectContaining({
                    ...userInfo,
                    isEmailConfirmed: true,
                    details: null
                })
            );

            expect(await api.user.getUserInfo(user.sid, 'full')).toEqual(
                expect.objectContaining({
                    ...userInfo,
                    isEmailConfirmed: true
                })
            );
        });
    }

    for (const userType of ['guest', 'linked']) {
        test(`Confirming email with another ${userType} user shall fail (security: token bound to user)`, async ({
            api
        }) => {
            const user = await api.testUsers.createLinked(mockAuth);

            const smtp = await startMockEmail();
            const mailPromise = smtp.waitMail();
            await api.user.startConfirmEmail(user.sid);
            const mail = await mailPromise;

            const token = getEmailLinkToken(mail);

            // Create different user type to attempt token theft
            const user2 =
                userType === 'guest' ? await api.testUsers.createGuest() : await api.testUsers.createLinked(mockAuth);
            const response = await api.user.completeConfirmEmailRequest(user2.sid, token!);
            expect(response).toHaveStatus(400);

            const error = await response.parse(ProblemSchema);
            expect(error).toEqual(
                expect.objectContaining({
                    type: 'email-token-expired',
                    status: 400,
                    sensitive: 'wrongUser'
                })
            );

            // Neither user's email should be confirmed
            expect(await api.user.getUserInfo(user.sid, 'fast')).toEqual(
                expect.objectContaining({ isEmailConfirmed: false, details: null })
            );
            expect(await api.user.getUserInfo(user.sid, 'full')).toEqual(
                expect.objectContaining({ isEmailConfirmed: false, details: user.userInfo?.details })
            );

            expect(await api.user.getUserInfo(user2.sid, 'fast')).toEqual(
                expect.objectContaining({ isEmailConfirmed: false, details: null })
            );
            expect(await api.user.getUserInfo(user2.sid, 'full')).toEqual(
                expect.objectContaining({ isEmailConfirmed: false, details: user2.userInfo?.details })
            );
        });
    }

    test('Concurrent confirmation requests shall enforce unique token constraint', async ({ api }) => {
        // Security: Database unique constraint prevents multiple email tokens per user
        // This protects against race conditions and ensures only one valid token exists
        const user = await api.testUsers.createLinked(mockAuth);

        const smtp = await startMockEmail();
        const mailsPromise = smtp.waitEmails(3);

        // Request email confirmation multiple times concurrently
        const requests = await Promise.all([
            api.user.startConfirmEmailRequest(user.sid),
            api.user.startConfirmEmailRequest(user.sid),
            api.user.startConfirmEmailRequest(user.sid)
        ]);

        // All API requests succeed (endpoint doesn't fail)
        requests.forEach((r) => expect(r).toHaveStatus(200));

        // Wait for all emails
        const mails = await mailsPromise;
        expect(mails).toHaveLength(3);

        // Extract tokens
        const tokens = mails.map((m) => getEmailLinkToken(m)!);
        expect(tokens.every((t) => t !== null)).toBe(true);

        // Due to unique constraint, only the LAST token should work
        // Earlier tokens were deleted from database when newer ones were created
        const lastToken = tokens[tokens.length - 1];

        // Try first token - should be invalidated
        const response1 = await api.user.completeConfirmEmailRequest(user.sid, tokens[0]);
        expect(response1).toHaveStatus(400);
        const problem1 = await response1.parseProblem();
        expect(problem1.type).toBe('email-token-expired');

        // Try second token - should also be invalidated
        const response2 = await api.user.completeConfirmEmailRequest(user.sid, tokens[1]);
        expect(response2).toHaveStatus(400);
        const problem2 = await response2.parseProblem();
        expect(problem2.type).toBe('email-token-expired');

        // Last token should work
        await api.user.completeConfirmEmail(user.sid, lastToken);
        await user.refreshUserInfo();
        expect(user.userInfo?.isEmailConfirmed).toBe(true);
    });

    test('Delete user with pending email confirmation shall prevent completing confirmation', async ({ api }) => {
        const smtp = await startMockEmail();
        const user = await api.testUsers.createLinked(mockAuth);

        const mailPromise = smtp.waitMail();
        await api.user.startConfirmEmail(user.sid);
        const mail = await mailPromise;
        const token = getEmailLinkToken(mail);
        expect(token).toBeString();

        await api.auth.deleteUserRequest(user.sid, user.name);

        // After deletion the session is revoked — completing confirmation is no longer possible
        const response = await api.user.completeConfirmEmailRequest(user.sid, token!);
        expect(response).toHaveStatus(401);
    });
});

test.describe('Email change', () => {
    let mockAuth: OAuth2MockServer = undefined!;
    let mockEmail: MockSmtp = undefined!;

    const startMockEmail = async () => (mockEmail = await ensureSmtpStarted(mockEmail));

    test.beforeAll(async () => {
        mockAuth = new OAuth2MockServer();
        await mockAuth.start();
    });

    test.afterEach(async () => {
        await mockEmail?.stop();
        mockEmail = undefined!;
    });

    test.afterAll(async () => {
        await mockAuth?.stop();
    });

    test('Requesting email change without session shall fail', async ({ api }) => {
        const response = await api.user.startChangeEmailRequest(null, 'sample@example.com');
        expect(response).toHaveStatus(401);
        expect(await response.parseProblem()).toEqual(
            expect.objectContaining({
                type: 'unauthorized',
                status: 401,
                sensitive: 'unauthenticated'
            })
        );
    });

    test('Requesting email change with 3rd party error shall fail', async ({ api }) => {
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });
        const response = await api.user.startChangeEmailRequest(user.sid, 'sample@example.com');
        expect(response).toHaveStatus(500);
    });

    test('Requesting email change with invalid lang shall fail', async ({ api }) => {
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });
        const response = await api.user.startChangeEmailRequest(user.sid, 'sample@example.com', 'invalid');
        expect(response).toHaveStatus(400);
        expect(await response.parseProblem()).toEqual(
            expect.objectContaining({
                type: 'input-query-format',
                status: 400,
                detail: expect.stringContaining('lang: unknown variant `invalid`, expected `en` or `hu`')
            })
        );
    });

    test('Requesting email change with invalid email format shall fail', async ({ api }) => {
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });

        const invalidEmails = ['invalid', 'no-at-sign', '@example.com', 'user@', 'user @example.com', ''];

        for (const invalidEmail of invalidEmails) {
            const response = await api.user.startChangeEmailRequest(user.sid, invalidEmail);
            expect(response, `Email "${invalidEmail}" should be rejected`).toHaveStatus(400);

            const problem = await response.parseProblem();
            expect(problem).toEqual(
                expect.objectContaining({
                    type: 'input-body-format',
                    status: 400,
                    detail: expect.stringContaining('email')
                })
            );
        }
    });

    test('Changing email with unconfirmed email shall succeed', async ({ api, adminUser }) => {
        // Note: Language support tested in confirmation tests above
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });
        const { sessionLength, remainingSessionTime, ...userInfo } = user.userInfo!;

        const smtp = await startMockEmail();
        const mailPromise = smtp.waitMail();
        const newEmail = `updated-${randomUUID()}@example.com`;
        expect(user.email).not.toEqual(newEmail);
        await api.user.startChangeEmail(user.sid, newEmail);
        const mail = await mailPromise;

        const token = getEmailLinkToken(mail);
        expect(token).toBeString();
        await api.user.completeConfirmEmail(user.sid, token!);

        expect(await api.user.getUserInfo(user.sid, 'fast')).toEqual(
            expect.objectContaining({
                ...userInfo,
                isEmailConfirmed: true,
                details: null
            })
        );
        expect(await api.user.getUserInfo(user.sid, 'full')).toEqual(
            expect.objectContaining({
                ...userInfo,
                isEmailConfirmed: true,
                details: { ...userInfo?.details, email: newEmail }
            })
        );

        // After email change: old email not searchable, new email finds the user
        const searchOld = await api.user.searchIdentities(adminUser.sid, { email });
        expect(searchOld.identities.some((i) => i.id === user.userId)).toBe(false);

        const searchNew = await api.user.searchIdentities(adminUser.sid, { email: newEmail });
        expect(searchNew.identities).toHaveLength(1);
        expect(searchNew.identities[0].id).toBe(user.userId);
    });

    test('Changing email without email shall succeed', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const { sessionLength, remainingSessionTime, ...userInfo } = user.userInfo!;
        expect(user.email).not.toBeNull();

        const smtp = await startMockEmail();
        const mailPromise = smtp.waitMail();
        const newEmail = `updated-${randomUUID()}@example.com`;
        await api.user.startChangeEmail(user.sid, newEmail);
        const mail = await mailPromise;

        const token = getEmailLinkToken(mail);
        expect(token).toBeString();
        await api.user.completeConfirmEmail(user.sid, token!);

        expect(await api.user.getUserInfo(user.sid, 'fast')).toEqual(
            expect.objectContaining({
                ...userInfo,
                isEmailConfirmed: true,
                isGuest: false,
                details: null
            })
        );
        expect(await api.user.getUserInfo(user.sid, 'full')).toEqual(
            expect.objectContaining({
                ...userInfo,
                isEmailConfirmed: true,
                isGuest: false,
                details: { ...userInfo?.details, email: newEmail }
            })
        );
    });

    test('Changing email with confirmed email shall succeed', async ({ api, adminUser }) => {
        const smtp = await startMockEmail();

        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });
        await user.confirmEmail(smtp);
        const { sessionLength, remainingSessionTime, ...userInfo } = user.userInfo!;

        const mailPromise = smtp.waitMail();
        const newEmail = `updated-${randomUUID()}@example.com`;
        expect(user.email).not.toEqual(newEmail);
        await api.user.startChangeEmail(user.sid, newEmail);
        const mail = await mailPromise;

        const token = getEmailLinkToken(mail);
        expect(token).toBeString();
        await api.user.completeConfirmEmail(user.sid, token!);

        expect(await api.user.getUserInfo(user.sid, 'fast')).toEqual(
            expect.objectContaining({
                ...userInfo,
                details: null
            })
        );
        expect(await api.user.getUserInfo(user.sid, 'full')).toEqual(
            expect.objectContaining({
                ...userInfo,
                details: { ...userInfo?.details, email: newEmail }
            })
        );

        // After email change: old email not searchable, new email finds the user
        const searchOld = await api.user.searchIdentities(adminUser.sid, { email });
        expect(searchOld.identities.some((i) => i.id === user.userId)).toBe(false);

        const searchNew = await api.user.searchIdentities(adminUser.sid, { email: newEmail });
        expect(searchNew.identities).toHaveLength(1);
        expect(searchNew.identities[0].id).toBe(user.userId);
    });

    test('Sequential email changes shall invalidate previous tokens (unique constraint)', async ({ api }) => {
        // Tests: change A→B, then B→C; token for B must be invalid
        const smtp = await startMockEmail();

        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });

        const mailPromise = smtp.waitMail();
        const newEmail = `updated-${randomUUID()}@example.com`;
        expect(user.email).not.toEqual(newEmail);
        await api.user.startChangeEmail(user.sid, newEmail);
        const mail = await mailPromise;
        const token = getEmailLinkToken(mail);
        expect(token).toBeString();

        const newEmail2 = `updated-${randomUUID()}@example.com`;
        await user.changeEmail(smtp, newEmail2);
        const { sessionLength, remainingSessionTime, ...userInfo } = user.userInfo!;

        // Old token is automatically deleted when second email change creates new token (unique constraint)
        const response = await api.user.completeConfirmEmailRequest(user.sid, token!);
        expect(response).toHaveStatus(400);
        expect(await response.parseProblem()).toEqual(
            expect.objectContaining({
                type: 'email-token-expired',
                status: 400,
                sensitive: 'tokenExpired'
            })
        );

        expect(await api.user.getUserInfo(user.sid, 'fast')).toEqual(
            expect.objectContaining({
                ...userInfo,
                details: null
            })
        );
        expect(await api.user.getUserInfo(user.sid, 'full')).toEqual(
            expect.objectContaining({
                ...userInfo
            })
        );
    });

    for (const confirmed of [true, false]) {
        test(`Changing email to an used ${confirmed ? 'confirmed' : 'unconfirmed'} email shall fail`, async ({
            api
        }) => {
            const smtp = await startMockEmail();

            const email = randomUUID() + '@example.com';
            const emailOwnerUser = await api.testUsers.createLinked(mockAuth, { email });
            if (confirmed) {
                await emailOwnerUser.confirmEmail(smtp);
            }

            const user = await api.testUsers.createLinked(mockAuth);
            const { sessionLength, remainingSessionTime, ...userInfo } = user.userInfo!;

            const mailPromise = smtp.waitMail();
            expect(user.email).not.toEqual(email);
            await api.user.startChangeEmail(user.sid, email);
            const mail = await mailPromise;
            const token = getEmailLinkToken(mail);
            expect(token).toBeString();

            const response = await api.user.completeConfirmEmailRequest(user.sid, token!);
            expect(response).toHaveStatus(412);
            expect(await response.parseProblem()).toEqual(
                expect.objectContaining({
                    type: 'email-conflict',
                    status: 412
                })
            );

            expect(await api.user.getUserInfo(user.sid, 'fast')).toEqual(
                expect.objectContaining({
                    ...userInfo,
                    details: null
                })
            );
            expect(await api.user.getUserInfo(user.sid, 'full')).toEqual(
                expect.objectContaining({
                    ...userInfo
                })
            );
        });
    }

    test('Delete user with pending email change shall prevent completing email change', async ({ api }) => {
        const smtp = await startMockEmail();
        const user = await api.testUsers.createLinked(mockAuth);

        const newEmail = `changed-${randomUUID()}@example.com`;
        const mailPromise = smtp.waitMail();
        await api.user.startChangeEmail(user.sid, newEmail);
        const mail = await mailPromise;
        const token = getEmailLinkToken(mail);
        expect(token).toBeString();

        await api.auth.deleteUserRequest(user.sid, user.name);

        // After deletion the session is revoked — completing the change is no longer possible
        const response = await api.user.completeConfirmEmailRequest(user.sid, token!);
        expect(response).toHaveStatus(401);
    });
});
