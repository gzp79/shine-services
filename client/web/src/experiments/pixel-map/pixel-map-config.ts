// Local-dev defaults; the builder needs the `sid` session cookie of an identity login on the same
// parent domain (*.local.scytta.com). Without nginx in front, the local builder serves 8444 itself
// (8443 is identity, which answers /builder/* with a 404); the test stage behind nginx uses 8443.
export const DEFAULT_WS_URL = 'wss://ws.local.scytta.com:8444/builder/api/connect';
const STREAM_KIND = 'pixel-map';
const DEFAULT_STREAM_IDENTIFIER = 'pixel-demo';
export const DEFAULT_STREAM_ID = `${STREAM_KIND}:${DEFAULT_STREAM_IDENTIFIER}`;

export interface PixelMapConfig {
    wsUrl: string;
    streamId: string;
}

// This validates the identifier portion. The wire stream ID carries the fixed pixel-map kind.
const STREAM_IDENTIFIER = /^[A-Za-z0-9_.:-]{1,64}$/;

export function resolveConfig(search: URLSearchParams): PixelMapConfig {
    const stream = search.get('stream');
    return {
        wsUrl: wsUrlFrom(search.get('ws')),
        streamId: stream !== null && STREAM_IDENTIFIER.test(stream) ? `${STREAM_KIND}:${stream}` : DEFAULT_STREAM_ID
    };
}

// `?ws=` points the experiment at another builder (a different port behind nginx, say). Only a
// secure websocket is taken: anything else is a typo rather than a reachable local builder.
function wsUrlFrom(value: string | null): string {
    if (value === null) return DEFAULT_WS_URL;
    try {
        const url = new URL(value);
        return url.protocol === 'wss:' ? url.toString() : DEFAULT_WS_URL;
    } catch {
        return DEFAULT_WS_URL;
    }
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
