function utils() {
    const appDomain = 'scytta-test.com';
    const serviceDomain = 'cloud.' + appDomain;
    const serviceUrl = 'http://' + serviceDomain;
    const identityUrl = serviceUrl + '/identity';

    const utils = {        
        appDomain: appDomain,
        serviceDomain: serviceDomain,
        serviceUrl: serviceUrl,
        identityUrl: identityUrl,

        defaultRedirects: {
            loginUrl: 'http://login.com/',
            redirectUrl: 'http://redirect.com/',
            errorUrl: 'http://error.com/'
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

    return utils;
}
