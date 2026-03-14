import { test as base, mergeExpects } from '@playwright/test';
import { ApiClient } from '$lib/api/api';
import { AuthAPI, DefaultRedirects } from '$lib/api/auth_api';
import { SessionAPI } from '$lib/api/session_api';
import { TestUser, TestUserHelper } from '$lib/api/test_user';
import { TokenAPI } from '$lib/api/token_api';
import { UserAPI } from '$lib/api/user_api';
import { expect as authExpect } from './expect/auth_exts';
import { expect as commonExpect } from './expect/common';
import { expect as mailExpect } from './expect/mail';
import { expect as responseExpect } from './expect/response';

export const expect = mergeExpects(commonExpect, responseExpect, authExpect, mailExpect);

export type ServiceOptions = {
    enableRequestLogging: boolean;
    appDomain: string;
    serviceDomain: string;

    homeUrl: string;
    linkUrl: string;
    identityUrl: string;
    builderUrl: string;

    skipMockService?: boolean;

    defaultRedirects: DefaultRedirects;

    masterAdminKey: string;
    adminUser: TestUser;
};

export type Api = {
    client: ApiClient;
    auth: AuthAPI;
    session: SessionAPI;
    token: TokenAPI;
    user: UserAPI;
    testUsers: TestUserHelper;
};

export type ServiceTestFixture = {
    api: Api;
};

export const test = base.extend<ServiceTestFixture, ServiceOptions>({
    appDomain: [undefined!, { scope: 'worker', option: true }],
    serviceDomain: [undefined!, { scope: 'worker', option: true }],
    homeUrl: [undefined!, { scope: 'worker', option: true }],
    linkUrl: [undefined!, { scope: 'worker', option: true }],
    identityUrl: [undefined!, { scope: 'worker', option: true }],
    builderUrl: [undefined!, { scope: 'worker', option: true }],
    masterAdminKey: [undefined!, { scope: 'worker', option: true }],
    defaultRedirects: [undefined!, { scope: 'worker', option: true }],
    skipMockService: [false, { scope: 'worker', option: true }],
    enableRequestLogging: [false, { scope: 'worker', option: true }],

    adminUser: [
        async ({ identityUrl, defaultRedirects, masterAdminKey, enableRequestLogging }, use) => {
            const auth = new AuthAPI(identityUrl, defaultRedirects, enableRequestLogging);
            const user = new UserAPI(identityUrl, masterAdminKey, enableRequestLogging);
            const testUsers = new TestUserHelper(auth, user);

            const admin = await testUsers.createGuest({ roles: ['SuperAdmin'] });

            await use(admin);
        },
        { scope: 'worker' }
    ],

    api: [
        async ({ identityUrl, defaultRedirects, masterAdminKey, enableRequestLogging }, use) => {
            const client = new ApiClient(enableRequestLogging);
            const auth = new AuthAPI(identityUrl, defaultRedirects, enableRequestLogging);
            const session = new SessionAPI(identityUrl, enableRequestLogging);
            const token = new TokenAPI(identityUrl, enableRequestLogging);
            const user = new UserAPI(identityUrl, masterAdminKey, enableRequestLogging);
            const testUsers = new TestUserHelper(auth, user);
            await use({
                client,
                auth,
                session,
                token,
                user,
                testUsers
            });
        },
        { scope: 'test' }
    ]
});
