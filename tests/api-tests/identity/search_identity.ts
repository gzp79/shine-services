import { expect, test } from '$fixtures/setup';
import { AuthAPI } from '$lib/api/auth_api';
import { TestUser, TestUserHelper } from '$lib/api/test_user';
import { UserAPI } from '$lib/api/user_api';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { randomUUID } from 'crypto';

test.describe('Search identities', () => {
    let mockAuth: OAuth2MockServer;
    let prefix: string;
    let userA: TestUser;
    let userB: TestUser;

    test.beforeAll(async ({ identityUrl, defaultRedirects, masterAdminKey, enableRequestLogging }) => {
        prefix = 'Srch' + randomUUID().replace(/-/g, '').slice(0, 8);

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
        await testUsers.createLinked(mockAuth, {
            name: `${prefix}Carol`,
            email: `${prefix.toLowerCase()}carol@example.com`
        });
    });

    test.afterAll(async () => {
        await mockAuth?.stop();
    });

    test('Search without session shall fail with 401', async ({ api }) => {
        const response = await api.user.searchIdentitiesRequest(null, {});
        expect(response).toHaveStatus(401);
    });

    test('Search without READ_ANY_IDENTITY permission shall fail with 403', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        const response = await api.user.searchIdentitiesRequest(user.sid, {});
        expect(response).toHaveStatus(403);
    });

    test('Search with SuperAdmin role shall succeed', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, { count: 1 });
        expect(Array.isArray(result.identities)).toBe(true);
    });

    test('Search by userId shall find the user', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, { userId: userA.userId });
        expect(result.identities).toHaveLength(1);
        expect(result.identities[0].id).toBe(userA.userId);
        expect(result.identities[0].name).toBe(userA.name);
        expect(result.isPartial).toBe(false);
    });

    test('Search by unknown userId shall return empty results', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, { userId: randomUUID() });
        expect(result.identities).toHaveLength(0);
        expect(result.isPartial).toBe(false);
    });

    test('Search by multiple userIds shall return all matching users', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, {
            userId: [userA.userId, userB.userId]
        });
        const ids = result.identities.map((i) => i.id);
        expect(ids).toContain(userA.userId);
        expect(ids).toContain(userB.userId);
    });

    test('Search by email shall find the user with that email', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, { email: userA.email });
        expect(result.identities).toHaveLength(1);
        expect(result.identities[0].id).toBe(userA.userId);
        expect(result.identities[0].email).toBe(userA.email);
    });

    test('Search by unknown email shall return empty results', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, {
            email: `nobody-${randomUUID()}@example.com`
        });
        expect(result.identities).toHaveLength(0);
    });

    test('Search by multiple emails shall return all matching users', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, {
            email: [userA.email!, userB.email!]
        });
        const ids = result.identities.map((i) => i.id);
        expect(ids).toContain(userA.userId);
        expect(ids).toContain(userB.userId);
    });

    test('Search by name fragment shall find users whose name contains the fragment', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, { name: prefix });
        expect(result.identities.some((i) => i.id === userA.userId)).toBe(true);
    });

    test('Search by name fragment shall be case-insensitive', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, { name: prefix.toLowerCase() });
        expect(result.identities.some((i) => i.id === userA.userId)).toBe(true);
    });

    test('Search by unknown name fragment shall return empty results', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, {
            name: 'ZzZzNobodyHasThisName' + randomUUID()
        });
        expect(result.identities).toHaveLength(0);
    });

    test('Search by multiple name fragments shall return users matching any (OR)', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, {
            name: [`${prefix}Alice`, `${prefix}Bob`]
        });
        const ids = result.identities.map((i) => i.id);
        expect(ids).toContain(userA.userId);
        expect(ids).toContain(userB.userId);
    });

    test('Search with userId AND name filters shall apply AND logic', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, {
            userId: userA.userId,
            name: prefix
        });
        expect(result.identities).toHaveLength(1);
        expect(result.identities[0].id).toBe(userA.userId);
        expect(result.identities.some((i) => i.id === userB.userId)).toBe(false);
    });

    test('isPartial shall be true when results are truncated by count', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, { name: prefix, count: 2 });
        expect(result.identities).toHaveLength(2);
        expect(result.isPartial).toBe(true);
    });

    test('isPartial shall be false when all results fit within count', async ({ api, adminUser }) => {
        const result = await api.user.searchIdentities(adminUser.sid, { name: prefix, count: 10 });
        expect(result.identities.some((i) => i.id === userA.userId)).toBe(true);
        expect(result.isPartial).toBe(false);
    });
});
