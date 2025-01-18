import { expect } from '@fixtures/service-fixture';
import { OptionalSchema } from '$lib/schema_utils';
import { joinURL } from '$lib/utils';
import { z } from 'zod';
import { ApiRequest } from './api';

const UserInfoSchema = z.object({
    isLinked: z.boolean(),
    userId: z.string(),
    name: z.string(),
    email: OptionalSchema(z.string()),
    isEmailConfirmed: z.boolean(),
    roles: z.array(z.string()),
    sessionLength: z.number()
});
export type UserInfo = z.infer<typeof UserInfoSchema>;

const AddUserRoleSchema = z.object({
    role: z.string()
});
export type AddUserRole = z.infer<typeof AddUserRoleSchema>;

const DeleteUserRoleSchema = z.object({
    role: z.string()
});
export type DeleteUserRole = z.infer<typeof DeleteUserRoleSchema>;

const UsersRoleSchema = z.object({
    roles: z.array(z.string())
});
export type UserRoles = z.infer<typeof UsersRoleSchema>;

export class UserAPI {
    constructor(
        public readonly serviceUrl: string,
        public readonly masterAdminKey: string
    ) {}

    urlFor(path: string) {
        return joinURL(new URL(this.serviceUrl), path);
    }

    getUserInfoRequest(sid: string | null): ApiRequest {
        const cs = sid && { sid };

        return ApiRequest.get(this.urlFor('api/auth/user/info')).withCookies({ ...cs });
    }

    async getUserInfo(sid: string, extraHeaders?: Record<string, string>): Promise<UserInfo> {
        let response = await this.getUserInfoRequest(sid)
            .withHeaders(extraHeaders ?? {})
            .send();
        expect(response).toHaveStatus(200);

        return await response.parse(UserInfoSchema);
    }

    getRolesRequest(sid: string | null, masterKey: boolean, userId: string): ApiRequest {
        const cs = sid && { sid };
        let av = masterKey ? `${this.masterAdminKey}` : null;

        return ApiRequest.get(this.urlFor(`/api/identities/${userId}/roles`))
            .withCookies({ ...cs })
            .withAuthIf(av);
    }

    async getRoles(
        sid: string,
        masterKey: boolean,
        userId: string,
        extraHeaders?: Record<string, string>
    ): Promise<string[]> {
        let response = await this.getRolesRequest(sid, masterKey, userId)
            .withHeaders(extraHeaders ?? {})
            .send();
        expect(response).toHaveStatus(200);

        return (await response.parse(UsersRoleSchema)).roles;
    }

    addRoleRequest(sid: string | null, masterKey: boolean, userId: string, role: string): ApiRequest<AddUserRole> {
        const cs = sid && { sid };
        let av = masterKey ? `${this.masterAdminKey}` : null;

        return ApiRequest.put<AddUserRole>(this.urlFor(`/api/identities/${userId}/roles`), { role })
            .withCookies({ ...cs })
            .withAuthIf(av);
    }

    async addRole(
        sid: string,
        masterKey: boolean,
        userId: string,
        role: string | string[],
        extraHeaders?: Record<string, string>
    ): Promise<string[]> {
        if (role instanceof Array) {
            let result: string[] = [];
            for (const r of role) {
                result = await this.addRole(sid, masterKey, userId, r, extraHeaders);
            }
            return result;
        } else {
            let response = await this.addRoleRequest(sid, masterKey, userId, role)
                .withHeaders(extraHeaders ?? {})
                .send();
            expect(response).toHaveStatus(200);
            return (await response.parse(UsersRoleSchema)).roles;
        }
    }

    deleteRoleRequest(
        sid: string | 'masterKey' | null,
        masterKey: boolean,
        userId: string,
        role: string
    ): ApiRequest<DeleteUserRole> {
        const cs = sid && { sid };
        let av = masterKey ? `${this.masterAdminKey}` : null;

        return ApiRequest.delete<DeleteUserRole>(this.urlFor(`/api/identities/${userId}/roles`))
            .withCookies({ ...cs })
            .withAuthIf(av)
            .withBody({ role });
    }

    async deleteRoles(
        sid: string,
        masterKey: boolean,
        userId: string,
        role: string | string[],
        extraHeaders?: Record<string, string>
    ): Promise<string[]> {
        if (role instanceof Array) {
            let result: string[] = [];
            for (const r of role) {
                result = await this.deleteRoles(sid, masterKey, userId, r, extraHeaders);
            }
            return result;
        } else {
            let response = await this.deleteRoleRequest(sid, masterKey, userId, role)
                .withHeaders(extraHeaders ?? {})
                .send();
            expect(response).toHaveStatus(200);
            return (await response.parse(UsersRoleSchema)).roles;
        }
    }
}
