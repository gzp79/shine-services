import { Cookie } from 'tough-cookie';
import { intoMatcherResult } from './utils';
import { expect } from './common';

const toBeClearCookie = async (received: Cookie) => {
    expect(received.secure).toBe(true);
    expect(received.httpOnly).toBe(true);
    expect(received.sameSite).toBe('lax');
    expect(new Date(received.expires)).toBeBefore(new Date());
};

const toBeValidTID = async (received: Cookie) => {
    expect(received.key).toEqual('tid');
    expect(received.secure).toEqual(true);
    expect(received.httpOnly).toEqual(true);
    expect(received.sameSite).toEqual('lax');
    expect(received.path).toEndWith('/auth/');
    expect(received.domain).toEqual('cloud.local-scytta.com');
    expect(received.expires).toBeAfter(new Date());
};

const toBeValidSID = async (received: Cookie) => {
    expect(received.key).toEqual('sid');
    expect(received.secure).toEqual(true);
    expect(received.httpOnly).toEqual(true);
    expect(received.sameSite).toEqual('lax');
    expect(received.path).toEqual('/');
    expect(received.domain).toEqual('local-scytta.com');
    expect(received.expires).toEqual('Infinity'); // session scoped
};

const toBeValidEID = async (received: Cookie) => {
    expect(received.key).toEqual('eid');
    expect(received.secure).toEqual(true);
    expect(received.httpOnly).toEqual(true);
    expect(received.sameSite).toEqual('lax');
    expect(received.path).toEndWith('/auth/');
    expect(received.domain).toEqual('cloud.local-scytta.com');
    expect(received.expires).toEqual('Infinity');
};

export { toBeClearCookie, toBeValidTID, toBeValidSID, toBeValidEID };
