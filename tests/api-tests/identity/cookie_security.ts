import { expect, test } from '$fixtures/setup';
import { getPageRedirectUrl } from '$lib/api/utils';

test.describe('Cookie security attributes', { tag: '@security' }, () => {
    test('Session cookies shall have HttpOnly flag', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        expect(response).toHaveStatus(200);
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();
        expect(cookies.sid.httpOnly).toBe(true);
        if (cookies.tid.value) {
            expect(cookies.tid.httpOnly).toBe(true);
        }
    });

    test('Session cookies shall have Secure flag in production', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        expect(response).toHaveStatus(200);
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();

        // In test environment, Secure might not be set
        // In production (HTTPS), should be true
        const isTestEnv = process.env.ENVIRONMENT === 'test';
        if (!isTestEnv) {
            expect(cookies.sid.secure).toBe(true);
            if (cookies.tid.value) {
                expect(cookies.tid.secure).toBe(true);
            }
        }
    });

    test('Session cookies shall have SameSite attribute', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        expect(response).toHaveStatus(200);
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();

        // Should be Lax or Strict for CSRF protection
        expect(['Lax', 'Strict', 'lax', 'strict']).toContain(cookies.sid.sameSite);
    });

    test('Session cookies shall have correct Domain attribute', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        expect(response).toHaveStatus(200);
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();

        // Domain should match service domain or be undefined (defaults to origin)
        if (cookies.sid.domain) {
            expect(cookies.sid.domain).toContain('scytta.com');
        }
    });

    test('Session cookies shall have correct Path attribute', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        expect(response).toHaveStatus(200);
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();

        // Path should be / or /identity
        expect(['/', '/identity']).toContain(cookies.sid.path);
    });

    test('Token cookie Max-Age shall match configured TTL for rememberMe', async ({ api }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        expect(response).toHaveStatus(200);
        const text = await response.text();
        expect(getPageRedirectUrl(text)).toEqual(api.auth.defaultRedirects.redirectUrl);

        const cookies = response.cookies();

        if (cookies.tid?.value) {
            // RememberMe token should have long TTL (14 days = 1209600 seconds)
            // Calculate from either Max-Age or Expires header
            let ttlSeconds: number;

            if (cookies.tid.expires) {
                ttlSeconds = Math.floor((cookies.tid.expires.getTime() - Date.now()) / 1000);
            } else {
                throw new Error('Token cookie has neither Max-Age nor Expires');
            }

            expect(ttlSeconds).toBeGreaterThan(1000000); // At least ~11 days
            expect(ttlSeconds).toBeLessThan(1500000); // At most ~17 days
        }
    });

    test('Cookies shall not be present on non-auth endpoints', async ({ api, identityUrl }) => {
        const response = await api.auth.loginWithGuestRequest(null, null, null);
        const sid = response.cookies().sid.value;

        // Health check should not return cookies
        const healthResponse = await api.client.get(`${identityUrl}/info/ready`).withCookies({ sid });

        const healthCookies = healthResponse.cookies();
        // Should not set new cookies
        expect(healthCookies.sid).toBeUndefined();
        expect(healthCookies.tid).toBeUndefined();
    });
});
