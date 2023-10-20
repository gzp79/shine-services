import { createUrlQueryString, generateRandomString } from '$lib/string_utils';
import { createGuestUser } from './login_utils';
import { getUserInfo } from './auth_utils';
import { randomUUID } from 'crypto';

export interface UserInfo {
    userId: string;
    name: string;
    sessionLength: number;
    roles: string[];
}

export class ExternalUser {
    public readonly id: string;
    public readonly name: string;
    public readonly email: string;

    constructor(id: string, name: string, email: string) {
        this.id = id;
        this.name = name;
        this.email = email;
    }

    static newRandomUser(): ExternalUser {
        const name = 'Random_' + generateRandomString(5);
        return new ExternalUser(randomUUID(), name, name + '@example.com');
    }

    toCode(params?: any): string {
        return createUrlQueryString({
            id: this.id,
            name: this.name,
            email: this.email,
            ...params
        });
    }
}

export class TestUser {
    public userId: string;
    public name?: string;
    public roles: string[] = [];

    public tid?: string;
    public sid?: string;

    public constructor(userId: string) {
        this.userId = userId;
    }

    public static async create(roles: string[]): Promise<TestUser> {
        const cookies = await createGuestUser();
        //todo: add roles using api key - this is not a role test, only a way to get roles for a user
        const info = await getUserInfo(cookies.sid);
        const testUser = new TestUser(info.userId);
        testUser.name = info.name;
        testUser.roles = info.roles;
        testUser.sid = cookies.sid.value;
        testUser.tid = cookies.tid.value;
        return testUser;
    }

    public getCookies(): string[] {
        return [`sid=${this.sid}`];
    }
}
