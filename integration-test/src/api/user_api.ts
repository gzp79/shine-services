import { RequestAPI } from './api';

export interface UserInfo {
    userId: string;
    name: string;
    sessionLength: number;
    roles: string[];
}

export class UserAPI {
    constructor(public readonly request: RequestAPI) {}

    async getUserInfo(sid: string, extraHeaders?: Record<string, string>): Promise<UserInfo> {
        let response = await this.request
            .getUserInfo(sid)
            .set(extraHeaders ?? {})
            .send();
        expect(response.statusCode).toEqual(200);
        //expect(response.body).toBeInstanceOf(UserInfo);
        return response.body;
    }

    async getRoles(
        sidOrKey: string | 'masterKey',
        userId: string,
        extraHeaders?: Record<string, string>
    ): Promise<string[]> {
        let response = await this.request
            .getRoles(sidOrKey, userId)
            .set(extraHeaders ?? {})
            .send();
        expect(response.statusCode).toEqual(200);
        return response.body.roles;
    }

    async addRole(
        sidOrKey: string | 'masterKey',
        userId: string,
        role: string | string[],
        extraHeaders?: Record<string, string>
    ): Promise<void> {
        if (role instanceof Array) {
            for (const r of role) {
                await this.addRole(sidOrKey, userId, r, extraHeaders);
            }
        } else {
            let response = await this.request
                .addRole(sidOrKey, userId, role)
                .set(extraHeaders ?? {})
                .send();
            expect(response.statusCode).toEqual(200);
        }
    }
}
