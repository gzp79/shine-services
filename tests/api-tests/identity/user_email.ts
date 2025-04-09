import { expect, test } from '$fixtures/setup';
import { ProblemSchema } from '$lib/api/api';
import { getEmailLink, getEmailLinkToken } from '$lib/api/utils';
import MockSmtp from '$lib/mocks/mock_smtp';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { randomUUID } from 'crypto';

test.describe('Email confirmation', () => {
    let mockAuth: OAuth2MockServer = undefined!;
    let mockEmail: MockSmtp = undefined!;

    const startMockEmail = async (): Promise<MockSmtp> => {
        if (!mockEmail) {
            mockEmail = new MockSmtp();
            await mockEmail.start();
        }
        return mockEmail as MockSmtp;
    };

    test.beforeEach(async () => {
        mockAuth = new OAuth2MockServer();
        await mockAuth.start();
    });

    test.afterEach(async () => {
        await mockAuth?.stop();
        mockAuth = undefined!;
        await mockEmail?.stop();
        mockEmail = undefined!;
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
                type: 'email-invalid-token',
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

    test('Confirming email with another guest user shall fail', async ({ api }) => {
        const user = await api.testUsers.createLinked(mockAuth);

        const smtp = await startMockEmail();
        const mailPromise = smtp.waitMail();
        await api.user.startConfirmEmail(user.sid);
        const mail = await mailPromise;

        const token = getEmailLinkToken(mail);

        const user2 = await api.testUsers.createGuest();
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

    test('Confirming email with another linked user shall fail', async ({ api }) => {
        const user = await api.testUsers.createLinked(mockAuth);

        const smtp = await startMockEmail();
        const mailPromise = smtp.waitMail();
        await api.user.startConfirmEmail(user.sid);
        const mail = await mailPromise;

        const token = getEmailLinkToken(mail);

        const user2 = await api.testUsers.createLinked(mockAuth);
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

        expect(await api.user.getUserInfo(user.sid, 'fast')).toEqual(
            expect.objectContaining({ isEmailConfirmed: false, details: null })
        );
        expect(await api.user.getUserInfo(user.sid, 'full')).toEqual(
            expect.objectContaining({ isEmailConfirmed: false, details: user.userInfo!.details })
        );

        expect(await api.user.getUserInfo(user2.sid, 'fast')).toEqual(
            expect.objectContaining({ isEmailConfirmed: false, details: null })
        );
        expect(await api.user.getUserInfo(user2.sid, 'full')).toEqual(
            expect.objectContaining({ isEmailConfirmed: false, details: user2.userInfo!.details })
        );
    });

    test('Confirming a changed email shall fail', async ({ api }) => {
        const smtp = await startMockEmail();

        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });

        const mailPromise = smtp.waitMail();
        await api.user.startConfirmEmail(user.sid);
        const mail = await mailPromise;
        const token = getEmailLinkToken(mail);
        expect(token).toBeString();

        const newEmail = `updated-${randomUUID()}@example.com`;
        await user.changeEmail(smtp, newEmail);
        const { sessionLength, remainingSessionTime, ...userInfo } = user.userInfo!;

        const response = await api.user.completeConfirmEmailRequest(user.sid, token!);
        expect(response).toHaveStatus(400);
        expect(await response.parseProblem()).toEqual(
            expect.objectContaining({
                type: 'email-token-expired',
                status: 400,
                sensitive: 'wrongEmail'
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
});

test.describe('Email change', () => {
    let mockAuth: OAuth2MockServer = undefined!;
    let mockEmail: MockSmtp = undefined!;

    const startMockEmail = async (): Promise<MockSmtp> => {
        if (!mockEmail) {
            mockEmail = new MockSmtp();
            await mockEmail.start();
        }
        return mockEmail as MockSmtp;
    };

    test.beforeEach(async () => {
        mockAuth = new OAuth2MockServer();
        await mockAuth.start();
    });

    test.afterEach(async () => {
        await mockAuth?.stop();
        mockAuth = undefined!;
        await mockEmail?.stop();
        mockEmail = undefined!;
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

    for (const lang of ['en', 'hu', undefined]) {
        test(`Changing email with unconfirmed email in lang ${lang} shall succeed`, async ({ api }) => {
            const email = randomUUID() + '@example.com';
            const user = await api.testUsers.createLinked(mockAuth, { email });
            const { sessionLength, remainingSessionTime, ...userInfo } = user.userInfo!;

            const smtp = await startMockEmail();
            const mailPromise = smtp.waitMail();
            const newEmail = `updated-${randomUUID()}@example.com`;
            expect(userInfo.email).not.toEqual(newEmail);
            await api.user.startChangeEmail(user.sid, newEmail, lang);
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
        });
    }

    test('Changing email without email shall succeed', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const { sessionLength, remainingSessionTime, ...userInfo } = user.userInfo!;
        expect(userInfo.email).not.toBeNull();

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
    });

    test('Changing email with confirmed email shall succeed', async ({ api }) => {
        const smtp = await startMockEmail();

        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });
        await user.confirmEmail(smtp);
        const { sessionLength, remainingSessionTime, ...userInfo } = user.userInfo!;

        const mailPromise = smtp.waitMail();
        const newEmail = `updated-${randomUUID()}@example.com`;
        expect(userInfo.email).not.toEqual(newEmail);
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
    });

    test('Changing email with and already changed email shall fail', async ({ api }) => {
        const smtp = await startMockEmail();

        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });

        const mailPromise = smtp.waitMail();
        const newEmail = `updated-${randomUUID()}@example.com`;
        expect(user.userInfo?.email).not.toEqual(newEmail);
        await api.user.startChangeEmail(user.sid, newEmail);
        const mail = await mailPromise;
        const token = getEmailLinkToken(mail);
        expect(token).toBeString();

        const newEmail2 = `updated-${randomUUID()}@example.com`;
        await user.changeEmail(smtp, newEmail2);
        const { sessionLength, remainingSessionTime, ...userInfo } = user.userInfo!;

        const response = await api.user.completeConfirmEmailRequest(user.sid, token!);
        expect(response).toHaveStatus(400);
        expect(await response.parseProblem()).toEqual(
            expect.objectContaining({
                type: 'email-token-expired',
                status: 400,
                sensitive: 'wrongEmail'
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
            expect(userInfo.email).not.toEqual(email);
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
});
