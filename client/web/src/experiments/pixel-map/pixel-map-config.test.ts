import { describe, expect, it } from 'vitest';
import { DEFAULT_STREAM_ID, DEFAULT_WS_URL, resolveConfig, userIdFromCookie } from './pixel-map-config';

describe('resolveConfig', () => {
    it('uses the shared demo stream and the local builder by default', () => {
        const config = resolveConfig(new URLSearchParams(''));

        expect(config.streamId).toBe(DEFAULT_STREAM_ID);
        expect(config.wsUrl).toBe(DEFAULT_WS_URL);
    });

    it('prefixes a valid stream identifier with the pixel-map kind', () => {
        expect(resolveConfig(new URLSearchParams('stream=my-room.1')).streamId).toBe('pixel-map:my-room.1');
    });

    it('accepts every allowed identifier character at the length boundary', () => {
        const identifier = `${'a'.repeat(60)}_.:-`;

        expect(resolveConfig(new URLSearchParams(`stream=${identifier}`)).streamId).toBe(`pixel-map:${identifier}`);
    });

    it('falls back to the default for an id the server would not accept', () => {
        for (const bad of ['', 'a b', 'x'.repeat(65), 'ö']) {
            const params = new URLSearchParams();
            params.set('stream', bad);

            expect(resolveConfig(params).streamId).toBe(DEFAULT_STREAM_ID);
        }
    });

    it('takes a secure builder url from the query', () => {
        const url = 'wss://ws.local.scytta.com:8443/builder/api/connect';

        expect(resolveConfig(new URLSearchParams(`ws=${encodeURIComponent(url)}`)).wsUrl).toBe(url);
    });

    it('keeps the default for a url that is not a secure websocket', () => {
        for (const bad of ['ws://ws.local.scytta.com:8444/builder/api/connect', 'https://example.com', 'nonsense']) {
            const params = new URLSearchParams();
            params.set('ws', bad);

            expect(resolveConfig(params).wsUrl).toBe(DEFAULT_WS_URL);
        }
    });
});

describe('userIdFromCookie', () => {
    const id = 'b28bfd48-d20e-4181-ac79-4bde3255e211';
    const sid = encodeURIComponent(`c2ln{"u":"${id}","key":"k","fp":"f"}`);

    it('reads the user id of the session cookie', () => {
        expect(userIdFromCookie(`a=1; sid=${sid}; b=2`)).toBe(id);
    });

    it('is null without a readable session', () => {
        expect(userIdFromCookie('')).toBeNull();
        expect(userIdFromCookie('a=1')).toBeNull();
        expect(userIdFromCookie('sid=%E0%A4%A')).toBeNull();
        expect(userIdFromCookie('sid=garbage')).toBeNull();
    });
});
