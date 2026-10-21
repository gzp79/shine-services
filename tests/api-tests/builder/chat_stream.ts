import { expect, test } from '$fixtures/setup';
import { StreamSocket, WireCommand, fetchSnapshot, wsOptions } from '$lib/api/builder_stream_ws';
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

    test('Chat shall order commands from both writers identically, with user id and content intact', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const a = await joinChat(homeUrl, builderWSUrl, alice);
        const b = await joinChat(homeUrl, builderWSUrl, bob);

        // A sequence of steps by both writers: add, then remove/replace/no-op remove by either of them.
        // Whether a command takes effect is the state's decision on replay (only the author may change a
        // message; covered by the Rust tests): the log accepts and orders all of them, so both sockets see
        // every command delivered, with the same version, user id, command and generation.
        const messageId = randomUUID();
        const steps: Array<{ socket: StreamSocket; other: StreamSocket; user: MintedSession; command: ChatCommand }> = [
            { socket: a.socket, other: b.socket, user: alice, command: { type: 'add', text: `mine ${messageId}` } },
            { socket: b.socket, other: a.socket, user: bob, command: { type: 'remove', id: messageId } },
            {
                socket: a.socket,
                other: b.socket,
                user: alice,
                command: { type: 'replace', id: messageId, text: 'edited' }
            },
            { socket: a.socket, other: b.socket, user: alice, command: { type: 'remove', id: randomUUID() } },
            { socket: a.socket, other: b.socket, user: alice, command: { type: 'remove', id: messageId } }
        ];
        let previous = 0;
        for (const step of steps) {
            const commandId = step.command.type === 'add' ? messageId : randomUUID();
            step.socket.command(CHAT, commandId, step.command);
            const own = await step.socket.next('streamCommands', ownsCommand(commandId));
            const other = await step.other.next('streamCommands', ownsCommand(commandId));
            for (const frame of [own, other]) {
                expect(frame.streamId).toBe(CHAT);
                const delivered = frame.commands.find((c) => c.commandId === commandId)!;
                // The author comes from the session, never from the payload.
                expect(delivered.userId).toBe(step.user.userId);
                // The command text is stored canonically and sent as a string.
                expect(JSON.parse(delivered.command)).toEqual(step.command);
                expect(delivered.version).toBeGreaterThan(previous);
            }
            const [ownVersion] = own.commands.filter((c) => c.commandId === commandId).map((c) => c.version);
            const [otherVersion] = other.commands.filter((c) => c.commandId === commandId).map((c) => c.version);
            expect(otherVersion).toBe(ownVersion);
            expect(own.generation).toBe(other.generation);
            previous = ownVersion;
        }

        // Two writers sending concurrently still get one consistent, gapless order, each keeping its own
        // sending order.
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
        for (const own of [ids.a, ids.b]) {
            expect(forAlice.filter((c) => own.includes(c.commandId)).map((c) => c.commandId)).toEqual(own);
        }
    });

    test('Chat shall store and deliver a command in its canonical form', async ({ homeUrl, builderWSUrl }) => {
        const a = await joinChat(homeUrl, builderWSUrl, alice);
        const commandId = randomUUID();
        const text = `canon ${randomUUID()}`;
        // Extra whitespace, a reordered key and an unknown field: none of it should survive storage.
        a.socket.sendRaw(
            JSON.stringify({
                type: 'streamCommand',
                streamId: CHAT,
                commandId,
                command: `{ "extra": 1, "text": "${text}", "type":   "add" }`
            })
        );
        const frame = await a.socket.next('streamCommands', ownsCommand(commandId));
        const delivered = frame.commands.find((c) => c.commandId === commandId)!;
        // Exact bytes, not `JSON.parse`: it must equal the canonical form byte for byte.
        expect(delivered.command).toBe(`{"type":"add","text":"${text}"}`);
    });

    test('Chat shall refuse a retried command id once', async ({ homeUrl, builderWSUrl }) => {
        const a = await joinChat(homeUrl, builderWSUrl, alice);

        const commandId = randomUUID();
        a.socket.command(CHAT, commandId, { type: 'add', text: `once ${randomUUID()}` });
        await a.socket.next('streamCommands', ownsCommand(commandId));

        // A client that did not see its command retries it with the same id: it is already in the log.
        a.socket.command(CHAT, commandId, { type: 'add', text: 'once again' });
        const duplicate = await a.socket.next('streamNack', (frame) => frame.commandId === commandId);
        expect(duplicate).toMatchObject({ streamId: CHAT, reason: 'duplicate' });
        await a.socket.expectNone('streamCommands', 500, ownsCommand(commandId));
    });

    test('Chat shall refuse a command id reused by another user as duplicate', async ({ homeUrl, builderWSUrl }) => {
        const a = await joinChat(homeUrl, builderWSUrl, alice);
        const b = await joinChat(homeUrl, builderWSUrl, bob);

        const commandId = randomUUID();
        a.socket.command(CHAT, commandId, { type: 'add', text: `first ${randomUUID()}` });
        await a.socket.next('streamCommands', ownsCommand(commandId));

        // Dedupe is keyed by command id alone; a different author reusing it is still a duplicate.
        b.socket.command(CHAT, commandId, { type: 'add', text: 'stolen id' });
        const duplicate = await b.socket.next('streamNack', (frame) => frame.commandId === commandId);
        expect(duplicate).toMatchObject({ streamId: CHAT, reason: 'duplicate' });
    });

    test('Chat shall refuse invalid chat commands', async ({ homeUrl, builderWSUrl }) => {
        const a = await joinChat(homeUrl, builderWSUrl, alice);

        const invalid: Array<[string, object | string]> = [
            ['an unknown command type', { type: 'shout', text: 'x' }],
            ['an empty text', { type: 'add', text: '   ' }],
            ['an oversized text', { type: 'add', text: 'x'.repeat(2001) }],
            ['a missing field', { type: 'remove' }],
            ['a non-JSON command payload', 'not json']
        ];
        for (const [title, command] of invalid) {
            const id = randomUUID();
            if (typeof command === 'string') {
                a.socket.sendRaw(JSON.stringify({ type: 'streamCommand', streamId: CHAT, commandId: id, command }));
            } else {
                a.socket.command(CHAT, id, command);
            }
            const nack = await a.socket.next('streamNack', (frame) => frame.commandId === id);
            expect(nack.reason, title).toBe('invalid');
        }
    });

    // The byte-length check runs before canonicalization, so garbage past maxCommandBytes (16 KiB
    // deployed) is refused on size alone. Kept here until WS-1 lowers the frame limit below it, after
    // which this becomes a Rust unit check.
    test('Chat shall refuse a command longer than maxCommandBytes as invalid', async ({ homeUrl, builderWSUrl }) => {
        const a = await joinChat(homeUrl, builderWSUrl, alice);
        const commandId = randomUUID();
        const oversized = 'x'.repeat(17 * 1024);
        a.socket.sendRaw(JSON.stringify({ type: 'streamCommand', streamId: CHAT, commandId, command: oversized }));
        expect((await a.socket.next('streamNack', (frame) => frame.commandId === commandId)).reason).toBe('invalid');
    });

    // Pins today's silence (no reply) for malformed input the socket cannot act on; WS-8 flips these
    // to an `error{invalid}` frame.
    test('Chat shall silently ignore a binary frame, a non-UUID command id and an invalid stream id', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);
        socket.subscribe(CHAT, 'chat');
        await socket.next('streamSync');

        socket.sendBinary(Buffer.from('binary frame'));
        socket.sendRaw(JSON.stringify({ type: 'streamSubscribe', streamId: 'not a stream id!', kind: 'chat' }));
        socket.sendRaw(JSON.stringify({ type: 'streamUnsubscribe', streamId: 'not a stream id!' }));
        socket.sendRaw(
            JSON.stringify({
                type: 'streamCommand',
                streamId: 'not a stream id!',
                commandId: randomUUID(),
                command: '{}'
            })
        );
        socket.sendRaw(
            JSON.stringify({ type: 'streamCommand', streamId: CHAT, commandId: 'not-a-uuid', command: '{}' })
        );
        await socket.expectNone('streamSubscribeFailed', 500);
        await socket.expectNone('streamNack', 500);

        // The socket is still usable after all of the above.
        const commandId = randomUUID();
        socket.command(CHAT, commandId, { type: 'add', text: `still alive ${randomUUID()}` });
        await socket.next('streamCommands', ownsCommand(commandId));
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

    test('Chat shall refuse commands for a stream whose subscribe failed', async ({ homeUrl, builderWSUrl }) => {
        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);

        socket.subscribe('chat-private', 'chat');
        await socket.next('streamSubscribeFailed');

        const commandId = randomUUID();
        socket.command('chat-private', commandId, { type: 'add', text: 'nobody listens either' });
        expect(await socket.next('streamNack', (frame) => frame.commandId === commandId)).toMatchObject({
            streamId: 'chat-private',
            reason: 'notSubscribed'
        });
    });

    test('Chat shall accept a command sent right after the subscribe, before the sync', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);

        // The subscribe is followed locally before the hub is asked, so a command sent right behind it,
        // with no wait for the sync, must still be accepted.
        socket.subscribe(CHAT, 'chat');
        const commandId = randomUUID();
        socket.command(CHAT, commandId, { type: 'add', text: `fast ${randomUUID()}` });

        await socket.next('streamSync');
        await socket.next('streamCommands', ownsCommand(commandId));
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

    test('Chat shall refuse commands after the socket unsubscribed', async ({ homeUrl, builderWSUrl }) => {
        const a = await joinChat(homeUrl, builderWSUrl, alice);
        a.socket.unsubscribe(CHAT);

        const commandId = randomUUID();
        a.socket.command(CHAT, commandId, { type: 'add', text: 'after the unsubscribe' });
        expect(await a.socket.next('streamNack', (frame) => frame.commandId === commandId)).toMatchObject({
            streamId: CHAT,
            reason: 'notSubscribed'
        });
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
            // The commands start right after the snapshot: no gap, no overlap.
            if (frame.commands.length > 0) {
                expect(frame.commands[0].version).toBe(frame.snapshot.version + 1);
            }
        }
        for (let i = 1; i < frame.commands.length; i++) {
            expect(frame.commands[i].version).toBe(frame.commands[i - 1].version + 1);
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

    test('Chat checkpoint hash shall equal the snapshot route hash', async ({ homeUrl, builderUrl, builderWSUrl }) => {
        const a = await joinChat(homeUrl, builderWSUrl, alice);

        // snapshotEvery is 5 in the test config; a checkpoint lands on one of the next few commands,
        // whatever the current tip already is.
        let checkpoint: { version: number; hash: string } | undefined;
        for (let i = 0; i < 6 && !checkpoint; i++) {
            const commandId = randomUUID();
            a.socket.command(CHAT, commandId, { type: 'add', text: `cp ${randomUUID()}` });
            await a.socket.next('streamCommands', ownsCommand(commandId));
            try {
                checkpoint = await a.socket.next('streamCheckpoint', () => true, 200);
            } catch {
                // No checkpoint landed on this command; keep sending.
            }
        }
        expect(checkpoint).toBeDefined();

        const snapshot = await fetchSnapshot(builderUrl, CHAT, alice.sessionCookie);
        expect(snapshot.status).toBe(200);
        expect(snapshot.frame!.snapshot?.version).toBe(checkpoint!.version);
        expect(snapshot.frame!.snapshot?.hash).toBe(checkpoint!.hash);
    });

    test('Chat resume from another generation shall be answered with a full sync that includes prior history', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const b = await joinChat(homeUrl, builderWSUrl, bob);
        const commandId = randomUUID();
        b.socket.command(CHAT, commandId, { type: 'add', text: `resync ${randomUUID()}` });
        const echo = await b.socket.next('streamCommands', ownsCommand(commandId));
        const version = echo.commands.find((c) => c.commandId === commandId)!.version;

        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);
        socket.subscribe(CHAT, 'chat', { generation: b.generation + 1000, version: 1 });
        const sync = await socket.next('streamSync');
        expect(sync.generation).toBe(b.generation);
        // The command is either still in the tail, or already folded into the snapshot.
        const covered =
            sync.commands.some((c) => c.commandId === commandId) || (sync.snapshot?.version ?? 0) >= version;
        expect(covered).toBe(true);
    });

    test('Chat resume at the head shall be answered with an empty streamCommands and then follow live', async ({
        homeUrl,
        builderUrl,
        builderWSUrl
    }) => {
        const b = await joinChat(homeUrl, builderWSUrl, bob);

        const snapshot = await fetchSnapshot(builderUrl, CHAT, alice.sessionCookie);
        const tip = Math.max(snapshot.frame!.snapshot?.version ?? 0, ...snapshot.frame!.commands.map((c) => c.version));

        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);
        socket.subscribe(CHAT, 'chat', { generation: snapshot.frame!.generation, version: tip });
        const empty = await socket.next('streamCommands');
        expect(empty.commands).toEqual([]);

        const commandId = randomUUID();
        b.socket.command(CHAT, commandId, { type: 'add', text: `live ${randomUUID()}` });
        const live = await socket.next('streamCommands', ownsCommand(commandId));
        expect(live.commands.map((c) => c.commandId)).toContain(commandId);
    });

    test('Chat resume above the head shall be answered with a full sync', async ({ homeUrl, builderWSUrl }) => {
        const b = await joinChat(homeUrl, builderWSUrl, bob);

        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);
        socket.subscribe(CHAT, 'chat', { generation: b.generation, version: Number.MAX_SAFE_INTEGER });
        const sync = await socket.next('streamSync');
        expect(sync.generation).toBe(b.generation);
    });

    test('Chat resume older than the newest snapshot shall be answered with a full sync', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const b = await joinChat(homeUrl, builderWSUrl, bob);

        let checkpoint: { version: number; hash: string } | undefined;
        for (let i = 0; i < 6 && !checkpoint; i++) {
            const commandId = randomUUID();
            b.socket.command(CHAT, commandId, { type: 'add', text: `old ${randomUUID()}` });
            await b.socket.next('streamCommands', ownsCommand(commandId));
            try {
                checkpoint = await b.socket.next('streamCheckpoint', () => true, 200);
            } catch {
                // No checkpoint landed on this command; keep sending.
            }
        }
        expect(checkpoint).toBeDefined();

        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);
        socket.subscribe(CHAT, 'chat', { generation: b.generation, version: checkpoint!.version - 1 });
        const sync = await socket.next('streamSync');
        expect(sync.generation).toBe(b.generation);
        expect(sync.snapshot?.version).toBe(checkpoint!.version);
    });

    test('Chat snapshot route shall reject a missing session and any other stream', async ({ builderUrl }) => {
        for (const rejectCase of [
            { title: 'missing session cookie', sid: undefined, streamId: CHAT, status: 401 },
            { title: 'tampered session cookie', sid: 'not-a-valid-signed-sid', streamId: CHAT, status: 401 },
            // Only the global chat exists for users: any other stream is closed to them.
            { title: 'another stream', sid: alice.sessionCookie, streamId: 'chat-private', status: 403 },
            { title: 'an invalid stream id', sid: alice.sessionCookie, streamId: 'not a stream id!', status: 400 }
        ]) {
            const result = await fetchSnapshot(builderUrl, rejectCase.streamId, rejectCase.sid);
            expect(result.status, rejectCase.title).toBe(rejectCase.status);
        }

        const ok = await fetchSnapshot(builderUrl, CHAT, alice.sessionCookie);
        expect(ok.cacheControl).toBe('no-store');
    });
});
