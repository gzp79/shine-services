import request from 'superagent';
import { getPageRedirectUrl } from '$lib/page_utils';
import config from '../test.config';
import { Cookie } from 'tough-cookie';
import { getCookies } from './auth_utils';
import { ExternalUser } from './user';

export async function createGuestUser(
    extraHeaders?: Record<string, string>
): Promise<Record<string, Cookie>> {
    const response = await request
        .get(config.getUrlFor('identity/auth/token/login'))
        .query({ rememberMe: true, ...config.defaultRedirects })
        .set(extraHeaders ?? {})
        .send()
        .catch((err) => err.response);

    expect(response.statusCode).toEqual(200);
    expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);
    const cookies = getCookies(response);
    expect(cookies.tid).toBeValidTID();
    expect(cookies.sid).toBeValidSID();
    expect(cookies.eid).toBeClearCookie();
    return cookies;
}

export async function requestLoginWithToken(
    tid: string,
    extraHeaders?: Record<string, string>
): Promise<any> {
    return await request
        .get(config.getUrlFor('identity/auth/token/login'))
        .query(config.defaultRedirects)
        .set('Cookie', [`tid=${tid}`])
        .set(extraHeaders ?? {})
        .send()
        .catch((err) => err.response);
}

export async function loginWithToken(
    tid: string,
    extraHeaders?: Record<string, string>
): Promise<Record<string, Cookie>> {
    const response = await requestLoginWithToken(tid, extraHeaders);

    expect(response.statusCode).toEqual(200);
    expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);

    const cookies = getCookies(response);
    expect(cookies.tid).toBeValidTID();
    expect(cookies.tid.value).toEqual(tid);
    expect(cookies.sid).toBeValidSID();
    expect(cookies.eid).toBeClearCookie();
    return cookies;
}

type StartLoginResult = {
    authParams: any;
    eid: Cookie;
};

export async function requestStartLoginWithOAuth2(
    rememberMe?: boolean,
    extraHeaders?: Record<string, string>
): Promise<any> {
    return await request
        .get(config.getUrlFor('identity/auth/oauth2_flow/login'))
        .query({ rememberMe: rememberMe, ...config.defaultRedirects })
        .set(extraHeaders ?? {})
        .send()
        .catch((err) => err.response);
}

export async function startLoginWithOAuth2(
    rememberMe?: boolean,
    extraHeaders?: Record<string, string>
): Promise<StartLoginResult> {
    const response = await requestStartLoginWithOAuth2(rememberMe, extraHeaders);

    expect(response.statusCode).toEqual(200);
    const redirectUrl = getPageRedirectUrl(response.text);
    expect(redirectUrl).toStartWith(config.getMockUrlFor('oauth2/authorize'));

    const cookies = getCookies(response);
    expect(cookies.tid).toBeClearCookie();
    expect(cookies.sid).toBeClearCookie();
    expect(cookies.eid).toBeValidEID();

    const authParams = redirectUrl!.parseQueryParamsFromUrl();
    return { authParams, eid: cookies.eid };
}

export async function requestLoginWithOAuth2(
    user: ExternalUser,
    rememberMe?: boolean,
    extraHeaders?: Record<string, string>
): Promise<any> {
    const start = await startLoginWithOAuth2(rememberMe, extraHeaders);

    return await request
        .get(config.getUrlFor('identity/auth/oauth2_flow/auth'))
        .query({
            code: user.toCode(),
            state: start.authParams.state
        })
        .set('Cookie', [`eid=${start.eid.value}`])
        .set(extraHeaders ?? {})
        .send()
        .catch((err) => err.response);
}

export async function loginWithOAuth2(
    user: ExternalUser,
    rememberMe?: boolean,
    extraHeaders?: Record<string, string>
): Promise<Record<string, Cookie>> {
    const response = await requestLoginWithOAuth2(user, rememberMe, extraHeaders);

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

export async function requestStartLinkWithOAuth2(
    sid: string,
    extraHeaders?: Record<string, string>
): Promise<any> {
    return await request
        .get(config.getUrlFor('identity/auth/oauth2_flow/link'))
        .query({ ...config.defaultRedirects })
        .set('Cookie', [`sid=${sid}`])
        .set(extraHeaders ?? {})
        .send()
        .catch((err) => err.response);
}

export async function startLinkWithOAuth2(
    sid: string,
    extraHeaders?: Record<string, string>
): Promise<StartLoginResult> {
    const response = await requestStartLinkWithOAuth2(sid, extraHeaders);

    expect(response.statusCode).toEqual(200);
    const redirectUrl = getPageRedirectUrl(response.text);
    expect(redirectUrl).toStartWith(config.getMockUrlFor('oauth2/authorize'));

    const cookies = getCookies(response);
    expect(cookies.tid).toBeClearCookie();
    expect(cookies.sid).toBeValidSID();
    expect(cookies.sid.value).toEqual(sid);
    expect(cookies.eid).toBeValidEID();

    const authParams = redirectUrl!.parseQueryParamsFromUrl();
    return { authParams, eid: cookies.eid };
}

export async function requestLinkWithOAuth2(
    sid: string,
    user: ExternalUser,
    extraHeaders?: Record<string, string>
): Promise<any> {
    const { authParams, eid } = await startLinkWithOAuth2(sid, extraHeaders);

    return await request
        .get(config.getUrlFor('identity/auth/oauth2_flow/auth'))
        .query({
            code: user.toCode(),
            state: authParams.state
        })
        .set('Cookie', [`eid=${eid.value}`, `sid=${sid}`])
        .set(extraHeaders ?? {})
        .send()
        .catch((err) => err.response);
}

export async function linkWithOAuth2(
    sid: string,
    user: ExternalUser,
    extraHeaders?: Record<string, string>
): Promise<Record<string, Cookie>> {
    const response = await requestLinkWithOAuth2(sid, user, extraHeaders);

    expect(response.statusCode).toEqual(200);
    expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);
    const cookies = getCookies(response);
    expect(cookies.tid).toBeClearCookie();
    expect(cookies.sid).toBeValidSID();
    expect(cookies.eid).toBeClearCookie();
    return cookies;
}

export async function requestStartLoginWithOpenId(
    rememberMe?: boolean,
    extraHeaders?: Record<string, string>
): Promise<any> {
    return await request
        .get(config.getUrlFor('identity/auth/openid_flow/login'))
        .query({ rememberMe: rememberMe, ...config.defaultRedirects })
        .set(extraHeaders ?? {})
        .send()
        .catch((err) => err.response);
}

export async function startLoginWithOpenId(
    rememberMe?: boolean,
    extraHeaders?: Record<string, string>
): Promise<StartLoginResult> {
    const response = await requestStartLoginWithOpenId(rememberMe, extraHeaders);
    expect(response.statusCode).toEqual(200);
    const redirectUrl = getPageRedirectUrl(response.text);
    expect(redirectUrl).toStartWith(config.getMockUrlFor('openid/authorize'));

    const cookies = getCookies(response);
    expect(cookies.tid).toBeClearCookie();
    expect(cookies.sid).toBeClearCookie();
    expect(cookies.eid).toBeValidEID();

    const authParams = redirectUrl!.parseQueryParamsFromUrl();
    return { authParams, eid: cookies.eid };
}

export async function requestLoginWithOpenId(
    user: ExternalUser,
    rememberMe?: boolean,
    extraHeaders?: Record<string, string>
): Promise<any> {
    const { authParams, eid } = await startLoginWithOpenId(rememberMe);

    return await request
        .get(config.getUrlFor('identity/auth/openid_flow/auth'))
        .query({
            code: user.toCode({ nonce: authParams.nonce }),
            state: authParams.state
        })
        .set('Cookie', [`eid=${eid.value}`])
        .set(extraHeaders ?? {})
        .send()
        .catch((err) => err.response);
}

export async function loginWithOpenId(
    user: ExternalUser,
    rememberMe?: boolean,
    extraHeaders?: Record<string, string>
): Promise<Record<string, Cookie>> {
    const response = await requestLoginWithOpenId(user, rememberMe, extraHeaders);

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

export async function requestStartLinkWithOpenId(
    sid: string,
    extraHeaders?: Record<string, string>
): Promise<any> {
    return await request
        .get(config.getUrlFor('identity/auth/openid_flow/link'))
        .query({ ...config.defaultRedirects })
        .set('Cookie', [`sid=${sid}`])
        .set(extraHeaders ?? {})
        .send()
        .catch((err) => err.response);
}

export async function startLinkWithOpenId(
    sid: string,
    extraHeaders?: Record<string, string>
): Promise<StartLoginResult> {
    const response = await requestStartLinkWithOpenId(sid, extraHeaders);

    expect(response.statusCode).toEqual(200);
    const redirectUrl = getPageRedirectUrl(response.text);
    expect(redirectUrl).toStartWith(config.getMockUrlFor('openid/authorize'));

    const cookies = getCookies(response);
    expect(cookies.tid).toBeClearCookie();
    expect(cookies.sid).toBeValidSID();
    expect(cookies.sid.value).toEqual(sid);
    expect(cookies.eid).toBeValidEID();

    const authParams = redirectUrl!.parseQueryParamsFromUrl();
    return { authParams, eid: cookies.eid };
}

export async function requestLinkWithOpenId(
    sid: string,
    user: ExternalUser,
    extraHeaders?: Record<string, string>
): Promise<any> {
    const { authParams, eid } = await startLinkWithOpenId(sid);

    return await request
        .get(config.getUrlFor('identity/auth/openid_flow/auth'))
        .query({
            code: user.toCode({ nonce: authParams.nonce }),
            state: authParams.state
        })
        .set('Cookie', [`eid=${eid.value}`, `sid=${sid}`])
        .set(extraHeaders ?? {})
        .send()
        .catch((err) => err.response);
}

export async function linkWithOpenId(
    sid: string,
    user: ExternalUser,
    extraHeaders?: Record<string, string>
): Promise<Record<string, Cookie>> {
    const response = await requestLinkWithOpenId(sid, user, extraHeaders);

    expect(response.statusCode).toEqual(200);
    expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);
    const cookies = getCookies(response);
    expect(cookies.tid).toBeClearCookie();
    expect(cookies.sid).toBeValidSID();
    expect(cookies.sid.value).toEqual(sid);
    expect(cookies.eid).toBeClearCookie();
    return cookies;
}
