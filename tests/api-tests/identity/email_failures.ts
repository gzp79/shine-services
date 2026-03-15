import { expect, test } from '$fixtures/setup';
import { getEmailRecipientsText, getPageProblem } from '$lib/api/utils';
import MockSmtp from '$lib/mocks/mock_smtp';
import { randomUUID } from 'crypto';

test.describe('Email (SMTP) failure tests', { tag: '@infrastructure' }, () => {
    let mockSmtp: MockSmtp | undefined;

    test.afterEach(async () => {
        await mockSmtp?.stop();
        mockSmtp = undefined;
    });

    test('SMTP connection timeout shall return user-friendly error', async ({ api }) => {
        // Don't start MockSMTP - connection will fail
        const email = `${randomUUID()}@example.com`;

        const response = await api.auth.loginWithEmailRequest(email, false, null);
        expect(response).toHaveStatus(200);

        const text = await response.text();
        const problem = getPageProblem(text);

        // Should return error indicating email send failed
        // Service might return 500 or specific email error
        expect(problem).toBeTruthy();
        expect(['auth-internal-error', 'email-send-failed']).toContain(problem?.type);
    });

    test('SMTP rejection shall not prevent user registration', async ({ api }) => {
        mockSmtp = new MockSmtp();

        // Configure mock to reject emails
        mockSmtp.onMail(() => {
            throw new Error('SMTP rejection');
        });
        await mockSmtp.start();

        const email = `${randomUUID()}@example.com`;

        const response = await api.auth.loginWithEmailRequest(email, false, null);

        // Page route always returns 200, check content for error
        expect(response).toHaveStatus(200);
        const text = await response.text();
        const problem = getPageProblem(text);

        // SMTP rejection should result in an error page
        expect(problem).toBeTruthy();
        expect(['auth-internal-error', 'email-send-failed']).toContain(problem?.type);
    });

    test('Partial SMTP failure shall allow retry', async ({ api }) => {
        let attemptCount = 0;
        mockSmtp = new MockSmtp();

        // Fail first attempt, succeed second
        mockSmtp.onMail(() => {
            attemptCount++;
            if (attemptCount === 1) {
                throw new Error('Temporary failure');
            }
            // Second attempt succeeds
        });
        await mockSmtp.start();

        const email = `${randomUUID()}@example.com`;

        // First attempt - should fail
        const response1 = await api.auth.loginWithEmailRequest(email, false, null);
        expect(response1).toHaveStatus(200);
        const text1 = await response1.text();
        const problem1 = getPageProblem(text1);
        expect(problem1).toBeTruthy(); // First attempt should error

        // Retry - should succeed
        const mailPromise = mockSmtp.waitMail();
        const response2 = await api.auth.loginWithEmailRequest(email, false, null);
        expect(response2).toHaveStatus(200);

        // Wait for email to be received
        const mail = await mailPromise;
        expect(mail).toBeDefined();
        expect(getEmailRecipientsText(mail)).toContain(email);
    });

    test('SMTP connection during high load shall not block service', async ({ api }) => {
        mockSmtp = new MockSmtp();

        // Add artificial delay to email sending
        mockSmtp.onMail(async () => {
            await new Promise((resolve) => setTimeout(resolve, 2000));
        });
        await mockSmtp.start();

        // Send multiple emails concurrently
        const emails = Array.from({ length: 5 }, () => `${randomUUID()}@example.com`);
        const start = Date.now();

        const responses = await Promise.all(emails.map((email) => api.auth.loginWithEmailRequest(email, false, null)));

        const duration = Date.now() - start;

        // 5 requests with 2s email delay each: if sequential would take ~10s
        // Async processing should complete well under that
        expect(duration).toBeLessThan(6000);

        // All should get responses (200 = success page, 500 = internal error)
        responses.forEach((r) => {
            expect([200, 500]).toContain(r.status());
        });
    });

    test('Email queue resilience during failures', async ({ api }) => {
        let emailsReceived = 0;

        mockSmtp = new MockSmtp();
        mockSmtp.onMail(() => {
            emailsReceived++;
        });
        await mockSmtp.start();

        // Send email while SMTP is up
        await api.auth.loginWithEmailRequest(`${randomUUID()}@example.com`, false, null);
        const receivedBeforeStop = emailsReceived;

        // Stop SMTP temporarily — next request should fail
        await mockSmtp.stop();
        mockSmtp = undefined;
        await api.auth.loginWithEmailRequest(`${randomUUID()}@example.com`, false, null);

        // Restart SMTP — new requests should work again
        mockSmtp = new MockSmtp();
        mockSmtp.onMail(() => {
            emailsReceived++;
        });
        await mockSmtp.start();

        const mailPromise = mockSmtp.waitMail();
        await api.auth.loginWithEmailRequest(`${randomUUID()}@example.com`, false, null);
        await mailPromise;

        // First email was received, and recovery email was received
        expect(receivedBeforeStop).toBeGreaterThanOrEqual(1);
        expect(emailsReceived).toBeGreaterThanOrEqual(2);
    });
});
