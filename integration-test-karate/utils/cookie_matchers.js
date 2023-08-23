function cookie_matchers() {
    const utils = require('utils.js');
    utils = utils();

    const appDomain = utils.appDomain;
    const serviceDomain = utils.serviceDomain;

    const matchers = {
        SESSION_SCOPE: -9223372036854775808,
        
        matchAuthCookiesValidate: {
            tid: {
                path: '/identity/auth',
                domain: serviceDomain,
                httponly: true,
                secure: true,
                value: '#notnull',
                samesite: 'Lax'
            },
            sid: {
                path: '/',
                domain: appDomain,
                httponly: true,
                secure: true,
                value: '#notnull',
                samesite: 'Lax'
            },
            eid: {
                path: '/identity/auth',
                domain: serviceDomain,
                httponly: true,
                secure: true,
                value: '#notnull',
                samesite: 'Lax'
            }
        },
        get matchClearAuthCookies() {
            return {
                ...utils.matchAuthCookiesValidate,
                tid: { 'max-age': '#? _ < 0' },
                sid: { 'max-age': '#? _ < 0' },
                eid: { 'max-age': '#? _ < 0' }
            };
        }
    };

    return matchers;
}
