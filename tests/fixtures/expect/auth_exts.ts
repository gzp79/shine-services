import { Cookie } from '$lib/api/api';
import { UserInfo } from '$lib/api/user_api';
import uuidValidate from 'uuid-validate';
import { expect as baseExpect } from './common';

export const expect = baseExpect.extend({
    toBeClearCookie(received: Cookie) {
        expect(received.secure).toBeTruthy();
        expect(received.httpOnly).toBeTruthy();
        expect(received.sameSite).toEqual('Lax');
        expect(received.expires!).toBeBefore(new Date());

        return {
            message: () => 'Cookie is cleared',
            pass: true
        };
    },

    toBeValidTID(received: Cookie) {
        expect(received.name).toEqual('tid');
        expect(received.secure).toBeTruthy();
        expect(received.httpOnly).toBeTruthy();
        expect(received.sameSite).toEqual('Lax');
        expect(received.path).toEndWith('/auth/');
        expect(received.domain).toEqual('cloud.local-scytta.com');
        expect(received.expires!).toBeAfter(new Date());

        return {
            message: () => 'Cookie is a valid TID',
            pass: true
        };
    },

    toBeValidSID(received: Cookie) {
        expect(received.name).toEqual('sid');
        expect(received.secure).toBeTruthy();
        expect(received.httpOnly).toBeTruthy();
        expect(received.sameSite).toEqual('Lax');
        expect(received.path).toEqual('/');
        expect(received.domain).toEqual('local-scytta.com');
        expect(received.expires).toBeUndefined(); // session scoped

        return {
            message: () => 'Cookie is a validSID',
            pass: true
        };
    },

    toBeValidEID(received: Cookie) {
        expect(received.name).toEqual('eid');
        expect(received.secure).toBeTruthy();
        expect(received.httpOnly).toBeTruthy();
        expect(received.sameSite).toEqual('Lax');
        expect(received.path).toEndWith('/auth/');
        expect(received.domain).toEqual('cloud.local-scytta.com');
        expect(received.expires).toBeUndefined();

        return {
            message: () => 'Cookie is a valid EID',
            pass: true
        };
    },

    toBeGuestUser(received: UserInfo) {
        expect(uuidValidate(received.userId)).toBeTruthy();
        expect(received.name).toStartWith('Freshman_');
        expect(received.sessionLength).toBeGreaterThanOrEqual(0);
        expect(received.roles).toEqual([]);

        return {
            message: () => 'User is a guest user',
            pass: true
        };
    }
});
