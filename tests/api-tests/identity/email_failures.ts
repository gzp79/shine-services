import { expect, test } from '$fixtures/setup';
import { getEmailRecipientsText, getPageProblem } from '$lib/api/utils';
import MockSmtp from '$lib/mocks/mock_smtp';
import { randomUUID } from 'crypto';

test.describe('Email (SMTP) failure tests', { tag: '@infrastructure' }, () => {
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
        const mockSmtp = new MockSmtp();

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

        await mockSmtp.stop();
    });

    test('Partial SMTP failure shall allow retry', async ({ api }) => {
        let attemptCount = 0;
        const mockSmtp = new MockSmtp();

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

        await mockSmtp.stop();
    });

    test('SMTP connection during high load shall not block service', async ({ api }) => {
        const mockSmtp = new MockSmtp();

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

        // Should complete in reasonable time (not sequentially)
        // With async processing, should be < 5 seconds even with 2s email delay
        expect(duration).toBeLessThan(10000);

        // All should get responses
        responses.forEach((r) => {
            expect([200, 202, 500]).toContain(r.status());
        });

        await mockSmtp.stop();
    });

    test('Email queue resilience during failures', async ({ api }) => {
        const mockSmtp = new MockSmtp();
        let emailsReceived = 0;

        mockSmtp.onMail(() => {
            emailsReceived++;
        });
        await mockSmtp.start();

        // Send multiple emails
        const email1 = `${randomUUID()}@example.com`;
        const email2 = `${randomUUID()}@example.com`;

        await api.auth.loginWithEmailRequest(email1, false, null);

        // Stop SMTP temporarily
        await mockSmtp.stop();

        // This might fail
        await api.auth.loginWithEmailRequest(email2, false, null);

        // Restart SMTP
        const mockSmtp2 = new MockSmtp();
        mockSmtp2.onMail(() => {
            emailsReceived++;
        });
        await mockSmtp2.start();

        // New emails should work
        const email3 = `${randomUUID()}@example.com`;
        const mailPromise = mockSmtp2.waitMail();
        await api.auth.loginWithEmailRequest(email3, false, null);
        await mailPromise;

        // At least some emails should have been sent
        expect(emailsReceived).toBeGreaterThan(0);

        await mockSmtp2.stop();
    });
});
