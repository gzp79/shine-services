import { DEFAULT_USER_AGENT, joinURL } from '$lib/utils';
import { IncomingMessage } from 'node:http';
import { WebSocket } from 'ws';

// The builder's stream protocol (see docs/services/builder/hub.md): JSON text frames tagged by `type`.

export type WireCommand = { version: number; commandId: string; userId: string; command: string };
export type WireSnapshot = { version: number; hash: string; data: string };

export type StreamSyncFrame = {
    type: 'streamSync';
    streamId: string;
    generation: number;
    snapshotEvery: number;
    snapshot: WireSnapshot | null;
    commands: WireCommand[];
};
export type StreamCommandsFrame = {
    type: 'streamCommands';
    streamId: string;
    generation: number;
    commands: WireCommand[];
};
export type StreamCheckpointFrame = { type: 'streamCheckpoint'; streamId: string; version: number; hash: string };
export type StreamResetFrame = { type: 'streamReset'; streamId: string };
export type StreamNackFrame = { type: 'streamNack'; streamId: string; commandId: string; reason: string };
export type StreamSubscribeFailedFrame = { type: 'streamSubscribeFailed'; streamId: string; reason: string };

export type ServerFrame =
    | StreamSyncFrame
    | StreamCommandsFrame
    | StreamCheckpointFrame
    | StreamResetFrame
    | StreamNackFrame
    | StreamSubscribeFailedFrame;

type FrameOf<T extends ServerFrame['type']> = Extract<ServerFrame, { type: T }>;

export type Resume = { generation: number; version: number };

export type StreamSocketOptions = {
    // Omit to send no Cookie header at all (unauthenticated).
    sid?: string;
    origin: string;
    // The builder resolves the host from this header, as the proxy would set it.
    forwardedHost: string;
};

export function wsConnectUrl(builderWSUrl: string): string {
    const wsUrl = new URL(joinURL(builderWSUrl, '/api/connect'));
    wsUrl.protocol = wsUrl.protocol === 'https:' ? 'wss:' : 'ws:';
    return wsUrl.toString();
}

// The origin of the web app and the host of the ws endpoint, as the builder checks them.
export function wsOptions(homeUrl: string, builderWSUrl: string, sid?: string): StreamSocketOptions {
    return { sid, origin: new URL(homeUrl).origin, forwardedHost: new URL(builderWSUrl).host };
}

// One stream socket of a user. Frames are kept in arrival order and consumed by the waits, so a test reads
// the conversation like a script.
export class StreamSocket {
    private readonly frames: ServerFrame[] = [];
    private readonly waiters: Array<() => void> = [];
    private closedResult: { code: number; reason: string } | undefined;
    public readonly closed: Promise<{ code: number; reason: string }>;

    private constructor(private readonly ws: WebSocket) {
        ws.on('message', (data: Buffer) => {
            this.frames.push(JSON.parse(data.toString()) as ServerFrame);
            this.waiters.splice(0).forEach((wake) => wake());
        });
        this.closed = new Promise((resolve) => {
            ws.once('close', (code: number, reason: Buffer) => {
                this.closedResult = { code, reason: reason.toString() };
                this.waiters.splice(0).forEach((wake) => wake());
                resolve(this.closedResult);
            });
        });
    }

    public static async open(builderWSUrl: string, options: StreamSocketOptions): Promise<StreamSocket> {
        const headers: Record<string, string> = {
            'User-Agent': DEFAULT_USER_AGENT,
            'x-forwarded-host': options.forwardedHost
        };
        if (options.sid !== undefined) {
            headers.Cookie = `sid=${options.sid}`;
        }
        const ws = new WebSocket(wsConnectUrl(builderWSUrl), {
            origin: options.origin,
            headers
        });
        const socket = new StreamSocket(ws);
        await new Promise<void>((resolve, reject) => {
            ws.once('open', () => resolve());
            ws.once('unexpected-response', (_req: unknown, res: IncomingMessage) =>
                reject(new Error(`unexpected upgrade response: ${res.statusCode}`))
            );
            ws.once('error', (err: Error) => reject(err));
        });
        return socket;
    }

    public subscribe(streamId: string, kind: string, resume?: Resume): void {
        this.send({ type: 'streamSubscribe', streamId, kind, ...(resume ? { resume } : {}) });
    }

    public unsubscribe(streamId: string): void {
        this.send({ type: 'streamUnsubscribe', streamId });
    }

    // `command` is sent as a JSON string, as the protocol requires.
    public command(streamId: string, commandId: string, command: object): void {
        this.send({ type: 'streamCommand', streamId, commandId, command: JSON.stringify(command) });
    }

    public sendRaw(frame: string): void {
        this.ws.send(frame);
    }

    public close(): void {
        this.ws.close();
    }

    // Consumes and returns the next frame of the type that matches, leaving the others in place.
    public async next<T extends ServerFrame['type']>(
        type: T,
        predicate: (frame: FrameOf<T>) => boolean = () => true,
        timeoutMs = 5000
    ): Promise<FrameOf<T>> {
        return (await this.take(
            (frame) => frame.type === type && predicate(frame as FrameOf<T>),
            type,
            timeoutMs
        )) as FrameOf<T>;
    }

    // Collects the commands of `streamSync` and `streamCommands` frames, in arrival order, until the one of
    // `version` arrived.
    public async commandsUpTo(version: number, timeoutMs = 5000): Promise<WireCommand[]> {
        const collected: WireCommand[] = [];
        while (collected.length === 0 || collected[collected.length - 1].version < version) {
            const frame = (await this.take(
                (candidate) => candidate.type === 'streamSync' || candidate.type === 'streamCommands',
                'streamSync/streamCommands',
                timeoutMs
            )) as StreamSyncFrame | StreamCommandsFrame;
            collected.push(...frame.commands);
        }
        return collected;
    }

    private async take(match: (frame: ServerFrame) => boolean, what: string, timeoutMs: number): Promise<ServerFrame> {
        const deadline = Date.now() + timeoutMs;
        for (;;) {
            const index = this.frames.findIndex(match);
            if (index >= 0) {
                return this.frames.splice(index, 1)[0];
            }
            if (this.closedResult) {
                throw new Error(
                    `no ${what} frame: socket closed (code ${this.closedResult.code}, reason ${this.closedResult.reason})`
                );
            }
            const left = deadline - Date.now();
            if (left <= 0) {
                throw new Error(`no ${what} frame after ${timeoutMs}ms; pending: ${JSON.stringify(this.frames)}`);
            }
            await new Promise<void>((resolve) => {
                const timer = setTimeout(resolve, left);
                this.waiters.push(() => {
                    clearTimeout(timer);
                    resolve();
                });
            });
        }
    }

    // Asserts nothing of the type arrives within the window.
    public async expectNone<T extends ServerFrame['type']>(
        type: T,
        windowMs = 1000,
        predicate: (frame: FrameOf<T>) => boolean = () => true
    ): Promise<void> {
        try {
            const frame = await this.next(type, predicate, windowMs);
            throw new Error(`unexpected ${type} frame: ${JSON.stringify(frame)}`);
        } catch (error) {
            if (error instanceof Error && error.message.startsWith('no ')) {
                return;
            }
            throw error;
        }
    }

    private send(frame: object): void {
        this.ws.send(JSON.stringify(frame));
    }
}

// Reads a stream over HTTP, as a client does before it has a socket.
export async function fetchSnapshot(
    builderUrl: string,
    streamId: string,
    sid?: string
): Promise<{ status: number; frame?: StreamSyncFrame }> {
    const headers: Record<string, string> = { 'User-Agent': DEFAULT_USER_AGENT };
    if (sid !== undefined) {
        headers.Cookie = `sid=${sid}`;
    }
    const response = await fetch(joinURL(builderUrl, `/api/streams/${streamId}/snapshot`), { headers });
    return response.ok
        ? { status: response.status, frame: (await response.json()) as StreamSyncFrame }
        : { status: response.status };
}
