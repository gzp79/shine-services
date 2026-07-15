import { expect, test } from '$fixtures/setup';
import { MintedSession, SessionMint } from '$lib/mocks/session_mint';
import { DEFAULT_USER_AGENT, joinURL } from '$lib/utils';
import { randomUUID } from 'node:crypto';
import { IncomingMessage } from 'node:http';
import { WebSocket } from 'ws';

type WsOutcome = { kind: 'open' } | { kind: 'http_error'; status: number } | { kind: 'error'; message: string };

type WsConnectOptions = {
    sid: string;
    origin?: string;
    userAgent?: string;
    extraHeaders?: Record<string, string>;
    timeoutMs?: number;
};

async function connectWs(url: string, options: WsConnectOptions): Promise<WsOutcome> {
    return await new Promise<WsOutcome>((resolve) => {
        const timeoutMs = options.timeoutMs ?? 5000;
        const headers: Record<string, string> = {
            Cookie: `sid=${options.sid}`,
            'User-Agent': options.userAgent ?? DEFAULT_USER_AGENT,
            ...(options.extraHeaders ?? {})
        };

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
    const httpUrl = joinURL(builderUrl, `/api/connect/${randomUUID()}`);
    const wsUrl = new URL(httpUrl);
    wsUrl.protocol = wsUrl.protocol === 'https:' ? 'wss:' : 'ws:';
    return wsUrl.toString();
}

test.describe('Builder websocket origin checks', { tag: ['@regression'] }, () => {
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
});
