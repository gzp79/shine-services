// connecting: no open socket yet; syncing: socket open, waiting for the state of the stream.
// replaced: the server gave this user's connection to a newer one (another tab); we do not take it back.
export type ConnectionState = 'connecting' | 'syncing' | 'live' | 'reconnecting' | 'replaced' | 'closed';

// What a stream connection needs from a stream: the replayed stream of the wasm side.
export interface StreamPort {
    subscribeFrame(): string;
    // Reacts to a frame of the server; returns the frames to send back.
    handleMessage(text: string): string[];
    status(): 'waiting' | 'live' | 'unsynced';
}

// The part of a `WebSocket` the connection uses.
export interface StreamSocket {
    readonly readyState: number;
    onopen: ((event: Event) => void) | null;
    onmessage: ((event: MessageEvent) => void) | null;
    onclose: ((event: CloseEvent) => void) | null;
    onerror: ((event: Event) => void) | null;
    send(data: string): void;
    close(): void;
}

export interface StreamConnectionOptions {
    url: string;
    stream: StreamPort;
    onState(state: ConnectionState): void;
    // A frame was handled, the stream may have changed.
    onChange(): void;
    createSocket?(url: string): StreamSocket;
}

const SOCKET_OPEN = 1;
// The reason of the close frame when the hub gave the connection of the user to a newer one.
const REPLACED_REASON = 'replaced';
const RECONNECT_MIN_MS = 1000;
const RECONNECT_MAX_MS = 15000;
const RESUBSCRIBE_MS = 5000;

// Keeps a stream subscribed over a WebSocket: connects, subscribes, reconnects with backoff.
// It moves text frames only; the stream decides what they mean.
export class StreamConnection {
    private socket: StreamSocket | null = null;
    private state: ConnectionState | null = null;
    private stopped = false;
    private attempt = 0;
    private retryTimer: ReturnType<typeof setTimeout> | null = null;
    private resubscribeTimer: ReturnType<typeof setTimeout> | null = null;

    constructor(private readonly options: StreamConnectionOptions) {}

    start(): void {
        this.stopped = false;
        this.connect();
    }

    // Sends a frame; false if the socket is not open (nothing is queued).
    send(frame: string): boolean {
        if (this.socket?.readyState !== SOCKET_OPEN) {
            return false;
        }
        this.socket.send(frame);
        return true;
    }

    stop(): void {
        this.stopped = true;
        this.clearTimers();
        const socket = this.socket;
        this.socket = null;
        if (socket) {
            detach(socket);
            socket.close();
        }
        this.setState('closed');
    }

    private connect(): void {
        this.setState(this.attempt === 0 ? 'connecting' : 'reconnecting');
        const socket = (this.options.createSocket ?? ((url) => new WebSocket(url)))(this.options.url);
        this.socket = socket;
        socket.onopen = () => {
            if (this.socket !== socket) return;
            this.attempt = 0;
            this.setState('syncing');
            socket.send(this.options.stream.subscribeFrame());
        };
        socket.onmessage = (event) => {
            if (this.socket !== socket || typeof event.data !== 'string') return;
            for (const frame of this.options.stream.handleMessage(event.data)) {
                socket.send(frame);
            }
            this.afterFrame();
            this.options.onChange();
        };
        socket.onclose = (event) => {
            if (this.socket !== socket) return;
            this.socket = null;
            this.clearTimers();
            if (event.reason === REPLACED_REASON) {
                // Reconnecting would only replace the other connection in turn, forever.
                this.setState('replaced');
                return;
            }
            this.setState('reconnecting');
            const delay = Math.min(RECONNECT_MIN_MS * 2 ** this.attempt, RECONNECT_MAX_MS);
            this.attempt += 1;
            this.retryTimer = setTimeout(() => {
                this.retryTimer = null;
                if (!this.stopped) this.connect();
            }, delay);
        };
        // A failed socket closes right after; the close handler retries.
        socket.onerror = () => {};
    }

    private afterFrame(): void {
        const status = this.options.stream.status();
        this.setState(status === 'live' ? 'live' : 'syncing');
        if (status === 'unsynced' && this.resubscribeTimer === null) {
            // The server cannot serve the stream right now: ask again, but not in a tight loop.
            this.resubscribeTimer = setTimeout(() => {
                this.resubscribeTimer = null;
                if (this.options.stream.status() === 'unsynced') {
                    this.send(this.options.stream.subscribeFrame());
                }
            }, RESUBSCRIBE_MS);
        }
    }

    private clearTimers(): void {
        if (this.retryTimer !== null) clearTimeout(this.retryTimer);
        if (this.resubscribeTimer !== null) clearTimeout(this.resubscribeTimer);
        this.retryTimer = null;
        this.resubscribeTimer = null;
    }

    private setState(state: ConnectionState): void {
        if (this.state === state) return;
        this.state = state;
        this.options.onState(state);
    }
}

function detach(socket: StreamSocket): void {
    socket.onopen = null;
    socket.onmessage = null;
    socket.onclose = null;
    socket.onerror = null;
}
