import { expect, test } from '$fixtures/setup';
import { Problem, ProblemSchema } from '$lib/api/api';
import { TestUser } from '$lib/api/test_user';
import { TokenKind } from '$lib/api/token_api';
import { getPageRedirectUrl } from '$lib/api/utils';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { delay } from '$lib/utils';

test.describe('Created token', () => {
    let mock: OAuth2MockServer = undefined!;
    let user: TestUser = undefined!;

    test.beforeEach(async ({ api }) => {
        mock = new OAuth2MockServer();
        await mock.start();
        user = await api.testUsers.createLinked(mock);
    });

    test.afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
        user = undefined!;
    });

    for (const kind of ['singleAccess', 'persistent']) {
        test(`Creating ${kind} token without session shall fail`, async ({ api }) => {
            const response = await api.token.createTokenRequest(null, kind as TokenKind, 20, false).send();
            expect(response).toHaveStatus(401);
        });
    }

    class InputValidation {
        constructor(
            public kind: TokenKind,
            public duration: number,
            public bindToSite: boolean,
            public expect: (error: Problem) => void
        ) {}
    }
    const inputValidationCases: InputValidation[] = [
        new InputValidation('singleAccess', 20000, false, (err) => {
            const ttlErr = err.extension.time_to_live[0];
            expect(ttlErr.code).toEqual('range');
            expect(ttlErr.message).toBeNull();
        }),
        new InputValidation('persistent', 31536001, false, (err) => {
            const ttlErr = err.extension.time_to_live[0];
            expect(ttlErr.code).toEqual('range');
            expect(ttlErr.message).toBeNull();
        }),
        new InputValidation('access', 200, false, (err) => {
            const ttlErr = err.extension.kind[0];
            expect(ttlErr.code).toEqual('oneOf');
            expect(ttlErr.message).toEqual('Access tokens are not allowed');
        })
    ];

    for (const input of inputValidationCases) {
        test(`Token creation with(kind: ${input.kind}, duration: ${input.duration}, bindToSite: ${input.bindToSite}) shall be rejected`, async ({
            api
        }) => {
            const response = await api.token
                .createTokenRequest(user.sid, input.kind, input.duration, input.bindToSite)
                .send();
            expect(response).toHaveStatus(400);
            const error = await response.parse(ProblemSchema);
            expect(error.type).toEqual('validation_error');
            input.expect(error);
        });
    }
});

test.describe('Single access token', () => {
    let mock: OAuth2MockServer = undefined!;
    let user: TestUser = undefined!;

    test.beforeEach(async ({ api }) => {
        mock = new OAuth2MockServer();
        await mock.start();
        user = await api.testUsers.createLinked(mock);
    });

    test.afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
        user = undefined!;
    });

    test('A failed login with a single access token shall clear the current user', async ({ api }) => {
        const response = await api.auth.loginWithTokenRequest(null, null, 'invalid', null, false, null).send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    test('A successful login with a single access token shall change the current user', async ({ api }) => {
        const token = await api.token.createSAToken(user.sid, 120, false);

        const response = await api.auth.loginWithTokenRequest(null, null, token.token, null, false, null).send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(api.auth.defaultRedirects.redirectUrl);
        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();
    });

    test('The single access token with client binding shall respect client fingerprint and revoke token on mismatch', async ({
        api
    }) => {
        const token = await api.token.createSAToken(user.sid, 120, true);

        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens).toEqual([token.tokenHash]);

        const response = await api.auth
            .loginWithTokenRequest(null, null, token.token, null, false, null)
            .withHeaders({ 'user-agent': 'agent2' })
            .send();
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=authError&status=400'
        );

        // token is revoked
        expect(await api.token.getTokens(user.sid)).toBeEmpty();
    });

    test('The single access token shall expire after the specified time', async ({ api }) => {
        test.setTimeout(35 * 1000);

        const now = new Date().getTime();
        const ttl = 10;
        const token = await api.token.createSAToken(user.sid, ttl, false);
        expect(token.expireAt).toBeAfter(new Date(now + ttl * 1000));
        expect(token.expireAt).toBeBefore(new Date(now + (ttl + 5) * 1000));

        console.log(`Waiting for the token to expire (${ttl} second)...`);
        await delay(ttl * 1000);
        const response = await api.auth.loginWithTokenRequest(null, null, token.token, null, false, null).send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );
    });

    test('The single access token shall be used only once', async ({ api }) => {
        const now = new Date().getTime();
        const token = await api.token.createSAToken(user.sid, 120, false);
        expect(token.expireAt).toBeAfter(new Date(now + 120 * 1000));
        expect(token.expireAt).toBeBefore(new Date(now + 130 * 1000));

        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens).toEqual([token.tokenHash]);

        const response1 = await api.auth.loginWithTokenRequest(null, null, token.token, null, false, null).send();
        expect(response1).toHaveStatus(200);
        const sid1 = response1.cookies().sid.value;
        const user1 = await api.user.getUserInfo(sid1);
        expect(user1.userId, 'It shall be the same use').toEqual(user.userId);

        const tokens1 = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens1, 'Token shall be removed').toEqual([]);

        const response2 = await api.auth.loginWithTokenRequest(null, null, token.token, null, false, null).send();
        expect(response2).toHaveStatus(200);
        expect(getPageRedirectUrl(await response2.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );
    });

    test('The single access token revoke shall work', async ({ api }) => {
        const now = new Date().getTime();
        const token = await api.token.createSAToken(user.sid, 120, false);
        expect(token.expireAt).toBeAfter(new Date(now + 120 * 1000));
        expect(token.expireAt).toBeBefore(new Date(now + 130 * 1000));

        const tokens = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens).toEqual([token.tokenHash]);

        let response = await api.token.revokeTokenRequest(user.sid, token.tokenHash).send();
        expect(response).toHaveStatus(200);

        const tokens1 = (await api.token.getTokens(user.sid)).map((x) => x.tokenHash).sort();
        expect(tokens1, 'Token shall be removed').toEqual([]);

        response = await api.auth.loginWithTokenRequest(null, null, token.token, null, false, null).send();
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(
            api.auth.defaultRedirects.errorUrl + '?type=tokenExpired&status=401'
        );
    });
});

test.describe('Persistent token', () => {
    let mock: OAuth2MockServer = undefined!;
    let user: TestUser = undefined!;

    test.beforeEach(async ({ api }) => {
        mock = new OAuth2MockServer();
        await mock.start();
        user = await api.testUsers.createLinked(mock);
    });

    test.afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
        user = undefined!;
    });

    test('A failed login with a persistent token shall clear the current user', async ({ api }) => {
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
        expect(await api.token.getTokens(user.sid)).toBeEmpty();
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
