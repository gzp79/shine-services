import '$lib/time_matchers';
import * as request from 'superagent';
import config from '../test.config';
import { Response } from 'superagent';
import { Cookie } from 'tough-cookie';
import { userInfo } from 'os';

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

function intoMatcherResult(
    self: jest.MatcherContext,
    received: any,
    expected: object
) {
    const pass = self.equals(received, expected);

    if (pass) {
        return {
            message: () =>
                `Expected: ${self.utils.printExpected(
                    expected
                )}\nReceived: ${self.utils.printReceived(received)}`,
            pass: true
        };
    }
    return {
        message: () =>
            `Expected: ${self.utils.printExpected(
                expected
            )}\nReceived: ${self.utils.printReceived(
                received
            )}\n\n${self.utils.diff(expected, received)}`,
        pass: false
    };
}

expect.extend({
    toBeClearCookie(received: Cookie) {
        const expected = expect.objectContaining({
            secure: true,
            httpOnly: true,
            sameSite: 'lax',
            expires: expect.toBeEarlier(new Date())
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
            domain: 'cloud.scytta-test.com',
            expires: expect.toBeLater(new Date())
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
            domain: 'scytta-test.com',
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
            domain: 'cloud.scytta-test.com',
            expires: 'Infinity'
        });
        return intoMatcherResult(this, received, expected);
    },

    toBeGuestUser(received: UserInfo) {
        const expected = expect.objectContaining({
            userId: expect.any(String), //todo: uuid()
            name: expect.stringMatching(/^Freshman_.*/),
            sessionLength: expect.any(Number),//todo: .greaterThanOrEqual(0),
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

export async function getUserInfo(cookie: Cookie): Promise<UserInfo> {
    expect(cookie.key).toBe('sid');
    let response = await request
        .get(config.getUrlFor('identity/api/auth/user/info'))
        .set('Cookie', [`sid=${cookie.value}`])
        .send();
    expect(response.statusCode).toBe(200);
    //expect(response.body).toBeInstanceOf(UserInfo);
    return response.body;
}
