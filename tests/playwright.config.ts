import { PlaywrightTestConfig } from '@playwright/test';
import { ServiceOptions } from '$fixtures/setup';
import { suppress_tls_certificate_warning } from '$lib/suppress_tls_certificate_warning';

// Allow self-signed certificates

suppress_tls_certificate_warning();
process.env.NODE_TLS_REJECT_UNAUTHORIZED = '0';
process.env.DEBUG = 'test:*';

const isBuildRun: boolean = !!process.env.CI;
if (isBuildRun) {
    console.log('Running in CI mode');
}

const config: PlaywrightTestConfig<ServiceOptions> = {
    testDir: './',
    fullyParallel: true,
    forbidOnly: isBuildRun,
    retries: isBuildRun ? 2 : 0,

    // due to the mock-server's port usage we can't run more than one worker
    workers: 1,

    reporter: [['list'], ['html', { outputFolder: 'reports/' }]],

    use: {
        trace: 'on-first-retry'
    },

    projects: [
        {
            name: 'local',
            testMatch: '{mock-tests,api-tests}/**/*.ts',
            use: {
                appDomain: 'local.scytta.com',
                serviceDomain: 'cloud.local.scytta.com',

                homeUrl: 'https://local.scytta.com:4443',
                linkUrl: 'https://local.scytta.com:4443/link',
                identityUrl: 'https://cloud.local.scytta.com:8443/identity',
                builderUrl: 'https://cloud.local.scytta.com:8444/builder',

                defaultRedirects: {
                    redirectUrl: 'https://redirect.com/',
                    errorUrl: 'https://error.com/'
                },

                /* spell-checker: disable */
                masterAdminKey: '2vazg4Rwe2uKkHABcbL8WdEAbqvPA49M' // hash: '$2b$05$0OWeMQAQuh9kmD642a0ZHeVl6VNa2g.z1HTI2rrQ3RPkmxoCNUohG';
                /* spell-checker: enable */
            }
        }
    ]
};

export default config;
