import { expect, test } from '$fixtures/setup';
import { TestUser } from '$lib/api/test_user';
import { randomUUID } from 'crypto';

// Tests authorization rules only (who can call the API), not the actual role operations.
// Actual role CRUD behavior is tested in 'User roles' below.
test.describe('Access to user role management', () => {
    const users: Record<string, TestUser> = {};

    test.beforeAll(async ({ api }) => {
        users.target = await api.testUsers.createGuest();
        users.general = await api.testUsers.createGuest();
        users.admin = await api.testUsers.createGuest({ roles: ['SuperAdmin'] });
    });

    class TestCase {
        constructor(
            public user: string | null,
            public apiKey: boolean,
            public targetUser: string,
            public expectedCode: number
        ) {}
    }

    const testCases = [
        // a user is required to be logged in, so we can track who altered the roles
        new TestCase(null, false, 'target', 401),
        new TestCase(null, true, 'target', 401),
        // general user manage roles only with the master key
        new TestCase('general', false, 'target', 403),
        new TestCase('general', false, 'general', 403), // own role is also prohibited
        new TestCase('general', true, 'target', 200),
        // admin can manage roles without master
        new TestCase('admin', false, 'target', 200),
        new TestCase('admin', true, 'target', 200)
    ];

    for (const tst of testCases) {
        test(`Get roles with (user:${tst.user}, apiKey:${tst.apiKey}, target:${tst.targetUser}) shall return ${tst.expectedCode}`, async ({
            api
        }) => {
            const target = users[tst.targetUser];
            const sid = tst.user ? users[tst.user].sid : null;
            const response = await api.user.getRolesRequest(sid, tst.apiKey, target.userId);
            expect(response).toHaveStatus(tst.expectedCode);
        });

        test(`Add role with (user:${tst.user}, apiKey:${tst.apiKey}, target:${tst.targetUser}) shall return ${tst.expectedCode}`, async ({
            api
        }) => {
            const target = users[tst.targetUser];
            const sid = tst.user ? users[tst.user].sid : null;
            const response = await api.user.addRoleRequest(sid, tst.apiKey, target.userId, 'Role_' + randomUUID());
            expect(response).toHaveStatus(tst.expectedCode);
        });

        test(`Delete role with (user:${tst.user}, apiKey:${tst.apiKey}, target:${tst.targetUser}) shall return ${tst.expectedCode}`, async ({
            api
        }) => {
            const target = users[tst.targetUser];
            const sid = tst.user ? users[tst.user].sid : null;
            const response = await api.user.deleteRoleRequest(sid, tst.apiKey, target.userId, 'Role2');
            expect(response).toHaveStatus(tst.expectedCode);
        });
    }
});

test.describe('User roles', () => {
    let admin: TestUser = undefined!;

    test.beforeAll(async ({ api }) => {
        admin = await api.testUsers.createGuest({ roles: ['SuperAdmin'] });
    });

    test('Getting role of non-existing user shall fail', async ({ api }) => {
        const response = await api.user.getRolesRequest(admin.sid, false, randomUUID());
        expect(response).toHaveStatus(404);
        expect(await response.json()).toEqual(expect.objectContaining({ type: 'not-found' }));
    });

    test('Setting role of non-existing user shall fail', async ({ api }) => {
        const response = await api.user.addRoleRequest(admin.sid, false, randomUUID(), 'Role1');
        expect(response).toHaveStatus(404);
        expect(await response.json()).toEqual(expect.objectContaining({ type: 'not-found' }));
    });

    test('Deleting role of non-existing user shall fail', async ({ api }) => {
        const response = await api.user.deleteRoleRequest(admin.sid, false, randomUUID(), 'Role1');
        expect(response).toHaveStatus(404);
        expect(await response.json()).toEqual(expect.objectContaining({ type: 'not-found' }));
    });

    test.describe('Role name validation', () => {
        const invalidRoles = [
            { role: '   ', label: 'whitespace-only' },
            { role: ' Admin', label: 'leading space' },
            { role: 'Admin ', label: 'trailing space' },
            { role: ' Admin ', label: 'leading and trailing spaces' },
            { role: 'Super Admin', label: 'embedded space' },
            { role: '\tAdmin', label: 'leading tab' },
            { role: 'Admin\nRole', label: 'embedded newline' }
        ];

        for (const { role, label } of invalidRoles) {
            test(`Adding role with ${label} shall be rejected`, async ({ api }) => {
                const user = await api.testUsers.createGuest();
                const response = await api.user.addRoleRequest(admin.sid, false, user.userId, role);
                expect(response).toHaveStatus(400);
                expect(await response.json()).toEqual(
                    expect.objectContaining({
                        type: 'input-validation',
                        extension: expect.objectContaining({
                            role: expect.arrayContaining([expect.objectContaining({ code: 'role_name_whitespace' })])
                        })
                    })
                );
            });

            test(`Deleting role with ${label} shall be rejected`, async ({ api }) => {
                const user = await api.testUsers.createGuest();
                const response = await api.user.deleteRoleRequest(admin.sid, false, user.userId, role);
                expect(response).toHaveStatus(400);
                expect(await response.json()).toEqual(
                    expect.objectContaining({
                        type: 'input-validation',
                        extension: expect.objectContaining({
                            role: expect.arrayContaining([expect.objectContaining({ code: 'role_name_whitespace' })])
                        })
                    })
                );
            });
        }
    });

    test('A complex flow with add, get, delete shall work', async ({ api }) => {
        const user = await api.testUsers.createGuest();

        const userApi = api.user;
        const userId = user.userId;
        const uSid = user.sid;
        const aSid = admin.sid;

        expect((await userApi.getRoles(aSid, false, userId)).sort()).toEqual([]);
        expect((await userApi.getUserInfo(uSid, 'fast')).roles.sort()).toEqual([]);
        expect((await userApi.getUserInfo(uSid, 'full')).roles.sort()).toEqual([]);

        // remove Role3 (not existing)
        expect((await userApi.deleteRoles(aSid, false, userId, 'Role3')).sort()).toEqual([]);
        expect((await userApi.getRoles(aSid, false, userId)).sort()).toEqual([]);
        expect((await userApi.getUserInfo(uSid, 'fast')).roles.sort()).toEqual([]);
        expect((await userApi.getUserInfo(uSid, 'full')).roles.sort()).toEqual([]);

        // add Role1
        expect((await userApi.addRole(aSid, false, userId, 'Role1')).sort()).toEqual(['Role1']);
        expect((await userApi.getRoles(aSid, false, userId)).sort()).toEqual(['Role1']);
        expect((await userApi.getUserInfo(uSid, 'fast')).roles.sort()).toEqual(['Role1']);
        expect((await userApi.getUserInfo(uSid, 'full')).roles.sort()).toEqual(['Role1']);

        // add Role1 (again)
        expect((await userApi.addRole(aSid, false, userId, 'Role1')).sort()).toEqual(['Role1']);
        expect((await userApi.getRoles(aSid, false, userId)).sort()).toEqual(['Role1']);
        expect((await userApi.getUserInfo(uSid, 'fast')).roles.sort()).toEqual(['Role1']);
        expect((await userApi.getUserInfo(uSid, 'full')).roles.sort()).toEqual(['Role1']);

        // add Role2
        expect((await userApi.addRole(aSid, false, userId, 'Role2')).sort()).toEqual(['Role1', 'Role2']);
        expect((await userApi.getRoles(aSid, false, userId)).sort()).toEqual(['Role1', 'Role2']);
        expect((await userApi.getUserInfo(uSid, 'fast')).roles.sort()).toEqual(['Role1', 'Role2']);
        expect((await userApi.getUserInfo(uSid, 'full')).roles.sort()).toEqual(['Role1', 'Role2']);

        // remove Role1
        expect((await userApi.deleteRoles(aSid, false, userId, 'Role1')).sort()).toEqual(['Role2']);
        expect((await userApi.getRoles(aSid, false, userId)).sort()).toEqual(['Role2']);
        expect((await userApi.getUserInfo(uSid, 'fast')).roles.sort()).toEqual(['Role2']);
        expect((await userApi.getUserInfo(uSid, 'full')).roles.sort()).toEqual(['Role2']);

        // remove Role3 (not existing)
        expect((await userApi.deleteRoles(aSid, false, userId, 'Role3')).sort()).toEqual(['Role2']);
        expect((await userApi.getRoles(aSid, false, userId)).sort()).toEqual(['Role2']);
        expect((await userApi.getUserInfo(uSid, 'fast')).roles.sort()).toEqual(['Role2']);
        expect((await userApi.getUserInfo(uSid, 'full')).roles.sort()).toEqual(['Role2']);

        // remove Role2
        expect((await userApi.deleteRoles(aSid, false, userId, 'Role2')).sort()).toEqual([]);
        expect((await userApi.getRoles(aSid, false, userId)).sort()).toEqual([]);
        expect((await userApi.getUserInfo(uSid, 'fast')).roles.sort()).toEqual([]);
        expect((await userApi.getUserInfo(uSid, 'full')).roles.sort()).toEqual([]);
    });
});
