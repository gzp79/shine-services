import { PlaywrightTestConfig } from '@playwright/test';
import { ServiceOptions } from '@fixtures/service-fixture';

// Allow self-signed certificates
process.env.NODE_TLS_REJECT_UNAUTHORIZED = '0';

const isBuildRun: boolean = !!process.env.CI;
if (isBuildRun) {
    console.log('Running in CI mode');
}

const config: PlaywrightTestConfig<ServiceOptions> = {
    testDir: './',
    fullyParallel: true,
    forbidOnly: isBuildRun,
    retries: isBuildRun ? 2 : 0,
    workers: isBuildRun ? 1 : undefined,

    reporter: [
        ['list'],
        ['html', { outputFolder: 'reports/' }]],

    use: {
        trace: 'on-first-retry'
    },

    projects: [
        {
            name: 'local',
            testMatch: 'api-tests/**/*.ts',
            use: {
                appDomain: 'local-scytta.com',
                serviceDomain: 'cloud.local-scytta.com',

                identityUrl: 'https://cloud.local-scytta.com:8443/identity',
                builderUrl: 'https://cloud.local-scytta.com:8444/identity',

                defaultRedirects: {
                    loginUrl: 'https://login.com/',
                    redirectUrl: 'https://redirect.com/',
                    errorUrl: 'https://error.com/'
                }
            }
        }
    ]
};

export default config;
