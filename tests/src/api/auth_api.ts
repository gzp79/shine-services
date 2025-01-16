import { APIRequestContext, APIResponse, expect } from '@playwright/test';
import { joinURL } from '$lib/utils';

export type DefaultRedirects = {
    loginUrl: string;
    redirectUrl: string;
    errorUrl: string;
};

interface UserCookies {
    tid: string;
    sid: string;
    eid: string;
}

interface StartLoginResult extends UserCookies {
    authParams: any;
}

function getCaptchaQuery(captcha: string | null | undefined): object {
    if (captcha === null) {
        // use the "always pass" token
        return { captcha: '1x00000000000000000000AA' };
    } else if (captcha) {
        // use provided captcha
        return { captcha };
    }
    return {};
}

function cookieHeader(...lists: string[][]): object {
    return { Cookie: lists.flat().join('; ') };
}

function authHeader(token?: string): object {
    return token ? { Authorization: `Bearer ${token}` } : {};
}

export class AuthAPI {
    constructor(
        protected request: APIRequestContext,
        protected serviceUrl: string,
        protected defaultRedirects: DefaultRedirects
    ) {}

    private urlFor(path: string) {
        return joinURL(new URL(this.serviceUrl), path);
    }

    validate(tid: string | null, sid: string | null, eid: string | null): Promise<APIResponse> {
        const ct = tid ? [`tid=${tid}`] : [];
        const cs = sid ? [`sid=${sid}`] : [];
        const ce = eid ? [`eid=${eid}`] : [];
        return this.request.get(this.urlFor('auth/validate'), {
            headers: { ...cookieHeader(ct, cs, ce) }
        });
    }

    loginWithToken(
        tid: string | null,
        sid: string | null,
        queryToken: string | null,
        apiKey: string | null,
        rememberMe: boolean | null,
        captcha: string | null | undefined
    ): Promise<APIResponse> {
        const qs = rememberMe ? { rememberMe } : {};
        const qt = queryToken ? { token: queryToken } : {};
        const ct = tid ? [`tid=${tid}`] : [];
        const cs = sid ? [`sid=${sid}`] : [];
        let qc = getCaptchaQuery(captcha);

        return this.request.get('auth/token/login', {
            params: { ...qs, ...qt, ...qc, ...this.defaultRedirects },
            headers: { ...authHeader(apiKey), ...cookieHeader(ct, cs) }
        });
    }

    loginWithOAuth2(
        tid: string | null,
        sid: string | null,
        rememberMe: boolean | null,
        captcha: string | null | undefined
    ): Promise<APIResponse> {
        const qs = rememberMe ? { rememberMe } : {};
        const ct = tid ? [`tid=${tid}`] : [];
        const cs = sid ? [`sid=${sid}`] : [];
        let qc = getCaptchaQuery(captcha);

        return this.request.get(this.urlFor('auth/oauth2_flow/login'), {
            params: { ...qs, ...qc, ...this.defaultRedirects },
            headers: { ...cookieHeader(ct, cs) }
        });
    }

    linkWithOAuth2(sid: string | null): Promise<APIResponse> {
        return this.request.get(this.urlFor('auth/oauth2_flow/link'), {
            params: { ...this.defaultRedirects },
            headers: { ...cookieHeader(sid ? [`sid=${sid}`] : []) }
        });
    }

    authorizeWithOAuth2(
        sid: string | null,
        eid: string | null,
        state: string | null,
        code: string | null
    ): Promise<APIResponse> {
        const qs = state ? { state } : {};
        const qc = code ? { code } : {};
        const cs = sid ? [`sid=${sid}`] : [];
        const ce = eid ? [`eid=${eid}`] : [];

        return this.request.get(this.urlFor('auth/oauth2_flow/auth'), {
            params: { ...qs, ...qc },
            headers: { ...cookieHeader([...cs, ...ce]) }
        });
    }

    loginWithOpenId(
        tid: string | null,
        sid: string | null,
        rememberMe: boolean | null,
        captcha: string | null | undefined
    ): Promise<APIResponse> {
        const qs = rememberMe ? { rememberMe } : {};
        const ct = tid ? [`tid=${tid}`] : [];
        const cs = sid ? [`sid=${sid}`] : [];
        let qc = getCaptchaQuery(captcha);

        return this.request.get(this.urlFor('auth/openid_flow/login'), {
            params: { ...qs, ...qc, ...this.defaultRedirects },
            headers: { ...cookieHeader(ct, cs) }
        });
    }

    linkWithOpenId(sid: string | null): Promise<APIResponse> {
        return this.request.get(this.urlFor('auth/openid_flow/link'), {
            params: { ...this.defaultRedirects },
            headers: { ...cookieHeader(sid ? [`sid=${sid}`] : []) }
        });
    }

    authorizeWithOpenId(
        sid: string | null,
        eid: string | null,
        state: string | null,
        code: string | null
    ): Promise<APIResponse> {
        const qs = state ? { state } : {};
        const qc = code ? { code } : {};
        const cs = sid ? [`sid=${sid}`] : [];
        const ce = eid ? [`eid=${eid}`] : [];

        return this.request.get(this.urlFor('auth/openid_flow/auth'), {
            params: { ...qs, ...qc },
            headers: { ...cookieHeader(cs, ce) }
        });
    }

    logout(sid: string | null, terminateAll: boolean | null): Promise<APIResponse> {
        return this.request.get(this.urlFor('/auth/logout'), {
            params: { terminateAll },
            headers: { ...cookieHeader(sid ? [`sid=${sid}`] : []) }
        });
    }
}

export class AuthAPIEx extends AuthAPI {
    constructor(request: APIRequestContext, serviceUrl: string, defaultRedirects: DefaultRedirects) {
        super(request, serviceUrl, defaultRedirects);
    }

    /*async loginAsGuestUser(extraHeaders?: Record<string, string>): Promise<UserCookies> {
        // use the default captcha to fast-login
        const response = await this.request.loginWithToken(null, null, null, null, true, null).set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(this.this.defaultRedirects.redirectUrl);
        const cookies = getCookies(response);
        expect(cookies.tid).toBeValidTID();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        return {
            tid: cookies.tid?.value,
            sid: cookies.sid?.value,
            eid: cookies.eid?.value
        };
    }

    async loginWithToken(
        tid: string,
        rememberMe: boolean | null,
        extraHeaders?: Record<string, string>
    ): Promise<UserCookies> {
        // no captcha as token login should work without it
        const response = await this.request
            .loginWithToken(tid, null, null, null, rememberMe, undefined)
            .set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(this.this.defaultRedirects.redirectUrl);

        const cookies = getCookies(response);
        expect(cookies.tid).toBeValidTID();
        expect(cookies.tid.value).not.toEqual(tid);
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        return {
            tid: cookies.tid?.value,
            sid: cookies.sid?.value,
            eid: cookies.eid?.value
        };
    }

    async startLoginWithOAuth2(
        mock: OAuth2MockServer,
        rememberMe: boolean | null,
        extraHeaders?: Record<string, string>
    ): Promise<StartLoginResult> {
        const response = await this.request.loginWithOAuth2(null, null, rememberMe, null).set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        const redirectUrl = getPageRedirectUrl(response.text);
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeValidEID();

        const authParams = redirectUrl!.parseQueryParamsFromUrl();
        expect(authParams.redirect_uri).toEqual(this.urlFor('auth/oauth2_flow/auth'));
        return {
            authParams,
            tid: cookies.tid?.value,
            sid: cookies.sid?.value,
            eid: cookies.eid?.value
        };
    }

    async loginWithOAuth2(
        mock: OAuth2MockServer,
        user: ExternalUser,
        rememberMe: boolean | null,
        extraHeaders?: Record<string, string>
    ): Promise<UserCookies> {
        const start = await this.startLoginWithOAuth2(mock, rememberMe, extraHeaders);
        const response = await this.request
            .authorizeWithOAuth2(start.sid, start.eid, start.authParams.state, user.toCode())
            .set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(this.this.defaultRedirects.redirectUrl);
        const cookies = getCookies(response);
        if (rememberMe) {
            expect(cookies.tid).toBeValidTID();
        } else {
            expect(cookies.tid).toBeClearCookie();
        }
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        return {
            tid: cookies.tid?.value,
            sid: cookies.sid?.value,
            eid: cookies.eid?.value
        };
    }

    async startLinkWithOAuth2(
        mock: OAuth2MockServer,
        sid: string,
        extraHeaders?: Record<string, string>
    ): Promise<StartLoginResult> {
        const response = await this.request.linkWithOAuth2(sid).set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        const redirectUrl = getPageRedirectUrl(response.text);
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.sid.value).toEqual(sid);
        expect(cookies.eid).toBeValidEID();

        const authParams = redirectUrl!.parseQueryParamsFromUrl();
        expect(authParams.redirect_uri).toEqual(this.urlFor('auth/oauth2_flow/auth'));
        return {
            authParams,
            tid: cookies.tid?.value,
            sid: cookies.sid?.value,
            eid: cookies.eid?.value
        };
    }

    async linkWithOAuth2(
        mock: OAuth2MockServer,
        sid: string,
        user: ExternalUser,
        extraHeaders?: Record<string, string>
    ): Promise<UserCookies> {
        const start = await this.startLinkWithOAuth2(mock, sid, extraHeaders);
        const response = await this.request
            .authorizeWithOAuth2(start.sid, start.eid, start.authParams.state, user.toCode())
            .set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(this.this.defaultRedirects.redirectUrl);
        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        return {
            tid: cookies.tid?.value,
            sid: cookies.sid?.value,
            eid: cookies.eid?.value
        };
    }

    async startLoginWithOpenId(
        mock: OpenIdMockServer,
        rememberMe: boolean | null,
        extraHeaders?: Record<string, string>
    ): Promise<StartLoginResult> {
        const response = await this.request.loginWithOpenId(null, null, rememberMe, null).set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        const redirectUrl = getPageRedirectUrl(response.text);
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeValidEID();

        const authParams = redirectUrl!.parseQueryParamsFromUrl();
        expect(authParams.redirect_uri).toEqual(this.urlFor('auth/openid_flow/auth'));
        return {
            authParams,
            tid: cookies.tid?.value,
            sid: cookies.sid?.value,
            eid: cookies.eid?.value
        };
    }

    async loginWithOpenId(
        mock: OpenIdMockServer,
        user: ExternalUser,
        rememberMe: boolean | null,
        extraHeaders?: Record<string, string>
    ): Promise<UserCookies> {
        const start = await this.startLoginWithOpenId(mock, rememberMe, extraHeaders);
        const response = await this.request
            .authorizeWithOpenId(
                start.sid,
                start.eid,
                start.authParams.state,
                user.toCode({ nonce: start.authParams.nonce })
            )
            .set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(this.this.defaultRedirects.redirectUrl);
        const cookies = getCookies(response);
        if (rememberMe) {
            expect(cookies.tid).toBeValidTID();
        } else {
            expect(cookies.tid).toBeClearCookie();
        }
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        return {
            tid: cookies.tid?.value,
            sid: cookies.sid?.value,
            eid: cookies.eid?.value
        };
    }

    async startLinkWithOpenId(
        mock: OpenIdMockServer,
        sid: string,
        extraHeaders?: Record<string, string>
    ): Promise<StartLoginResult> {
        const response = await this.request.linkWithOpenId(sid).set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        const redirectUrl = getPageRedirectUrl(response.text);
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.sid.value).toEqual(sid);
        expect(cookies.eid).toBeValidEID();

        const authParams = redirectUrl!.parseQueryParamsFromUrl();
        expect(authParams.redirect_uri).toEqual(this.urlFor('auth/openid_flow/auth'));
        return {
            authParams,
            tid: cookies.tid?.value,
            sid: cookies.sid?.value,
            eid: cookies.eid?.value
        };
    }

    async linkWithOpenId(
        mock: OpenIdMockServer,
        sid: string,
        user: ExternalUser,
        extraHeaders?: Record<string, string>
    ): Promise<UserCookies> {
        const start = await this.startLinkWithOpenId(mock, sid, extraHeaders);
        const response = await this.request
            .authorizeWithOpenId(
                start.sid,
                start.eid,
                start.authParams.state,
                user.toCode({ nonce: start.authParams.nonce })
            )
            .set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(response.text)).toEqual(this.this.defaultRedirects.redirectUrl);
        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        return {
            tid: cookies.tid?.value,
            sid: cookies.sid?.value,
            eid: cookies.eid?.value
        };
    }

    async logout(sid: string, terminateAll: boolean | null, extraHeaders?: Record<string, string>): Promise<void> {
        let response = await this.request.logout(sid, terminateAll).set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
    }*/
}
