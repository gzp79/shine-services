import request from 'superagent';
import config from '../test.config';
import { TestUser } from '$lib/user';
import { randomUUID } from 'crypto';
//import requestLogger from 'superagent-logger';

// It checks only for the access of the feature, but not if it does what it have to.
describe('User role access', () => {
    let users: Record<string, TestUser> = {};

    beforeAll(async () => {
        users.target = await TestUser.create([]);
        users.general = await TestUser.create([]);
        users.admin = await TestUser.create(['SuperAdmin']);
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
        new TestCase(null, false, 'target', 401),
        new TestCase(null, true, 'target', 401),
        new TestCase('general', false, 'target', 403),
        new TestCase('general', false, 'general', 403), // own role is also prohibited
        new TestCase('general', true, 'target', 200),
        new TestCase('admin', false, 'target', 200),
        new TestCase('admin', true, 'target', 200)
    ];

    it.each(testCases)(
        '$# Get roles. user:$user, apiKey:$apiKey, target:$targetUser',
        async (test) => {
            let target = users[test.targetUser];
            let req = request.get(
                config.getUrlFor(`/identity/api/identities/${target.userId}/roles`)
            );
            if (test.user) {
                req.set('Cookie', users[test.user].getCookies());
            }
            if (test.apiKey) {
                req.set('Authorization', `Bearer ${config.masterKey}`);
            }
            let response = await req.send().catch((err) => err.response);
            expect(response.statusCode).toEqual(test.expectedCode);
        }
    );

    it.each(testCases)(
        '$# Add role. user:$user, apiKey:$apiKey, target:$targetUser',
        async (test) => {
            let target = users[test.targetUser];
            let req = request.put(
                config.getUrlFor(`/identity/api/identities/${target.userId}/roles`)
            );
            if (test.user) {
                req.set('Cookie', users[test.user].getCookies());
            }
            if (test.apiKey) {
                req.set('Authorization', `Bearer ${config.masterKey}`);
            }
            let response = await req
                .type('json')
                .send({ role: 'Role_' + randomUUID() })
                .catch((err) => err.response);
            expect(response.statusCode).toEqual(test.expectedCode);
        }
    );

    it.each(testCases)(
        '$# Delete role. user:$user, apiKey:$apiKey, target:$targetUser',
        async (test) => {
            let target = users[test.targetUser];
            let req = request.delete(
                config.getUrlFor(`/identity/api/identities/${target.userId}/roles`)
            );
            if (test.user) {
                req.set('Cookie', users[test.user].getCookies());
            }
            if (test.apiKey) {
                req.set('Authorization', `Bearer ${config.masterKey}`);
            }
            let response = await req
                .type('json')
                .send({ role: 'Role2' })
                .catch((err) => err.response);
            expect(response.statusCode).toEqual(test.expectedCode);
        }
    );
});

describe('User role features', () => {
    let admin: TestUser = undefined!;

    beforeAll(async () => {
        admin = await TestUser.create(['SuperAdmin']);
    });

    it('Getting role of non-existing user', async () => {
        let response = await request
            .get(config.getUrlFor(`/identity/api/identities/${randomUUID()}/roles`))
            .set('Cookie', admin.getCookies())
            .send()
            .catch((err) => err.response);
        expect(response.statusCode).toEqual(404);
    });

    it('Setting role of non-existing user', async () => {
        let response = await request
            .put(config.getUrlFor(`/identity/api/identities/${randomUUID()}/roles`))
            .set('Cookie', admin.getCookies())
            .type('json')
            .send({ role: 'Role1' })
            .catch((err) => err.response);
        expect(response.statusCode).toEqual(404);
    });

    it('Deleting role of non-existing user', async () => {
        let response = await request
            .delete(config.getUrlFor(`/identity/api/identities/${randomUUID()}/roles`))
            .set('Cookie', admin.getCookies())
            .type('json')
            .send({ role: 'Role1' })
            .catch((err) => err.response);
        expect(response.statusCode).toEqual(404);
    });
});
