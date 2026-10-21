import { expect, test } from '$fixtures/setup';
import { StreamSocket, simulateRegistryTakeover, wsConnectUrl, wsOptions } from '$lib/api/builder_stream_ws';
import { MintedSession, SessionMint } from '$lib/mocks/session_mint';
import { DEFAULT_USER_AGENT } from '$lib/utils';
import { randomUUID } from 'node:crypto';
import { IncomingMessage } from 'node:http';
import { Toxiproxy } from 'toxiproxy-node-client';
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

    // Invariant 2: a user has at most one connection; a new one kills the previous, whether it is the
    // same session reconnecting or another session of the same user. This is driven by the
    // ConnectUser command, so no interval wait is needed.
    for (const secondSessionCase of [
        { title: 'the same session', secondSession: () => user },
        {
            title: 'another session of the same user',
            secondSession: () => mint.createUserSession({ userId: user.userId })
        }
    ]) {
        test(`WS second connection for a user shall close the first with replaced and keep serving the second (${secondSessionCase.title})`, async ({
            homeUrl,
            builderWSUrl
        }) => {
            const first = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, user.sessionCookie));
            const secondSession = await secondSessionCase.secondSession();
            const second = await StreamSocket.open(
                builderWSUrl,
                wsOptions(homeUrl, builderWSUrl, secondSession.sessionCookie)
            );

            expect(await first.closed).toEqual({ code: HUB_DROPPED_CLOSE_CODE, reason: 'replaced' });

            // The second socket keeps serving: it can still subscribe and get a command delivered.
            second.subscribe('chat-global', 'chat');
            await second.next('streamSync');
            const commandId = randomUUID();
            second.command('chat-global', commandId, { type: 'add', text: `still here ${randomUUID()}` });
            await second.next('streamCommands', (frame) => frame.commands.some((c) => c.commandId === commandId));

            second.close();
        });
    }

    test('WS connection taken over through the registry shall close with registry_inactive', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const url = wsConnectUrl(builderWSUrl);

        const closeCtx = openAndWaitClose(url, {
            sid: user.sessionCookie,
            ...wsHeaders(homeUrl, builderWSUrl),
            holdTimeoutMs: 10_000
        });
        expect(await closeCtx.opened).toBe(true);

        // Simulate another instance taking this user's row over directly in Redis (no second
        // instance needed): the hub's registry listener reacts to the change notice.
        await simulateRegistryTakeover(user.userId);

        const result = await closeCtx.settled;
        expect(result).toEqual({ kind: 'closed', code: HUB_DROPPED_CLOSE_CODE, reason: 'registry_inactive' });
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

test.describe('Builder websocket failures', { tag: ['@infrastructure'] }, () => {
    // Shares the redis proxy with the other @infrastructure files; keep this file's own
    // infra tests serial too, so a failure here can't leave the proxy disabled for a later file.
    test.describe.configure({ mode: 'serial' });

    let toxiproxy: Toxiproxy;
    let mint: SessionMint;
    let user: MintedSession;

    async function enableAllProxies(): Promise<void> {
        for (const name of ['redis', 'postgres']) {
            const proxy = await toxiproxy.get(name);
            await proxy.update({ enabled: true, listen: proxy.listen, upstream: proxy.upstream });
        }
    }

    test.beforeAll(async () => {
        toxiproxy = new Toxiproxy('http://localhost:8474');
        await enableAllProxies();
    });

    test.beforeEach(async () => {
        // SessionMint writes straight into Redis, so mint before the test disables the proxy.
        mint = await SessionMint.fromServerConfig();
        user = await mint.createUserSession({ userId: randomUUID() });
    });

    test.afterEach(async () => {
        await toxiproxy.reset();
        await enableAllProxies();
        await mint.teardownCreatedSessions();
    });

    test('WS connect shall be refused while Redis is down and accepted after recovery', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const url = wsConnectUrl(builderWSUrl);
        const options = { sid: user.sessionCookie, ...wsHeaders(homeUrl, builderWSUrl) };

        const redisProxy = await toxiproxy.get('redis');
        await redisProxy.update({ enabled: false, listen: redisProxy.listen, upstream: redisProxy.upstream });

        // The session check needs Redis, so the upgrade is refused before it opens a socket. The
        // session pool's own pool_timeout (5s in the test config) is the slow path here, so give
        // the client a longer timeout than that rather than racing it.
        const down = await connectWs(url, { ...options, timeoutMs: 8_000 });
        expect(down).toEqual({ kind: 'http_error', status: 503 });

        await redisProxy.update({ enabled: true, listen: redisProxy.listen, upstream: redisProxy.upstream });

        const recovered = await connectWs(url, options);
        expect(recovered).toEqual({ kind: 'open' });
    });
});
