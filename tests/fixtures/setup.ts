import { test as base, mergeExpects } from '@playwright/test';
import { ApiClient } from '$lib/api/api';
import { AuthAPI, DefaultRedirects } from '$lib/api/auth_api';
import { SessionAPI } from '$lib/api/session_api';
import { TestUser, TestUserHelper } from '$lib/api/test_user';
import { TokenAPI } from '$lib/api/token_api';
import { UserAPI } from '$lib/api/user_api';
import OAuth2MockServer from '$lib/mocks/oauth2';
import { SharedServiceName, waitForCondition } from '$lib/utils';
import { Toxiproxy } from 'toxiproxy-node-client';
import { expect as authExpect } from './expect/auth_exts';
import { expect as commonExpect } from './expect/common';
import { expect as mailExpect } from './expect/mail';
import { expect as responseExpect } from './expect/response';

export const expect = mergeExpects(commonExpect, responseExpect, authExpect, mailExpect);

type ProxyState = { enabled: boolean; toxics: unknown[] };

async function hasInjectedFault(toxiproxyUrl: string): Promise<boolean> {
    const response = await fetch(`${toxiproxyUrl}/proxies`);
    const proxies = (await response.json()) as Record<string, ProxyState>;
    return Object.values(proxies).some((proxy) => !proxy.enabled || proxy.toxics.length > 0);
}

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
    toxiproxy: Toxiproxy;
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

    // Every service reaches its database through toxiproxy, hence a fault left behind by any test
    // breaks the whole suite, not just the fault-injection tests. Restore a known good state both
    // before and after each test.
    toxiproxy: [
        async ({ toxiproxyUrl, identityUrl, defaultRedirects, enableRequestLogging }, use) => {
            const toxiproxy = new Toxiproxy(toxiproxyUrl);

            // A test that restores its proxy itself still leaves the services with broken
            // connections, so the state of toxiproxy alone cannot tell whether a fault was
            // injected. Track whether the test reached for a proxy at all.
            let faultInjected = false;
            const getProxy = toxiproxy.get.bind(toxiproxy);
            toxiproxy.get = async (name: string) => {
                faultInjected = true;
                return await getProxy(name);
            };

            const restore = async (faulted: boolean) => {
                faulted = faulted || (await hasInjectedFault(toxiproxyUrl));
                await toxiproxy.reset();
                if (faulted) {
                    // Cutting the proxied connections leaves dead sockets in the service pools that
                    // are evicted only once a request stumbles on them. Drain them with a request
                    // covering both the database and the cache, so the fault stops here.
                    const auth = new AuthAPI(identityUrl, defaultRedirects, enableRequestLogging);
                    await waitForCondition(async () => await auth.loginAsGuestUser(), {
                        timeout: 15000,
                        errorMessage: 'Service did not recover from the injected fault'
                    });
                }
            };

            await restore(false);
            await use(toxiproxy);
            await restore(faultInjected);
        },
        { scope: 'test', auto: true }
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
