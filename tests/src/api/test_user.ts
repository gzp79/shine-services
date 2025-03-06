import { expect } from '$fixtures/setup';
import MockSmtp from '$lib/mocks/mock_smtp';
import OAuth2MockServer from '$lib/mocks/oauth2';
import OpenIdMockServer from '$lib/mocks/openid';
import { generateRandomString } from '$lib/string_utils';
import { randomUUID } from 'crypto';
import { MockServer } from '../mocks/mock_server';
import { AuthAPI } from './auth_api';
import { ExternalUser } from './external_user';
import { UserAPI, UserInfo } from './user_api';
import { getEmailLinkToken } from './utils';

export class TestUser {
    public sid: string = '';
    public tid?: string;

    public externalUser?: ExternalUser;
    public userInfo?: UserInfo;

    public constructor(
        public readonly userId: string,
        private readonly authAPI: AuthAPI,
        private readonly userAPI: UserAPI
    ) {}

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
            const newCookies = await this.authAPI.loginWithToken(this.tid, null, extraHeaders);
            this.tid = newCookies.tid;
        }
    }

    public async refreshUserInfo(extraHeaders?: Record<string, string>) {
        const info = await this.userAPI.getUserInfo(this.sid, extraHeaders);
        this.userInfo = info;
    }

    public async confirmEmail(smtp: MockSmtp) {
        const mailPromise = smtp.waitMail();
        await this.userAPI.startConfirmEmail(this.sid);
        const mail = await mailPromise;

        const token = getEmailLinkToken(mail);
        expect(token).toBeString();
        await this.userAPI.completeConfirmEmail(this.sid, token!);

        await this.refreshUserInfo();
        expect(this.userInfo?.isEmailConfirmed).toBe(true);
    }

    public async changeEmail(smtp: MockSmtp, newEmail: string) {
        const mailPromise = smtp.waitMail();
        await this.userAPI.startChangeEmail(this.sid, newEmail);
        const mail = await mailPromise;

        const token = getEmailLinkToken(mail);
        expect(token).toBeString();
        await this.userAPI.completeConfirmEmail(this.sid, token!);

        await this.refreshUserInfo();
        expect(this.userInfo?.isEmailConfirmed).toBe(true);
        expect(this.userInfo?.email).toEqual(newEmail);
    }
}

export class TestUserHelper {
    constructor(
        public readonly authAPI: AuthAPI,
        public readonly userAPI: UserAPI
    ) {}

    public async createGuest(
        props?: {
            roles?: string[];
        },
        extraHeaders?: Record<string, string>
    ): Promise<TestUser> {
        const cookies = await this.authAPI.loginAsGuestUser(extraHeaders);
        {
            // add roles using api key
            const info = await this.userAPI.getUserInfo(cookies.sid, extraHeaders);
            await this.userAPI.addRole(cookies.sid, true, info.userId, props?.roles ?? [], extraHeaders);
        }
        const info = await this.userAPI.getUserInfo(cookies.sid, extraHeaders);
        const testUser = new TestUser(info.userId, this.authAPI, this.userAPI);
        testUser.userInfo = info;
        testUser.sid = cookies.sid;
        testUser.tid = cookies.tid;
        return testUser;
    }

    public async createLinked(
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
            cookies = await this.authAPI.loginWithOAuth2(mock, user, props?.rememberMe ?? false, extraHeaders);
        } else if (mock instanceof OpenIdMockServer) {
            cookies = await this.authAPI.loginWithOpenId(mock, user, props?.rememberMe ?? false, extraHeaders);
        } else {
            throw new Error('Invalid mock server type');
        }

        {
            // add roles using api key
            const info = await this.userAPI.getUserInfo(cookies.sid, extraHeaders);
            await this.userAPI.addRole(cookies.sid, true, info.userId, props?.roles ?? [], extraHeaders);
        }

        const info = await this.userAPI.getUserInfo(cookies.sid, extraHeaders);
        const testUser = new TestUser(info.userId, this.authAPI, this.userAPI);
        testUser.externalUser = user;
        testUser.userInfo = info;
        testUser.sid = cookies.sid;
        testUser.tid = cookies.tid;
        return testUser;
    }
}
