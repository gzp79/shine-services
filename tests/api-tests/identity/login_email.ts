/*
todo: 
 login_email:
   - new email, user created, email received, confirm (verify email)
   - with confirmed email, should send new login to same email. (verify email)
   - without confirmed email, should send new login to same email, should confirm (verify email)

- email verify:
  - user without email, should fail
  - user with confirmed email, should fail (no op)
  - user without confirmed email, email received, confirm

- email change/delete:
  - user without email, change email, email received, accept
  - user with confirmed email, change email, 2 email received, accept, 1 email received

  - user without confirmed email, change email, 1 email received, accept  - don't send change requests to unconfirmed email
  - user without email, delete email, change
  - user with confirmed email, delete email
  - user without confirmed email, delete email

  - new user, change email to A, change email to B, accept A, B should be rejected
  - new user, change email to A, delete email, accept A, delete should be rejected
  - new user, delete email, change email to A, delete accepted, A should be rejected

  - new user, change email, revoke token, change should be rejected
*/
import { expect, test } from '$fixtures/setup';
import { getEmailLink } from '$lib/api/utils';
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
