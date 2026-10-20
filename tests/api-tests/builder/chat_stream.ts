import { expect, test } from '$fixtures/setup';
import { StreamSocket, StreamSocketOptions, WireCommand, fetchSnapshot, wsOptions } from '$lib/api/builder_stream_ws';
import { MintedSession, SessionMint } from '$lib/mocks/session_mint';
import { randomUUID } from 'node:crypto';

// The global chat is one room shared by every run, so the tests never assume it is empty: they identify
// their messages by the commandId (a message id is the commandId of its add) and by unique texts.
const CHAT = 'chat-global';

type ChatCommand =
    | { type: 'add'; text: string }
    | { type: 'remove'; id: string }
    | { type: 'replace'; id: string; text: string };

test.describe('Builder chat stream', { tag: ['@regression'] }, () => {
    let mint: SessionMint;
    let alice: MintedSession;
    let bob: MintedSession;
    const sockets: StreamSocket[] = [];

    test.beforeEach(async () => {
        mint = await SessionMint.fromServerConfig();
        alice = await mint.createUserSession({ userId: randomUUID() });
        bob = await mint.createUserSession({ userId: randomUUID() });
    });

    test.afterEach(async () => {
        sockets.splice(0).forEach((socket) => socket.close());
        await mint.teardownCreatedSessions();
    });

    // Opens a socket of a user that follows the chat, past its first `streamSync`.
    async function joinChat(
        homeUrl: string,
        builderWSUrl: string,
        user: MintedSession
    ): Promise<{ socket: StreamSocket; generation: number }> {
        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, user.sessionCookie));
        sockets.push(socket);
        socket.subscribe(CHAT, 'chat');
        const sync = await socket.next('streamSync', (frame) => frame.streamId === CHAT);
        return { socket, generation: sync.generation };
    }

    function ownsCommand(commandId: string): (frame: { commands: WireCommand[] }) => boolean {
        return (frame) => frame.commands.some((command) => command.commandId === commandId);
    }

    test('Chat shall deliver a message to its sender and to other subscribers', async ({ homeUrl, builderWSUrl }) => {
        const a = await joinChat(homeUrl, builderWSUrl, alice);
        const b = await joinChat(homeUrl, builderWSUrl, bob);

        const commandId = randomUUID();
        const text = `hello ${randomUUID()}`;
        const add: ChatCommand = { type: 'add', text };
        a.socket.command(CHAT, commandId, add);

        // No ack: the writer sees its own commandId in the ordered commands, like everybody else.
        const own = await a.socket.next('streamCommands', ownsCommand(commandId));
        const other = await b.socket.next('streamCommands', ownsCommand(commandId));
        for (const frame of [own, other]) {
            expect(frame.streamId).toBe(CHAT);
            const command = frame.commands.find((c) => c.commandId === commandId)!;
            // The author comes from the session, never from the payload.
            expect(command.userId).toBe(alice.userId);
            // The command text is stored canonically and sent as a string.
            expect(JSON.parse(command.command)).toEqual(add);
        }
        const [ownCommand] = own.commands.filter((c) => c.commandId === commandId);
        const [otherCommand] = other.commands.filter((c) => c.commandId === commandId);
        expect(otherCommand.version).toBe(ownCommand.version);
        expect(own.generation).toBe(other.generation);
    });

    test('Chat shall order the commands of two writers identically for everyone', async ({ homeUrl, builderWSUrl }) => {
        const a = await joinChat(homeUrl, builderWSUrl, alice);
        const b = await joinChat(homeUrl, builderWSUrl, bob);

        const tag = randomUUID();
        const ids = { a: [] as string[], b: [] as string[] };
        for (let i = 0; i < 5; i++) {
            const idA = randomUUID();
            const idB = randomUUID();
            ids.a.push(idA);
            ids.b.push(idB);
            a.socket.command(CHAT, idA, { type: 'add', text: `${tag} a${i}` });
            b.socket.command(CHAT, idB, { type: 'add', text: `${tag} b${i}` });
        }

        const mine = new Set([...ids.a, ...ids.b]);
        const received = async (socket: StreamSocket) => {
            const seen: WireCommand[] = [];
            while (seen.length < mine.size) {
                const frame = await socket.next('streamCommands');
                seen.push(...frame.commands.filter((c) => mine.has(c.commandId)));
            }
            return seen;
        };
        const [forAlice, forBob] = await Promise.all([received(a.socket), received(b.socket)]);

        expect(forAlice.map((c) => c.commandId)).toEqual(forBob.map((c) => c.commandId));
        const versions = forAlice.map((c) => c.version);
        expect([...versions].sort((x, y) => x - y)).toEqual(versions);
        expect(new Set(versions).size).toBe(versions.length);
        // Each writer's own commands keep their sending order.
        for (const own of [ids.a, ids.b]) {
            expect(forAlice.filter((c) => own.includes(c.commandId)).map((c) => c.commandId)).toEqual(own);
        }
    });

    test('Chat shall record removes and replaces of any message as ordered commands', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const a = await joinChat(homeUrl, builderWSUrl, alice);
        const b = await joinChat(homeUrl, builderWSUrl, bob);

        const messageId = randomUUID();
        a.socket.command(CHAT, messageId, { type: 'add', text: `mine ${randomUUID()}` });
        await b.socket.next('streamCommands', ownsCommand(messageId));

        // Whether a command takes effect is the state's decision on replay (only the author may change a
        // message; covered by the Rust tests): the log accepts and orders all of them, so the writers
        // below all see their commands delivered, including the ones that change nothing.
        const steps: Array<{ socket: StreamSocket; user: MintedSession; command: ChatCommand }> = [
            { socket: b.socket, user: bob, command: { type: 'remove', id: messageId } },
            { socket: a.socket, user: alice, command: { type: 'replace', id: messageId, text: 'edited' } },
            { socket: a.socket, user: alice, command: { type: 'remove', id: randomUUID() } },
            { socket: a.socket, user: alice, command: { type: 'remove', id: messageId } }
        ];
        let previous = 0;
        for (const step of steps) {
            const commandId = randomUUID();
            step.socket.command(CHAT, commandId, step.command);
            const frame = await b.socket.next('streamCommands', ownsCommand(commandId));
            const delivered = frame.commands.find((c) => c.commandId === commandId)!;
            expect(delivered.userId).toBe(step.user.userId);
            expect(JSON.parse(delivered.command)).toEqual(step.command);
            expect(delivered.version).toBeGreaterThan(previous);
            previous = delivered.version;
        }
    });

    test('Chat shall refuse a retried command id once and invalid commands', async ({ homeUrl, builderWSUrl }) => {
        const a = await joinChat(homeUrl, builderWSUrl, alice);

        const commandId = randomUUID();
        a.socket.command(CHAT, commandId, { type: 'add', text: `once ${randomUUID()}` });
        await a.socket.next('streamCommands', ownsCommand(commandId));

        // A client that did not see its command retries it with the same id: it is already in the log.
        a.socket.command(CHAT, commandId, { type: 'add', text: 'once again' });
        const duplicate = await a.socket.next('streamNack', (frame) => frame.commandId === commandId);
        expect(duplicate).toMatchObject({ streamId: CHAT, reason: 'duplicate' });
        await a.socket.expectNone('streamCommands', 500, ownsCommand(commandId));

        const invalid: Array<[string, object]> = [
            ['an unknown command type', { type: 'shout', text: 'x' }],
            ['an empty text', { type: 'add', text: '   ' }],
            ['an oversized text', { type: 'add', text: 'x'.repeat(2001) }],
            ['a missing field', { type: 'remove' }]
        ];
        for (const [title, command] of invalid) {
            const id = randomUUID();
            a.socket.command(CHAT, id, command);
            const nack = await a.socket.next('streamNack', (frame) => frame.commandId === id);
            expect(nack.reason, title).toBe('invalid');
        }

        const notJson = randomUUID();
        a.socket.sendRaw(
            JSON.stringify({ type: 'streamCommand', streamId: CHAT, commandId: notJson, command: 'not json' })
        );
        expect((await a.socket.next('streamNack', (frame) => frame.commandId === notJson)).reason).toBe('invalid');
    });

    test('Chat socket shall ignore malformed frames without dropping the connection', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);

        // Frames the service cannot parse are logged and skipped, the old chat frame included.
        socket.sendRaw('this is not json');
        socket.sendRaw(JSON.stringify({ type: 'unknown', text: 'x' }));
        socket.sendRaw(JSON.stringify({ type: 'chat', text: 'old protocol' }));
        socket.sendRaw(JSON.stringify({ type: 'streamSubscribe', kind: 'chat' }));

        socket.subscribe(CHAT, 'chat');
        const sync = await socket.next('streamSync');
        expect(sync.streamId).toBe(CHAT);
    });

    test('Chat shall refuse commands for a stream the socket does not follow', async ({ homeUrl, builderWSUrl }) => {
        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);

        const commandId = randomUUID();
        socket.command(CHAT, commandId, { type: 'add', text: 'nobody listens' });
        expect(await socket.next('streamNack', (frame) => frame.commandId === commandId)).toMatchObject({
            streamId: CHAT,
            reason: 'notSubscribed'
        });
    });

    // The failure reasons of a subscribe, as a table.
    for (const failure of [
        { title: 'an unknown kind', streamId: CHAT, kind: 'nonsense', reason: 'unknownKind' },
        { title: 'another stream than the global chat', streamId: 'chat-private', kind: 'chat', reason: 'forbidden' }
    ]) {
        test(`Chat subscribe shall fail for ${failure.title}`, async ({ homeUrl, builderWSUrl }) => {
            const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
            sockets.push(socket);

            socket.subscribe(failure.streamId, failure.kind);
            expect(await socket.next('streamSubscribeFailed')).toMatchObject({
                streamId: failure.streamId,
                reason: failure.reason
            });
        });
    }

    test('Chat unsubscribe shall stop the frames and keep the socket open', async ({ homeUrl, builderWSUrl }) => {
        const a = await joinChat(homeUrl, builderWSUrl, alice);
        const b = await joinChat(homeUrl, builderWSUrl, bob);

        a.socket.unsubscribe(CHAT);
        // The unsubscribe and the write travel on different sockets: wait until bob's own write is delivered,
        // then give the unsubscribed socket time to (wrongly) get the frame as well.
        const commandId = randomUUID();
        b.socket.command(CHAT, commandId, { type: 'add', text: `after ${randomUUID()}` });
        await b.socket.next('streamCommands', ownsCommand(commandId));
        await a.socket.expectNone('streamCommands', 1000, ownsCommand(commandId));

        // Still connected, and following the chat again starts with a fresh sync.
        a.socket.subscribe(CHAT, 'chat');
        const sync = await a.socket.next('streamSync');
        expect(sync.commands.length + (sync.snapshot ? 1 : 0)).toBeGreaterThan(0);
    });

    test('Chat snapshot shall be readable over HTTP and resume with only the tail', async ({
        homeUrl,
        builderUrl,
        builderWSUrl
    }) => {
        // bob keeps the chat loaded: a resume is served from a worker that already holds the history, a
        // cold worker answers every subscriber with a full sync.
        const b = await joinChat(homeUrl, builderWSUrl, bob);

        const snapshot = await fetchSnapshot(builderUrl, CHAT, alice.sessionCookie);
        expect(snapshot.status).toBe(200);
        const frame = snapshot.frame!;
        expect(frame).toMatchObject({ type: 'streamSync', streamId: CHAT, snapshotEvery: 5 });
        const tip = Math.max(frame.snapshot?.version ?? 0, ...frame.commands.map((c) => c.version));
        if (frame.snapshot) {
            expect(frame.snapshot.hash).toMatch(/^[0-9a-f]{16}$/);
            expect(Buffer.from(frame.snapshot.data, 'base64').length).toBeGreaterThan(0);
        }

        let commandId = randomUUID();
        b.socket.command(CHAT, commandId, { type: 'add', text: `tail ${randomUUID()}` });
        let tailFrame = await b.socket.next('streamCommands', ownsCommand(commandId));
        let resumeFrom = tip;

        // A snapshot is stored asynchronously right after the command that lands on a snapshot
        // boundary. Resuming from before that boundary races the snapshot write: if it wins, the
        // resume looks "older than the newest snapshot" and gets a full sync instead of the tail.
        // Resuming from the boundary version itself is race-free (it is always still ≤ the newest
        // snapshot once that snapshot exists), so push past it and resume from there instead.
        if (tailFrame.commands.find((c) => c.commandId === commandId)!.version % frame.snapshotEvery === 0) {
            resumeFrom = tailFrame.commands.find((c) => c.commandId === commandId)!.version;
            commandId = randomUUID();
            b.socket.command(CHAT, commandId, { type: 'add', text: `tail ${randomUUID()}` });
            tailFrame = await b.socket.next('streamCommands', ownsCommand(commandId));
        }

        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);
        socket.subscribe(CHAT, 'chat', { generation: frame.generation, version: resumeFrom });
        const tail = await socket.next('streamCommands');
        expect(tail.generation).toBe(frame.generation);
        expect(tail.commands.map((c) => c.commandId)).toContain(commandId);
        expect(tail.commands[0].version).toBe(resumeFrom + 1);
        await socket.expectNone('streamSync', 500);
    });

    test('Chat resume from another generation shall be answered with a full sync', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const b = await joinChat(homeUrl, builderWSUrl, bob);

        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);
        socket.subscribe(CHAT, 'chat', { generation: b.generation + 1000, version: 1 });
        const sync = await socket.next('streamSync');
        expect(sync.generation).toBe(b.generation);
    });

    test('Chat snapshot route shall reject a missing session and any other stream', async ({ builderUrl }) => {
        expect((await fetchSnapshot(builderUrl, CHAT)).status).toBe(401);
        expect((await fetchSnapshot(builderUrl, CHAT, 'not-a-valid-signed-sid')).status).toBe(401);
        // Only the global chat exists for users: any other stream is closed to them.
        expect((await fetchSnapshot(builderUrl, 'chat-private', alice.sessionCookie)).status).toBe(403);
        expect((await fetchSnapshot(builderUrl, 'not a stream id!', alice.sessionCookie)).status).toBe(400);
    });

    test('Chat socket shall be closed when its session is deleted', async ({ homeUrl, builderWSUrl }) => {
        const options: StreamSocketOptions = wsOptions(homeUrl, builderWSUrl, alice.sessionCookie);
        const socket = await StreamSocket.open(builderWSUrl, options);
        sockets.push(socket);
        socket.subscribe(CHAT, 'chat');
        await socket.next('streamSync');

        // The session checker (every 2 s in the test config) drops the connection of a deleted session.
        await mint.deleteUser(alice);
        const closed = await Promise.race([
            socket.closed,
            new Promise<never>((_, reject) => setTimeout(() => reject(new Error('socket still open')), 12_000))
        ]);
        expect(closed).toEqual({ code: 4000, reason: 'session_invalid' });
    });
});
