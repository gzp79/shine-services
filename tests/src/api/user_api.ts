import { expect } from '$fixtures/setup';
import { DateStringSchema, OptionalSchema } from '$lib/schema_utils';
import { joinURL } from '$lib/utils';
import { z } from 'zod';
import { ApiRequest } from './api';

export type GetUserInfoMethod = 'fast' | 'full' | 'fillWithRefresh';

export const UserInfoDetailSchema = z.object({
    kind: z.string(),
    email: OptionalSchema(z.string()),
    createdAt: DateStringSchema
});
export type UserInfoDetail = z.infer<typeof UserInfoDetailSchema>;

export const UserInfoSchema = z.object({
    isLinked: z.boolean(),
    userId: z.string(),
    name: z.string(),
    email: OptionalSchema(z.string()),
    isEmailConfirmed: z.boolean(),
    roles: z.array(z.string()),
    sessionLength: z.number(),
    remainingSessionTime: z.number(),
    details: UserInfoDetailSchema.nullable()
});
export type UserInfo = z.infer<typeof UserInfoSchema>;

export const AddUserRoleSchema = z.object({
    role: z.string()
});
export type AddUserRole = z.infer<typeof AddUserRoleSchema>;

export const DeleteUserRoleSchema = z.object({
    role: z.string()
});
export type DeleteUserRole = z.infer<typeof DeleteUserRoleSchema>;

const UsersRoleSchema = z.object({
    roles: z.array(z.string())
});
export type UserRoles = z.infer<typeof UsersRoleSchema>;

// eslint-disable-next-line @typescript-eslint/no-unused-vars
const EmailChangeSchema = z.object({
    email: z.string()
});
export type EmailChange = z.infer<typeof EmailChangeSchema>;

export class UserAPI {
    constructor(
        public readonly serviceUrl: string,
        public readonly masterAdminKey: string
    ) {}

    urlFor(path: string) {
        return joinURL(new URL(this.serviceUrl), path);
    }

    getUserInfoRequest(sid: string | null, method: GetUserInfoMethod | null): ApiRequest {
        const cs = sid && { sid };

        return ApiRequest.get(this.urlFor('api/auth/user/info'))
            .withParams(method ? { method } : {})
            .withCookies({ ...cs });
    }

    async getUserInfo(
        sid: string,
        method: GetUserInfoMethod | null,
        extraHeaders?: Record<string, string>
    ): Promise<UserInfo> {
        const response = await this.getUserInfoRequest(sid, method).withHeaders(extraHeaders ?? {});
        expect(response).toHaveStatus(200);

        return await response.parse(UserInfoSchema);
    }

    getRolesRequest(sid: string | null, masterKey: boolean, userId: string): ApiRequest {
        const cs = sid && { sid };
        const av = masterKey ? `${this.masterAdminKey}` : null;

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
        const response = await this.getRolesRequest(sid, masterKey, userId).withHeaders(extraHeaders ?? {});
        expect(response).toHaveStatus(200);

        return (await response.parse(UsersRoleSchema)).roles;
    }

    addRoleRequest(sid: string | null, masterKey: boolean, userId: string, role: string): ApiRequest<AddUserRole> {
        const cs = sid && { sid };
        const av = masterKey ? `${this.masterAdminKey}` : null;

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
            const response = await this.addRoleRequest(sid, masterKey, userId, role).withHeaders(extraHeaders ?? {});
            expect(response).toHaveStatus(200);
            return (await response.parse(UsersRoleSchema)).roles;
        }
    }

    deleteRoleRequest(
        sid: string | null,
        masterKey: boolean,
        userId: string,
        role: string
    ): ApiRequest<DeleteUserRole> {
        const cs = sid && { sid };
        const av = masterKey ? `${this.masterAdminKey}` : null;

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
            const response = await this.deleteRoleRequest(sid, masterKey, userId, role).withHeaders(extraHeaders ?? {});
            expect(response).toHaveStatus(200);
            return (await response.parse(UsersRoleSchema)).roles;
        }
    }

    startConfirmEmailRequest(sid: string | null, lang?: string): ApiRequest {
        const cs = sid && { sid };
        const pl = lang && { lang };

        return ApiRequest.post(this.urlFor('/api/auth/user/email/confirm'))
            .withCookies({ ...cs })
            .withParams({ ...pl });
    }

    async startConfirmEmail(sid: string | null, lang?: string): Promise<void> {
        const response = await this.startConfirmEmailRequest(sid, lang);
        expect(response).toHaveStatus(200);
    }

    startChangeEmailRequest(sid: string | null, email: string, lang?: string): ApiRequest<EmailChange> {
        const cs = sid && { sid };
        const pl = lang && { lang };

        return ApiRequest.post<EmailChange>(this.urlFor('/api/auth/user/email/change'))
            .withCookies({ ...cs })
            .withParams({ ...pl })
            .withBody({ email });
    }

    async startChangeEmail(sid: string | null, email: string, lang?: string): Promise<void> {
        const response = await this.startChangeEmailRequest(sid, email, lang);
        expect(response).toHaveStatus(200);
    }

    completeConfirmEmailRequest(sid: string | null, token: string): ApiRequest {
        const cs = sid && { sid };

        return ApiRequest.post(this.urlFor(`/api/auth/user/email/complete?token=${token}`)).withCookies({
            ...cs
        });
    }

    async completeConfirmEmail(sid: string | null, token: string): Promise<void> {
        const response = await this.completeConfirmEmailRequest(sid, token);
        expect(response).toHaveStatus(200);
    }
}
