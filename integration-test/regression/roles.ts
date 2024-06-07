import { randomUUID } from 'crypto';
import api from '$lib/api/api';
import request from '$lib/request';
import { TestUser } from '$lib/test_user';
import config from '../test.config';

// It checks only for the access of the feature, but not if it does what it have to.
describe('Access to user role management', () => {
    let users: Record<string, TestUser> = {};

    beforeAll(async () => {
        users.target = await TestUser.createGuest();
        users.general = await TestUser.createGuest();
        users.admin = await TestUser.createGuest({ roles: ['SuperAdmin'] });
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

    it.each(testCases)(
        'Get roles ($#) with (user:$user, apiKey:$apiKey, target:$targetUser) shall return $expectedCode',
        async (test) => {
            let target = users[test.targetUser];
            const sid = test.user ? users[test.user].sid : null;
            const response = await api.request.getRoles(sid, test.apiKey, target.userId);
            expect(response).toHaveStatus(test.expectedCode);
        }
    );

    it.each(testCases)(
        'Add role ($#) with (user:$user, apiKey:$apiKey, target:$targetUser) shall return $expectedCode',
        async (test) => {
            let target = users[test.targetUser];
            const sid = test.user ? users[test.user].sid : null;
            const response = await api.request.addRole(
                sid,
                test.apiKey,
                target.userId,
                'Role_' + randomUUID()
            );
            expect(response).toHaveStatus(test.expectedCode);
        }
    );

    it.each(testCases)(
        'Delete role ($#) with (user:$user, apiKey:$apiKey, target:$targetUser) shall return $expectedCode',
        async (test) => {
            let target = users[test.targetUser];
            const sid = test.user ? users[test.user].sid : null;
            const response = await api.request.deleteRole(sid, test.apiKey, target.userId, 'Role2');
            expect(response).toHaveStatus(test.expectedCode);
        }
    );
});

describe('User roles', () => {
    let admin: TestUser = undefined!;

    beforeAll(async () => {
        admin = await TestUser.createGuest({ roles: ['SuperAdmin'] });
    });

    it('Getting role of non-existing user shall fail', async () => {
        let response = await api.request.getRoles(admin.sid, false, randomUUID());
        expect(response).toHaveStatus(404);
    });

    it('Setting role of non-existing user shall fail', async () => {
        let response = await api.request.addRole(admin.sid, false, randomUUID(), 'Role1');
        expect(response).toHaveStatus(404);
    });

    it('Deleting role of non-existing user shall fail', async () => {
        let response = await api.request.deleteRole(admin.sid, false, randomUUID(), 'Role1');
        expect(response).toHaveStatus(404);
    });

    it('A complex flow with add, get, delete shall work', async () => {
        const user = await TestUser.createGuest();

        const userApi = api.user;
        const userId = user.userId;
        const uSid = user.sid;
        const aSid = admin.sid;

        expect(await userApi.getRoles(aSid, false, userId)).toIncludeSameMembers([]);
        expect((await userApi.getUserInfo(uSid)).roles).toIncludeSameMembers([]);

        // remove Role3 (not existing)
        expect(await userApi.deleteRoles(aSid, false, userId, 'Role3')).toIncludeSameMembers([]);
        expect(await userApi.getRoles(aSid, false, userId)).toIncludeSameMembers([]);
        expect((await userApi.getUserInfo(uSid)).roles).toIncludeSameMembers([]);

        // add Role1
        expect(await userApi.addRole(aSid, false, userId, 'Role1')).toIncludeSameMembers(['Role1']);
        expect(await userApi.getRoles(aSid, false, userId)).toIncludeSameMembers(['Role1']);
        expect((await userApi.getUserInfo(uSid)).roles).toIncludeSameMembers(['Role1']);

        //add Role2
        expect(await userApi.addRole(aSid, false, userId, 'Role2')).toIncludeSameMembers(['Role1', 'Role2']);
        expect(await userApi.getRoles(aSid, false, userId)).toIncludeSameMembers(['Role1', 'Role2']);
        expect((await userApi.getUserInfo(uSid)).roles).toIncludeSameMembers(['Role1', 'Role2']);

        // remove Role1
        expect(await userApi.deleteRoles(aSid, false, userId, 'Role1')).toIncludeSameMembers(['Role2']);
        expect(await userApi.getRoles(aSid, false, userId)).toIncludeSameMembers(['Role2']);
        expect((await userApi.getUserInfo(uSid)).roles).toIncludeSameMembers(['Role2']);

        // remove Role3 (not existing)
        expect(await userApi.deleteRoles(aSid, false, userId, 'Role3')).toIncludeSameMembers(['Role2']);
        expect(await userApi.getRoles(aSid, false, userId)).toIncludeSameMembers(['Role2']);
        expect((await userApi.getUserInfo(uSid)).roles).toIncludeSameMembers(['Role2']);

        // remove Role2
        expect(await userApi.deleteRoles(aSid, false, userId, 'Role2')).toIncludeSameMembers([]);
        expect(await userApi.getRoles(aSid, false, userId)).toIncludeSameMembers([]);
        expect((await userApi.getUserInfo(uSid)).roles).toIncludeSameMembers([]);
    });
});
