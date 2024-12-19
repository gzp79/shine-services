import { toEndWith } from 'jest-extended';
import { Cookie } from 'tough-cookie';
import { intoMatcherResult } from './utils';

interface CustomMatchers<R = unknown> {
    toBeClearCookie(): R;
    toBeValidTID(): R;
    toBeValidSID(): R;
    toBeValidEID(): R;
    //toBeGuestUser(): R;
}

declare global {
    namespace jest {
        interface Expect extends CustomMatchers {}
        interface Matchers<R> extends CustomMatchers<R> {}
        interface InverseAsymmetricMatchers extends CustomMatchers {}
    }
}

const matchers: jest.ExpectExtendMap = {
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
            path: expect.toEndWith('/auth/'),
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
            path: expect.toEndWith('/auth/'),
            domain: 'cloud.sandbox.com',
            expires: 'Infinity'
        });
        return intoMatcherResult(this, received, expected);
    }
};

export default matchers;
