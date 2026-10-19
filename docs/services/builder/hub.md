# Builder hub

*The builder service's real-time layer: one live connection per user across all instances, and replicated command streams (chat is one of them) delivered to clients as a snapshot plus commands.*

## Problem

Clients hold a WebSocket to the builder service. The hub must keep each user to **one active connection** across every service instance, end a connection when its session dies, and never wedge on a slow or dead socket. On top of the connections it delivers shared state — chat, game state — as **command streams**: an ordered log of commands per stream, from which every client rebuilds the same state (a snapshot, then the commands after it). Chat is only one stream kind; the hub and the stream protocol know nothing chat-specific.

## Architecture

All connection state changes go through a single command loop (the sole writer); Redis is the shared registry and the ground truth for who is connected. Background tasks keep their own view of the connections from the hub's lifecycle events.

```mermaid
flowchart LR
    classDef indigo fill:#6366f122,stroke:#6366f1
    classDef blue fill:#0ea5e922,stroke:#0ea5e9
    classDef green fill:#22c55e22,stroke:#22c55e
    classDef orange fill:#fb923c22,stroke:#fb923c
    classDef violet fill:#a855f722,stroke:#a855f7

    client(["WS client"]):::indigo
    subgraph instance ["builder instance"]
        ws["ws socket<br/>connect.rs"]:::indigo
        subgraph hub ["HubService"]
            cmdloop["command loop<br/>sole writer"]:::blue
            conns[("Connections<br/>user, connection, egress tx")]:::green
            subs[("Subscribers<br/>internal consumers")]:::green
        end
        hb["Heartbeat"]:::orange
        sc["SessionChecker"]:::orange
        sd["StreamDispatcher<br/>one worker per stream"]:::orange
        ing["CommandStreamService<br/>ingest"]:::blue
        rl["registry listener"]:::orange
    end
    redis[("Redis<br/>registry rows, pub/sub, stream logs, sessions")]:::violet
    pg[("Postgres<br/>command log, snapshots")]:::violet
    other["other builder instances"]

    client <--> ws
    ws -- ControlCommand --> cmdloop
    ws -- streamCommand --> ing
    ing --> redis
    cmdloop --> conns
    cmdloop -- HubEvent, StreamEvent --> subs
    subs --> hb & sc & sd
    hb & sc -- request_drop --> cmdloop
    sd -- send_to_connection --> conns
    sd <--> redis
    sd --> pg
    conns -- HubMessage --> ws
    cmdloop <--> redis
    hb & sc <--> redis
    rl -- HubRegistryChanged --> cmdloop
    redis -- hub-registry-changed --> rl
    other <--> redis
```

| Component | Role |
|---|---|
| `HubService` | Public API, command loop (`process`), publishing, shutdown. Cheap to clone (`Arc`). |
| `Connections` | `user_id → connection_id` and `connection_id → (session key, egress tx, topics)`. Read by senders, written only by the loop. |
| `Subscribers` | Unbounded, lossless channels of the internal consumers; each receives every `HubEvent` and `StreamEvent`. |
| `ConnectionTracker` | Shared loop of the background consumers: applies events to a private view, runs `on_tick` periodically and on wake. |
| `Heartbeat` | Refreshes the registry TTL of every local connection; drops those the registry no longer holds. |
| `SessionChecker` | Re-validates each connection's session in batches; drops expired or compromised ones. |
| `StreamDispatcher` | Routes subscriptions to the `StreamWorker` of each resident stream, unloads a stream `unloadGraceSeconds` after its last subscriber, restarts a worker that ended unexpectedly (at most once a second per stream) and runs the orphan sweep. |
| `CommandStreamService` | Opens streams (kind, access, creation, Redis hydration), ingests commands (validation, dedupe, version assignment), builds snapshot reads, persists orphaned streams. |
| `RedisHubConnectionDb` | Registry rows `hub-connection:<user>` → active connection id, with TTL; publishes `hub-registry-changed`. |
| ws socket | Registers the socket, forwards egress to the client, passes client frames on, ends the connection when either side closes. |

| Type | Direction | Variants |
|---|---|---|
| `ControlCommand` | anyone → command loop | `ConnectUser`, `DisconnectUser { drop_reason }`, `DropConnection { reason }`, `HubRegistryChanged`, `SubscribeStream`, `UnsubscribeStream`, `Shutdown` |
| `HubEvent` | loop → all subscribers (and Hub-topic connections) | `UserConnected`, `UserDisconnected`, `Shutdown` |
| `StreamEvent` | loop → internal subscribers only (`TopicKey::Stream`) | a connection subscribed to or left a stream |
| `HubMessage` | hub → one connection's egress | `Hub(HubEvent)`, `Stream(frame)` |

## Key rules

| Concern | Rule |
|---|---|
| One writer | Only the command loop mutates `Connections`; commands run one at a time, in FIFO order. Callers request (`request_connection`, `request_disconnection`, `request_drop`, …). |
| One connection per user | A new connection replaces the user's previous one: directly on this instance, through the registry and its change notifications on others. Ids are fresh v4 UUIDs, never reused. |
| Redis is the ground truth | The registry row names the active connection. A connection whose row is missing or holds another id is not registered and is dropped at the next heartbeat; removing a row is how a user is disconnected. |
| Idempotent removal | Every removal is CAS'd on the connection id: a stale or repeated disconnect/drop is a no-op, never removes a newer connection, never counts twice. |
| Drops are counted once | A hub-initiated removal is a drop with one reason and notifies its socket. A client-requested disconnect is not a drop. |
| Lifecycle events | `UserConnected` is published once, `UserDisconnected` at most once, only while active; nothing is published after `Shutdown`. |
| Lossless consumers | Internal subscribers are registered before the loop starts and never drop an event; addressable connections are bounded (1000 per socket) and a full or closed channel prunes the connection. |
| Hub ends sockets | A removed connection receives its own `UserDisconnected{reason}` (best effort), then its channel is dropped and the socket closes with a matching close frame. |
| Redis errors never wedge a socket | Removals always complete locally; a row that could not be removed expires by TTL. |
| Streams | Redis orders the commands, Postgres is the truth; a stream is delivered as snapshot plus commands and re-synced by subscribing again (see [Command streams](#command-streams)). |

## Lifecycles

The command loop is the `Hub` participant; activation bars mark the part that runs on it.

### Connect, with replacement

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant WS as ws socket
    participant H as Hub (command loop)
    participant R as Redis registry
    participant S as Subscribers
    participant Old as old socket

    C->>+WS: upgrade (session cookie)
    WS->>H: request_connection → ConnectUser(user, id, tx, topics)
    Note over WS: returns the new id immediately
    deactivate WS
    activate H
    H->>+R: SET hub-connection:user = id EX ttl + PUBLISH change
    R-->>-H: ok
    alt user had another connection here
        H->>H: remove old (CAS), count drop "replaced"
        H-)Old: UserDisconnected{replaced}, drop tx
        H->>S: UserDisconnected(old)
    end
    H->>H: register (user, id, tx)
    H->>S: UserConnected(id)
    deactivate H
    Old-->>C: close frame 4000 "replaced" (old client)
```

If the registry write **fails**, the hub reads the row back: if it already holds the new id (the write landed, its reply was lost) the connect proceeds; otherwise it is rejected: the new socket's `tx` is dropped (socket closes, nothing published) and the previous connection stays intact.

### Client disconnect

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant WS as ws socket
    participant H as Hub (command loop)
    participant R as Redis registry
    participant S as Subscribers

    C->>WS: close / socket error
    activate WS
    Note over WS: receive side ended
    WS->>H: request_disconnection → DisconnectUser(user, id, None)
    Note over WS: keeps its egress open (up to 5 s) until the hub drops it,<br/>so in-flight sends are not mistaken for a dead channel
    activate H
    H->>H: remove (CAS on id), not a drop
    H->>+R: remove row if it holds id + PUBLISH change
    R-->>-H: ok / error (row left to TTL)
    H->>S: UserDisconnected(id)
    H-->>WS: tx dropped
    deactivate H
    WS->>WS: send Close(None), exit
    deactivate WS
```

### Forced drop

Shown for the session check; the heartbeat is the same with reason `heartbeat_inactive`.

```mermaid
sequenceDiagram
    autonumber
    participant SC as SessionChecker
    participant Ses as Redis sessions
    participant H as Hub (command loop)
    participant WS as ws socket
    participant C as Client

    loop every auth check interval
        activate SC
        SC->>+Ses: are_sessions_active(batch)
        Ses-->>-SC: not active
        SC->>H: request_drop(user, id, "session_invalid")
        deactivate SC
    end
    activate H
    H->>H: remove (CAS), count drop "session_invalid"
    H-)WS: UserDisconnected{session_invalid}, drop tx
    H->>H: remove registry row, publish UserDisconnected
    deactivate H
    activate WS
    WS-->>C: close frame 4000 "session_invalid"
    deactivate WS
```

The check is read-only (an open socket does not extend the session) and runs in batches of 256 sessions per Redis round trip. Transient errors skip the batch for this tick; only a missing session drops a connection.

### Heartbeat and TTL

```mermaid
sequenceDiagram
    autonumber
    participant HB as Heartbeat
    participant R as Redis registry
    participant H as Hub (command loop)

    loop every hubHeartbeatSeconds
        activate HB
        HB->>+R: MGET rows of all tracked connections
        R-->>-HB: current ids
        HB->>R: EXPIRE matching rows (ttl = 3 × heartbeat)
        opt rows missing or holding another id
            HB->>H: request_drop(user, id, "heartbeat_inactive")
        end
        deactivate HB
    end
```

The TTL only cleans up after crashed instances and failed removals; live rows are refreshed every heartbeat. A heartbeat that fails (Redis down) drops nothing and retries next tick. If Redis loses its data (flush, failover to a lagging replica, eviction), every row is missing and every connection is dropped; clients reconnect.

### Another instance takes the user over

```mermaid
sequenceDiagram
    autonumber
    participant CB as Client (new tab)
    participant B as Hub on instance B
    participant R as Redis
    participant LA as listener on A
    participant A as Hub on instance A
    participant WA as socket on A

    CB->>B: connect
    activate B
    B->>R: SET row = idB + PUBLISH "B:user"
    deactivate B
    R-)LA: hub-registry-changed "B:user"
    Note over LA: own notifications (A:...) are ignored
    LA->>A: notify_registry_changed → HubRegistryChanged(user)
    activate A
    A->>+R: heartbeat CAS(row == idA)?
    R-->>-A: false (row holds idB)
    A->>A: remove idA (CAS), count drop "registry_inactive"
    A-)WA: UserDisconnected{registry_inactive}, drop tx
    A->>A: publish UserDisconnected(idA)
    deactivate A
```

Notifications are not ordered with commands, so instances converge *eventually*: the last registry write wins, and every other instance drops its connection when the notification arrives. A lost notification (listener reconnect) is caught by the next heartbeat.

### Dead and slow connections

```mermaid
sequenceDiagram
    autonumber
    participant P as Producer (stream worker / broadcast)
    participant Cn as Connections
    participant H as Hub (command loop)

    P->>+Cn: try_send(message)
    Cn-->>-P: Full → slow_consumer / Closed → dead_channel
    P->>H: DropConnection(id, reason)
    Note over P: several producers may report the same connection
    activate H
    H->>H: remove by id, first report wins, later ones are no-ops
    H->>H: count drop, own UserDisconnected (lost if full), remove row, publish UserDisconnected
    deactivate H
```

A slow socket still receives what was buffered, then its channel ends and it closes without a reason. Inside the loop, `publish` prunes connections found dead during its own broadcast the same way.

### Shutdown

```mermaid
sequenceDiagram
    autonumber
    participant App as web app shutdown hook
    participant H as Hub (command loop)
    participant S as Subscribers / consumers
    participant R as Redis
    participant WS as every socket

    App->>H: request_shutdown → Shutdown
    activate H
    H->>S: Shutdown event (consumers stop)
    H->>S: release subscribers (nothing published after)
    H->>R: unlisten hub-registry-changed
    loop every connection
        H->>H: remove, count drop "shutdown"
        H-)WS: UserDisconnected{shutdown}
        H->>R: remove row
    end
    H->>H: exit loop, abort background tasks
    deactivate H
    WS-->>WS: close frame 1001 "shutdown"
```

Commands queued behind `Shutdown` are never processed; requests after the loop exits fail with `SendCommandFailed`, and a socket opened then is closed right away.

## Command streams

A **stream** is an ordered log of **commands** that every client replays into the same state. The hub only moves frames; what a command means, and what the state is, belongs to the stream's **kind**. There is one socket per user and a user follows only some of the streams, so a connection starts with **no stream**: nothing is sent for a stream until the client subscribes to it. A client subscribes when it needs a stream (for example when the chat panel opens) and unsubscribes when it stops; the end of the connection ends all its subscriptions. Subscribing delivers a **snapshot** (the state at some version) plus the commands after it, and from then on the live commands. Subscribing again is the client's resync request.

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant WS as ws socket
    participant H as Hub (command loop)
    participant SD as StreamDispatcher
    participant W as StreamWorker
    participant R as Redis stream log

    Note over C,WS: connected, following no stream
    C->>WS: streamSubscribe(streamId, kind)
    Note over WS: open the stream: kind, access, create it, hydrate Redis
    WS->>H: SubscribeStream, behind the connection's registration
    H-)SD: StreamEvent: subscribed
    SD->>W: route (spawn the worker if the stream is not resident)
    W-)C: streamSync(generation, snapshot, commands after it)
    Note over C: replaces its state for the stream and follows it from here
    C->>WS: streamCommand(streamId, commandId, command)
    WS->>R: ingest: dedupe by commandId, assign the next version, append, wake
    R-)W: wake
    W->>R: read commands after the worker's version
    W-)C: streamCommands(version, commandId, userId, command)
    C->>WS: streamUnsubscribe(streamId)
    WS->>H: UnsubscribeStream
    H-)SD: StreamEvent: unsubscribed
    Note over W: sends nothing more for this stream to this connection;<br/>a stream without subscribers is unloaded
    Note over C,WS: the socket stays connected and follows its other streams
```

Only the end of the connection (close, drop, replacement) removes every subscription at once; the hub keeps no subscription set per connection, so the stream dispatcher drops every subscription of a connection when it sees `UserDisconnected`.

| Direction | Frame | Fields |
|---|---|---|
| client → server | `streamSubscribe` | `streamId, kind, resume?: {generation, version}` |
| client → server | `streamUnsubscribe` | `streamId` |
| client → server | `streamCommand` | `streamId, commandId, command` (a JSON **string**) |
| server → client | `streamSync` | `streamId, generation, snapshotEvery, snapshot: {version, hash, data} \| null, commands` |
| server → client | `streamCommands` | `streamId, generation, commands: [{version, commandId, userId, command}]` |
| server → client | `streamCheckpoint` | `streamId, version, hash` |
| server → client | `streamReset` | `streamId` (a fresh `streamSync` follows) |
| server → client | `streamNack` | `streamId, commandId, reason` |
| server → client | `streamSubscribeFailed` | `streamId, reason` |

`streamId` is a short string (letters, digits, `_ . : -`, at most 64 characters) fixed by the kind or the application; the chat room is `chat-global`. A frame that does not parse, or names an invalid stream id, is ignored and counted in `ws_messages_received`. Frames are JSON text with `type` as the tag (camelCase). `command` is JSON text inside a JSON string, so a browser must not `JSON.parse` it (integers above 2^53 would round); `hash` is 16 lower-case hex digits; `snapshot.data` is base64 of the postcard encoding of the state's serde form (`encode_snapshot` / `decode_snapshot` in `shine-game`, which also builds for wasm).

| Concern | Rule |
|---|---|
| Subscription | Per connection and per stream. It starts with `streamSubscribe` and ends with `streamUnsubscribe`, which leaves the socket connected, or when the connection ends. Frames already in flight for an unsubscribed stream can still arrive, so the client ignores frames of streams it no longer follows; subscribing again starts with a fresh `streamSync`. A subscribe for a connection that is not registered is ignored, and a stream with no subscribers is unloaded. A client pays only for the streams it follows. |
| Snapshot and listening start together | On a plain subscribe the `streamSync` is built from the worker's replica and the subscriber joins the worker's shared cursor in the same step, so the commands after the snapshot cannot be missed. A subscribe that resumes from an HTTP snapshot gets the same guarantee from the versions (see *Starting from an HTTP snapshot*). |
| Position | A stream's worker keeps one shared delivery cursor for all its subscribers; a client's position is the last version it applied. The server stores no per-client position. |
| Ordering | Within one sync epoch a client receives every version exactly once, in order, and versions are contiguous; a gap means a frame was lost and the client subscribes again. |
| Sync | Every `streamSync` replaces the client's state for that stream, with or without a preceding `streamReset`. A large backlog is one `streamSync` followed by `streamCommands` frames of at most 256 commands. |
| Writes | A command is accepted once per `commandId`; there is no success ack, the writer sees its `commandId` in `streamCommands`. `streamNack` reports a failure: `duplicate` (already in the log: drop it from pending), `invalid` (not a valid command of the kind, or over `maxCommandBytes`), `notSubscribed`, `forbidden` (read-only), `notReady` (the Redis copy is being rebuilt: retry), `busy` (too many unpersisted commands: retry), `failed`. |
| Subscribe failures | `streamSubscribeFailed` carries `unknownKind`, `kindMismatch` (the stream has another kind), `forbidden`, `failed`, or `unavailable` (streams are disabled, or a stored command of the stream cannot be decoded and the stream cannot be replayed). |
| Order and truth | Redis orders the commands (version assignment is atomic in Redis); Postgres is the truth and wins every conflict, after which the Redis copy is rebuilt from it. |
| Checkpoints | Every `snapshotEvery` versions a snapshot is stored once (the first writer is authoritative) and its hash is pushed as `streamCheckpoint`, never ahead of the commands already delivered. The `streamSync` announces `snapshotEvery`, so a client hashes its own state at those versions and compares; a client whose hash differs resubscribes. A worker whose own state differs from the stored snapshot counts it as drift (`command_stream_drift`), takes the stored state and pushes its hash. |
| Retention | A snapshot is stored every `snapshotEvery` commands and Redis keeps the commands from the `keepSnapshots`-th newest stored snapshot on (at most about `snapshotEvery` × `keepSnapshots` persisted commands), plus the commands not persisted yet (never trimmed). Postgres keeps the latest `keepSnapshots` snapshots and the commands from the oldest of them on: the worker that stores a snapshot prunes the rest (`PgCommandLogDb::prune`, one statement), unless the kind sets `keepHistory`, which keeps every command and snapshot in Postgres. State older than the oldest kept snapshot is never served from history: a resume below the worker's snapshot, and a snapshot read, are answered from the newest snapshot, as a full sync, and a worker that fell behind the retained history starts over from the newest snapshot. Both settings are per kind, with global defaults. |
| Slow client | A full egress channel drops the connection; the client reconnects and subscribes again. Delivery never queues for a single client. |
| Guarantee | Per stream and connection: ordered, no duplicates inside a sync epoch. End to end: a worker persists the commands it has applied before the stream is rebuilt, so only a command that Redis accepted but no worker has read yet is lost when Redis loses data; the clients then get `streamReset`, a new sync, and the writer resends what it has not seen. |
| Residency | A stream is loaded on an instance while it has local subscribers and for `unloadGraceSeconds` after the last one; unloading persists what the worker holds. An orphan sweep on every instance persists and unlists the streams nobody has written to for `orphanIdleSeconds`, whichever instance wrote them. |

A stream **kind** (registered in `StreamKindRegistry`) supplies the command validator, the replayed state type and its snapshot encoding; the stream's kind is fixed at creation and never taken from a later client frame. Replaying a command gives the kind its **context** (version, `commandId` and the author's `userId`), so a state can record who wrote what and decide for itself whether a command takes effect (for example that only the author may remove a message). A command the state refuses is stored in the log and ignored on replay; ingest checks only the command's shape, its size and its `commandId`.

### Starting from an HTTP snapshot

A client can read a stream's state over plain HTTP, whether or not it has a live connection (first paint before the socket exists, or a socket that is already up without a subscription), then subscribe and follow from there. The read needs no connection and no instance affinity.

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant API as snapshot route
    participant S as Redis log + Postgres
    participant WS as ws socket
    participant W as StreamWorker

    C->>+API: GET streams/streamId/snapshot (session cookie)
    API->>+S: latest snapshot + commands after it, up to the head
    S-->>-API: state at version V of generation G
    API-->>-C: same payload as streamSync
    Note over C: renders the state at V
    opt no live connection yet
        C->>WS: upgrade (connect), following no stream
    end
    C->>WS: streamSubscribe(streamId, kind, resume {G, V})
    WS->>W: SubscribeStream, routed to the stream's worker
    alt the worker can continue from V in generation G
        W-)C: streamCommands from V + 1
    else generation changed, or V is older than the worker keeps
        W-)C: streamSync(snapshot, commands)
        Note over C: replaces its state
    end
    W-)C: streamCommands (live)
```

| Concern | Rule |
|---|---|
| Snapshot read | `GET /api/streams/{streamId}/snapshot` (next to `/api/connect`): authenticated by the session, same payload as `streamSync`. It is built from storage, not from a worker, so any instance answers it: the newest stored snapshot, the stored commands after it, and the Redis commands above the stored head, contiguous by version. The response is `no-store`. Statuses: `401` no session, `403` the user may not read the stream, `404` unknown stream, `400` invalid id, `503` streams disabled or storage unavailable. A Redis that is down, or holds another generation, only leaves the Redis part out. |
| Resume | `streamSubscribe` with `resume {generation, version}` continues from that version when the stream's generation is the same and the version is not older than the version of the worker's latest snapshot (the commands after it are what the worker holds); the first `streamCommands` then starts at `version + 1`. Otherwise the worker answers with a normal `streamSync` and the client replaces its state. A worker that has just loaded (nothing was resident on this instance) answers every subscriber, resuming ones included, with a full `streamSync`; a resume is a tail only while another subscriber keeps the history loaded. |
| Same guarantees | Snapshot and listening still start together on the worker's cursor, so nothing between the HTTP read and the subscribe is missed; a gap after a resume means the client subscribes again without `resume`. |

### Example: chat

Chat is one kind of stream: a single global room whose state is the most recent messages. Users add messages, and replace or remove their own.

| Concern | Chat kind |
|---|---|
| Stream | The stream id `chat-global`, created by the first subscribe. |
| Access | Every authenticated user may read and write `chat-global` and nothing else: any other stream id is refused with `forbidden` and creates nothing (`chat_access`). Every message carries its sender's `userId`. |
| State | The last 200 messages (a constant of the kind, so every replica and client agrees): id, author, text and an `edited` flag (set once the author has replaced the text). |
| Add | `{"type":"add","text":"..."}`; the text is trimmed, not empty, at most 2000 bytes. The message id is the `commandId` of the add; the author is the session's user from the command context, never from the payload. |
| Remove | `{"type":"remove","id":"<message id>"}`. Removes the message with that id when the sender wrote it. Any other remove (an unknown id, another user's message) is stored and ignored on replay. |
| Replace | `{"type":"replace","id":"<message id>","text":"..."}`. Replaces the text of the message with that id, keeping its place and id, and marks it edited, when the sender wrote it; the text follows the rules of an add. Any other replace (an unknown or removed id, another user's message) is stored and ignored on replay. |
| Snapshot | The state at a version divisible by `snapshotEvery` (50); Postgres keeps the latest `keepSnapshots` (5) and Redis at most 50 × 5 = 250 persisted commands; a removal is a command like an add. |
| Client | Renders the replayed state, from the HTTP snapshot first when the socket is not up yet; ordering and deduplication are by `version` and `commandId`. |

Anything else that needs shared, replayed state (a game session, a shared document) is another kind on the same hub and protocol.

### Delivery

A worker reads a stream from its own version. The triggers run idempotent passes, and a pass with nothing new sends nothing:

| Trigger | Role |
|---|---|
| Redis wake (pub/sub on the stream's key) | Primary: prompt delivery after an append. Wakes coalesce; a burst is one pass. |
| Persist timer (`persistDelayMs`, plus the same again as jitter, armed when commands are unpersisted) | Appends the applied commands to Postgres, records the persisted version in Redis, stores the checkpoint snapshot and pushes `streamCheckpoint`. |
| Periodic tick (`syncIntervalMs`) | Safety net: runs delivery and persistence, so it covers wakes missed while a listener was disconnected and retries after errors. |

A wake only says "something changed"; its payload is ignored, which is what makes coalescing and the reconnect wake safe.

## Client contract

- **One live connection per user, held by the active tab.** A tab opens its socket when it becomes the active one and closes it when it loses focus.
- **After a drop** the client reconnects with jittered backoff if its tab is active, and subscribes again to every stream it follows; each `streamSync` replaces its state.
- **A write is retried** with the same `commandId` until it appears in `streamCommands`.

## Drop reasons and close codes

| Reason | Trigger | Close frame |
|---|---|---|
| `replaced` | same user connected again on this instance | 4000 `replaced` |
| `registry_inactive` | another instance took the user (registry change) | 4000 `registry_inactive` |
| `heartbeat_inactive` | registry row expired or replaced, found by the heartbeat | 4000 `heartbeat_inactive` |
| `session_invalid` | session expired or compromised | 4000 `session_invalid` |
| `slow_consumer` | egress channel full | none (notice does not fit), plain close after the buffer |
| `dead_channel` | egress channel closed without a disconnect | – (socket already gone) |
| `shutdown` | hub shutdown | 1001 `shutdown` |
| *(no reason)* | client disconnected | plain close; not counted as a drop |

## Failure behaviour (Redis)

| Situation | Behaviour |
|---|---|
| Connect, registry write fails and did not land | new socket rejected (closed, no events), old connection kept |
| Connect, write landed but reply lost | detected by read-back; connect proceeds normally |
| Disconnect / drop, row removal fails | local removal and events still happen; row expires by TTL |
| Heartbeat or session check fails | nothing dropped; retried next tick |
| Registry listener disconnected | notifications lost meanwhile; the heartbeat reconciles |
| A hub task or the command loop dies unexpectedly (a bug) | the process stops after logging, and the orchestrator restarts it |
| Redis, Postgres or network error | never stops the process: operations are retried with backoff and the hub recovers when the dependency returns |
| Redis down entirely | live sockets stay; new connects are rejected; streams stop advancing until Redis returns |
| Postgres down | live sockets stay and the streams go on from Redis (ingest, delivery); persistence retries, and ingest answers `busy` once `redisMaxUnpersisted` commands wait; the snapshot route answers `503` |
| Redis loses data or goes back in time | every registry row is gone, so every connection is dropped at the next heartbeat; a stream's Redis copy is detected as stale, dropped and rebuilt from Postgres, and its subscribers get `streamReset` then a new `streamSync` |

Redis must run with `maxmemory-policy noeviction`: eviction would silently drop registry rows and stream state.

## Configuration

`builder` section of the service config unless noted.

| Key | Default | Effect |
|---|---|---|
| `hubHeartbeatSeconds` | 300 | heartbeat interval; registry TTL = 3 × this |
| `authCheckInterval` | 60 | session check interval (s); also the longest a revoked session keeps its socket |
| `ws.allowedHosts` | none | regular expressions for the accepted target host of a WebSocket upgrade (`X-Forwarded-Host`, `Forwarded`, then `Host`) |
| `service.allowedOrigins` (service section) | none | regular expressions for the accepted `Origin` of a WebSocket upgrade; the cross-site WebSocket hijacking defence |
| `commandStream.enabled` | false (true in the shipped configs) | switches the command streams on; needs `db.sqlCns` |
| `commandStream.snapshotEvery`, `keepSnapshots` | 100, 3 | defaults for every stream kind |
| `commandStream.kinds.<kind>.snapshotEvery`, `keepSnapshots` | chat: 50, 5 | per-kind override; Redis retention follows from the two |
| `commandStream.kinds.<kind>.keepHistory` | false | never prune the stream of this kind from Postgres (the Redis window is unaffected) |
| `commandStream.syncIntervalMs` | 1000 | periodic tick of the stream workers and the dispatcher |
| `commandStream.persistDelayMs` | 20 | delay before new commands are persisted (plus the same again as jitter) |
| `commandStream.unloadGraceSeconds` | 60 | how long a stream stays loaded without local subscribers |
| `commandStream.orphanSweepSeconds`, `orphanIdleSeconds` | 30, 10 | interval of the orphan sweep, idle time before a stream is swept |
| `commandStream.redisMaxUnpersisted` | 5000 | unpersisted commands per stream before writes get `busy` |
| `commandStream.redisTtlSeconds` | 86400 | inactivity TTL of a stream's Redis keys |
| `commandStream.maxCommandBytes` | 16384 | longest command JSON accepted |
| `db.sqlCns` | none | the builder's own Postgres database (command log, snapshots); the command-log schema is applied at startup |
| Redis connection string `timeout` | redis-rs: 1 s connect, 500 ms response | milliseconds, connect and per-command response timeout |
| Redis connection string `pool_timeout` | 30000 | milliseconds to get a pooled connection |
| Redis connection string `max_size` | 10 | pooled connections; the registry and the stream logs share the pool |

Constants: ws egress capacity 1000, client-close drain 5 s (`connect.rs`), session check batch 256 (`session_checker.rs`). Stream settings (snapshot cadence, Redis retention, persistence cadence) belong to the command-stream configuration.

## Observability

Status (`/info/status`, provider `hub`): `connections`, `subscribers`, and the completed ticks `heartbeatTicks`, `sessionCheckTicks` (a counter that stops is a stuck loop). Drops by reason are the `hub_connections_dropped` metric and the `reason` of `UserDisconnected`.

| Metric | Labels |
|---|---|
| `hub_connections` (up/down) | – |
| `hub_connections_dropped` | `reason` |
| `hub_commands`, `hub_command_duration`, `hub_command_queue_delay` | `command` |
| `hub_messages_sent` | `kind` (hub/stream/stream_event), `outcome` (ok/full/closed) |
| `hub_tick_duration` | `consumer` |
| `ws_messages_received` | `kind`, `outcome` |
| `command_stream_ingested` | `outcome` (accepted/duplicate/invalid/notReady/busy/error) |
| `command_stream_snapshot_reads` | `outcome` (ok/not_found/forbidden/error) |
| `command_stream_drift`, `command_stream_decode_errors` | – |

Labels are fixed small sets, never user or connection ids. Each command runs in a root `hub command` span linked to the request that queued it; ws open / close / message spans are linked the same way, so a socket does not keep its upgrade request open. Drops log at info: `[user] [connection] Connection dropped: <reason>`.

## Testing

| File | Needs | Covers |
|---|---|---|
| `hub_connections.rs`, `hub_subscribers.rs`, `tasks/*` unit tests | – | send outcomes, topic filter, CAS removal, drop event, tracker starvation |
| `hub/tests/test_utils.rs` | – | harness: FIFO `barrier`, `wait_ticks`, `drops` (drops by reason from the events), `replay` invariant checker, egress inspection, seeded `Rng` |
| `hub/tests/lifecycle.rs` | Redis | every lifecycle edge case with exact events / egress / drops / registry |
| `hub/tests/contention.rs` | Redis | parallel users, same-user races, churn under traffic, two instances |
| `hub/tests/fault.rs` | Redis + toxiproxy | lost registry reply, failed write, outages |
| `hub/tests/stress.rs` | Redis + Postgres | 300 connections with every consumer ticking, writers and reconnects, then shutdown |
| `hub/tests/stream.rs`, `stream_dispatcher.rs` | Redis (+ Postgres) | subscription events, worker residency, restarts, slow subscriber, subscribe churn |
| `command_stream` tests (`stream_core`, service, worker, integration, chat) | Redis + Postgres | core state machine, ordering, dedupe, resume, Redis loss and rollback, snapshot disagreement, two instances, Postgres outage, chat semantics |
| `routes/ws/connect_test.rs`, `routes/command_stream_test.rs` | Redis + Postgres | close frames, the stream frames over a real hub, resume, snapshot route statuses |
| `app_state_test.rs` | – | configuration to heartbeat, session check and registry TTL |
| `tests/api-tests/builder/` | running service | gating, close codes, the chat stream over a real socket and the snapshot route |

```bash
export CI=true
export SHINE_TEST_REDIS_CNS="redis://localhost:6379"
export SHINE_TEST_PG_CNS="postgres://username:password@localhost:5432/database-test?sslmode=disable"
export SHINE_TEST_TOXIPROXY_URL="http://localhost:8474"   # fault tests skip without it
cargo test -p shine-builder
```

With `CI` set, a missing database connection string fails the test instead of skipping it. Tests that cut a database off through toxiproxy wait for the other tests that use a service, so an outage cannot hit them.

## File map

| File | Role |
|---|---|
| `services/hub/hub_service.rs` | Command loop, sole writer; connect / disconnect / drop / registry-change handling; event publish; shutdown. |
| `services/hub/hub_command.rs` | `ControlCommand` enum, the loop's input vocabulary. |
| `services/hub/hub_connections.rs` | Addressable connection registry; bounded egress; dead-channel detection. |
| `services/hub/hub_subscribers.rs` | Internal lossless fan-out subscribers. |
| `services/hub/hub_status.rs`, `hub_metrics.rs` | Health provider and metrics. |
| `services/hub/tasks/connection_tracker.rs` | Event-sourced per-consumer tracker and the periodic loop driver. |
| `services/hub/tasks/heartbeat.rs` | Batched registry TTL refresh; drops connections the registry no longer holds. |
| `services/hub/tasks/session_checker.rs` | Batched session validation; drops on expiry. |
| `services/hub/tasks/stream_dispatcher.rs` | Routes subscriptions to stream workers and supervises them. |
| `services/command_stream/` | `CommandStreamService`, stream kinds, `StreamWorker`, the pure replay core and the egress port. |
| `services/hub/stream_egress.rs` | The hub's implementation of the stream egress port. |
| `routes/ws/connect.rs`, `routes/ws/message.rs` | Socket lifecycle, close codes, conversion of stream messages to wire frames. |
| `routes/ws/stream_socket.rs` | The stream side of a socket: opens streams, checks and ingests commands, answers its own failures through a weak sender. |
| `routes/command_stream.rs` | HTTP snapshot read of a stream. |
| `models/messages/hub.rs`, `models/messages/stream.rs` | `HubEvent`, `HubMessage`, `TopicKey`, stream frames and events. |
| `repositories/hub_registry/` | `HubRegistry` trait and Redis implementation: SET / EXPIRE / DEL, CAS, pub/sub, registry-change listener. |
| `repositories/command_stream/redis/` | Redis stream log, meta, dedupe ids and the atomic ingest script. |
| `crates/shine-infra/src/db/command_log/` | Postgres command log and snapshots. |
| `crates/shine-game/src/replay/` | Pure replay core, chat state and the wire frames (`protocol.rs`), shared by server and client. |
| `services/command_stream/chat.rs` | The chat stream id and its access rule. |
| `app_state.rs`, `settings/command_stream.rs` | Wiring of the services and the hub cadences; validated stream settings. |

<details data-agent="implementation">
<summary>The registry write precedes the local teardown in ConnectUser</summary>

In `process(ConnectUser)` the order is: (1) `create_registry_connection`, (2) supersede the old local connection, (3) register the new one. If the registry write fails the loop returns before touching local state, so the previously active connection stays tracked. Reversing the order would let a transient Redis error leave the user with no connection and an orphaned, untracked new socket. The registry removal on the disconnect paths is best effort by contrast: a failure is logged and left to TTL, because the local removal and `UserDisconnected` must still happen so sockets and trackers never wedge.

</details>

<details data-agent="implementation">
<summary>Pub/sub is lossy by design; the heartbeat is the reconciler</summary>

The registry-change listener treats a `None` payload (Redis reconnect) as "notices during the outage were lost" and does nothing. Correctness does not depend on any single notification: the periodic heartbeat independently detects a connection the registry no longer holds. Pub/sub only shortens the window between a cross-instance takeover and the local teardown. Stream wakes follow the same rule: the periodic tick is the backstop.

</details>

<details data-agent="implementation">
<summary>The test barrier and tick counters replace sleeps</summary>

Commands are FIFO, so a sentinel connect + disconnect whose events arrived proves every earlier command was processed; assertions after it are exact. To assert that a periodic consumer did not act, wait for its tick counter (`wait_ticks`, backed by `HubService::ticks`). Hub tests share the `serial = "hub-service"` group.

</details>

<details data-agent="implementation">
<summary>Working on the hub</summary>

- **Changing connection state:** add or extend a `ControlCommand` and handle it in `HubService::process`. Never mutate `Connections` from a consumer, route or test helper.
- **Removing a connection:** use the `remove_*` methods (CAS), then `record_drop(removed, reason)` for a hub-initiated removal (counts, logs, notifies the socket), then remove the registry row and publish `UserDisconnected`. Drop reasons are the `DisconnectReason` variants (metric labels via `as_str()`); adding a variant makes the compiler point at the close-code mapping in `connect.rs`.
- **New background consumer:** implement `ConnectionConsumer` and start it in `HubService::new` before the dispatchers, via `spawn_connection_loop`, which subscribes before the loop runs.
- **New egress message:** add a `HubMessage` variant; the socket forwards only what `event_to_wire_message` maps, and `try_send` labels its `kind`.
- **New stream kind:** register it in `StreamKindRegistry` (validator, state type, snapshot encoding); the hub and the protocol stay unchanged.
- **Sending to clients** happens off the loop (`send_to_connection`, `send_to_user`, `broadcast`); a dead target is reported, not removed, by the sender.
- Keep `HubEvent` for broadcast lifecycle facts; addressed messages belong on `HubMessage`.
- Per-tick logs at debug or below; user and connection ids are fine in logs, never as metric labels.

</details>
