import { expect, test } from '$fixtures/setup';
import { StreamSocket, fetchSnapshot, wsOptions } from '$lib/api/builder_stream_ws';
import { MintedSession, SessionMint } from '$lib/mocks/session_mint';
import { randomUUID } from 'node:crypto';
import { Toxiproxy } from 'toxiproxy-node-client';

// The global chat is one room shared by every run; see chat_stream.ts.
const CHAT = 'chat-global';

test.describe('Builder chat stream failures', { tag: ['@infrastructure'] }, () => {
    // All toxiproxy-driven tests toggle the same shared proxies; the suite's global `workers: 1`
    // already keeps every test from running concurrently, but `serial` makes that explicit here and
    // skips the rest of the file once one test leaves the proxies in a bad state, rather than letting
    // every later test fail the same way.
    test.describe.configure({ mode: 'serial' });

    let toxiproxy: Toxiproxy;
    let mint: SessionMint;
    let alice: MintedSession;
    const sockets: StreamSocket[] = [];

    // `toxiproxy.reset()` only clears toxics, not a proxy's `enabled` flag: a test that fails before
    // re-enabling a disabled proxy would otherwise leave every later test (in any file) stuck against
    // a down dependency. Re-enable explicitly, both before and after each test.
    async function enableAllProxies(): Promise<void> {
        for (const name of ['redis', 'postgres']) {
            const proxy = await toxiproxy.get(name);
            await proxy.update({ enabled: true, listen: proxy.listen, upstream: proxy.upstream });
        }
    }

    test.beforeAll(async () => {
        toxiproxy = new Toxiproxy('http://localhost:8474');
        await enableAllProxies();
    });

    test.beforeEach(async () => {
        mint = await SessionMint.fromServerConfig();
        alice = await mint.createUserSession({ userId: randomUUID() });
    });

    test.afterEach(async () => {
        // Proxies first: teardownCreatedSessions() itself needs Redis, so restoring the proxies must
        // happen before any other cleanup that depends on them, however the test body failed.
        await toxiproxy.reset();
        await enableAllProxies();
        sockets.splice(0).forEach((socket) => socket.close());
        await mint.teardownCreatedSessions();
    });

    function ownsCommand(commandId: string): (frame: { commands: { commandId: string }[] }) => boolean {
        return (frame) => frame.commands.some((command) => command.commandId === commandId);
    }

    test('Chat stream shall nack writes during a Redis outage and continue without a gap or a reset after it', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);
        socket.subscribe(CHAT, 'chat');
        const sync = await socket.next('streamSync');
        const before = Math.max(sync.snapshot?.version ?? 0, ...sync.commands.map((c) => c.version));

        const redisProxy = await toxiproxy.get('redis');
        await redisProxy.update({ enabled: false, listen: redisProxy.listen, upstream: redisProxy.upstream });

        // Ingest needs Redis; the write is nacked, not silently dropped, and the socket stays open
        // (the session check skips transient infrastructure errors). The pool checkout has its own
        // short timeout (RDS-16), well under the pool's 30s, so the nack arrives quickly.
        const duringId = randomUUID();
        socket.command(CHAT, duringId, { type: 'add', text: `during ${randomUUID()}` });
        const nack = await socket.next('streamNack', (frame) => frame.commandId === duringId, 8_000);
        expect(nack.streamId).toBe(CHAT);

        await redisProxy.update({ enabled: true, listen: redisProxy.listen, upstream: redisProxy.upstream });

        // Live delivery resumes right where it left off: no gap, no reset.
        const afterId = randomUUID();
        socket.command(CHAT, afterId, { type: 'add', text: `after ${randomUUID()}` });
        const frame = await socket.next('streamCommands', ownsCommand(afterId));
        const delivered = frame.commands.find((c) => c.commandId === afterId)!;
        expect(delivered.version).toBe(before + 1);
    });

    test('Chat stream shall keep delivering during a Postgres outage and push the checkpoint once Postgres returns', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);
        socket.subscribe(CHAT, 'chat');
        await socket.next('streamSync');

        const postgresProxy = await toxiproxy.get('postgres');
        await postgresProxy.update({ enabled: false, listen: postgresProxy.listen, upstream: postgresProxy.upstream });

        // Ingest and live delivery only need Redis; Postgres is only on the persistence path.
        for (let i = 0; i < 2; i++) {
            const commandId = randomUUID();
            socket.command(CHAT, commandId, { type: 'add', text: `outage ${i} ${randomUUID()}` });
            await socket.next('streamCommands', ownsCommand(commandId));
        }

        await postgresProxy.update({ enabled: true, listen: postgresProxy.listen, upstream: postgresProxy.upstream });

        // A checkpoint needs a stored snapshot, which needs Postgres: it lands again once Postgres is back.
        let checkpoint: { version: number; hash: string } | undefined;
        for (let i = 0; i < 10 && !checkpoint; i++) {
            const commandId = randomUUID();
            socket.command(CHAT, commandId, { type: 'add', text: `recovered ${randomUUID()}` });
            await socket.next('streamCommands', ownsCommand(commandId));
            try {
                checkpoint = await socket.next('streamCheckpoint', () => true, 300);
            } catch {
                // No checkpoint landed on this command; keep sending.
            }
        }
        expect(checkpoint).toBeDefined();
    });

    test('Chat snapshot route shall answer 503 while Postgres is down and recover', async ({ builderUrl }) => {
        const postgresProxy = await toxiproxy.get('postgres');
        await postgresProxy.update({ enabled: false, listen: postgresProxy.listen, upstream: postgresProxy.upstream });

        const down = await fetchSnapshot(builderUrl, CHAT, alice.sessionCookie);
        expect(down.status).toBe(503);

        await postgresProxy.update({ enabled: true, listen: postgresProxy.listen, upstream: postgresProxy.upstream });

        const recovered = await fetchSnapshot(builderUrl, CHAT, alice.sessionCookie);
        expect(recovered.status).toBe(200);
    });

    test('Chat subscribe shall fail with streamSubscribeFailed while Postgres is down', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const postgresProxy = await toxiproxy.get('postgres');
        await postgresProxy.update({ enabled: false, listen: postgresProxy.listen, upstream: postgresProxy.upstream });

        // open() always writes to Postgres before it can use any resident Redis copy (STR-14), so a
        // subscribe fails while Postgres is down regardless of whether the stream is warm or cold.
        const socket = await StreamSocket.open(builderWSUrl, wsOptions(homeUrl, builderWSUrl, alice.sessionCookie));
        sockets.push(socket);
        socket.subscribe(CHAT, 'chat');
        const failed = await socket.next('streamSubscribeFailed');
        expect(failed.streamId).toBe(CHAT);
    });
});
