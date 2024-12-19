import { RequestAPI } from './api';

export interface UserInfo {
    userId: string;
    name: string;
    sessionLength: number;
    roles: string[];
    isLinked: boolean;
}

export class UserAPI {
    constructor(public readonly request: RequestAPI) {}

    async getUserInfo(sid: string, extraHeaders?: Record<string, string>): Promise<UserInfo> {
        let response = await this.request.getUserInfo(sid).set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        //expect(response.body).toBeInstanceOf(UserInfo);
        return response.body;
    }

    async getRoles(
        sid: string,
        masterKey: boolean,
        userId: string,
        extraHeaders?: Record<string, string>
    ): Promise<string[]> {
        let response = await this.request.getRoles(sid, masterKey, userId).set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        return response.body.roles;
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
            let response = await this.request.addRole(sid, masterKey, userId, role).set(extraHeaders ?? {});
            expect(response).toHaveStatus(200);
            return response.body.roles;
        }
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
            let response = await this.request
                .deleteRole(sid, masterKey, userId, role)
                .set(extraHeaders ?? {});
            expect(response).toHaveStatus(200);
            return response.body.roles;
        }
    }
}
