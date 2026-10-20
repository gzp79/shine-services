import { expect, test } from '$fixtures/setup';
import { StreamSocket, wsConnectUrl, wsOptions } from '$lib/api/builder_stream_ws';
import { MintedSession, SessionMint } from '$lib/mocks/session_mint';
import { DEFAULT_USER_AGENT } from '$lib/utils';
import { randomUUID } from 'node:crypto';
import { IncomingMessage } from 'node:http';
import { WebSocket } from 'ws';

// Close code of a socket the hub dropped; the close reason carries the drop reason.
const HUB_DROPPED_CLOSE_CODE = 4000;

type WsOutcome = { kind: 'open' } | { kind: 'http_error'; status: number } | { kind: 'error'; message: string };

type WsCloseOutcome =
    | { kind: 'closed'; code: number; reason: string }
    | { kind: 'http_error'; status: number }
    | { kind: 'error'; message: string };

type WsHoldResult = {
    opened: Promise<boolean>;
    settled: Promise<WsCloseOutcome>;
};

type WsConnectOptions = {
    // Omit to send no Cookie header at all (unauthenticated); pass a bad value to test a tampered sid.
    sid?: string;
    origin?: string;
    userAgent?: string;
    extraHeaders?: Record<string, string>;
    timeoutMs?: number;
};

function buildHeaders(options: WsConnectOptions): Record<string, string> {
    const headers: Record<string, string> = {
        'User-Agent': options.userAgent ?? DEFAULT_USER_AGENT,
        ...(options.extraHeaders ?? {})
    };
    if (options.sid !== undefined) {
        headers.Cookie = `sid=${options.sid}`;
    }
    return headers;
}

async function connectWs(url: string, options: WsConnectOptions): Promise<WsOutcome> {
    return await new Promise<WsOutcome>((resolve) => {
        const timeoutMs = options.timeoutMs ?? 5000;
        const headers = buildHeaders(options);

        const ws = new WebSocket(url, {
            origin: options.origin,
            headers,
            rejectUnauthorized: false
        });

        let settled = false;

        const finish = (value: WsOutcome) => {
            if (settled) {
                return;
            }
            settled = true;
            clearTimeout(timeout);
            resolve(value);
        };

        const timeout = setTimeout(() => {
            ws.terminate();
            finish({ kind: 'error', message: `timeout after ${timeoutMs}ms` });
        }, timeoutMs);

        ws.once('open', () => {
            ws.close();
            finish({ kind: 'open' });
        });

        ws.once('unexpected-response', (_request: unknown, response: IncomingMessage) => {
            finish({ kind: 'http_error', status: response.statusCode ?? 0 });
        });

        ws.once('error', (error: Error) => {
            finish({ kind: 'error', message: error.message });
        });
    });
}

// x-forwarded-host mirrors the endpoint authority so the builder resolves the same host whether it
// reads the header directly (bare service) or the proxy rewrites it from the connected host.
function wsHeaders(homeUrl: string, endpointUrl: string): WsConnectOptions {
    return {
        origin: new URL(homeUrl).origin,
        extraHeaders: { 'x-forwarded-host': new URL(endpointUrl).host }
    };
}

type WsHoldOptions = WsConnectOptions & { holdTimeoutMs?: number };

function openAndWaitClose(url: string, options: WsHoldOptions): WsHoldResult {
    let resolveOpened!: (opened: boolean) => void;
    const opened = new Promise<boolean>((resolve) => {
        resolveOpened = resolve;
    });

    const settled = new Promise<WsCloseOutcome>((resolve) => {
        const holdTimeoutMs = options.holdTimeoutMs ?? 10_000;
        const headers = buildHeaders(options);

        const ws = new WebSocket(url, {
            origin: options.origin,
            headers,
            rejectUnauthorized: false
        });

        let settled = false;
        let wasOpened = false;
        let openedResolved = false;

        const markOpened = (value: boolean) => {
            if (openedResolved) {
                return;
            }
            openedResolved = true;
            resolveOpened(value);
        };

        const finish = (value: WsCloseOutcome) => {
            if (settled) return;
            settled = true;
            clearTimeout(timeout);
            markOpened(false);
            resolve(value);
        };

        const timeout = setTimeout(() => {
            ws.terminate();
            finish({ kind: 'error', message: `timeout after ${holdTimeoutMs}ms` });
        }, holdTimeoutMs);

        ws.once('unexpected-response', (_request: unknown, response: IncomingMessage) => {
            finish({ kind: 'http_error', status: response.statusCode ?? 0 });
        });

        ws.once('open', () => {
            wasOpened = true;
            markOpened(true);
        });

        ws.once('error', (error: Error) => {
            // After a successful upgrade, forced disconnects can surface as
            // either close or error depending on timing and transport teardown.
            if (wasOpened) {
                return;
            }
            finish({ kind: 'error', message: error.message });
        });

        ws.once('close', (code: number, reason: Buffer) => {
            finish({ kind: 'closed', code, reason: reason.toString() });
        });
    });

    return { opened, settled };
}

test.describe('Builder websocket', { tag: ['@regression'] }, () => {
    let mint: SessionMint;
    let user: MintedSession;

    test.beforeEach(async () => {
        mint = await SessionMint.fromServerConfig();
        user = await mint.createUserSession({ userId: randomUUID() });
    });

    test.afterEach(async () => {
        await mint.teardownCreatedSessions();
    });

    test('WS connect shall reject a malformed or unauthenticated request before opening a socket', async ({
        homeUrl,
        builderUrl,
        builderWSUrl
    }) => {
        for (const rejectCase of [
            {
                title: 'missing Origin header',
                status: 400,
                url: wsConnectUrl(builderWSUrl),
                options: {
                    sid: user.sessionCookie,
                    extraHeaders: { 'x-forwarded-host': new URL(builderWSUrl).host }
                }
            },
            {
                title: 'disallowed Origin header',
                status: 403,
                url: wsConnectUrl(builderWSUrl),
                options: {
                    sid: user.sessionCookie,
                    origin: 'https://example.com',
                    extraHeaders: { 'x-forwarded-host': new URL(builderWSUrl).host }
                }
            },
            {
                title: 'non-ws host even with an allowed origin',
                status: 403,
                url: wsConnectUrl(builderUrl),
                options: { sid: user.sessionCookie, ...wsHeaders(homeUrl, builderUrl) }
            },
            {
                title: 'missing session cookie',
                status: 401,
                url: wsConnectUrl(builderWSUrl),
                options: wsHeaders(homeUrl, builderWSUrl)
            },
            {
                title: 'tampered session cookie',
                status: 401,
                url: wsConnectUrl(builderWSUrl),
                options: { sid: 'not-a-valid-signed-sid', ...wsHeaders(homeUrl, builderWSUrl) }
            }
        ]) {
            const result = await connectWs(rejectCase.url, rejectCase.options);
            expect(result, rejectCase.title).toEqual({ kind: 'http_error', status: rejectCase.status });
        }
    });

    // The global chat is the only stream open to users; subscribing to it and getting a sync proves
    // the socket is usable, not just that the upgrade was accepted. (A `hello` frame joins this once WS-11 lands.)
    test('WS connect shall allow a configured Origin header and let the socket subscribe and sync', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, user.sessionCookie));
        socket.subscribe('chat-global', 'chat');
        const sync = await socket.next('streamSync');
        expect(sync.streamId).toBe('chat-global');
        socket.close();
    });

    test('WS second connection for a user shall drop the first (last connection wins)', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const url = wsConnectUrl(builderWSUrl);
        const headers = wsHeaders(homeUrl, builderWSUrl);

        // Invariant 2: a user has at most one connection; a new one kills the previous. This is
        // driven by the ConnectUser command, so no interval wait is needed.
        const first = openAndWaitClose(url, {
            sid: user.sessionCookie,
            ...headers,
            holdTimeoutMs: 10_000
        });
        expect(await first.opened).toBe(true);

        const second = openAndWaitClose(url, {
            sid: user.sessionCookie,
            ...headers,
            holdTimeoutMs: 10_000
        });
        expect(await second.opened).toBe(true);

        // The first socket must be force-closed by the hub, telling the client why; the second stays open.
        const firstResult = await first.settled;
        expect(firstResult).toEqual({ kind: 'closed', code: HUB_DROPPED_CLOSE_CODE, reason: 'replaced' });
    });

    test('WS connection shall be dropped when session is deleted', async ({ homeUrl, builderWSUrl }) => {
        const url = wsConnectUrl(builderWSUrl);
        const authCheckIntervalMs = 2_000;

        // SessionChecker runs every 2s in test config; allow several intervals
        // for detection, hub command processing, and websocket teardown.
        const closeCtx = openAndWaitClose(url, {
            sid: user.sessionCookie,
            ...wsHeaders(homeUrl, builderWSUrl),
            holdTimeoutMs: authCheckIntervalMs * 4
        });

        expect(await closeCtx.opened).toBe(true);
        await mint.deleteUser(user);

        const result = await closeCtx.settled;
        expect(result).toEqual({ kind: 'closed', code: HUB_DROPPED_CLOSE_CODE, reason: 'session_invalid' });
    });
});
