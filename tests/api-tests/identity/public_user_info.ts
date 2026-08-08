import { expect, test } from '$fixtures/setup';
import { unstructured } from '$lib/api/api';
import { AuthAPI } from '$lib/api/auth_api';
import { TestUser, TestUserHelper } from '$lib/api/test_user';
import { UserAPI } from '$lib/api/user_api';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { randomUUID } from 'crypto';

test.describe('Public user info', () => {
    let mockAuth: OAuth2MockServer;
    let prefix: string;
    let userA: TestUser;
    let userB: TestUser;

    test.beforeAll(async ({ identityUrl, defaultRedirects, masterAdminKey, enableRequestLogging }) => {
        prefix = 'Pub' + randomUUID().replace(/-/g, '').slice(0, 8);

        const auth = new AuthAPI(identityUrl, defaultRedirects, enableRequestLogging);
        const user = new UserAPI(identityUrl, masterAdminKey, enableRequestLogging);
        const testUsers = new TestUserHelper(auth, user);

        mockAuth = new OAuth2MockServer();
        await mockAuth.start();

        userA = await testUsers.createLinked(mockAuth, {
            name: `${prefix}Alice`,
            email: `${prefix.toLowerCase()}alice@example.com`
        });
        userB = await testUsers.createLinked(mockAuth, {
            name: `${prefix}Bob`,
            email: `${prefix.toLowerCase()}bob@example.com`
        });
    });

    test.afterAll(async () => {
        await mockAuth?.stop();
    });

    test('Request without session shall fail with 401', async ({ api }) => {
        const response = await api.user.getPublicUserInfoRequest(null, [userA.userId]);
        expect(response).toHaveStatus(401);
    });

    test('Any authenticated user shall resolve public info (name only, no email)', async ({ api }) => {
        const guest = await api.testUsers.createGuest();
        const result = await api.user.getPublicUserInfo(guest.sid, [userA.userId, userB.userId]);

        expect(result.users[userA.userId]).toEqual({ name: userA.name });
        expect(result.users[userB.userId]).toEqual({ name: userB.name });
        // only the public name is exposed
        expect(Object.keys(result.users[userA.userId])).toEqual(['name']);
    });

    test('Unknown ids shall resolve to Anonymous', async ({ api }) => {
        const guest = await api.testUsers.createGuest();
        const unknownId = randomUUID();
        const result = await api.user.getPublicUserInfo(guest.sid, [unknownId]);

        expect(result.users[unknownId]).toEqual({ name: 'Anonymous' });
    });

    test('Mixed known and unknown ids shall each resolve correctly', async ({ api }) => {
        const guest = await api.testUsers.createGuest();
        const unknownId = randomUUID();
        const result = await api.user.getPublicUserInfo(guest.sid, [userA.userId, unknownId]);

        expect(result.users[userA.userId]).toEqual({ name: userA.name });
        expect(result.users[unknownId]).toEqual({ name: 'Anonymous' });
    });

    test('Duplicate ids shall be deduplicated in the response', async ({ api }) => {
        const guest = await api.testUsers.createGuest();
        const result = await api.user.getPublicUserInfo(guest.sid, [userA.userId, userA.userId]);

        expect(Object.keys(result.users)).toEqual([userA.userId]);
        expect(result.users[userA.userId]).toEqual({ name: userA.name });
    });

    test('Empty id list shall be rejected with 400', async ({ api }) => {
        const guest = await api.testUsers.createGuest();
        const response = await api.user.getPublicUserInfoRequest(guest.sid, []);
        expect(response).toHaveStatus(400);
    });

    test('Over-cap id list shall be rejected with 400 without echoing the input', async ({ api }) => {
        const guest = await api.testUsers.createGuest();
        const userIds = Array.from({ length: 101 }, () => randomUUID());
        const response = await api.user.getPublicUserInfoRequest(guest.sid, []).withBody(unstructured({ userIds }));
        expect(response).toHaveStatus(400);

        // the rejection must not echo the (possibly huge) input back to the client
        const body = await response.text();
        for (const id of userIds) {
            expect(body).not.toContain(id);
        }
    });
});
