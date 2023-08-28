export class Config {
    appDomain = 'scytta-test.com';
    serviceDomain = 'cloud.scytta-test.com';
    serviceUrl = 'http://cloud.scytta-test.com';
    identityUrl = 'http://cloud.scytta-test.com/identity';

    defaultRedirects = {
        loginUrl: 'http://login.com/',
        redirectUrl: 'http://redirect.com/',
        errorUrl: 'http://error.com/'
    };
}
