import { DEFAULT_USER_AGENT, joinURL } from '$lib/utils';
import { IncomingMessage } from 'node:http';
import { WebSocket } from 'ws';

export type WsConnectOptions = {
    // Omit to send no Cookie header at all (unauthenticated); pass a bad value to test a tampered sid.
    sid?: string;
    origin?: string;
    userAgent?: string;
    extraHeaders?: Record<string, string>;
    timeoutMs?: number;
};

export function buildHeaders(options: WsConnectOptions): Record<string, string> {
    const headers: Record<string, string> = {
        'User-Agent': options.userAgent ?? DEFAULT_USER_AGENT,
        ...(options.extraHeaders ?? {})
    };
    if (options.sid !== undefined) {
        headers.Cookie = `sid=${options.sid}`;
    }
    return headers;
}

export function wsConnectUrl(builderUrl: string): string {
    const httpUrl = joinURL(builderUrl, '/api/connect');
    const wsUrl = new URL(httpUrl);
    wsUrl.protocol = wsUrl.protocol === 'https:' ? 'wss:' : 'ws:';
    return wsUrl.toString();
}

// x-forwarded-host mirrors the endpoint authority so the builder resolves the same host whether it
// reads the header directly (bare service) or the proxy rewrites it from the connected host.
export function wsHeaders(homeUrl: string, endpointUrl: string): WsConnectOptions {
    return {
        origin: new URL(homeUrl).origin,
        extraHeaders: { 'x-forwarded-host': new URL(endpointUrl).host }
    };
}

// The stream frames mirror WSMessageRequest/WSMessageResponse in the service (internally tagged,
// camelCased). Hashes are hex text, snapshot data is base64, and a command is the JSON of its kind.
export type StreamCommandFrame = {
    version: number;
    commandId: string;
    userId: string;
    command: Record<string, unknown>;
};

export type StreamSnapshotFrame = { version: number; chainHash: string; stateHash: string; data: string };

export type StreamFrame =
    | { type: 'streamSync'; streamId: string; snapshot: StreamSnapshotFrame | null; commands: StreamCommandFrame[] }
    | { type: 'streamCommands'; streamId: string; commands: StreamCommandFrame[] }
    | { type: 'streamCheckpoint'; streamId: string; version: number; stateHash: string }
    | { type: 'streamReset'; streamId: string }
    | { type: 'streamUnsynced'; streamId: string }
    | { type: 'streamNack'; streamId: string; commandId: string; reason: 'notSubscribed' | 'invalid' | 'failed' }
    | {
          type: 'streamSubscribeFailed';
          streamId: string;
          reason: 'unauthorized' | 'failed';
      };

export type StreamFrameOf<T extends StreamFrame['type']> = Extract<StreamFrame, { type: T }>;

// An authenticated socket kept live to talk streams. Frames are buffered in arrival order and each
// can be taken once by `waitFrame`, so a test reads the conversation like a script.
export class StreamSession {
    private readonly queue: StreamFrame[] = [];
    private readonly waiters: Array<{ predicate: (frame: StreamFrame) => boolean; resolve: (f: StreamFrame) => void }> =
        [];

    private constructor(private readonly ws: WebSocket) {
        ws.on('message', (data: Buffer) => {
            const frame = JSON.parse(data.toString()) as StreamFrame;
            const index = this.waiters.findIndex((w) => w.predicate(frame));
            if (index >= 0) {
                this.waiters.splice(index, 1)[0].resolve(frame);
            } else {
                this.queue.push(frame);
            }
        });
    }

    static async open(url: string, options: WsConnectOptions): Promise<StreamSession> {
        // The local service has a self-signed certificate; the Playwright config accepts it for the whole run.
        const ws = new WebSocket(url, {
            origin: options.origin,
            headers: buildHeaders(options)
        });
        const session = new StreamSession(ws);
        await new Promise<void>((resolve, reject) => {
            ws.once('open', () => resolve());
            ws.once('unexpected-response', (_req: unknown, res: IncomingMessage) =>
                reject(new Error(`unexpected upgrade response: ${res.statusCode}`))
            );
            ws.once('error', (err: Error) => reject(err));
        });
        return session;
    }

    send(frame: object): void {
        this.ws.send(JSON.stringify(frame));
    }

    sendRaw(text: string): void {
        this.ws.send(text);
    }

    // The next frame (not yet taken) that matches, or a rejection after `timeoutMs`.
    waitFrame<T extends StreamFrame['type']>(
        type: T,
        predicate: (frame: StreamFrameOf<T>) => boolean = () => true,
        timeoutMs = 5000
    ): Promise<StreamFrameOf<T>> {
        const matches = (frame: StreamFrame): frame is StreamFrameOf<T> =>
            frame.type === type && predicate(frame as StreamFrameOf<T>);
        const index = this.queue.findIndex(matches);
        if (index >= 0) {
            return Promise.resolve(this.queue.splice(index, 1)[0] as StreamFrameOf<T>);
        }
        return new Promise<StreamFrameOf<T>>((resolve, reject) => {
            const waiter = {
                predicate: matches,
                resolve: (frame: StreamFrame) => {
                    clearTimeout(timer);
                    resolve(frame as StreamFrameOf<T>);
                }
            };
            // A waiter that gave up must not be left behind: it would swallow a later frame.
            const timer = setTimeout(() => {
                const at = this.waiters.indexOf(waiter);
                if (at >= 0) {
                    this.waiters.splice(at, 1);
                }
                reject(new Error(`no ${type} frame after ${timeoutMs}ms; got ${JSON.stringify(this.queue)}`));
            }, timeoutMs);
            this.waiters.push(waiter);
        });
    }

    // Resolves once no matching frame arrived for `quietMs`; rejects if one does.
    async expectNoFrame(
        type: StreamFrame['type'],
        predicate: (frame: StreamFrame) => boolean = () => true,
        quietMs = 1000
    ): Promise<void> {
        const arrived = await this.waitFrame(type, predicate, quietMs).catch(() => undefined);
        if (arrived) {
            throw new Error(`unexpected ${type} frame: ${JSON.stringify(arrived)}`);
        }
    }

    // Subscribes and waits for the sync of that stream.
    async subscribe(streamId: string): Promise<StreamFrameOf<'streamSync'>> {
        this.send({ type: 'streamSubscribe', streamId });
        return await this.waitFrame('streamSync', (frame) => frame.streamId === streamId);
    }

    unsubscribe(streamId: string): void {
        this.send({ type: 'streamUnsubscribe', streamId });
    }

    command(streamId: string, commandId: string, command: object): void {
        this.send({ type: 'streamCommand', streamId, commandId, command });
    }

    close(): void {
        this.ws.close();
    }
}
