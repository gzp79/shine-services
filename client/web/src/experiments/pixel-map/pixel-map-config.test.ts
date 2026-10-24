import { describe, expect, it } from 'vitest';
import { DEFAULT_STREAM_ID, DEFAULT_WS_URL, resolveConfig, userIdFromCookie } from './pixel-map-config';

describe('resolveConfig', () => {
    it('uses the shared demo stream and the local builder by default', () => {
        const config = resolveConfig(new URLSearchParams(''));

        expect(config.streamId).toBe(DEFAULT_STREAM_ID);
        expect(config.wsUrl).toBe(DEFAULT_WS_URL);
    });

    it('takes the stream from the query', () => {
        expect(resolveConfig(new URLSearchParams('stream=my-room.1')).streamId).toBe('my-room.1');
    });

    it('falls back to the default for an id the server would not accept', () => {
        for (const bad of ['', 'a b', 'x'.repeat(65), 'ö']) {
            const params = new URLSearchParams();
            params.set('stream', bad);

            expect(resolveConfig(params).streamId).toBe(DEFAULT_STREAM_ID);
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
