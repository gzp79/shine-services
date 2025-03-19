import { expect, test } from '$fixtures/setup';
import { ProblemSchema } from '$lib/api/api';
import MockSmtp from '$lib/mocks/mock_smtp';
import { randomUUID } from 'crypto';

test.describe('Login with email', () => {
    let mock: MockSmtp;

    test.beforeAll(async () => {
        mock = new MockSmtp();
        await mock.start();
        console.log('Mock started');
    });

    test.afterAll(async () => {
        await mock.stop();
        mock = undefined!;
    });

    test(`Creating emailAccess with api shall be rejected`, async ({ api }) => {
        const user = await api.testUsers.createGuest();

        const response = await api.token.createTokenRequest(user.sid, 'emailAccess', 20, false);
        expect(response).toHaveStatus(400);

        const error = await response.parse(ProblemSchema);
        expect(error).toEqual(
            expect.objectContaining({
                type: 'input-body-format',
                status: 400,
                detail: expect.stringContaining(
                    `kind: unknown variant \`emailAccess\`, expected \`persistent\` or \`singleAccess\` at line 1`
                )
            })
        );
    });

    test.skip('Login with new email should create user and send email', async ({ api, appDomain }) => {
        const targetEmailAddress = `${randomUUID()}@example.com`;

        const mailPromise = mock.waitMail();
        const response = await api.auth.loginWithEmailRequest(targetEmailAddress, null, null, null, null, null, null);
        expect(response).toHaveStatus(200);

        const mail = await mailPromise;
        console.log('Mail received', mail);
        expect(mail).toHaveMailTo(targetEmailAddress);
        expect(mail).toHaveMailFrom(`no-replay@${appDomain}`);
        //expect(mail.text).toContain('Please confirm your email address');
        //let url = getEmailLink(mail);
        //expect(url).toStartWith);
    });
});
