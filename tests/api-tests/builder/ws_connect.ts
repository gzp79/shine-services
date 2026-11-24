import { expect, test } from '$fixtures/setup';
import { type WsConnectOptions, buildHeaders, wsConnectUrl, wsHeaders } from '$lib/api/builder_ws';
import { MintedSession, SessionMint } from '$lib/mocks/session_mint';
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
});
