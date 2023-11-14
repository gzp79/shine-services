import request from 'superagent';
import config from '../test.config';
import { createUrlQueryString, generateRandomString } from '$lib/string_utils';
import { createGuestUser, loginWithOAuth2 } from './login_utils';
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
    public externalUser?: ExternalUser;

    public userId: string;
    public name?: string;
    public roles: string[] = [];

    public sid: string = '';
    public tid?: string;

    public constructor(userId: string) {
        this.userId = userId;
    }

    public static async createGuest(props?: {
        roles?: string[];
        extraHeaders?: Record<string, string>;
    }): Promise<TestUser> {
        const cookies = await createGuestUser(props?.extraHeaders);
        {
            // add roles using api key
            const info = await getUserInfo(cookies.sid.value, props?.extraHeaders);

            for (const role of props?.roles ?? []) {
                const response = await request
                    .put(config.getUrlFor(`/identity/api/identities/${info.userId}/roles`))
                    .set('Cookie', [`sid=${cookies.sid.value}`])
                    .set('Authorization', `Bearer ${config.masterKey}`)
                    .type('json')
                    .send({ role: role });
                expect(response.statusCode).toEqual(200);
            }
        }
        const info = await getUserInfo(cookies.sid.value, props?.extraHeaders);
        const testUser = new TestUser(info.userId);
        testUser.name = info.name;
        testUser.roles = info.roles;
        testUser.sid = cookies.sid.value;
        testUser.tid = cookies.tid.value;
        return testUser;
    }

    public static async createLinked(props?: {
        roles?: string[];
        name?: string;
        email?: string;
        rememberMe?: boolean;
        extraHeaders?: Record<string, string>;
    }): Promise<TestUser> {
        const id = randomUUID().toString();
        const name = props?.name ?? 'Random_' + generateRandomString(5);
        const email = props?.email ?? name + '@example.com';
        const user = new ExternalUser(id, name, email);

        const cookies = await loginWithOAuth2(user, props?.rememberMe ?? false, props?.extraHeaders);
        {
            // add roles using api key
            const info = await getUserInfo(cookies.sid.value, props?.extraHeaders);

            for (const role of props?.roles ?? []) {
                const response = await request
                    .put(config.getUrlFor(`/identity/api/identities/${info.userId}/roles`))
                    .set('Cookie', [`sid=${cookies.sid.value}`])
                    .set('Authorization', `Bearer ${config.masterKey}`)
                    .type('json')
                    .send({ role: role });
                expect(response.statusCode).toEqual(200);
            }
        }
        const info = await getUserInfo(cookies.sid.value, props?.extraHeaders);
        const testUser = new TestUser(info.userId);
        testUser.externalUser = user;
        testUser.name = info.name;
        testUser.roles = info.roles;
        testUser.sid = cookies.sid.value;
        testUser.tid = cookies.tid?.value;
        return testUser;
    }

    public getCookies(): string[] {
        return [`sid=${this.sid}`];
    }
}
