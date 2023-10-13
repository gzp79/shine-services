export class Config {
    appDomain = 'scytta-test.com';
    serviceDomain = 'cloud.scytta-test.com';
    serviceUrl = 'http://cloud.scytta-test.com:7080';
    identityUrl = 'http://cloud.scytta-test.com:7080/identity';

    getUrlFor(path: string): string {
        return new URL(path, this.serviceUrl).toString();
    }

    defaultRedirects = {
        loginUrl: 'http://login.com/',
        redirectUrl: 'http://redirect.com/',
        errorUrl: 'http://error.com/'
    };

    openid_mock_url = 'http://mock.localhost.com:8090/openid';

    // new key set can be generated at https://mkjwk.org/ quite easily
    // (RSA, size:2048 (smaller is rejected by the jose module as of specification), Use:Signature, Alg:RS256, ID:Sha-1 )
    //spell-checker:disable
    openid_jwks = {
        p: '21pzZgFcZqxR3CXwJ4uaXhAZHPHCi2MdNwe6MFUr8i85ehj9-za1qlnW1Jb5XmusJQhu-iFMPhlR0h51n5rM_O_XRVBSp9uu-yh-cAYNwYFxMbtlkXvCnRhpAwKimNehokJ2YyRpLlW6Kn47dd3JjxYH3DRBBSPohQnHNzozARU',
        kty: 'RSA',
        q: 'xW3XRPacjFnGXt6x1RbFV48wIGfeYEAKrFPbcQRL2uY1pq2htGDmso8umEK7lIFUFonqBJKR3dw8t3NuQN8P9rZSGdXVhQ52DKnKvLAQT4IKoyXOGdOuugBbRh57VEpTw8fMfyzdJwccLmWSTPtVj_0GCa6T6oZCDCDuEnPJfPk',
        d: 'cHehvcojcKjS6pkdmCjHsWJGHiOunw0PHSArkvEKTZIekw_nekfYYKw7BPt4ZH6NeD9A-s0v_y0lwvQ7_OPtj1BUlicgPnOIfvzEaYdCr2Qx9XYWyqHKJANZ9FGUAFxFzVI1xnKB6sUC1zt3PiiJZXsq3-LL5ke6OGA3G6g2e0a8I67bQbbZd_TOe8Jh0N5IUyfnkv8jYiC5waNjZSVY9_DZE2rSZ-CmIhypUTTUfXhgNxciZGMMB3mtzMG3vR_kUv-VooXqsWgecUu9Af97maSBwoC2MessJ7VvvR553ZeYkfoCsRs8k1au2O3qLW6TON6QVZr1D602nQ0murgUIQ',
        e: 'AQAB',
        use: 'sig',
        qi: 'sX4jokfUgFeUBjTBQA7mFZ6Hg8dcIidDcSa11heUb9TZt24oR-c3wsWT11cOdT6-wjEL9b-H0UZd3iC8YjwNBu6cHwQJb9sJ3-ZLSRSQJ0HuAozhMuB4n-7Oewzb63AHgwuBSb_gwxWl0X-KYERYxK7vtu38PnHFjxWCeyqtYJc',
        dp: 'gV5rSPHsiTGAZhKJ_Qi81lUwOn3re0HNbTNFgFP7Qy7O-0_aG1s88Wdi6KbSE_n04TKEIUmaKdXNB9unC6bE1zitAdhJp25NWRuc1nz7h_DLzcT0NkWDlhtbc8cOFo62aXhBUl-bGRS-Y2lnsDBKO_WGVT0MS_fNnwkRUWUlx7E',
        alg: 'RS256',
        dq: 'nycH1Vk0I9QvHMVK-CtuFEKimk0BL_gQYpELIlVDTQgtkdsAsyc2chUIi8en7XRANBcjZmI9YmsrKvvLklH_TXP2RUti3-sjcNvjSi5oR5_eMVzFg35oqRqmeaUS6IUud3H2QUMKWG7b4e8RfCtT80oWdvGb3gAy-BIHuSpL8Ak',
        n: 'qSq4xK-7D9wEIgfo1athchJvLZMn0oWh8lRXL8zwED4FtMX4nxqLGU8oir8E__Pic3sOn9ZS-bnRMlXJkIS0uZT1zBIoU6RQIfe2ScI6AaZ6QTTK5Viu10wy4S4wXdIyIInVSgnWcccrkWnrewxyj1pcZFzgzT1ZRD8BZ0roOxLefrCN0WOODABI4zTY-L5q0X5JpBk0jC1wk6YofQZYtEO4XU-wvHZIugKnjSsAvyRgcWZq1niH2_8tdnXrnvDlTnC6IZzRBjLrVW7nHu1KtiDAnwL3NRrsnW0wu1fjQCG_YUNCFRkIHwpnq5X8Zn7gsnvdTBAosJn9urnqmJ85bQ'
    };
    //spell-checker:enable
}

const config = new Config();
export default config;
