import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { type ConnectionState, StreamConnection, type StreamPort, type StreamSocket } from './stream-connection';

class FakeSocket implements StreamSocket {
    static all: FakeSocket[] = [];
    readyState = 0;
    sent: string[] = [];
    closed = false;
    onopen: (() => void) | null = null;
    onmessage: ((event: { data: unknown }) => void) | null = null;
    onclose: ((event: { reason: string }) => void) | null = null;
    onerror: (() => void) | null = null;

    constructor(readonly url: string) {
        FakeSocket.all.push(this);
    }

    send(data: string): void {
        this.sent.push(data);
    }

    close(): void {
        this.closed = true;
        this.readyState = 3;
    }

    open(): void {
        this.readyState = 1;
        this.onopen?.();
    }

    receive(data: unknown): void {
        this.onmessage?.({ data });
    }

    drop(reason = ''): void {
        this.readyState = 3;
        this.onclose?.({ reason });
    }
}

type Status = 'waiting' | 'live' | 'unsynced';

class FakeStream implements StreamPort {
    state: Status = 'waiting';
    handled: string[] = [];
    reply: string[] = [];
    subscribeFrame = (): string => 'SUB';
    handleMessage = (text: string): string[] => {
        this.handled.push(text);
        return this.reply;
    };
    status = (): Status => this.state;
}

function setup() {
    const stream = new FakeStream();
    const states: ConnectionState[] = [];
    const changes = vi.fn();
    const connection = new StreamConnection({
        url: 'wss://host/ws',
        stream,
        onState: (state) => states.push(state),
        onChange: changes,
        createSocket: (url) => new FakeSocket(url)
    });
    return { stream, states, changes, connection };
}

describe('StreamConnection', () => {
    beforeEach(() => {
        vi.useFakeTimers();
        FakeSocket.all = [];
    });
    afterEach(() => {
        vi.useRealTimers();
    });

    it('subscribes when the socket opens and pumps frames both ways', () => {
        const { stream, states, changes, connection } = setup();

        connection.start();
        const socket = FakeSocket.all[0];
        expect(socket.url).toBe('wss://host/ws');
        expect(states).toEqual(['connecting']);

        socket.open();
        expect(socket.sent).toEqual(['SUB']);

        stream.reply = ['A', 'B'];
        stream.state = 'live';
        socket.receive('{"type":"streamSync"}');

        expect(stream.handled).toEqual(['{"type":"streamSync"}']);
        expect(socket.sent).toEqual(['SUB', 'A', 'B']);
        expect(changes).toHaveBeenCalledTimes(1);
        expect(states).toEqual(['connecting', 'syncing', 'live']);
    });

    it('ignores frames that are not text', () => {
        const { stream, changes, connection } = setup();
        connection.start();
        FakeSocket.all[0].open();

        FakeSocket.all[0].receive(new ArrayBuffer(4));

        expect(stream.handled).toEqual([]);
        expect(changes).not.toHaveBeenCalled();
    });

    it('goes back to syncing when the stream asks for a new sync', () => {
        const { stream, states, connection } = setup();
        connection.start();
        FakeSocket.all[0].open();
        stream.state = 'live';
        FakeSocket.all[0].receive('x');

        stream.state = 'waiting';
        FakeSocket.all[0].receive('reset');

        expect(states).toEqual(['connecting', 'syncing', 'live', 'syncing']);
    });

    it('reconnects with a growing delay and resets it after a successful open', () => {
        const { states, connection } = setup();
        connection.start();
        FakeSocket.all[0].open();

        FakeSocket.all[0].drop();
        expect(states.at(-1)).toBe('reconnecting');
        vi.advanceTimersByTime(999);
        expect(FakeSocket.all).toHaveLength(1);
        vi.advanceTimersByTime(1);
        expect(FakeSocket.all).toHaveLength(2);

        FakeSocket.all[1].drop();
        vi.advanceTimersByTime(1999);
        expect(FakeSocket.all).toHaveLength(2);
        vi.advanceTimersByTime(1);
        expect(FakeSocket.all).toHaveLength(3);

        FakeSocket.all[2].open();
        FakeSocket.all[2].drop();
        vi.advanceTimersByTime(1000);
        expect(FakeSocket.all).toHaveLength(4);
    });

    it('caps the reconnect delay at 15 seconds', () => {
        const { connection } = setup();
        connection.start();

        for (let attempt = 0; attempt < 8; attempt++) {
            FakeSocket.all[attempt].drop();
            vi.advanceTimersByTime(15000);
        }

        expect(FakeSocket.all).toHaveLength(9);
    });

    it('does not reconnect when the server replaced the connection with a newer one', () => {
        const { states, connection } = setup();
        connection.start();
        FakeSocket.all[0].open();

        FakeSocket.all[0].drop('replaced');
        vi.advanceTimersByTime(60000);

        expect(states.at(-1)).toBe('replaced');
        expect(FakeSocket.all).toHaveLength(1);
    });

    it('subscribes again on the new socket', () => {
        const { connection } = setup();
        connection.start();
        FakeSocket.all[0].open();
        FakeSocket.all[0].drop();
        vi.advanceTimersByTime(1000);

        FakeSocket.all[1].open();

        expect(FakeSocket.all[1].sent).toEqual(['SUB']);
    });

    it('stop closes the socket, cancels the retry and ignores late events', () => {
        const { stream, states, changes, connection } = setup();
        connection.start();
        const socket = FakeSocket.all[0];
        socket.open();

        connection.stop();
        socket.receive('late');
        socket.drop();
        vi.advanceTimersByTime(60000);

        expect(socket.closed).toBe(true);
        expect(states.at(-1)).toBe('closed');
        expect(stream.handled).toEqual([]);
        expect(changes).not.toHaveBeenCalled();
        expect(FakeSocket.all).toHaveLength(1);
    });

    it('stop while a retry is waiting cancels it', () => {
        const { connection } = setup();
        connection.start();
        FakeSocket.all[0].drop();

        connection.stop();
        vi.advanceTimersByTime(60000);

        expect(FakeSocket.all).toHaveLength(1);
    });

    it('send is false unless the socket is open', () => {
        const { connection } = setup();
        expect(connection.send('x')).toBe(false);
        connection.start();
        expect(connection.send('x')).toBe(false);

        FakeSocket.all[0].open();
        expect(connection.send('x')).toBe(true);
        expect(FakeSocket.all[0].sent).toEqual(['SUB', 'x']);

        FakeSocket.all[0].drop();
        expect(connection.send('y')).toBe(false);
    });

    it('subscribes again 5 seconds after the stream is unsynced', () => {
        const { stream, connection } = setup();
        connection.start();
        FakeSocket.all[0].open();

        stream.state = 'unsynced';
        FakeSocket.all[0].receive('unsynced');
        vi.advanceTimersByTime(4999);
        expect(FakeSocket.all[0].sent).toEqual(['SUB']);
        vi.advanceTimersByTime(1);

        expect(FakeSocket.all[0].sent).toEqual(['SUB', 'SUB']);
    });

    it('does not subscribe again if the stream recovered meanwhile', () => {
        const { stream, connection } = setup();
        connection.start();
        FakeSocket.all[0].open();
        stream.state = 'unsynced';
        FakeSocket.all[0].receive('unsynced');

        stream.state = 'live';
        FakeSocket.all[0].receive('sync');
        vi.advanceTimersByTime(10000);

        expect(FakeSocket.all[0].sent).toEqual(['SUB']);
    });
});
