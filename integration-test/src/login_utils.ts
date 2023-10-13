import * as request from 'superagent';
import { getPageRedirectUrl } from '$lib/page_utils';
import config from '../test.config';
import { Cookie } from 'tough-cookie';
import { getCookies } from './auth_utils';
import { ExternalUser } from './models/external_user';

export async function createGuestUser(): Promise<Record<string, Cookie>> {
    const response = await request
        .get(config.getUrlFor('identity/auth/token/login'))
        .query({ rememberMe: true, ...config.defaultRedirects })
        //.use(requestLogger)
        .send();

    expect(response.statusCode).toEqual(200);
    expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);
    const cookies = getCookies(response);
    expect(cookies.tid).toBeValidTID();
    expect(cookies.sid).toBeValidSID();
    expect(cookies.eid).toBeClearCookie();
    return cookies;
}

export async function loginWithToken(tid: Cookie): Promise<Record<string, Cookie>> {
    const response = await request
        .get(config.getUrlFor('identity/auth/token/login'))
        .query(config.defaultRedirects)
        .set('Cookie', [`tid=${tid.value}`])
        //.use(requestLogger)
        .send();

    expect(response.statusCode).toEqual(200);
    expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

    const cookies = getCookies(response);
    expect(cookies.tid).toBeValidTID();
    expect(cookies.tid.value).toEqual(tid.value);
    expect(cookies.sid).toBeValidSID();
    expect(cookies.eid).toBeClearCookie();
    return cookies;
}

type StartLoginResult = {
    authParams: any;
    eid: Cookie;
};

export async function startLoginWithOAuth2(rememberMe?: boolean): Promise<StartLoginResult> {
    const response = await request
        .get(config.getUrlFor('identity/auth/oauth2_flow/login'))
        .query({ rememberMe: rememberMe, ...config.defaultRedirects })
        //.use(requestLogger)
        .send();

    expect(response.statusCode).toEqual(200);
    const redirectUrl = getPageRedirectUrl(response.text);
    expect(redirectUrl).toStartWith('http://mock.localhost.com:8090/oauth2/authorize');

    const cookies = getCookies(response);
    expect(cookies.tid).toBeClearCookie();
    expect(cookies.sid).toBeClearCookie();
    expect(cookies.eid).toBeValidEID();

    const authParams = redirectUrl!.parseQueryParamsFromUrl();
    return { authParams, eid: cookies.eid };
}

export async function loginWithOAuth2(
    user: ExternalUser,
    rememberMe?: boolean
): Promise<Record<string, Cookie>> {
    const { authParams, eid } = await startLoginWithOAuth2(rememberMe);

    const response = await request
        .get(config.getUrlFor('identity/auth/oauth2_flow/auth'))
        .query({
            code: user.toCode(),
            state: authParams.state
        })
        .set('Cookie', [`eid=${eid.value}`])
        //.use(requestLogger)
        .send();

    expect(response.statusCode).toEqual(200);
    expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);
    const cookies = getCookies(response);
    if (rememberMe) {
        expect(cookies.tid).toBeValidTID();
    } else {
        expect(cookies.tid).toBeClearCookie();
    }
    expect(cookies.sid).toBeValidSID();
    expect(cookies.eid).toBeClearCookie();
    return cookies;
}

export async function startLoginWithOpenId(rememberMe?: boolean): Promise<StartLoginResult> {
    const response = await request
        .get(config.getUrlFor('identity/auth/openid_flow/login'))
        .query({ rememberMe: rememberMe, ...config.defaultRedirects })
        //.use(requestLogger)
        .send();

    expect(response.statusCode).toEqual(200);
    const redirectUrl = getPageRedirectUrl(response.text);
    expect(redirectUrl).toStartWith('http://mock.localhost.com:8090/openid/authorize');

    const cookies = getCookies(response);
    expect(cookies.tid).toBeClearCookie();
    expect(cookies.sid).toBeClearCookie();
    expect(cookies.eid).toBeValidEID();

    const authParams = redirectUrl!.parseQueryParamsFromUrl();
    return { authParams, eid: cookies.eid };
}

export async function loginWithOpenId(
    user: ExternalUser,
    rememberMe?: boolean
): Promise<Record<string, Cookie>> {
    const { authParams, eid } = await startLoginWithOpenId(rememberMe);

    const response = await request
        .get(config.getUrlFor('identity/auth/openid_flow/auth'))
        .query({
            code: user.toCode({ nonce: authParams.nonce }),
            state: authParams.state
        })
        .set('Cookie', [`eid=${eid.value}`])
        //.use(requestLogger)
        .send();

    expect(response.statusCode).toEqual(200);
    expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);
    const cookies = getCookies(response);
    if (rememberMe) {
        expect(cookies.tid).toBeValidTID();
    } else {
        expect(cookies.tid).toBeClearCookie();
    }
    expect(cookies.sid).toBeValidSID();
    expect(cookies.eid).toBeClearCookie();
    return cookies;
}
