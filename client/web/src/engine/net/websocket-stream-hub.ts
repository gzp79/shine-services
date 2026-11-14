import type { StreamHub, StreamListener, StreamSubscription } from './stream-hub';

interface SocketLike {
    readonly readyState: number;
    send(data: string): void;
    close(): void;
    addEventListener(type: 'open', listener: () => void): void;
    addEventListener(type: 'message', listener: (event: MessageEvent) => void): void;
    addEventListener(type: 'close', listener: () => void): void;
    addEventListener(type: 'error', listener: () => void): void;
    removeEventListener(type: 'open', listener: () => void): void;
    removeEventListener(type: 'message', listener: (event: MessageEvent) => void): void;
    removeEventListener(type: 'close', listener: () => void): void;
    removeEventListener(type: 'error', listener: () => void): void;
}

type Entry = {
    createListener: (userId: string) => StreamListener;
    listener: StreamListener | null;
    resolve?: () => void;
};
const SOCKET_OPEN = 1;
const RECONNECT_MIN_MS = 1000;
const RECONNECT_MAX_MS = 15000;

export interface WebSocketStreamHubOptions {
    createSocket?: (url: string) => SocketLike;
}

// Standalone development implementation. It owns one reconnecting WebSocket and all stream subscriptions.
export class WebSocketStreamHub implements StreamHub {
    private readonly streams = new Map<string, Entry>();
    private socket: SocketLike | null = null;
    private userId: string | null = null;
    private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    private reconnectAttempt = 0;
    private disposed = false;

    constructor(
        private readonly url: string,
        private readonly options: WebSocketStreamHubOptions = {}
    ) {}

    get isConnected(): boolean {
        return this.socket?.readyState === SOCKET_OPEN && this.userId !== null;
    }

    connect(): void {
        if (this.disposed || this.socket) return;
        const socket: SocketLike = (this.options.createSocket ?? ((url) => new WebSocket(url)))(this.url);
        this.socket = socket;
        socket.addEventListener('open', this.onOpen);
        socket.addEventListener('message', this.onMessage);
        socket.addEventListener('close', this.onClose);
        socket.addEventListener('error', this.onError);
    }

    createSubscription(streamId: string, createListener: (userId: string) => StreamListener): StreamSubscription {
        const entry: Entry = { createListener, listener: null };
        const live = (): boolean => this.streams.get(streamId) === entry;
        console.log(`Creating subscription for stream '${streamId}'`);
        return {
            subscribe: () => {
                console.log(`Subscribing to stream '${streamId}'`);
                const current = this.streams.get(streamId);
                if (current && current !== entry) {
                    return Promise.reject(new Error(`Stream '${streamId}' is already subscribed.`));
                }
                if (current === entry && this.userId !== null) {
                    this.sendSubscribe(streamId);
                    return Promise.resolve();
                }
                this.streams.set(streamId, entry);
                if (this.userId !== null) {
                    this.activate(streamId, entry, this.userId);
                    return Promise.resolve();
                }
                return new Promise((resolve) => {
                    entry.resolve = resolve;
                });
            },
            sendCommand: (commandId, command) => {
                console.log(`Sending command '${commandId}' to stream '${streamId}'`);
                return live() && this.send({ type: 'streamCommand', streamId, commandId, command });
            },
            unsubscribe: () => {
                console.log(`Unsubscribing from stream '${streamId}'`);
                if (!live()) return;
                this.streams.delete(streamId);
                entry.resolve?.();
                entry.resolve = undefined;
                this.send({ type: 'streamUnsubscribe', streamId });
            }
        };
    }

    dispose(): void {
        this.disposed = true;
        if (this.reconnectTimer !== null) clearTimeout(this.reconnectTimer);
        this.reconnectTimer = null;
        const socket = this.socket;
        this.socket = null;
        if (socket) {
            this.detach(socket);
            socket.close();
        }
        for (const entry of this.streams.values()) entry.resolve?.();
        this.streams.clear();
    }

    private sendSubscribe(streamId: string): void {
        this.send({ type: 'streamSubscribe', streamId });
    }

    private activate(streamId: string, entry: Entry, userId: string): void {
        entry.listener ??= entry.createListener(userId);
        this.sendSubscribe(streamId);
        entry.listener.connected();
        entry.resolve?.();
        entry.resolve = undefined;
    }

    private send(frame: object): boolean {
        if (!this.isConnected) return false;
        this.socket!.send(JSON.stringify(frame));
        return true;
    }

    private onOpen = (): void => {
        this.reconnectAttempt = 0;
    };

    private onMessage = (event: MessageEvent): void => {
        if (typeof event.data !== 'string') return;
        let frame: unknown;
        try {
            frame = JSON.parse(event.data);
        } catch {
            return;
        }
        if (isConnectedFrame(frame)) {
            this.userId = frame.userId;
            for (const [streamId, entry] of this.streams) {
                this.activate(streamId, entry, frame.userId);
            }
            return;
        }
        if (
            typeof frame !== 'object' ||
            frame === null ||
            !('streamId' in frame) ||
            typeof frame.streamId !== 'string'
        ) {
            return;
        }
        this.streams.get(frame.streamId)?.listener?.frame(frame);
    };

    private onClose = (): void => {
        const socket = this.socket;
        if (socket) this.detach(socket);
        this.socket = null;
        this.userId = null;
        for (const entry of this.streams.values()) entry.listener?.disconnected();
        if (this.disposed) return;
        const delay = Math.min(RECONNECT_MIN_MS * 2 ** this.reconnectAttempt, RECONNECT_MAX_MS);
        this.reconnectAttempt += 1;
        this.reconnectTimer = setTimeout(() => {
            this.reconnectTimer = null;
            this.connect();
        }, delay);
    };

    // Browsers expose no useful WebSocket error details; close drives the reconnect path.
    private onError = (): void => {};

    private detach(socket: SocketLike): void {
        socket.removeEventListener('open', this.onOpen);
        socket.removeEventListener('message', this.onMessage);
        socket.removeEventListener('close', this.onClose);
        socket.removeEventListener('error', this.onError);
    }
}

function isConnectedFrame(frame: unknown): frame is { type: 'connected'; userId: string } {
    return (
        typeof frame === 'object' &&
        frame !== null &&
        'type' in frame &&
        frame.type === 'connected' &&
        'userId' in frame &&
        typeof frame.userId === 'string'
    );
}
