import OAuth2MockServer from '$lib/mocks/oauth2';
import OpenIdMockServer from '$lib/mocks/openid';
import { getCookies, getPageRedirectUrl } from '$lib/response_utils';
import { Config } from '../../test.config';
import { RequestAPI } from './api';
import { ExternalUser } from './external_user';

interface UserCookies {
    tid: string;
    sid: string;
    eid: string;
}

interface StartLoginResult extends UserCookies {
    authParams: any;
}

export class AuthAPI {
    private readonly config: Config;

    constructor(public readonly request: RequestAPI) {
        this.config = request.config;
    }

    async loginAsGuestUser(extraHeaders?: Record<string, string>): Promise<UserCookies> {
        const response = await this.request.loginWithToken(null, null, null, true).set(extraHeaders ?? {});
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(this.config.defaultRedirects.redirectUrl);
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
        const response = await this.request
            .loginWithToken(tid, null, null, rememberMe)
            .set(extraHeaders ?? {});
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(this.config.defaultRedirects.redirectUrl);

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
        const response = await this.request.loginWithOAuth2(null, null, rememberMe).set(extraHeaders ?? {});
        expect(response.statusCode).toEqual(200);
        const redirectUrl = getPageRedirectUrl(response.text);
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeValidEID();

        const authParams = redirectUrl!.parseQueryParamsFromUrl();
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
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(this.config.defaultRedirects.redirectUrl);
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
        expect(response.statusCode).toEqual(200);
        const redirectUrl = getPageRedirectUrl(response.text);
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.sid.value).toEqual(sid);
        expect(cookies.eid).toBeValidEID();

        const authParams = redirectUrl!.parseQueryParamsFromUrl();
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
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(this.config.defaultRedirects.redirectUrl);
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
        const response = await this.request.loginWithOpenId(null, null, rememberMe).set(extraHeaders ?? {});
        expect(response.statusCode).toEqual(200);
        const redirectUrl = getPageRedirectUrl(response.text);
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeValidEID();

        const authParams = redirectUrl!.parseQueryParamsFromUrl();
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
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(this.config.defaultRedirects.redirectUrl);
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
        expect(response.statusCode).toEqual(200);
        const redirectUrl = getPageRedirectUrl(response.text);
        expect(redirectUrl).toStartWith(mock.getUrlFor('authorize'));

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.sid.value).toEqual(sid);
        expect(cookies.eid).toBeValidEID();

        const authParams = redirectUrl!.parseQueryParamsFromUrl();
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
        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(this.config.defaultRedirects.redirectUrl);
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

    async logout(
        sid: string,
        terminateAll: boolean | null,
        extraHeaders?: Record<string, string>
    ): Promise<void> {
        let response = await this.request.logout(sid, terminateAll).set(extraHeaders ?? {});
        expect(response.statusCode).toEqual(200);
    }
}
