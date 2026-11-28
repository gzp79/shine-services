import { expect, test } from '$fixtures/setup';
import {
    type StreamCommandFrame,
    StreamSession,
    addCommand,
    encodeCommand,
    paintCommand,
    wsConnectUrl,
    wsHeaders
} from '$lib/api/builder_ws';
import { MintedSession, SessionMint } from '$lib/mocks/session_mint';
import { randomUUID } from 'node:crypto';

// A stream is a shared, ordered log of commands of one kind. A client subscribes with the stream's kind:id,
// gets the state to start from (`streamSync`), then every command after it in version order
// (`streamCommands`). Commands are queued with `streamCommand`; there is no success reply, the sender sees its
// own command come back with its version. A refusal is a `streamNack` or `streamSubscribeFailed`.

const CHAT = 'chat';
const PIXEL_MAP = 'pixel-map';

// Stream names are 1 to 64 letters, digits and `_ . : -`. Unique per test, so tests never share a stream.
const newStreamId = (kind: string, prefix: string) => `${kind}:${prefix}-${randomUUID().replace(/-/g, '')}`;

const within = async <T>(promise: Promise<T>, ms: number, what: string): Promise<T> => {
    let timer: NodeJS.Timeout | undefined;
    const timeout = new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(new Error(`${what} did not happen within ${ms}ms`)), ms);
    });
    try {
        return await Promise.race([promise, timeout]);
    } finally {
        clearTimeout(timer);
    }
};

const add = addCommand;
const paint = paintCommand;

test.describe('Builder streams', { tag: ['@regression'] }, () => {
    let mint: SessionMint;
    let alice: MintedSession;
    let bob: MintedSession;
    const sessions: StreamSession[] = [];

    test.beforeEach(async () => {
        mint = await SessionMint.fromServerConfig();
        alice = await mint.createUserSession({ userId: randomUUID() });
        bob = await mint.createUserSession({ userId: randomUUID() });
    });

    test.afterEach(async () => {
        for (const session of sessions.splice(0)) {
            session.close();
        }
        await mint.teardownCreatedSessions();
    });

    const connect = async (homeUrl: string, builderWSUrl: string, user: MintedSession) => {
        const session = await StreamSession.open(wsConnectUrl(builderWSUrl), {
            sid: user.sessionCookie,
            ...wsHeaders(homeUrl, builderWSUrl)
        });
        sessions.push(session);
        return session;
    };

    test('WS stream subscribers shall get an empty sync and then every command in order, once', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const streamId = newStreamId(CHAT, 'room');
        const a = await connect(homeUrl, builderWSUrl, alice);
        const b = await connect(homeUrl, builderWSUrl, bob);

        const syncA = await a.subscribe(streamId);
        const syncB = await b.subscribe(streamId);
        expect(syncA).toEqual({ type: 'streamSync', streamId, snapshot: null, commands: [] });
        expect(syncB).toEqual({ type: 'streamSync', streamId, snapshot: null, commands: [] });

        // Commands from both users interleave into one order; the writer gets its own back too, with the
        // command id it chose and its user id.
        const ids = [randomUUID(), randomUUID(), randomUUID()];
        a.command(streamId, ids[0], add('first'));
        a.command(streamId, ids[1], add('second'));
        b.command(streamId, ids[2], add('third'));

        for (const session of [a, b]) {
            const received: StreamCommandFrame[] = [];
            while (received.length < 3) {
                const frame = await session.waitFrame('streamCommands', (f) => f.streamId === streamId);
                received.push(...frame.commands);
            }
            expect(received.map((c) => c.version)).toEqual([1, 2, 3]);
            expect(new Set(received.map((c) => c.commandId))).toEqual(new Set(ids));
            const byId = new Map(received.map((c) => [c.commandId, c]));
            expect(byId.get(ids[0])).toMatchObject({ userId: alice.userId, command: add('first') });
            expect(byId.get(ids[2])).toMatchObject({ userId: bob.userId, command: add('third') });
            // Nothing more arrives: no duplicate delivery.
            await session.expectNoFrame('streamCommands', (f) => 'streamId' in f && f.streamId === streamId, 1000);
        }
    });

    test('WS stream message over the socket limit shall end the socket and leave the user free to reconnect', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const a = await connect(homeUrl, builderWSUrl, alice);
        const streamId = newStreamId(CHAT, 'room');
        await a.subscribe(streamId);

        a.command(streamId, randomUUID(), encodeCommand(Buffer.alloc(40_000)));

        await within(a.closed, 5000, 'the close of the socket');
        const again = await connect(homeUrl, builderWSUrl, alice);
        expect(await again.subscribe(streamId)).toMatchObject({ type: 'streamSync', commands: [] });
    });

    // Snapshots are taken every 100 commands. The pixel map is a state worth one: a 32 by 32 grid of colors.
    test('WS stream shall announce a checkpoint every 100 commands and sync late subscribers from its snapshot', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const streamId = newStreamId(PIXEL_MAP, 'canvas');
        const a = await connect(homeUrl, builderWSUrl, alice);
        await a.subscribe(streamId);

        // Paint 100 different pixels: row by row, a color from 1 to 15.
        const pixel = (n: number) => ({ x: n % 32, y: Math.floor(n / 32), color: (n % 15) + 1 });
        for (let n = 0; n < 100; n++) {
            const { x, y, color } = pixel(n);
            a.command(streamId, randomUUID(), paint(x, y, color));
        }

        const checkpoint = await a.waitFrame('streamCheckpoint', (f) => f.streamId === streamId, 15_000);
        expect(checkpoint.version).toBe(100);
        expect(checkpoint.stateHash).toMatch(/^[0-9a-f]{16}$/);

        const b = await connect(homeUrl, builderWSUrl, bob);
        const sync = await b.subscribe(streamId);
        expect(sync.commands).toEqual([]);
        expect(sync.snapshot).toMatchObject({ version: 100, stateHash: checkpoint.stateHash });
        expect(sync.snapshot?.chainHash).toMatch(/^[0-9a-f]{16}$/);

        // The snapshot data is the state in postcard: the length of the pixel list (1024, a varint of two
        // bytes), then one byte per pixel.
        const data = Buffer.from(sync.snapshot!.data, 'base64');
        expect(data.length).toBe(2 + 32 * 32);
        expect(Array.from(data.subarray(0, 2))).toEqual([0x80, 0x08]);
        const pixels = data.subarray(2);
        for (let n = 0; n < 100; n++) {
            const { x, y, color } = pixel(n);
            expect(pixels[y * 32 + x], `pixel ${x},${y}`).toBe(color);
        }
        expect(pixels.subarray(100).every((c) => c === 0)).toBe(true);

        // And the stream goes on from the snapshot.
        a.command(streamId, randomUUID(), paint(0, 31, 7));
        const next = await b.waitFrame('streamCommands', (f) => f.streamId === streamId);
        expect(next.commands[0]).toMatchObject({ version: 101, command: paint(0, 31, 7) });
    });
});
