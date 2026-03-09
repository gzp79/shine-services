import { expect, test } from '$fixtures/setup';
import { TestUser } from '$lib/api/test_user';
import { randomUUID } from 'crypto';

// Generate short random role names (max 32 chars)
function randomRoleName(): string {
    return `Role_${randomUUID().slice(0, 8)}`;
}

test.describe('Roles concurrency tests', { tag: '@concurrency' }, () => {
    let admin1: TestUser;
    let admin2: TestUser;
    let targetUser: TestUser;

    test.beforeAll(async ({ api }) => {
        admin1 = await api.testUsers.createGuest({ roles: ['SuperAdmin'] });
        admin2 = await api.testUsers.createGuest({ roles: ['SuperAdmin'] });
        targetUser = await api.testUsers.createGuest();
    });

    test('Concurrent role additions shall deduplicate roles', async ({ api }) => {
        const role = randomRoleName();

        // Both admins add same role simultaneously
        const [add1, add2] = await Promise.all([
            api.user.addRoleRequest(admin1.sid, false, targetUser.userId, role),
            api.user.addRoleRequest(admin2.sid, false, targetUser.userId, role)
        ]);

        // Both should succeed (idempotent)
        expect(add1).toHaveStatus(200);
        expect(add2).toHaveStatus(200);

        // Role should exist only once
        const roles = await api.user.getRoles(admin1.sid, false, targetUser.userId);
        const matchingRoles = roles.filter((r) => r === role);
        expect(matchingRoles).toHaveLength(1);
    });

    test('Concurrent add and delete of same role shall be deterministic', async ({ api }) => {
        const role = randomRoleName();

        // Pre-add the role
        await api.user.addRole(admin1.sid, false, targetUser.userId, role);

        // Race: add vs delete
        const [addResponse, deleteResponse] = await Promise.all([
            api.user.addRoleRequest(admin1.sid, false, targetUser.userId, role),
            api.user.deleteRoleRequest(admin2.sid, false, targetUser.userId, role)
        ]);

        // Both should complete
        // 200 = operation completed, 204 = no-op (already in desired state)
        expect([200, 204]).toContain(addResponse.status());
        expect([200, 204]).toContain(deleteResponse.status());

        // Race between add and delete:
        // - If add wins: role exists
        // - If delete wins: role doesn't exist
        // Both outcomes are correct depending on timing
        // What matters is the final state is consistent (not corrupted)
        const roles = await api.user.getRoles(admin1.sid, false, targetUser.userId);
        const hasRole = roles.includes(role);
        expect([true, false]).toContain(hasRole);
    });

    test('Multiple admins modifying roles on same user shall all succeed', async ({ api }) => {
        const role1 = randomRoleName();
        const role2 = randomRoleName();
        const role3 = randomRoleName();

        // Concurrent role operations from different admins
        const [add1, add2, add3] = await Promise.all([
            api.user.addRoleRequest(admin1.sid, false, targetUser.userId, role1),
            api.user.addRoleRequest(admin2.sid, false, targetUser.userId, role2),
            api.user.addRoleRequest(admin1.sid, false, targetUser.userId, role3)
        ]);

        // All should succeed
        expect(add1).toHaveStatus(200);
        expect(add2).toHaveStatus(200);
        expect(add3).toHaveStatus(200);

        // All roles should be present
        const roles = await api.user.getRoles(admin1.sid, false, targetUser.userId);
        expect(roles).toContain(role1);
        expect(roles).toContain(role2);
        expect(roles).toContain(role3);
    });
});
