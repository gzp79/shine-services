import { test as base, mergeExpects } from '@playwright/test';
import { ApiClient } from '$lib/api/api';
import { AuthAPI, DefaultRedirects } from '$lib/api/auth_api';
import { SessionAPI } from '$lib/api/session_api';
import { TestUser, TestUserHelper } from '$lib/api/test_user';
import { TokenAPI } from '$lib/api/token_api';
import { UserAPI } from '$lib/api/user_api';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { SharedServiceName } from '$lib/utils';
import { expect as authExpect } from './expect/auth_exts';
import { expect as commonExpect } from './expect/common';
import { expect as mailExpect } from './expect/mail';
import { expect as responseExpect } from './expect/response';
import { InfraOutage, OutageService } from './infra_outage';

export const expect = mergeExpects(commonExpect, responseExpect, authExpect, mailExpect);

export type ServiceOptions = {
    enableRequestLogging: boolean;

    homeUrl: string;
    identityUrl: string;
    builderUrl: string;
    builderWSUrl: string;
    toxiproxyUrl: string;

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
    serviceUrl: (serviceName: SharedServiceName) => string;
    /**
     * Breaks the database or the cache of the given services for the test, see {@link InfraOutage}. The services
     * have to work again by the end of the test.
     */
    infraOutage: (...services: [OutageService, ...OutageService[]]) => InfraOutage;
};

export const test = base.extend<ServiceTestFixture, ServiceOptions>({
    homeUrl: [undefined!, { scope: 'worker', option: true }],
    identityUrl: [undefined!, { scope: 'worker', option: true }],
    builderUrl: [undefined!, { scope: 'worker', option: true }],
    builderWSUrl: [undefined!, { scope: 'worker', option: true }],
    toxiproxyUrl: [undefined!, { scope: 'worker', option: true }],
    masterAdminKey: [undefined!, { scope: 'worker', option: true }],
    defaultRedirects: [undefined!, { scope: 'worker', option: true }],
    skipMockService: [false, { scope: 'worker', option: true }],
    enableRequestLogging: [false, { scope: 'worker', option: true }],

    adminUser: [
        async ({ identityUrl, defaultRedirects, masterAdminKey, enableRequestLogging }, use) => {
            const auth = new AuthAPI(identityUrl, defaultRedirects, enableRequestLogging);
            const user = new UserAPI(identityUrl, masterAdminKey, enableRequestLogging);
            const testUsers = new TestUserHelper(auth, user);

            const mock = new OAuth2MockServer();
            await mock.start();
            let admin: TestUser;
            try {
                admin = await testUsers.createLinked(mock, { roles: ['SuperAdmin'] });
            } finally {
                await mock.stop();
            }

            await use(admin!);
        },
        { scope: 'worker' }
    ],

    infraOutage: [
        async ({ toxiproxyUrl, homeUrl, identityUrl, builderWSUrl, defaultRedirects, enableRequestLogging }, use) => {
            const outages: InfraOutage[] = [];
            await use((...services) => {
                const outage = new InfraOutage(
                    { toxiproxyUrl, homeUrl, identityUrl, builderWSUrl, defaultRedirects, enableRequestLogging },
                    services
                );
                outages.push(outage);
                return outage;
            });

            // Whatever happened in the test, the outage is cleared and the services have to work again.
            for (const outage of outages) {
                await outage.dispose();
            }
        },
        { scope: 'test' }
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
    ],

    serviceUrl: [
        async ({ identityUrl, builderUrl }, use) => {
            await use((serviceName: SharedServiceName): string => {
                switch (serviceName) {
                    case 'identity':
                        return identityUrl;
                    case 'builder':
                        return builderUrl;
                }
            });
        },
        { scope: 'test' }
    ]
});
