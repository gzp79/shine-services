import config, { Config } from '../../test.config';
import request, { Request } from '../request';
import { AuthAPI } from './auth_api';
import { ExternalLinkAPI } from './external_link_api';
import { SessionAPI } from './session_api';
import { TokenAPI } from './token_api';
import { UserAPI } from './user_api';

export type TokenKind = 'access' | 'persistent' | 'singleAccess';

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

export class RequestAPI {
    constructor(public readonly config: Config) {}

    validate(tid: string | null, sid: string | null, eid: string | null) {
        const ct = tid ? [`tid=${tid}`] : [];
        const cs = sid ? [`sid=${sid}`] : [];
        const ce = eid ? [`eid=${eid}`] : [];

        return request
            .get(this.config.getUrlFor('auth/validate'))
            .set('Cookie', [...ct, ...cs, ...ce]);
    }

    loginWithToken(
        tid: string | null,
        sid: string | null,
        queryToken: string | null,
        apiKey: string | null,
        rememberMe: boolean | null,
        captcha: string | null | undefined
    ): Request {
        const qs = rememberMe ? { rememberMe } : {};
        const qt = queryToken ? { token: queryToken } : {};
        const ct = tid ? [`tid=${tid}`] : [];
        const cs = sid ? [`sid=${sid}`] : [];
        const ht = apiKey ? { Authorization: `Bearer ${apiKey}` } : {};
        let qc = getCaptchaQuery(captcha);

        return request
            .get(this.config.getUrlFor('auth/token/login'))
            .query({ ...qs, ...qt, ...qc, ...config.defaultRedirects })
            .set('Cookie', [...ct, ...cs])
            .set({ ...ht });
    }

    loginWithOAuth2(
        tid: string | null,
        sid: string | null,
        rememberMe: boolean | null,
        captcha: string | null | undefined
    ): Request {
        const qs = rememberMe ? { rememberMe } : {};
        const ct = tid ? [`tid=${tid}`] : [];
        const cs = sid ? [`sid=${sid}`] : [];
        let qc = getCaptchaQuery(captcha);

        return request
            .get(config.getUrlFor('auth/oauth2_flow/login'))
            .query({ ...qs, ...qc, ...config.defaultRedirects })
            .set('Cookie', [...ct, ...cs]);
    }

    linkWithOAuth2(sid: string | null): Request {
        return request
            .get(config.getUrlFor('auth/oauth2_flow/link'))
            .query({ ...config.defaultRedirects })
            .set('Cookie', sid ? [`sid=${sid}`] : []);
    }

    authorizeWithOAuth2(
        sid: string | null,
        eid: string | null,
        state: string | null,
        code: string | null
    ): Request {
        const qs = state ? { state } : {};
        const qc = code ? { code } : {};
        const cs = sid ? [`sid=${sid}`] : [];
        const ce = eid ? [`eid=${eid}`] : [];

        return request
            .get(config.getUrlFor('auth/oauth2_flow/auth'))
            .query({ ...qs, ...qc })
            .set('Cookie', [...cs, ...ce]);
    }

    loginWithOpenId(
        tid: string | null,
        sid: string | null,
        rememberMe: boolean | null,
        captcha: string | null | undefined
    ): Request {
        const qs = rememberMe ? { rememberMe } : {};
        const ct = tid ? [`tid=${tid}`] : [];
        const cs = sid ? [`sid=${sid}`] : [];
        let qc = getCaptchaQuery(captcha);

        return request
            .get(config.getUrlFor('auth/openid_flow/login'))
            .query({ ...qs, ...qc, ...config.defaultRedirects })
            .set('Cookie', [...ct, ...cs]);
    }

    linkWithOpenId(sid: string | null): Request {
        return request
            .get(config.getUrlFor('auth/openid_flow/link'))
            .query({ ...config.defaultRedirects })
            .set('Cookie', sid ? [`sid=${sid}`] : []);
    }

    authorizeWithOpenId(
        sid: string | null,
        eid: string | null,
        state: string | null,
        code: string | null
    ): Request {
        const qs = state ? { state } : {};
        const qc = code ? { code } : {};
        const cs = sid ? [`sid=${sid}`] : [];
        const ce = eid ? [`eid=${eid}`] : [];

        return request
            .get(config.getUrlFor('auth/openid_flow/auth'))
            .query({ ...qs, ...qc })
            .set('Cookie', [...cs, ...ce]);
    }

    logout(sid: string | null, terminateAll: boolean | null): Request {
        return request
            .get(config.getUrlFor('/auth/logout'))
            .query({ terminateAll })
            .set('Cookie', sid ? [`sid=${sid}`] : []);
    }

    getUserInfo(sid: string | null): Request {
        return request
            .get(config.getUrlFor('api/auth/user/info'))
            .set('Cookie', sid ? [`sid=${sid}`] : []);
    }

    getRoles(sid: string | null, masterKey: boolean, userId: string): Request {
        let av = masterKey ? `Bearer ${config.masterKey}` : null;

        return request
            .get(config.getUrlFor(`/api/identities/${userId}/roles`))
            .set('Cookie', sid ? [`sid=${sid}`] : [])
            .set(av ? { Authorization: av } : {});
    }

    addRole(sid: string | null, masterKey: boolean, userId: string, role: string): Request {
        let av = masterKey ? `Bearer ${config.masterKey}` : null;

        return request
            .put(config.getUrlFor(`/api/identities/${userId}/roles`))
            .set('Cookie', sid ? [`sid=${sid}`] : [])
            .set(av ? { Authorization: av } : {})
            .type('json')
            .send({ role: role });
    }

    deleteRole(sid: string | 'masterKey' | null, masterKey: boolean, userId: string, role: string): Request {
        let av = masterKey ? `Bearer ${config.masterKey}` : null;

        return request
            .delete(config.getUrlFor(`/api/identities/${userId}/roles`))
            .set('Cookie', sid ? [`sid=${sid}`] : [])
            .set(av ? { Authorization: av } : {})
            .type('json')
            .send({ role: role });
    }

    getSessions(sid: string | null): Request {
        return request
            .get(config.getUrlFor('api/auth/user/sessions'))
            .set('Cookie', sid ? [`sid=${sid}`] : []);
    }

    getTokens(sid: string | null): Request {
        return request
            .get(config.getUrlFor('api/auth/user/tokens'))
            .set('Cookie', sid ? [`sid=${sid}`] : []);
    }

    getToken(sid: string | null, tokenId: string): Request {
        return request
            .get(config.getUrlFor(`api/auth/user/tokens/${tokenId}`))
            .set('Cookie', sid ? [`sid=${sid}`] : []);
    }

    revokeToken(sid: string | null, tokenId: string): Request {
        return request
            .delete(config.getUrlFor(`api/auth/user/tokens/${tokenId}`))
            .set('Cookie', sid ? [`sid=${sid}`] : []);
    }

    createToken(sid: string | null, kind: TokenKind, duration: number, bindToSite: boolean): Request {
        return request
            .post(this.config.getUrlFor('api/auth/user/tokens'))
            .set('Cookie', sid ? [`sid=${sid}`] : [])
            .type('json')
            .send({
                kind,
                timeToLive: duration,
                bindToSite: bindToSite
            });
    }

    getExternalLinks(sid: string | null): Request {
        return request
            .get(config.getUrlFor('api/auth/user/links'))
            .set('Cookie', sid ? [`sid=${sid}`] : []);
    }

    unlink(sid: string | null, provider: string, providerUserId: string): Request {
        let url = `api/auth/user/links/${provider}/${providerUserId}`;

        return request.delete(config.getUrlFor(url)).set('Cookie', sid ? [`sid=${sid}`] : []);
    }
}

export class API {
    public readonly request: RequestAPI;
    public readonly auth: AuthAPI;
    public readonly externalLink: ExternalLinkAPI;
    public readonly session: SessionAPI;
    public readonly user: UserAPI;
    public readonly token: TokenAPI;

    constructor(public readonly config: Config) {
        this.request = new RequestAPI(this.config);
        this.auth = new AuthAPI(this.request);
        this.externalLink = new ExternalLinkAPI(this.request);
        this.session = new SessionAPI(this.request);
        this.user = new UserAPI(this.request);
        this.token = new TokenAPI(this.request);
    }
}

const api = new API(config);
export default api;
