function fn() {
    const appDomain = 'scytta-test.com';
    const serviceDomain = 'cloud.' + appDomain;
    const serviceUrl = 'http://' + serviceDomain;
    const identityUrl = serviceUrl + '/identity';

    const utils = {
        SESSION_SCOPE: -9223372036854775808,
        appDomain: appDomain,
        serviceDomain: serviceDomain,
        serviceUrl: serviceUrl,
        identityUrl: identityUrl,

        defaultRedirects: {
            loginUrl: 'http://login.com/',
            redirectUrl: 'http://redirect.com/',
            errorUrl: 'http://error.com/'
        },

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
        },

        uuid: function () {
            const UUID = Java.type('java.util.UUID');
            return UUID.randomUUID().toString();
        },

        getRandomString: function (len, alpha) {
            const characters = alpha ?? 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
            let result = '';
            for (i = 0; i < len; i++) {
                result += characters.charAt(
                    Math.floor(Math.random() * characters.length)
                );
            }
            return result;
        },

        getRedirectUrl: function (response) {
            return /.*<meta http-equiv[^>]*url='([^']*)'[^>]*>.*/.exec(
                response
            )[1];
        },

        parseQueryParams: function (queryString) {
            let o = {};
            queryString
                .split('&')
                .map((x) => x.split('='))
                .forEach((x) => (o[x[0]] = karate.urlDecode(x[1])));
            return o;
        },

        getUrlQueryParams: function (url) {
            return utils.parseQueryParams(url.split('?')[1]);
        },

        createUrlQueryString: function (params) {
            const p = [];
            for (const k in params) {
                p.push(`${k}=${karate.urlEncode(params[k])}`);
            }
            return p.join('&');
        }
    };

    karate.properties['utils'] = utils;
    karate.configure('logPrettyResponse', true);
}
