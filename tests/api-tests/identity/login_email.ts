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
import MockSmtp from '$lib/mocks/mock_smtp';
import { randomUUID } from 'crypto';
import { ParsedMail } from 'mailparser';

test.describe('Login with email', () => {
    let mock: MockSmtp | undefined;

    const startMock = async (check: (mail: ParsedMail) => void): Promise<MockSmtp> => {
        if (!mock) {
            mock = new MockSmtp();
            await mock.start(check);
            console.log('Mock started');
        }
        return mock as MockSmtp;
    };

    test.afterEach(async () => {
        await mock?.stop();
        mock = undefined;
    });

    test('Login with new email should create user and send email', async ({ api }) => {
        const targetEmailAddress = `${randomUUID()}@example.com`;
        const _mock = await startMock((mail) => {
            expect(mail).toHaveSingleTo(targetEmailAddress);
            expect(mail.text).toContain('Please confirm your email address');
        });

        const response = await api.auth
            .loginWithEmailRequest(targetEmailAddress, null, null, null, null, null, null)
            .send();
        expect(response).toHaveStatus(200);
    });
});
