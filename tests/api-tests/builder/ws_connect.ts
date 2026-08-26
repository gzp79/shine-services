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
// camelCased): client sends {type:'chat', text}; the server persists it to the room stream and the
// chat dispatcher later pushes a batch {type:'chat', messages:[{id, from, text}]}.
type WsChatComment = { id: string; from: string; text: string };
type WsChatBatch = { type: 'chat'; messages: WsChatComment[] };

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

        ws.once('close', (code: number) => {
            finish({ kind: 'closed', code });
        });
    });

    return { opened, settled };
}

type WsChatSession = {
    // Resolves with the next chat comment whose predicate matches, or rejects on timeout/close.
    waitComment: (predicate: (comment: WsChatComment) => boolean, timeoutMs?: number) => Promise<WsChatComment>;
    // Resolves once `count` comments matching the predicate have arrived, in arrival order.
    waitComments: (
        predicate: (comment: WsChatComment) => boolean,
        count: number,
        timeoutMs?: number
    ) => Promise<WsChatComment[]>;
    send: (text: string) => void;
    sendRaw: (frame: string) => void;
    close: () => void;
};

// Opens an authenticated socket and keeps it live. The server pushes chat as batches; we flatten
// each batch into its comments so a caller can send a frame and await the specific comment the chat
// dispatcher later fans back in. Used by the message round-trip test.
async function openChatSession(url: string, options: WsConnectOptions): Promise<WsChatSession> {
    const ws = new WebSocket(url, {
        origin: options.origin,
        headers: buildHeaders(options),
        rejectUnauthorized: false
    });

    const buffered: WsChatComment[] = [];
    const waiters: Array<{
        predicate: (comment: WsChatComment) => boolean;
        resolve: (comment: WsChatComment) => void;
    }> = [];

    const offer = (comment: WsChatComment) => {
        const idx = waiters.findIndex((w) => w.predicate(comment));
        if (idx >= 0) {
            waiters.splice(idx, 1)[0].resolve(comment);
        } else {
            buffered.push(comment);
        }
    };

    ws.on('message', (data: Buffer) => {
        const batch = JSON.parse(data.toString()) as WsChatBatch;
        for (const comment of batch.messages) {
            offer(comment);
        }
    });

    await new Promise<void>((resolve, reject) => {
        ws.once('open', () => resolve());
        ws.once('unexpected-response', (_req: unknown, res: IncomingMessage) =>
            reject(new Error(`unexpected upgrade response: ${res.statusCode}`))
        );
        ws.once('error', (err: Error) => reject(err));
    });

    const waitComment = (predicate: (comment: WsChatComment) => boolean, timeoutMs = 5000) =>
        new Promise<WsChatComment>((resolve, reject) => {
            const idx = buffered.findIndex(predicate);
            if (idx >= 0) {
                resolve(buffered.splice(idx, 1)[0]);
                return;
            }
            const timer = setTimeout(
                () => reject(new Error(`no matching chat comment after ${timeoutMs}ms`)),
                timeoutMs
            );
            waiters.push({
                predicate,
                resolve: (comment) => {
                    clearTimeout(timer);
                    resolve(comment);
                }
            });
        });

    return {
        waitComment,
        waitComments: async (predicate, count, timeoutMs = 5000) => {
            const collected: WsChatComment[] = [];
            for (let i = 0; i < count; i++) {
                collected.push(await waitComment(predicate, timeoutMs));
            }
            return collected;
        },
        send: (text: string) => ws.send(JSON.stringify({ type: 'chat', text })),
        sendRaw: (frame: string) => ws.send(frame),
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

    test('WS connect shall reject missing Origin header', async ({ builderWSUrl }) => {
        const result = await connectWs(wsConnectUrl(builderWSUrl), {
            sid: user.sessionCookie,
            extraHeaders: { 'x-forwarded-host': new URL(builderWSUrl).host }
        });

        expect(result).toEqual({ kind: 'http_error', status: 400 });
    });

    test('WS connect shall reject disallowed Origin header', async ({ builderWSUrl }) => {
        const result = await connectWs(wsConnectUrl(builderWSUrl), {
            sid: user.sessionCookie,
            origin: 'https://example.com',
            extraHeaders: { 'x-forwarded-host': new URL(builderWSUrl).host }
        });

        expect(result).toEqual({ kind: 'http_error', status: 403 });
    });

    test('WS connect shall reject non-ws host even with allowed origin', async ({ homeUrl, builderUrl }) => {
        const result = await connectWs(wsConnectUrl(builderUrl), {
            sid: user.sessionCookie,
            ...wsHeaders(homeUrl, builderUrl)
        });

        expect(result).toEqual({ kind: 'http_error', status: 403 });
    });

    test('WS connect shall allow configured Origin header', async ({ homeUrl, builderWSUrl }) => {
        const result = await connectWs(wsConnectUrl(builderWSUrl), {
            sid: user.sessionCookie,
            ...wsHeaders(homeUrl, builderWSUrl)
        });

        expect(result).toEqual({ kind: 'open' });
    });

    // Auth is the primary gate: with a valid origin/host, a missing or tampered session must still
    // be rejected with 401 before any socket opens.
    for (const authCase of [
        { title: 'missing session cookie', sid: undefined },
        { title: 'tampered session cookie', sid: 'not-a-valid-signed-sid' }
    ]) {
        test(`WS connect with ${authCase.title} shall be rejected`, async ({ homeUrl, builderWSUrl }) => {
            const result = await connectWs(wsConnectUrl(builderWSUrl), {
                sid: authCase.sid,
                ...wsHeaders(homeUrl, builderWSUrl)
            });

            expect(result).toEqual({ kind: 'http_error', status: 401 });
        });
    }

    test('WS chat message shall be persisted and fanned back to the sender', async ({ homeUrl, builderWSUrl }) => {
        const session = await openChatSession(wsConnectUrl(builderWSUrl), {
            sid: user.sessionCookie,
            ...wsHeaders(homeUrl, builderWSUrl)
        });

        try {
            // A sent chat is persisted to the room stream and then delivered by the chat dispatcher
            // on its next tick (syncIntervalSeconds=1 in test config), not echoed synchronously. The
            // unique text distinguishes our comment from any retained room history the dispatcher
            // replays to a freshly connected client.
            const text = `hello ${randomUUID()}`;
            session.send(text);

            const comment = await session.waitComment((c) => c.text === text);
            expect(comment).toMatchObject({ from: user.userId, text });
            // The dispatcher tags each comment with its ordered Redis stream id (`<ms>-<seq>`) so the
            // client can order and deduplicate under at-least-once delivery.
            expect(comment.id).toMatch(/^\d+-\d+$/);
        } finally {
            session.close();
        }
    });

    test('WS chat messages shall be delivered in order without duplicates', async ({ homeUrl, builderWSUrl }) => {
        const session = await openChatSession(wsConnectUrl(builderWSUrl), {
            sid: user.sessionCookie,
            ...wsHeaders(homeUrl, builderWSUrl)
        });

        try {
            // A single tag isolates this test's comments from retained room history. Several messages
            // sent back to back must come back in send order and each exactly once — the dispatcher
            // advances a per-connection stream cursor so a comment is never redelivered.
            const tag = randomUUID();
            const texts = [0, 1, 2, 3, 4].map((n) => `${tag} msg-${n}`);
            for (const text of texts) {
                session.send(text);
            }

            const received = await session.waitComments((c) => c.text.startsWith(tag), texts.length);
            expect(received.map((c) => c.text)).toEqual(texts);
            // Stream ids are strictly increasing, matching send order.
            const ids = received.map((c) => c.id);
            expect([...ids].sort()).toEqual(ids);
            expect(new Set(ids).size).toBe(ids.length);

            // Nothing more with this tag should arrive after the expected batch (no duplicate fan-out).
            await expect(session.waitComment((c) => c.text.startsWith(tag), 1500)).rejects.toThrow();
        } finally {
            session.close();
        }
    });

    test('WS chat shall fan out between two connected users in both directions', async ({ homeUrl, builderWSUrl }) => {
        const url = wsConnectUrl(builderWSUrl);
        const headers = wsHeaders(homeUrl, builderWSUrl);
        const other = await mint.createUserSession({ userId: randomUUID() });

        const alice = await openChatSession(url, { sid: user.sessionCookie, ...headers });
        const bob = await openChatSession(url, { sid: other.sessionCookie, ...headers });

        try {
            // Chat is a single global room: each user's comment is fanned out by the dispatcher to
            // every connected user (sender included), tagged with the author's id.
            const fromAlice = `alice ${randomUUID()}`;
            const fromBob = `bob ${randomUUID()}`;
            alice.send(fromAlice);
            bob.send(fromBob);

            // Bob receives Alice's message and vice versa, each attributed to its author.
            expect(await bob.waitComment((c) => c.text === fromAlice)).toMatchObject({ from: user.userId });
            expect(await alice.waitComment((c) => c.text === fromBob)).toMatchObject({ from: other.userId });
            // Each author also sees their own message fanned back.
            expect(await alice.waitComment((c) => c.text === fromAlice)).toMatchObject({ from: user.userId });
            expect(await bob.waitComment((c) => c.text === fromBob)).toMatchObject({ from: other.userId });
        } finally {
            alice.close();
            bob.close();
        }
    });

    test('WS chat shall replay recent history to a newly connected user', async ({ homeUrl, builderWSUrl }) => {
        const url = wsConnectUrl(builderWSUrl);
        const headers = wsHeaders(homeUrl, builderWSUrl);

        // A user sends messages, then a second user connects fresh. Because a new connection starts
        // from cursor "0", the dispatcher replays the retained room history to it.
        const tag = randomUUID();
        const texts = [`${tag} past-0`, `${tag} past-1`];

        const author = await openChatSession(url, { sid: user.sessionCookie, ...headers });
        try {
            for (const text of texts) {
                author.send(text);
            }
            // Ensure the messages are persisted (author sees them) before the latecomer connects.
            await author.waitComments((c) => c.text.startsWith(tag), texts.length);

            const latecomer = await mint.createUserSession({ userId: randomUUID() });
            const late = await openChatSession(url, { sid: latecomer.sessionCookie, ...headers });
            try {
                const replayed = await late.waitComments((c) => c.text.startsWith(tag), texts.length);
                expect(replayed.map((c) => c.text)).toEqual(texts);
                expect(replayed.every((c) => c.from === user.userId)).toBe(true);
            } finally {
                late.close();
            }
        } finally {
            author.close();
        }
    });

    test('WS malformed chat frame shall be ignored without dropping the connection', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const session = await openChatSession(wsConnectUrl(builderWSUrl), {
            sid: user.sessionCookie,
            ...wsHeaders(homeUrl, builderWSUrl)
        });

        try {
            // The service logs and skips frames it cannot parse (bad JSON, unknown type, missing
            // field) rather than tearing down the socket. A valid chat sent afterwards must still
            // round-trip, proving the connection survived.
            session.sendRaw('this is not json');
            session.sendRaw(JSON.stringify({ type: 'unknown', text: 'x' }));
            session.sendRaw(JSON.stringify({ type: 'chat' }));

            const text = `after-garbage ${randomUUID()}`;
            session.send(text);

            const comment = await session.waitComment((c) => c.text === text);
            expect(comment).toMatchObject({ from: user.userId, text });
        } finally {
            session.close();
        }
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

        // The first socket must be force-closed by the hub; the second stays open.
        const firstResult = await first.settled;
        expect(firstResult.kind).toBe('closed');
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
        expect(result.kind).toBe('closed');
    });
});
