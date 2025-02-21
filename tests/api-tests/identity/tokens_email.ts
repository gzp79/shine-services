import { expect, test } from '$fixtures/setup';
import { ApiRequest, ProblemSchema } from '$lib/api/api';
import { getEmailLink, getPageProblem, getPageRedirectUrl } from '$lib/api/utils';
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

    test.beforeEach(async ({ api }) => {
        mockAuth = new OAuth2MockServer();
        await mockAuth.start();
    });

    test.afterEach(async () => {
        await mockAuth?.stop();
        mockAuth = undefined!;
        await mockEmail?.stop();
        mockEmail = undefined!;
    });

    test(`Creating emailVerify with api shall be rejected`, async ({ api }) => {
        const user = await api.testUsers.createGuest();

        const response = await api.token.createTokenRequest(user.sid, 'emailVerify', 20, false);
        expect(response).toHaveStatus(400);

        const error = await response.parse(ProblemSchema);
        expect(error).toEqual(
            expect.objectContaining({
                type: 'input-body-format',
                status: 400,
                detail: expect.stringContaining(`kind: unknown variant \`emailVerify\``)
            })
        );
    });

    test(`Requesting email confirmation without session shall fail`, async ({ api }) => {
        const response = await api.user.confirmEmailRequest(null);
        expect(response).toHaveStatus(401);
    });

    test(`Requesting email confirmation without email shall fail`, async ({ api }) => {
        const smtp = await startMockEmail();
        smtp.onMail((_mail) => {
            expect(true, 'No message should arrive').toBe(false);
        });

        const user = await api.testUsers.createGuest();
        const response = await api.user.confirmEmailRequest(user.sid);
        expect(response).toHaveStatus(412);
        const error = await response.parse(ProblemSchema);
        expect(error).toEqual(
            expect.objectContaining({
                type: 'identity-missing-email',
                status: 412,
                detail: expect.stringContaining('User has no valid email address')
            })
        );
    });

    test(`Requesting email confirmation with email address shall succeed`, async ({ identityUrl, api }) => {
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });

        const smtp = await startMockEmail();
        const mailPromise = smtp.waitMail();
        await api.user.confirmEmail(user.sid);
        const mail = await mailPromise;
        expect(mail).toHaveMailTo(email);
        expect(mail).toContainMailBody(`<p>Hello ${user.name},</p>`);
        expect(mail).toContainMailBody('Confirm Email Address');
        expect(mail).toContainMailBody(identityUrl + '/auth/token/login?token=');
        expect(mail).toContainMailBody('<p>Best regards,<br/>Scytta</p>');
    });

    test(`Requesting email confirmation with 3rd party error shall fail`, async ({ api }) => {
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });
        const response = await api.user.confirmEmailRequest(user.sid);

        expect(response).toHaveStatus(500);
    });

    test(`Login with requested email confirmation shall work`, async ({ homeUrl, identityUrl, api }) => {
        const smtp = await startMockEmail();
        const user = await api.testUsers.createLinked(mockAuth);
        const { sessionLength, ...userInfo } = await api.user.getUserInfo(user.sid);

        const mailPromise = smtp.waitMail();
        await api.user.confirmEmail(user.sid);
        const mail = await mailPromise;

        const url = getEmailLink(mail);
        expect(url).toStartWith(identityUrl + '/auth/token/login');
        const response = await ApiRequest.get(url);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(`${homeUrl}/`);
        expect(getPageProblem(text)).toBeNull();

        const newCookies = response.cookies();
        expect(newCookies.tid).toBeClearCookie();
        expect(newCookies.sid).toBeValidSID();
        expect(newCookies.eid).toBeClearCookie();
        expect(await api.user.getUserInfo(newCookies.sid.value)).toEqual(
            expect.objectContaining({ ...userInfo, isEmailConfirmed: true })
        );

        expect(await api.token.getTokens(user.sid)).toEqual([]);
    });

    test(`Login with requested email confirmation and corrupted email hash shall fail`, async ({ homeUrl, api }) => {
        const smtp = await startMockEmail();
        const user = await api.testUsers.createLinked(mockAuth);
        const { sessionLength, ...userInfo } = await api.user.getUserInfo(user.sid);

        const mailPromise = smtp.waitMail();
        await api.user.confirmEmail(user.sid);
        const mail = await mailPromise;

        const url = getEmailLink(mail).replace(/emailHash=[^&]+/, 'emailHash=corrupted');
        const response = await ApiRequest.get(url);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(`${homeUrl}/error?type=auth-token-expired&status=401`);
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-token-expired',
                status: 401,
                extension: null,
                sensitive: 'emailConflict'
            })
        );

        const newCookies = response.cookies();
        expect(newCookies.tid).toBeClearCookie();
        expect(newCookies.sid).toBeClearCookie();
        expect(newCookies.eid).toBeClearCookie();

        expect(await api.token.getTokens(user.sid)).toEqual([]);
    });

    test(`Login with revoked requested email token shall fail`, async ({ homeUrl, api }) => {
        const smtp = await startMockEmail();
        const user = await api.testUsers.createLinked(mockAuth);

        const mailPromise = smtp.waitMail();
        await api.user.confirmEmail(user.sid);
        const mail = await mailPromise;
        const url = getEmailLink(mail);

        await test.step('Revoke token', async () => {
            const tokens = await api.token.getTokens(user.sid);
            expect(tokens).toEqual([expect.objectContaining({ kind: 'emailVerify', isExpired: false })]);
            api.token.revokeToken(user.sid, tokens[0].tokenHash);
            expect(await api.token.getTokens(user.sid)).toEqual([]);
        });

        const response = await ApiRequest.get(url);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(`${homeUrl}/error?type=auth-token-expired&status=401`);
        expect(getPageProblem(text)).toEqual(
            expect.objectContaining({
                type: 'auth-token-expired',
                status: 401,
                extension: null,
                sensitive: 'expiredToken'
            })
        );

        const newCookies = response.cookies();
        expect(newCookies.tid).toBeClearCookie();
        expect(newCookies.sid).toBeClearCookie();
        expect(newCookies.eid).toBeClearCookie();

        expect(await api.token.getTokens(user.sid)).toEqual([]);
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

    test.beforeEach(async ({ api }) => {
        mockAuth = new OAuth2MockServer();
        await mockAuth.start();
    });

    test.afterEach(async () => {
        await mockAuth?.stop();
        mockAuth = undefined!;
        await mockEmail?.stop();
        mockEmail = undefined!;
    });

    test(`Creating emailChange with api shall be rejected`, async ({ api }) => {
        const user = await api.testUsers.createGuest();

        const response = await api.token.createTokenRequest(user.sid, 'emailChange', 20, false);
        expect(response).toHaveStatus(400);

        const error = await response.parse(ProblemSchema);
        expect(error).toEqual(
            expect.objectContaining({
                type: 'input-body-format',
                status: 400,
                detail: expect.stringContaining(`kind: unknown variant \`emailChange\``)
            })
        );
    });

    test(`Changing email without session shall fail`, async ({ api }) => {
        throw new Error('Not implemented');
    });

    test(`Changing email with 3rd party error shall fail`, async ({ api }) => {
        throw new Error('Not implemented');
    });

    test(`Changing email without email shall succeed`, async ({ api }) => {
        throw new Error('Not implemented');
    });

    test(`Changing email with unconfirmed email shall succeed`, async ({ api }) => {
        throw new Error('Not implemented');
    });

    test(`Changing email with confirmed email shall succeed`, async ({ api }) => {
        throw new Error('Not implemented');
    });

    test(`Changing email with corrupted email shall fail`, async ({ api }) => {
        throw new Error('Not implemented');
        //expect(await api.token.getTokens(user.sid)).toEqual([]);
    });

    test(`Changing email with revoked token shall fail`, async ({ api }) => {
        throw new Error('Not implemented');
        //expect(await api.token.getTokens(user.sid)).toEqual([]);
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

    test.beforeEach(async ({ api }) => {
        mockAuth = new OAuth2MockServer();
        await mockAuth.start();
    });

    test.afterEach(async () => {
        await mockAuth?.stop();
        mockAuth = undefined!;
        await mockEmail?.stop();
        mockEmail = undefined!;
    });

    test(`Deleting email without session shall fail`, async ({ api }) => {
        throw new Error('Not implemented');
    });

    test(`Deleting email with 3rd party error shall fail`, async ({ api }) => {
        throw new Error('Not implemented');
    });

    test(`Deleting email without email shall succeed`, async ({ api }) => {
        throw new Error('Not implemented');
    });

    test(`Deleting email with unconfirmed email shall succeed`, async ({ api }) => {
        throw new Error('Not implemented');
    });

    test(`Deleting email with confirmed email shall succeed`, async ({ api }) => {
        throw new Error('Not implemented');
    });

    test(`Deleting email with corrupted email shall fail`, async ({ api }) => {
        throw new Error('Not implemented');
        //expect(await api.token.getTokens(user.sid)).toEqual([]);
    });

    test(`Deleting email with revoked token shall fail`, async ({ api }) => {
        throw new Error('Not implemented');
        //expect(await api.token.getTokens(user.sid)).toEqual([]);
    });
});
