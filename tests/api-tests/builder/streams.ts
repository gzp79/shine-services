import { expect, test } from '$fixtures/setup';
import { type StreamCommandFrame, StreamSession, wsConnectUrl, wsHeaders } from '$lib/api/builder_ws';
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

const add = (text: string) => ({ type: 'add', text });
const paint = (x: number, y: number, color: number) => ({ type: 'paint', x, y, color });

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

    test('WS stream late subscriber shall get the history in its sync and only newer commands after it', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const streamId = newStreamId(CHAT, 'room');
        const a = await connect(homeUrl, builderWSUrl, alice);
        await a.subscribe(streamId);
        for (const text of ['one', 'two', 'three']) {
            a.command(streamId, randomUUID(), add(text));
        }
        let delivered = 0;
        while (delivered < 3) {
            delivered += (await a.waitFrame('streamCommands', (f) => f.streamId === streamId)).commands.length;
        }

        const b = await connect(homeUrl, builderWSUrl, bob);
        const sync = await b.subscribe(streamId);
        // No snapshot yet (they come every 100 commands), so the sync carries the whole log.
        expect(sync.snapshot).toBeNull();
        expect(sync.commands.map((c) => c.version)).toEqual([1, 2, 3]);
        expect(sync.commands.map((c) => c.command)).toEqual([add('one'), add('two'), add('three')]);

        a.command(streamId, randomUUID(), add('four'));
        const next = await b.waitFrame('streamCommands', (f) => f.streamId === streamId);
        expect(next.commands.map((c) => c.version)).toEqual([4]);
    });

    test('WS stream of different kinds shall work side by side on one connection', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const room = newStreamId(CHAT, 'room');
        const canvas = newStreamId(PIXEL_MAP, 'canvas');
        const a = await connect(homeUrl, builderWSUrl, alice);

        await a.subscribe(room);
        const sync = await a.subscribe(canvas);
        expect(sync.commands).toEqual([]);
        a.command(room, randomUUID(), add('hello'));
        a.command(canvas, randomUUID(), paint(3, 4, 5));

        const fromRoom = await a.waitFrame('streamCommands', (f) => f.streamId === room);
        const fromCanvas = await a.waitFrame('streamCommands', (f) => f.streamId === canvas);
        expect(fromRoom.commands[0]).toMatchObject({ version: 1, command: add('hello') });
        expect(fromCanvas.commands[0]).toMatchObject({ version: 1, command: paint(3, 4, 5) });
    });

    test('WS stream a command id shall be the clients own, so a repeated one gets a version each', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const streamId = newStreamId(CHAT, 'room');
        const a = await connect(homeUrl, builderWSUrl, alice);
        await a.subscribe(streamId);

        const commandId = randomUUID();
        a.command(streamId, commandId, add('same'));
        a.command(streamId, commandId, add('same'));

        const received: StreamCommandFrame[] = [];
        while (received.length < 2) {
            received.push(...(await a.waitFrame('streamCommands', (f) => f.streamId === streamId)).commands);
        }
        expect(received.map((c) => [c.version, c.commandId])).toEqual([
            [1, commandId],
            [2, commandId]
        ]);
    });

    test('WS stream unsubscribe shall stop the delivery and refuse further commands', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const streamId = newStreamId(CHAT, 'room');
        const a = await connect(homeUrl, builderWSUrl, alice);
        const b = await connect(homeUrl, builderWSUrl, bob);
        await a.subscribe(streamId);
        await b.subscribe(streamId);

        a.unsubscribe(streamId);
        // The unsubscribe is a request to the hub, in order with what follows on this connection.
        const refused = randomUUID();
        a.command(streamId, refused, add('too late'));
        expect(await a.waitFrame('streamNack', (f) => f.commandId === refused)).toMatchObject({
            streamId,
            reason: 'notSubscribed'
        });

        b.command(streamId, randomUUID(), add('still here'));
        await b.waitFrame('streamCommands', (f) => f.streamId === streamId);
        await a.expectNoFrame('streamCommands', (f) => 'streamId' in f && f.streamId === streamId, 1000);
    });

    // What the client is told when it asks for something the service will not do.
    test('WS stream command before subscribing shall be refused as not subscribed', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const a = await connect(homeUrl, builderWSUrl, alice);
        const streamId = newStreamId(CHAT, 'room');
        const commandId = randomUUID();

        a.command(streamId, commandId, add('hello'));

        expect(await a.waitFrame('streamNack', (f) => f.commandId === commandId)).toEqual({
            type: 'streamNack',
            streamId,
            commandId,
            reason: 'notSubscribed'
        });
    });

    // A command the kind does not accept is refused at once, before it reaches the stream.
    for (const invalid of [
        { title: 'a paint off the map', kind: PIXEL_MAP, command: paint(99, 0, 1) },
        { title: 'a paint with a color outside the palette', kind: PIXEL_MAP, command: paint(0, 0, 200) },
        { title: 'an empty chat message', kind: CHAT, command: add('   ') },
        { title: 'a command of another kind', kind: CHAT, command: paint(0, 0, 1) },
        { title: 'a command that is not a command', kind: CHAT, command: { nonsense: true } }
    ]) {
        test(`WS stream ${invalid.title} shall be refused as invalid`, async ({ homeUrl, builderWSUrl }) => {
            const a = await connect(homeUrl, builderWSUrl, alice);
            const streamId = newStreamId(invalid.kind, 'stream');
            await a.subscribe(streamId);
            const commandId = randomUUID();

            a.command(streamId, commandId, invalid.command);

            expect(await a.waitFrame('streamNack', (f) => f.commandId === commandId)).toMatchObject({
                streamId,
                reason: 'invalid'
            });
            // The refusal did not use a version, and the stream goes on.
            const next = invalid.kind === CHAT ? add('fine') : paint(1, 1, 1);
            a.command(streamId, randomUUID(), next);
            const delivered = await a.waitFrame('streamCommands', (f) => f.streamId === streamId);
            expect(delivered.commands[0]).toMatchObject({ version: 1, command: next });
        });
    }

    test('WS stream malformed stream request shall be ignored without dropping the connection', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const a = await connect(homeUrl, builderWSUrl, alice);

        // Not valid stream ids, and frames missing their fields.
        a.send({ type: 'streamSubscribe', streamId: 'has spaces' });
        a.send({ type: 'streamSubscribe', streamId: '' });
        a.sendRaw(JSON.stringify({ type: 'streamSubscribe' }));
        a.sendRaw(JSON.stringify({ type: 'streamCommand', streamId: 'x' }));

        const streamId = newStreamId(CHAT, 'room');
        expect(await a.subscribe(streamId)).toMatchObject({ type: 'streamSync', streamId });
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
