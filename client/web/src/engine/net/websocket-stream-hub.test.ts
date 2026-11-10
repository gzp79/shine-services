import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { WebSocketStreamHub } from './websocket-stream-hub';

class FakeSocket {
    static all: FakeSocket[] = [];
    readyState = 0;
    sent: string[] = [];
    private readonly listeners = new Map<string, Set<(event: MessageEvent) => void>>();

    constructor(readonly url: string) {
        FakeSocket.all.push(this);
    }

    send(data: string): void {
        this.sent.push(data);
    }

    addEventListener(type: string, listener: (event: MessageEvent) => void): void {
        const listeners = this.listeners.get(type) ?? new Set();
        listeners.add(listener);
        this.listeners.set(type, listeners);
    }

    removeEventListener(type: string, listener: (event: MessageEvent) => void): void {
        this.listeners.get(type)?.delete(listener);
    }

    close(): void {
        this.readyState = 3;
    }

    drop(): void {
        this.readyState = 3;
        this.emit('close');
    }

    emit(type: string, data?: string): void {
        for (const listener of this.listeners.get(type) ?? []) listener({ data } as MessageEvent);
    }
}

describe('WebSocketStreamHub', () => {
    beforeEach(() => {
        FakeSocket.all = [];
        vi.useFakeTimers();
    });

    afterEach(() => vi.useRealTimers());

    it('waits for Connected, then subscribes, routes frames, and exposes the authenticated user', async () => {
        const hub = new WebSocketStreamHub('wss://builder/connect', {
            createSocket: (url) => new FakeSocket(url)
        });
        const listener = { connected: vi.fn(), disconnected: vi.fn(), frame: vi.fn() };
        const subscription = hub.createSubscription('pixel-map:demo', () => listener);
        const connected = subscription.subscribe();

        hub.connect();
        const socket = FakeSocket.all[0];
        socket.readyState = 1;
        socket.emit('open');
        expect(socket.sent).toEqual([]);
        const userId = '00000000-0000-0000-0000-000000000001';
        socket.emit('message', JSON.stringify({ type: 'connected', userId }));
        socket.emit('message', JSON.stringify({ type: 'streamSync', streamId: 'pixel-map:demo', commands: [] }));
        await connected;
        // PixelMapCommand::Paint { x: 1, y: 2, color: 3 }: variant 0, then the fields, in base64.
        subscription.sendCommand('command-1', 'AAECAw==');
        subscription.unsubscribe();

        expect(socket.sent.map((frame) => JSON.parse(frame))).toEqual([
            { type: 'streamSubscribe', streamId: 'pixel-map:demo' },
            {
                type: 'streamCommand',
                streamId: 'pixel-map:demo',
                commandId: 'command-1',
                command: 'AAECAw=='
            },
            { type: 'streamUnsubscribe', streamId: 'pixel-map:demo' }
        ]);
        expect(listener.connected).toHaveBeenCalledOnce();
        expect(listener.frame).toHaveBeenCalledWith({
            type: 'streamSync',
            streamId: 'pixel-map:demo',
            commands: []
        });
        hub.dispose();
    });

    it('reconnects and restores active subscriptions after a socket closes', async () => {
        const hub = new WebSocketStreamHub('wss://builder/connect', {
            createSocket: (url) => new FakeSocket(url)
        });
        const listener = { connected: vi.fn(), disconnected: vi.fn(), frame: vi.fn() };
        const subscription = hub.createSubscription('pixel-map:demo', () => listener);
        const subscribed = subscription.subscribe();

        hub.connect();
        const first = FakeSocket.all[0];
        first.readyState = 1;
        first.emit('open');
        first.emit('message', JSON.stringify({ type: 'connected', userId: 'user-1' }));
        await subscribed;

        first.drop();
        expect(listener.disconnected).toHaveBeenCalledOnce();
        vi.advanceTimersByTime(1000);

        const second = FakeSocket.all[1];
        second.readyState = 1;
        second.emit('open');
        second.emit('message', JSON.stringify({ type: 'connected', userId: 'user-1' }));

        expect(second.sent.map((frame) => JSON.parse(frame))).toEqual([
            { type: 'streamSubscribe', streamId: 'pixel-map:demo' }
        ]);
        expect(listener.connected).toHaveBeenCalledTimes(2);
        hub.dispose();
    });
});
