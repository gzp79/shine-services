import request, { Response } from '$lib/request';
import { Cookie } from 'tough-cookie';
import config from '../test.config';

export function getCookies(response?: Response): Record<string, Cookie> {
    return (response?.headers['set-cookie'] ?? [])
        .map((cookieStr: string) => Cookie.parse(cookieStr))
        .reduce((cookies: Record<string, Cookie>, cookie: Cookie) => {
            cookies[cookie.key] = cookie;
            return cookies;
        }, {});
}

export interface UserInfo {
    userId: string;
    name: string;
    sessionLength: number;
    roles: string[];
}

export async function getUserInfo(
    cookieValue: string,
    extraHeaders?: Record<string, string>
): Promise<UserInfo> {
    let response = await request
        .get(config.getUrlFor('identity/api/auth/user/info'))
        .set('Cookie', [`sid=${cookieValue}`])
        .set(extraHeaders ?? {})
        .send();
    expect(response.statusCode).toEqual(200);
    //expect(response.body).toBeInstanceOf(UserInfo);
    return response.body;
}

export interface ActiveSession {
    userId: string;
    createdAt: Date;
    agent: string;
    country: string | null;
    region: string | null;
    city: string | null;
}

export async function getSessions(
    cookieValue: string,
    extraHeaders?: Record<string, string>
): Promise<ActiveSession[]> {
    let response = await request
        .get(config.getUrlFor('identity/api/auth/user/sessions'))
        .set('Cookie', [`sid=${cookieValue}`])
        .set(extraHeaders ?? {})
        .send();
    expect(response.statusCode).toEqual(200);

    response.body?.sessions?.forEach((s: ActiveSession) => {
        s.createdAt = new Date(s.createdAt);
    });
    return response.body?.sessions ?? [];
}

export interface ActiveToken {
    userId: string;
    kind: string;
    tokenFingerprint: string;
    createdAt: Date;
    expireAt: Date;
    isExpired: boolean;
    agent: string;
    country: string | null;
    region: string | null;
    city: string | null;
}

export async function getTokens(
    cookieValue: string,
    extraHeaders?: Record<string, string>
): Promise<ActiveToken[]> {
    let response = await request
        .get(config.getUrlFor('identity/api/auth/user/tokens'))
        .set('Cookie', [`sid=${cookieValue}`])
        .set(extraHeaders ?? {})
        .send();
    expect(response.statusCode).toEqual(200);

    response.body?.tokens?.forEach((t: ActiveToken) => {
        t.createdAt = new Date(t.createdAt);
        t.expireAt = new Date(t.expireAt);
    });

    return response.body?.tokens ?? [];
}

export interface ExternalLink {
    userId: string;
    provider: string;
    providerUserId: string;
    linkedAt: Date;
    name: string | null;
    email: string | null;
}

export async function getExternalLinks(
    cookieValue: string,
    extraHeaders?: Record<string, string>
): Promise<ExternalLink[]> {
    let response = await request
        .get(config.getUrlFor('identity/api/auth/user/links'))
        .set('Cookie', [`sid=${cookieValue}`])
        .set(extraHeaders ?? {})
        .send();
    expect(response.statusCode).toEqual(200);

    response.body?.links?.forEach((l: ExternalLink) => {
        l.linkedAt = new Date(l.linkedAt);
    });

    return response.body?.links ?? [];
}
