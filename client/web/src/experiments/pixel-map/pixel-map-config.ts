// Local-dev defaults; the builder needs the `sid` session cookie of an identity login on the same
// parent domain (*.local.scytta.com).
export const DEFAULT_WS_URL = 'wss://ws.local.scytta.com:8443/builder/api/connect';
export const DEFAULT_STREAM_ID = 'pixel-demo';

export interface PixelMapConfig {
    wsUrl: string;
    streamId: string;
}

// The rules of a stream id of the server.
const STREAM_ID = /^[A-Za-z0-9_.:-]{1,64}$/;

export function resolveConfig(search: URLSearchParams): PixelMapConfig {
    const stream = search.get('stream');
    return {
        wsUrl: DEFAULT_WS_URL,
        streamId: stream !== null && STREAM_ID.test(stream) ? stream : DEFAULT_STREAM_ID
    };
}

// The user id inside the `sid` session cookie. Only a cookie that script can read has it (the local
// dev one); a real HttpOnly session gives null.
export function userIdFromCookie(cookies: string): string | null {
    const sid = /(?:^|;\s*)sid=([^;]*)/.exec(cookies)?.[1];
    if (!sid) return null;
    let value: string;
    try {
        value = decodeURIComponent(sid);
    } catch {
        return null;
    }
    return /"u":"([0-9a-f-]{36})"/.exec(value)?.[1] ?? null;
}
