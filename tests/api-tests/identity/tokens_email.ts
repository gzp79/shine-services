import { expect, test } from '$fixtures/setup';
import { ProblemSchema } from '$lib/api/api';
import { TestUser } from '$lib/api/test_user';
import { TokenKind } from '$lib/api/token_api';
import MockSmtp from '$lib/mocks/mock_smtp';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { randomUUID } from 'crypto';
import { ParsedMail } from 'mailparser';

test.describe('Email tokens', () => {
    let mockAuth: OAuth2MockServer = undefined!;
    let mockEmail: MockSmtp = undefined!;
    let user: TestUser = undefined!;

    const startMockEmail = async (check: (mail: ParsedMail) => void): Promise<MockSmtp> => {
        if (!mockEmail) {
            mockEmail = new MockSmtp();
            await mockEmail.start(check);
        }
        return mockEmail as MockSmtp;
    };

    test.beforeEach(async ({ api }) => {
        mockAuth = new OAuth2MockServer();
        await mockAuth.start();
        user = await api.testUsers.createLinked(mockAuth);
    });

    test.afterEach(async () => {
        await mockAuth?.stop();
        mockAuth = undefined!;
        await mockEmail?.stop();
        mockEmail = undefined!;
        user = undefined!;
    });

    for (const tokenKind of ['emailVerify', 'emailChange']) {
        test(`Creating ${tokenKind} with api shall be rejected`, async ({ api }) => {
            const user = await api.testUsers.createGuest();

            const response = await api.token.createTokenRequest(user.sid, tokenKind as TokenKind, 20, false).send();
            expect(response).toHaveStatus(400);

            const error = await response.parse(ProblemSchema);
            expect(error).toEqual(
                expect.objectContaining({
                    type: 'body_format_error',
                    status: 400,
                    detail: expect.stringContaining(`kind: unknown variant \`${tokenKind}\``)
                })
            );
        });
    }

    test(`Requesting email confirmation without session shall fail`, async ({ api }) => {
        const response = await api.user.confirmEmailRequest(user.sid).send();
        expect(response).toHaveStatus(401);
    });

    test(`Requesting email confirmation without email address shall fail`, async ({ api }) => {
        const smtp = await startMockEmail((mail) => {
            expect(true, 'No email shall be sent').toBe(false);
        });

        const user = await api.testUsers.createGuest();
        await api.user.confirmEmailRequest(user.sid);
    });

    test(`Requesting email with email address shall succeed`, async ({ api }) => {
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });

        await startMockEmail((mail) => {
            expect(mail).toHaveSingleTo(email);
            //expect(mail).ToHaveFrom()
        });
        await api.user.confirmEmail(user.sid);
    });

    test(`Requesting email with 3rd party error shall fail`, async ({ api }) => {
        const email = randomUUID() + '@example.com';
        const user = await api.testUsers.createLinked(mockAuth, { email });
        const response = await api.user.confirmEmailRequest(user.sid).send();

        expect(response).toHaveStatus(500);
    });
});
