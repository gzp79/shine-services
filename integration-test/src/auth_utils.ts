import request from 'superagent';
import config from '../test.config';
import { Response } from 'superagent';
import { Cookie } from 'tough-cookie';
import uuidValidate from 'uuid-validate';
import { TestUser } from './user';

interface CustomMatchers<R = unknown> {
    toBeClearCookie(): R;
    toBeValidTID(): R;
    toBeValidSID(): R;
    toBeValidEID(): R;

    toBeGuestUser(): R;
}

declare global {
    namespace jest {
        interface Expect extends CustomMatchers {}
        interface Matchers<R> extends CustomMatchers<R> {}
        interface InverseAsymmetricMatchers extends CustomMatchers {}
    }
}

function intoMatcherResult(self: jest.MatcherContext, received: any, expected: object) {
    const pass = self.equals(received, expected);

    if (pass) {
        return {
            message: () =>
                `Expected: ${self.utils.printExpected(expected)}\nReceived: ${self.utils.printReceived(
                    received
                )}`,
            pass: true
        };
    }
    return {
        message: () =>
            `Expected: ${self.utils.printExpected(expected)}\nReceived: ${self.utils.printReceived(
                received
            )}\n\n${self.utils.diff(expected, received, {})}`,
        pass: false
    };
}

expect.extend({
    toBeClearCookie(received: Cookie) {
        const expected = expect.objectContaining({
            secure: true,
            httpOnly: true,
            sameSite: 'lax',
            expires: expect.toBeBefore(new Date())
        });
        return intoMatcherResult(this, received, expected);
    },

    toBeValidTID(received: Cookie) {
        const expected = expect.objectContaining({
            key: 'tid',
            secure: true,
            httpOnly: true,
            sameSite: 'lax',
            path: '/identity/auth',
            domain: 'cloud.sandbox.com',
            expires: expect.toBeAfter(new Date())
        });
        return intoMatcherResult(this, received, expected);
    },

    toBeValidSID(received: Cookie) {
        const expected = expect.objectContaining({
            key: 'sid',
            secure: true,
            httpOnly: true,
            sameSite: 'lax',
            path: '/',
            domain: 'sandbox.com',
            expires: 'Infinity' //session scoped
        });
        return intoMatcherResult(this, received, expected);
    },

    toBeValidEID(received: Cookie) {
        const expected = expect.objectContaining({
            key: 'eid',
            secure: true,
            httpOnly: true,
            sameSite: 'lax',
            path: '/identity/auth',
            domain: 'cloud.sandbox.com',
            expires: 'Infinity'
        });
        return intoMatcherResult(this, received, expected);
    },

    toBeGuestUser(received: UserInfo) {
        const expected = expect.objectContaining({
            userId: expect.toSatisfy((id: any) => uuidValidate(id)),
            name: expect.toStartWith('Freshman_'),
            sessionLength: expect.not.toBeNegative(),
            roles: []
        });
        return intoMatcherResult(this, received, expected);
    }
});

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

export async function logout(cookieValue: string, everywhere: boolean): Promise<void> {
    let response = await request
        .get(config.getUrlFor(`/identity/auth/logout?terminateAll=${everywhere}`))
        .set('Cookie', [`sid=${cookieValue}`])
        .send();
    expect(response.statusCode).toEqual(200);
}
