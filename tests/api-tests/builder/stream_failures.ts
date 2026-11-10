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
    // Injected toxics affect every service, hence these tests must never overlap with others.
    test.describe.configure({ mode: 'serial' });
    // The builder gives up on a database call after several seconds, and the recovery waits on its pools.
    test.setTimeout(90_000);

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

    test('Subscribing while Postgres is down shall be told the stream is unsynced and work again after', async ({
        homeUrl,
        builderWSUrl,
        toxiproxy
    }) => {
        const a = await connect(homeUrl, builderWSUrl);
        const streamId = newStreamId('room');

        const postgres = await toxiproxy.get('postgres');
        await postgres.update({ enabled: false, listen: postgres.listen, upstream: postgres.upstream });

        // The stream cannot be loaded, so the worker gives up and tells the subscriber who was waiting for it.
        a.send({ type: 'streamSubscribe', streamId });
        await a.waitFrame('streamUnsynced', (f) => f.streamId === streamId, 20_000);

        await postgres.update({ enabled: true, listen: postgres.listen, upstream: postgres.upstream });

        // The next subscribe starts the stream again; the pools of the builder may need a try to drop the
        // connections that were cut.
        const sync = await waitForCondition(
            async () => {
                a.send({ type: 'streamSubscribe', streamId });
                return await a.waitFrame('streamSync', (f) => f.streamId === streamId, 8000);
            },
            { timeout: 40_000, interval: 500, errorMessage: 'The stream did not come back after Postgres' }
        );
        expect(sync.commands).toEqual([]);
    });

    test('Command while Redis is down shall be refused as failed and the stream shall carry on after', async ({
        homeUrl,
        builderWSUrl,
        toxiproxy
    }) => {
        const a = await connect(homeUrl, builderWSUrl);
        const streamId = newStreamId('room');
        await a.subscribe(streamId);

        const redis = await toxiproxy.get('redis');
        await redis.update({ enabled: false, listen: redis.listen, upstream: redis.upstream });

        // The commands wait in Redis; that they cannot be queued is the sender's to know, it may send again.
        const refused = randomUUID();
        a.command(streamId, refused, add('down'));
        expect(await a.waitFrame('streamNack', (f) => f.commandId === refused, 20_000)).toMatchObject({
            streamId,
            reason: 'failed'
        });

        await redis.update({ enabled: true, listen: redis.listen, upstream: redis.upstream });

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
