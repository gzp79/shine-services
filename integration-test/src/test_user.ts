import { randomUUID } from 'crypto';
import OAuth2MockServer from '$lib/mocks/oauth2';
import OpenIdMockServer from '$lib/mocks/openid';
import { generateRandomString } from '$lib/string_utils';
import api from './api/api';
import { ExternalUser } from './api/external_user';
import { UserInfo } from './api/user_api';
import { MockServer } from './mock_server';

export class TestUser {
    public userId: string;

    public sid: string = '';
    public tid?: string;

    public externalUser?: ExternalUser;
    public userInfo?: UserInfo;

    public constructor(userId: string) {
        this.userId = userId;
    }

    public static async createGuest(
        props?: {
            roles?: string[];
        },
        extraHeaders?: Record<string, string>
    ): Promise<TestUser> {
        const cookies = await api.auth.loginAsGuestUser(extraHeaders);
        {
            // add roles using api key
            const info = await api.user.getUserInfo(cookies.sid, extraHeaders);
            await api.user.addRole(cookies.sid, true, info.userId, props?.roles ?? [], extraHeaders);
        }
        const info = await api.user.getUserInfo(cookies.sid, extraHeaders);
        const testUser = new TestUser(info.userId);
        testUser.userInfo = info;
        testUser.sid = cookies.sid;
        testUser.tid = cookies.tid;
        return testUser;
    }

    public static async createLinked(
        mock: MockServer,
        props?: {
            roles?: string[];
            name?: string;
            email?: string;
            rememberMe?: boolean;
        },
        extraHeaders?: Record<string, string>
    ): Promise<TestUser> {
        const id = randomUUID().toString();
        const name = props?.name ?? 'Random_' + generateRandomString(5);
        const email = props?.email ?? name + '@example.com';
        const user = new ExternalUser(id, name, email);

        let cookies;
        if (mock instanceof OAuth2MockServer) {
            cookies = await api.auth.loginWithOAuth2(mock, user, props?.rememberMe ?? false, extraHeaders);
        } else if (mock instanceof OpenIdMockServer) {
            cookies = await api.auth.loginWithOpenId(mock, user, props?.rememberMe ?? false, extraHeaders);
        } else {
            throw new Error('Invalid mock server type');
        }

        {
            // add roles using api key
            const info = await api.user.getUserInfo(cookies.sid, extraHeaders);
            await api.user.addRole(cookies.sid, true, info.userId, props?.roles ?? [], extraHeaders);
        }

        const info = await api.user.getUserInfo(cookies.sid, extraHeaders);
        const testUser = new TestUser(info.userId);
        testUser.externalUser = user;
        testUser.userInfo = info;
        testUser.sid = cookies.sid;
        testUser.tid = cookies.tid;
        return testUser;
    }

    public get name(): string {
        return this.userInfo?.name ?? '';
    }

    public get roles(): string[] {
        return this.userInfo?.roles ?? [];
    }

    public get isLinked(): boolean {
        return this.userInfo?.isLinked ?? false;
    }

    public async rotateTID(extraHeaders?: Record<string, string>) {
        if (this.tid) {
            const newCookies = await api.auth.loginWithToken(this.tid, null, extraHeaders);
            this.tid = newCookies.tid;
        }
    }

    public async refreshUserInfo(extraHeaders?: Record<string, string>) {
        const info = await api.user.getUserInfo(this.sid, extraHeaders);
        this.userInfo = info;
    }
}
