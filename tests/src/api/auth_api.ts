import { expect } from '$fixtures/setup';
import OAuth2MockServer from '$lib/mocks/oauth2';
import OpenIdMockServer from '$lib/mocks/openid';
import { OptionalSchema } from '$lib/schema_utils';
import { joinURL } from '$lib/utils';
import { z } from 'zod';
import { ApiRequest } from './api';
import { ExternalUser } from './external_user';
import { getPageRedirectUrl } from './utils';

export type DefaultRedirects = {
    loginUrl: string;
    redirectUrl: string;
    errorUrl: string;
};

export type UserCookies = {
    tid: string;
    sid: string;
    eid: string;
};

interface StartLoginResult extends UserCookies {
    authParams: Record<string, string>;
}

export const ProvidersSchema = z.object({
    providers: z.array(z.string())
});
export type Providers = z.infer<typeof ProvidersSchema>;

export const LinkedIdentitySchema = z.object({
    userId: z.string(),
    provider: z.string(),
    providerUserId: z.string(),
    linkedAt: z.string().transform((str) => new Date(str)),
    name: OptionalSchema(z.string()),
    email: OptionalSchema(z.string())
});
export type LinkedIdentity = z.infer<typeof LinkedIdentitySchema>;

export const LinkedIdentitiesSchema = z.object({
    links: z.array(LinkedIdentitySchema)
});
export type LinkedIdentities = z.infer<typeof LinkedIdentitiesSchema>;

function getCaptchaQuery(captcha: string | null | undefined): Record<string, string> {
    if (captcha === null) {
        // use the "always pass" token
        return { captcha: '1x00000000000000000000AA' };
    } else if (captcha) {
        // use provided captcha
        return { captcha };
    }
    return {};
}

export class AuthAPI {
    constructor(
        public readonly serviceUrl: string,
        public readonly defaultRedirects: DefaultRedirects
    ) {}

    urlFor(path: string) {
        return joinURL(new URL(this.serviceUrl), path);
    }

    validateRequest(tid: string | null, sid: string | null, eid: string | null): ApiRequest {
        const ct = tid && { tid };
        const cs = sid && { sid };
        const ce = eid && { eid };
        return ApiRequest.get(this.urlFor('auth/validate')).withCookies({ ...ct, ...cs, ...ce });
    }

    loginWithTokenRequest(
        tid: string | null,
        sid: string | null,
        queryToken: string | null,
        apiKey: string | null,
        rememberMe: boolean | null,
        captcha: string | null | undefined
    ): ApiRequest {
        const qs = rememberMe && { rememberMe };
        const qt = queryToken && { token: queryToken };
        const ct = tid && { tid };
        const cs = sid && { sid };
        const qc = getCaptchaQuery(captcha);

        return ApiRequest.get(this.urlFor('auth/token/login'))
            .withParams({ ...qs, ...qt, ...qc, ...this.defaultRedirects })
            .withAuthIf(apiKey)
            .withCookies({ ...ct, ...cs });
    }

    loginWithEmailRequest(
        email: string | null,
        tid: string | null,
        sid: string | null,
        queryToken: string | null,
        apiKey: string | null,
        rememberMe: boolean | null,
        captcha: string | null | undefined
    ): ApiRequest {
        const qe = email && { email };
        const qs = rememberMe && { rememberMe };
        const qt = queryToken && { token: queryToken };
        const ct = tid && { tid };
        const cs = sid && { sid };
        const qc = getCaptchaQuery(captcha);

        return ApiRequest.get(this.urlFor('auth/email/login'))
            .withParams({ ...qe, ...qs, ...qt, ...qc, ...this.defaultRedirects })
            .withAuthIf(apiKey)
            .withCookies({ ...ct, ...cs });
    }

    async loginAsGuestUser(extraHeaders?: Record<string, string>): Promise<UserCookies> {
        // use the default captcha to fast-login
        const response = await this.loginWithTokenRequest(null, null, null, null, true, null).withHeaders(
            extraHeaders ?? {}
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(this.defaultRedirects.redirectUrl);

        const cookies = response.cookies();
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
        const response = await this.loginWithTokenRequest(tid, null, null, null, rememberMe, undefined).withHeaders(
            extraHeaders ?? {}
        );
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(this.defaultRedirects.redirectUrl);

        const cookies = response.cookies();
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

    getProvidersRequest(): ApiRequest {
        return ApiRequest.get(this.urlFor('/api/auth/providers'));
    }

    async getProviders(): Promise<string[]> {
        const response = await this.getProvidersRequest();
        expect(response).toHaveStatus(200);
        return (await response.parse(ProvidersSchema)).providers;
    }

    loginWithOAuth2Request(
        tid: string | null,
        sid: string | null,
        rememberMe: boolean | null,
        captcha: string | null | undefined
    ): ApiRequest {
        const qs = rememberMe && { rememberMe };
        const ct = tid && { tid };
        const cs = sid && { sid };
        const qc = getCaptchaQuery(captcha);

        return ApiRequest.get(this.urlFor('auth/oauth2_flow/login'))
            .withParams({ ...qs, ...qc, ...this.defaultRedirects })
            .withCookies({ ...ct, ...cs });
    }

    linkWithOAuth2Request(sid: string | null): ApiRequest {
        const cs = sid && { sid };

        return ApiRequest.get(this.urlFor('auth/oauth2_flow/link'))
            .withParams({ ...this.defaultRedirects })
            .withCookies({ ...cs });
    }

    authorizeWithOAuth2Request(
        sid: string | null,
        eid: string | null,
        state: string | null,
        code: string | null
    ): ApiRequest {
        const qs = state && { state };
        const qc = code && { code };
        const cs = sid && { sid };
        const ce = eid && { eid };

        return ApiRequest.get(this.urlFor('auth/oauth2_flow/auth'))
            .withParams({ ...qs, ...qc })
            .withCookies({ ...cs, ...ce });
    }

    async startLoginWithOAuth2(
        mock: OAuth2MockServer,
        rememberMe: boolean | null,
        extraHeaders?: Record<string, string>
    ): Promise<StartLoginResult> {
        const response = await this.loginWithOAuth2Request(null, null, rememberMe, null).withHeaders(
            extraHeaders ?? {}
        );
        expect(response).toHaveStatus(200);
        const redirectUrl = getPageRedirectUrl(await response.text());
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const cookies = response.cookies();
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
        const response = await this.authorizeWithOAuth2Request(
            start.sid,
            start.eid,
            start.authParams.state,
            user.toCode()
        ).withHeaders(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(this.defaultRedirects.redirectUrl);

        const cookies = response.cookies();
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
        const response = await this.linkWithOAuth2Request(sid).withHeaders(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        const redirectUrl = getPageRedirectUrl(await response.text());
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const cookies = response.cookies();
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
        const response = await this.authorizeWithOAuth2Request(
            start.sid,
            start.eid,
            start.authParams.state,
            user.toCode()
        ).withHeaders(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(this.defaultRedirects.redirectUrl);

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        return {
            tid: cookies.tid?.value,
            sid: cookies.sid?.value,
            eid: cookies.eid?.value
        };
    }

    loginWithOpenIdRequest(
        tid: string | null,
        sid: string | null,
        rememberMe: boolean | null,
        captcha: string | null | undefined
    ): ApiRequest {
        const qs = rememberMe && { rememberMe };
        const ct = tid && { tid };
        const cs = sid && { sid };
        const qc = getCaptchaQuery(captcha);

        return ApiRequest.get(this.urlFor('auth/openid_flow/login'))
            .withParams({ ...qs, ...qc, ...this.defaultRedirects })
            .withCookies({ ...ct, ...cs });
    }

    linkWithOpenIdRequest(sid: string | null): ApiRequest {
        const cs = sid && { sid };

        return ApiRequest.get(this.urlFor('auth/openid_flow/link'))
            .withParams({ ...this.defaultRedirects })
            .withCookies({ ...cs });
    }

    authorizeWithOpenIdRequest(
        sid: string | null,
        eid: string | null,
        state: string | null,
        code: string | null
    ): ApiRequest {
        const qs = state && { state };
        const qc = code && { code };
        const cs = sid && { sid };
        const ce = eid && { eid };

        return ApiRequest.get(this.urlFor('auth/openid_flow/auth'))
            .withParams({ ...qs, ...qc })
            .withCookies({ ...cs, ...ce });
    }

    async startLoginWithOpenId(
        mock: OpenIdMockServer,
        rememberMe: boolean | null,
        extraHeaders?: Record<string, string>
    ): Promise<StartLoginResult> {
        const response = await this.loginWithOpenIdRequest(null, null, rememberMe, null).withHeaders(
            extraHeaders ?? {}
        );
        expect(response).toHaveStatus(200);
        const redirectUrl = getPageRedirectUrl(await response.text());
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const cookies = response.cookies();
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
        const response = await this.authorizeWithOpenIdRequest(
            start.sid,
            start.eid,
            start.authParams.state,
            user.toCode({ nonce: start.authParams.nonce })
        ).withHeaders(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(this.defaultRedirects.redirectUrl);

        const cookies = response.cookies();
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
        const response = await this.linkWithOpenIdRequest(sid).withHeaders(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        const redirectUrl = getPageRedirectUrl(await response.text());
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const cookies = response.cookies();
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
        const response = await this.authorizeWithOpenIdRequest(
            start.sid,
            start.eid,
            start.authParams.state,
            user.toCode({ nonce: start.authParams.nonce })
        ).withHeaders(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
        expect(getPageRedirectUrl(await response.text())).toEqual(this.defaultRedirects.redirectUrl);

        const cookies = response.cookies();
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        return {
            tid: cookies.tid?.value,
            sid: cookies.sid?.value,
            eid: cookies.eid?.value
        };
    }

    getExternalLinksRequest(sid: string | null): ApiRequest {
        const cs = sid && { sid };

        return ApiRequest.get(this.urlFor('api/auth/user/links')).withCookies({ ...cs });
    }

    async getExternalLinks(sid: string, extraHeaders?: Record<string, string>): Promise<LinkedIdentity[]> {
        const response = await this.getExternalLinksRequest(sid).withHeaders(extraHeaders ?? {});
        expect(response).toHaveStatus(200);

        const links = (await response.parse(LinkedIdentitiesSchema)).links;
        links.forEach((l) => {
            l.linkedAt = new Date(l.linkedAt);
        });

        return links;
    }

    unlinkRequest(sid: string | null, provider: string, providerUserId: string): ApiRequest {
        const url = `api/auth/user/links/${provider}/${providerUserId}`;
        const cs = sid && { sid };

        return ApiRequest.delete(this.urlFor(url)).withCookies({ ...cs });
    }

    async tryUnlink(
        sid: string,
        provider: string,
        providerUserId: string,
        extraHeaders?: Record<string, string>
    ): Promise<boolean> {
        const response = await this.unlinkRequest(sid, provider, providerUserId).withHeaders(extraHeaders ?? {});
        if (response.status() == 404) {
            return false;
        }

        expect(response).toHaveStatus(200);
        return true;
    }

    logoutRequest(sid: string | null, tid: string | null, terminateAll: boolean | null): ApiRequest {
        const qt = terminateAll && { terminateAll };
        const cs = sid && { sid };
        const ct = tid && { tid };

        return ApiRequest.get(this.urlFor('/auth/logout'))
            .withParams({ ...qt })
            .withCookies({ ...cs, ...ct });
    }

    async logout(
        sid: string,
        tid: string | null,
        terminateAll: boolean | null,
        extraHeaders?: Record<string, string>
    ): Promise<void> {
        const response = await this.logoutRequest(sid, tid, terminateAll).withHeaders(extraHeaders ?? {});
        expect(response).toHaveStatus(200);
    }
}
