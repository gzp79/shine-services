import { expect, test } from '$fixtures/setup';
import { type StreamFrameOf, StreamSession, addCommand, wsConnectUrl, wsHeaders } from '$lib/api/builder_ws';
import { MintedSession, SessionMint } from '$lib/mocks/session_mint';
import { waitForCondition } from '$lib/utils';
import { randomUUID } from 'node:crypto';

// What a stream client sees while the database behind the builder is down, and that it can carry on once
// the database is back. The builder keeps its sockets in both cases: the clients are not dropped for it.

const newStreamId = (prefix: string) => `chat:${prefix}-${randomUUID().replace(/-/g, '')}`;
const add = addCommand;

test.describe('Builder stream failure tests', { tag: '@infrastructure' }, () => {
    test.describe.configure({ mode: 'serial' });

    let mint: SessionMint;
    let user: MintedSession;
    const sessions: StreamSession[] = [];

    test.beforeEach(async () => {
        mint = await SessionMint.fromServerConfig();
        user = await mint.createUserSession({ userId: randomUUID() });
    });

    test.afterEach(async () => {
        for (const session of sessions.splice(0)) {
            session.close();
        }
        await mint.teardownCreatedSessions();
    });

    const connect = async (homeUrl: string, builderWSUrl: string) => {
        const session = await StreamSession.open(wsConnectUrl(builderWSUrl), {
            sid: user.sessionCookie,
            ...wsHeaders(homeUrl, builderWSUrl)
        });
        sessions.push(session);
        return session;
    };

    test('Subscribing while Postgres is permanently down shall be told the subscription failed', async ({
        homeUrl,
        builderWSUrl,
        infraOutage
    }) => {
        const a = await connect(homeUrl, builderWSUrl);
        const streamId = newStreamId('room');

        await infraOutage.stopPostgres();

        // Each retry includes the pool acquisition timeout as well as the backoff.
        a.send({ type: 'streamSubscribe', streamId });

        const failed = await a.waitFrame('streamSubscribeFailed', (f) => f.streamId === streamId, 40_000);
        expect(failed.streamId).toBe(streamId);
        expect(failed.reason).toBe('failed');

        const refused = randomUUID();
        a.command(streamId, refused, add('after failure'));
        expect(await a.waitFrame('streamNack', (f) => f.commandId === refused)).toMatchObject({
            streamId,
            reason: 'notSubscribed'
        });

        const isConnected = await Promise.race([
            a.closed.then(() => false),
            new Promise<boolean>((resolve) => setTimeout(() => resolve(true), 1000))
        ]);
        expect(isConnected).toBe(true);

        await infraOutage.startPostgres();
        expect(await a.subscribe(streamId)).toMatchObject({ type: 'streamSync', streamId, commands: [] });
    });

    test('Subscribing while Postgres is temporarily down shall resume after recovery', async ({
        homeUrl,
        builderWSUrl,
        infraOutage
    }) => {
        const a = await connect(homeUrl, builderWSUrl);
        const streamId = newStreamId('room');

        await infraOutage.stopPostgres();

        a.send({ type: 'streamSubscribe', streamId });
        const synced = a.waitFrame('streamSync', (f) => f.streamId === streamId, 20_000);
        await new Promise((resolve) => setTimeout(resolve, 500));
        await infraOutage.startPostgres();
        const sync = await synced;
        expect(sync.commands).toEqual([]);
        await a.expectNoFrame('streamSubscribeFailed', (f) => 'streamId' in f && f.streamId === streamId);
    });

    test('Command while Redis is down shall be refused as failed and the stream shall carry on after', async ({
        homeUrl,
        builderWSUrl,
        infraOutage
    }) => {
        const a = await connect(homeUrl, builderWSUrl);
        const streamId = newStreamId('room');
        await a.subscribe(streamId);

        await infraOutage.stopRedis();

        // The commands wait in Redis; that they cannot be queued is the sender's to know, it may send again.
        const refused = randomUUID();
        a.command(streamId, refused, add('down'));
        expect(await a.waitFrame('streamNack', (f) => f.commandId === refused, 20_000)).toMatchObject({
            streamId,
            reason: 'failed'
        });

        await infraOutage.startRedis();

        // A refused command may or may not have been queued, so the later ones are matched by their ids. The
        // worker looks at the inbox again at the latest on its backstop tick.
        const delivered = await waitForCondition(
            async () => {
                const commandId = randomUUID();
                a.command(streamId, commandId, add('up'));
                const frame: StreamFrameOf<'streamCommands'> = await a.waitFrame(
                    'streamCommands',
                    (f) => f.streamId === streamId,
                    12_000
                );
                return frame.commands.find((c) => c.command === add('up'));
            },
            { timeout: 60_000, interval: 500, errorMessage: 'The stream did not carry on after Redis' }
        );
        expect(delivered?.command).toEqual(add('up'));
        // The socket was kept all along.
        expect(await a.subscribe(streamId)).toMatchObject({ type: 'streamSync', streamId });
    });
});
