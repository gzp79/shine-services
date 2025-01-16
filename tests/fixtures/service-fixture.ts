import { AuthAPIEx, DefaultRedirects } from '$lib/api/auth_api';
//import { ExternalLinkAPI } from '$lib/api/external_link_api';
//import { SessionAPI } from '$lib/api/session_api';
//import { TokenAPI } from '$lib/api/token_api';
//import { UserAPI } from '$lib/api/user_api';
import { APIRequest, APIRequestContext, test as base, mergeExpects, request } from '@playwright/test';
import { expect as commonExpect } from './expect/common';
import { expect as responseExpect } from './expect/response';

export const expect = mergeExpects(commonExpect, responseExpect);

export type ServiceOptions = {
    appDomain: string;
    serviceDomain: string;

    identityUrl: string;
    builderUrl: string;

    defaultRedirects: DefaultRedirects;
};

export type ServiceTestFixture = {
    apis: {
        auth: AuthAPIEx;
        //externalLink: ExternalLinkAPI;
        //session: SessionAPI;
        //token: TokenAPI;
        //user: UserAPI;
    };
};

// export type ServiceWorkerFixture = {};

export const test = base.extend<ServiceTestFixture, ServiceOptions /*& ServiceWorkerFixture*/>({
    appDomain: [undefined, { scope: 'worker', option: true }],
    serviceDomain: [undefined, { scope: 'worker', option: true }],
    identityUrl: [undefined, { scope: 'worker', option: true }],
    builderUrl: [undefined, { scope: 'worker', option: true }],
    defaultRedirects: [undefined, { scope: 'worker', option: true }],

    apis: [
        async ({ identityUrl, defaultRedirects }, use) => {
            const auth = new AuthAPIEx(await request.newContext(), identityUrl, defaultRedirects);
            await use({
                auth
            });
        },
        { scope: 'test' }
    ]
});
