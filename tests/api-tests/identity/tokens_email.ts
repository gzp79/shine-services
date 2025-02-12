import { expect, test } from '$fixtures/setup';
import { TestUser } from '$lib/api/test_user';
import { getPageRedirectUrl } from '$lib/api/utils';
import MockSmtp from '$lib/mocks/mock_smtp';
import OAuth2MockServer from '$lib/mocks/oauth2';

test.describe('Email confirmation token', () => {
    let mockAuth: OAuth2MockServer = undefined!;
    let mockEmail: MockSmtp = undefined!;
    let user: TestUser = undefined!;

    /*const startMock = async (check: (mail: ParsedMail) => void): Promise<MockSmtp> => {
        if (!mockEmail) {
            mockEmail = new MockSmtp();
            await mockEmail.start(check);
        }
        return mockEmail as MockSmtp;
    };*/

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

    test('A failed login with a email token shall clear the current user', async ({ api }) => {
        const response = await api.auth.loginWithTokenRequest(null, null, null, 'invalid', false, null).send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('A successful login with a persistent token shall change the current user', async ({ api }) => {
        const token = await api.token.createPersistentToken(user.sid, 120, false);

        const response = await api.auth.loginWithTokenRequest(null, null, null, token.token, false, null).send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(api.auth.defaultRedirects.redirectUrl);
        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();
    });

    test('The persistent token with client binding shall respect client fingerprint and revoke token on mismatch', async ({
        api
    }) => {
        const token = await api.token.createPersistentToken(user.sid, 120, true);

        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens).toEqual([token.tokenHash]);

        const response = await api.auth
            .loginWithTokenRequest(null, null, null, token.token, false, null)
            .withHeaders({ 'user-agent': 'agent2' })
            .send();
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=authError&status=400'
        );

        // token is revoked
        expect(await api.token.getTokens(user.sid)).toBeEmptyValue();
    });

    test('The persistent token should be available for use at all times.', async ({ api }) => {
        const now = new Date().getTime();
        const token = await api.token.createPersistentToken(user.sid, 120, false);
        expect(token.expireAt).toBeAfter(new Date(now + 120 * 1000));
        expect(token.expireAt).toBeBefore(new Date(now + 130 * 1000));

        for (let i = 0; i < 3; i++) {
            const response1 = await api.auth.loginWithTokenRequest(null, null, null, token.token, false, null).send();
            expect(response1).toHaveStatus(200);
            const sid1 = response1.cookies().sid.value;
            const user1 = await api.user.getUserInfo(sid1);
            expect(user1.userId, 'It shall be the same use').toEqual(user.userId);

            const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
            expect(tokens).toEqual([token.tokenHash]);
        }
    });

    test('The persistent token revoke shall work', async ({ api }) => {
        const now = new Date().getTime();
        const token = await api.token.createPersistentToken(user.sid, 120, false);
        expect(token.expireAt).toBeAfter(new Date(now + 120 * 1000));
        expect(token.expireAt).toBeBefore(new Date(now + 130 * 1000));

        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens).toEqual([token.tokenHash]);

        let response = await api.token.revokeTokenRequest(user.sid, token.tokenHash).send();
        expect(response).toHaveStatus(200);

        const tokens1 = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens1, 'Token shall be removed').toEqual([]);

        response = await api.auth.loginWithTokenRequest(null, null, null, token.token, false, null).send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );
    });
});
