import { expect, test } from '$fixtures/setup';
import { MintedSession, SessionMint } from '$lib/mocks/session_mint';
import { DEFAULT_USER_AGENT, joinURL } from '$lib/utils';
import { randomUUID } from 'node:crypto';
import { IncomingMessage } from 'node:http';
import { WebSocket } from 'ws';

type WsOutcome = { kind: 'open' } | { kind: 'http_error'; status: number } | { kind: 'error'; message: string };
type WsCloseOutcome =
    | { kind: 'closed'; code: number }
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

// The wire protocol mirrors WSMessageRequest/WSMessageResponse in the service (internally tagged,
// camelCased): client sends {type:'chat', text}, server pushes {type:'chat', from, text}.
type WsChatIn = { type: 'chat'; from: string; text: string };

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

function wsConnectUrl(builderUrl: string): string {
    const httpUrl = joinURL(builderUrl, '/api/connect');
    const wsUrl = new URL(httpUrl);
    wsUrl.protocol = wsUrl.protocol === 'https:' ? 'wss:' : 'ws:';
    return wsUrl.toString();
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

        ws.once('close', (code: number) => {
            finish({ kind: 'closed', code });
        });
    });

    return { opened, settled };
}

type WsChatSession = {
    // Resolves with the next chat message whose predicate matches, or rejects on timeout/close.
    waitChat: (predicate: (msg: WsChatIn) => boolean, timeoutMs?: number) => Promise<WsChatIn>;
    send: (text: string) => void;
    close: () => void;
};

// Opens an authenticated socket and keeps it live, buffering inbound chat messages so a caller can
// send a frame and await the server's echo. Used by the message round-trip test.
async function openChatSession(url: string, options: WsConnectOptions): Promise<WsChatSession> {
    const ws = new WebSocket(url, {
        origin: options.origin,
        headers: buildHeaders(options),
        rejectUnauthorized: false
    });

    const buffered: WsChatIn[] = [];
    const waiters: Array<{ predicate: (msg: WsChatIn) => boolean; resolve: (msg: WsChatIn) => void }> = [];

    ws.on('message', (data: Buffer) => {
        const msg = JSON.parse(data.toString()) as WsChatIn;
        const idx = waiters.findIndex((w) => w.predicate(msg));
        if (idx >= 0) {
            waiters.splice(idx, 1)[0].resolve(msg);
        } else {
            buffered.push(msg);
        }
    });

    await new Promise<void>((resolve, reject) => {
        ws.once('open', () => resolve());
        ws.once('unexpected-response', (_req: unknown, res: IncomingMessage) =>
            reject(new Error(`unexpected upgrade response: ${res.statusCode}`))
        );
        ws.once('error', (err: Error) => reject(err));
    });

    return {
        waitChat: (predicate, timeoutMs = 5000) =>
            new Promise<WsChatIn>((resolve, reject) => {
                const idx = buffered.findIndex(predicate);
                if (idx >= 0) {
                    resolve(buffered.splice(idx, 1)[0]);
                    return;
                }
                const timer = setTimeout(
                    () => reject(new Error(`no matching chat message after ${timeoutMs}ms`)),
                    timeoutMs
                );
                waiters.push({
                    predicate,
                    resolve: (msg) => {
                        clearTimeout(timer);
                        resolve(msg);
                    }
                });
            }),
        send: (text: string) => ws.send(JSON.stringify({ type: 'chat', text })),
        close: () => ws.close()
    };
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

    test('WS connect shall reject missing Origin header', async ({ builderUrl }) => {
        const result = await connectWs(wsConnectUrl(builderUrl), {
            sid: user.sessionCookie,
            extraHeaders: {
                'x-forwarded-host': 'ws.local.scytta.com:8444'
            }
        });

        expect(result).toEqual({ kind: 'http_error', status: 400 });
    });

    test('WS connect shall reject disallowed Origin header', async ({ builderUrl }) => {
        const result = await connectWs(wsConnectUrl(builderUrl), {
            sid: user.sessionCookie,
            origin: 'https://example.com',
            extraHeaders: {
                'x-forwarded-host': 'ws.local.scytta.com:8444'
            }
        });

        expect(result).toEqual({ kind: 'http_error', status: 403 });
    });

    test('WS connect shall reject non-ws host even with allowed origin', async ({ builderUrl }) => {
        const result = await connectWs(wsConnectUrl(builderUrl), {
            sid: user.sessionCookie,
            origin: 'https://cloud.local.scytta.com:8443',
            extraHeaders: {
                'x-forwarded-host': 'cloud.local.scytta.com:8444'
            }
        });

        expect(result).toEqual({ kind: 'http_error', status: 403 });
    });

    test('WS connect shall allow configured Origin header', async ({ builderUrl }) => {
        const result = await connectWs(wsConnectUrl(builderUrl), {
            sid: user.sessionCookie,
            origin: 'https://cloud.local.scytta.com:8443',
            extraHeaders: {
                'x-forwarded-host': 'ws.local.scytta.com:8444'
            }
        });

        expect(result).toEqual({ kind: 'open' });
    });

    const allowedWsHeaders = {
        origin: 'https://cloud.local.scytta.com:8443',
        extraHeaders: { 'x-forwarded-host': 'ws.local.scytta.com:8444' }
    };

    // Auth is the primary gate: with a valid origin/host, a missing or tampered session must still
    // be rejected with 401 before any socket opens.
    for (const authCase of [
        { title: 'missing session cookie', sid: undefined },
        { title: 'tampered session cookie', sid: 'not-a-valid-signed-sid' }
    ]) {
        test(`WS connect with ${authCase.title} shall be rejected`, async ({ builderUrl }) => {
            const result = await connectWs(wsConnectUrl(builderUrl), {
                sid: authCase.sid,
                ...allowedWsHeaders
            });

            expect(result).toEqual({ kind: 'http_error', status: 401 });
        });
    }

    test('WS chat message shall be echoed back to the sender', async ({ builderUrl }) => {
        const session = await openChatSession(wsConnectUrl(builderUrl), {
            sid: user.sessionCookie,
            ...allowedWsHeaders
        });

        try {
            // The hub pushes a localized "Connected" greeting on connect; the echo path is the same
            // one a client-sent chat travels, so seeing it confirms the send_task/serialization path.
            const greeting = await session.waitChat((msg) => msg.from === user.userId);
            expect(greeting.type).toBe('chat');

            const text = `hello ${randomUUID()}`;
            session.send(text);

            const echo = await session.waitChat((msg) => msg.text === text);
            expect(echo).toMatchObject({ type: 'chat', from: user.userId, text });
        } finally {
            session.close();
        }
    });

    test('WS second connection for a user shall drop the first (last connection wins)', async ({ builderUrl }) => {
        const url = wsConnectUrl(builderUrl);

        // Invariant 2: a user has at most one connection; a new one kills the previous. This is
        // driven by the ConnectUser command, so no interval wait is needed.
        const first = openAndWaitClose(url, {
            sid: user.sessionCookie,
            ...allowedWsHeaders,
            holdTimeoutMs: 10_000
        });
        expect(await first.opened).toBe(true);

        const second = openAndWaitClose(url, {
            sid: user.sessionCookie,
            ...allowedWsHeaders,
            holdTimeoutMs: 10_000
        });
        expect(await second.opened).toBe(true);

        // The first socket must be force-closed by the hub; the second stays open.
        const firstResult = await first.settled;
        expect(firstResult.kind).toBe('closed');
    });

    test('WS connection shall be dropped when session is deleted', async ({ builderUrl }) => {
        const url = wsConnectUrl(builderUrl);
        const authCheckIntervalMs = 2_000;

        // SessionChecker runs every 2s in test config; allow several intervals
        // for detection, hub command processing, and websocket teardown.
        const closeCtx = openAndWaitClose(url, {
            sid: user.sessionCookie,
            ...allowedWsHeaders,
            holdTimeoutMs: authCheckIntervalMs * 4
        });

        expect(await closeCtx.opened).toBe(true);
        await mint.deleteUser(user);

        const result = await closeCtx.settled;
        expect(result.kind).toBe('closed');
    });
});
