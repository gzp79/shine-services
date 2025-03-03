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

    test(`Requesting email confirmation without session shall fail`, async ({ api }) => {
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

    test(`Requesting email confirmation without email shall fail`, async ({ api }) => {
        const smtp = await startMockEmail();
        smtp.onMail((_mail) => {
            expect(true, 'No message should arrive').toBe(false);
        });

        const user = await api.testUsers.createGuest();
        const response = await api.user.startConfirmEmailRequest(user.sid);
        expect(response).toHaveStatus(412);
        expect(await response.parseProblem()).toEqual(
            expect.objectContaining({
                type: 'identity-missing-email',
                status: 412,
                detail: expect.stringContaining('User has no valid email address')
            })
        );
    });

    test(`Requesting email confirmation with email address shall succeed`, async ({ linkUrl, identityUrl, api }) => {
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

        const authUrl = getEmailLink(mail);
        expect(authUrl).toStartWith(identityUrl);
        const authParams = new URL(authUrl).searchParams;
        const confirmUrl = authParams.get('redirectUrl') ?? '';

        expect(confirmUrl).toStartWith(linkUrl);
        const confirmParams = new URL(confirmUrl).searchParams;
        expect(confirmParams.get('token')).toBeString();
    });

    test(`Requesting email confirmation with 3rd party error shall fail`, async ({ api }) => {
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });
        const response = await api.user.startConfirmEmailRequest(user.sid);
        expect(response).toHaveStatus(500);
    });

    test(`Completing email confirmation with invalid token shall fail`, async ({ api }) => {
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });
        const response = await api.user.completeConfirmEmailRequest(user.sid, 'invalid');
        expect(response).toHaveStatus(400);

        const error = await response.parse(ProblemSchema);
        expect(error).toEqual(
            expect.objectContaining({
                type: 'auth-invalid-token',
                status: 400
            })
        );
    });

    test(`Confirm email with same user shall work`, async ({ api }) => {
        const smtp = await startMockEmail();
        const user = await api.testUsers.createLinked(mockAuth);
        const { sessionLength, ...userInfo } = await api.user.getUserInfo(user.sid);

        const mailPromise = smtp.waitMail();
        await api.user.startConfirmEmail(user.sid);
        const mail = await mailPromise;

        const token = getEmailLinkToken(mail);
        expect(token).toBeString();
        await api.user.completeConfirmEmail(user.sid, token!);

        expect(await api.user.getUserInfo(user.sid)).toEqual(
            expect.objectContaining({
                ...userInfo,
                isEmailConfirmed: true
            })
        );
    });

    test(`Confirm email with another guest user shall fail`, async ({ api }) => {
        const smtp = await startMockEmail();
        const user = await api.testUsers.createLinked(mockAuth);

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
                type: 'auth-token-expired',
                status: 400,
                sensitive: 'tokenExpired' // for the missing email
            })
        );

        expect(await api.user.getUserInfo(user.sid)).toEqual(
            expect.objectContaining({ email: user.userInfo?.email, isEmailConfirmed: false })
        );
        expect(await api.user.getUserInfo(user2.sid)).toEqual(expect.objectContaining({ isEmailConfirmed: false }));
    });

    test(`Confirm email with another linked user shall fail`, async ({ api }) => {
        const smtp = await startMockEmail();
        const user = await api.testUsers.createLinked(mockAuth);

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
                type: 'auth-token-expired',
                status: 400,
                sensitive: 'tokenMissMatch' // for the non matching data
            })
        );

        expect(await api.user.getUserInfo(user.sid)).toEqual(
            expect.objectContaining({ email: user.userInfo?.email, isEmailConfirmed: false })
        );
        expect(await api.user.getUserInfo(user2.sid)).toEqual(
            expect.objectContaining({ email: user2.userInfo?.email, isEmailConfirmed: false })
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

    test.skip(`Changing email without session shall fail`, async () => {
        const smtp = await startMockEmail();
        smtp.onMail((_mail) => {
            expect(true, 'No message should arrive').toBe(false);
        });

        throw new Error('Not implemented');
    });

    test.skip(`Changing email with 3rd party error shall fail`, async () => {
        throw new Error('Not implemented');
    });

    test.skip(`Changing email without email shall succeed`, async () => {
        throw new Error('Not implemented');
    });

    test.skip(`Changing email with unconfirmed email shall succeed`, async () => {
        throw new Error('Not implemented');
    });

    test.skip(`Changing email with confirmed email shall succeed`, async () => {
        throw new Error('Not implemented');
    });
});

test.describe('Email delete', () => {
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

    test.skip(`Deleting email without session shall fail`, async () => {
        const smtp = await startMockEmail();
        smtp.onMail((_mail) => {
            expect(true, 'No message should arrive').toBe(false);
        });

        throw new Error('Not implemented');
    });

    test.skip(`Deleting email with 3rd party error shall fail`, async () => {
        throw new Error('Not implemented');
    });

    test.skip(`Deleting email without email shall succeed`, async () => {
        throw new Error('Not implemented');
    });

    test.skip(`Deleting email with unconfirmed email shall succeed`, async () => {
        throw new Error('Not implemented');
    });

    test.skip(`Deleting email with confirmed email shall succeed`, async () => {
        throw new Error('Not implemented');
    });
});
