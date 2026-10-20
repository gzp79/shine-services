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
    ing -- "open: create, hydrate" --> pg
    cmdloop --> conns
    cmdloop -- "HubEvent, StreamEvent" --> subs
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
| `Subscribers` | Unbounded, lossless channels of the internal consumers; each receives the events of the topics it subscribed to (heartbeat and session checker: `Hub`; stream dispatcher: `Hub` and `Stream`). |
| `ConnectionTracker` | Shared loop of the heartbeat and the session checker: applies `HubEvent`s to a private view and runs `on_tick` periodically. The stream dispatcher runs its own loop. |
| `Heartbeat` | Refreshes the registry TTL of every local connection; drops those the registry no longer holds. |
| `SessionChecker` | Re-checks in batches that each connection's session still exists; drops the connections whose session is gone. |
| `StreamDispatcher` | Routes subscriptions to the `StreamWorker` of each resident stream, unloads a stream `unloadGraceSeconds` after its last subscriber, restarts a worker that ended unexpectedly (at most once a second per stream) and runs the orphan sweep. |
| `CommandStreamService` | Opens streams (kind, access, creation, Redis hydration), ingests commands (validation, dedupe, version assignment), builds snapshot reads, persists orphaned streams. |
| `RedisHubConnectionDb` | Registry rows `hub-connection:<user>` (the user id without hyphens) → active connection id, with TTL; publishes `hub-registry-changed` with the payload `<instance id>:<user id>`, the instance id being fresh per process. A removal publishes only when its CAS delete removed the row. |
| ws socket | Registers the socket, forwards egress to the client, passes client frames on, ends the connection when either side closes. |

| Type | Direction | Variants |
|---|---|---|
| `ControlCommand` | anyone → command loop | `ConnectUser`, `DisconnectUser { reason }` (`Client` for a client close), `DropConnection { reason }`, `HubRegistryChanged`, `SubscribeStream`, `UnsubscribeStream`, `Shutdown` |
| `HubEvent` | loop → all subscribers (and Hub-topic connections) | `UserConnected`, `UserDisconnected`, `Shutdown` |
| `StreamEvent` | loop → internal subscribers only (`TopicKey::Stream`) | a connection subscribed to or left a stream |
| `HubMessage` | hub → one connection's egress, or the internal subscribers | `Hub(HubEvent)`, `Stream(StreamMessage)` (egress), `StreamEvent(StreamEvent)` (internal only) |

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
    WS->>H: request_disconnection → DisconnectUser(user, id, Client)
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
    H->>H: remove every connection at once
    loop every removed connection
        H->>H: count drop "shutdown"
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
    Note over W: sends nothing more for this stream to this connection,<br/>and a stream without subscribers is unloaded
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
| Resume | `streamSubscribe` with `resume {generation, version}` continues from that version when the stream's generation is the same and the version lies between the version of the worker's latest snapshot and the worker's head (the commands in between are what the worker holds); the first `streamCommands` then starts at `version + 1`; a client already at the head gets an empty list, so a resume is always answered. Otherwise (another generation, below the snapshot, above the head) the worker answers with a normal `streamSync` and the client replaces its state. A worker that has just loaded (nothing was resident on this instance) answers every subscriber, resuming ones included, with a full `streamSync`; a resume is a tail only while the worker is loaded, that is while another subscriber follows the stream or within `unloadGraceSeconds` of the last one. |
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
| Redis wake (one pub/sub channel `{prefix}:wake` for all streams; the payload is the stream id) | Primary: prompt delivery after an append. The dispatcher routes a wake to the stream's resident worker; wakes coalesce, so a burst is one pass. |
| Persist timer (`persistDelayMs`, plus the same again as jitter, armed when commands are unpersisted) | Appends the applied commands to Postgres, records the persisted version in Redis, stores the checkpoint snapshot and pushes `streamCheckpoint`. |
| Periodic tick (`syncIntervalMs`) | Safety net: runs delivery and persistence, so it covers wakes missed while a listener was disconnected and retries after errors. |

A wake only says "something changed in this stream", which is what makes coalescing safe; after a listener reconnect every resident stream is woken, because wakes may have been lost meanwhile. Every instance receives every wake and drops those for streams it does not hold.

## Stream internals

How a stream is stored, ordered and replayed; what a client sees is in *Command streams*. A stream is split so that every rule can be tested without a database: the pure replay core is shared with the web client, one pure state machine decides what a worker does, and thin I/O shells run it.

```mermaid
flowchart LR
    classDef indigo fill:#6366f122,stroke:#6366f1
    classDef blue fill:#0ea5e922,stroke:#0ea5e9
    classDef green fill:#22c55e22,stroke:#22c55e
    classDef orange fill:#fb923c22,stroke:#fb923c
    classDef violet fill:#a855f722,stroke:#a855f7

    sess["StreamSession<br/>ws socket"]:::indigo
    route["snapshot route"]:::indigo
    svc["CommandStreamService<br/>open, ingest, hydrate, sweep"]:::blue
    disp["StreamDispatcher"]:::orange
    worker["StreamWorker<br/>StreamCore + replica"]:::green
    hub["HubService<br/>StreamEgress"]:::blue
    redis[("Redis<br/>log, meta, ids, wake")]:::violet
    pg[("Postgres<br/>streams, commands, snapshots")]:::violet

    sess -- "open, ingest" --> svc
    route -- read_snapshot --> svc
    svc --> redis & pg
    redis -- wake --> disp
    disp -- "wake, subscribers" --> worker
    worker <--> redis
    worker -- "append, snapshots, prune" --> pg
    worker -- frames --> hub
```

| Layer | Role |
|---|---|
| `shine-game::replay` | Pure and wasm-compatible, shared with the client: `ReplayState`, `Replica`, `StableHash`, the codecs, the wire `protocol`, `ClientStream`. Knows no storage. |
| `command_log` (`shine-infra`) | `PgCommandLogDb`: streams, immutable commands, snapshots (opaque bytes), pruning. |
| `repositories/command_stream/redis` | `RedisCommandStreamDb`: the Lua scripts (`INGEST`, `HYDRATE`, `MARK_PERSISTED`, `RESET`, `UNLOCK`, `IDLE_STREAMS`, `RETIRE_ACTIVE`, `REQUEUE_ACTIVE`), the hydration lock, the active set and the wake channel. |
| `StreamCore` | Pure per-stream state machine, no I/O: replica, shared delivery cursor, subscribers, unpersisted commands, checkpoints, faults. |
| `StreamWorker` | The I/O shell around a core; the only code that reads Redis for delivery and persists. |
| `CommandStreamService` | Open, ingest, hydrate, snapshot read, persist an orphaned tail, orphan sweep. |
| `StreamDispatcher`, `StreamSession` | Worker residency (hub side) and the stream side of a socket. |

| Concern | Rule |
|---|---|
| State contract | `ReplayState::apply(&mut self, &CommandContext, &Command)` is infallible and deterministic, and is called only for the next version (`Replica` never passes a gap or a repeat). Whether a command takes effect is the state's private decision, for example an author check: a command it refuses is stored, ignored on replay, and still consumes its version. Ingest checks the shape (`validate`), the size and the `commandId`, never another command's author. |
| Command text | Stored as canonical `TEXT` (decoded as the kind's typed command, validated, re-serialized), never `jsonb`. A command that does not decode is never stored. |
| Epoch | `generation` is the history epoch, a fencing token owned by the log: `begin_generation` bumps it whenever Redis is rebuilt from Postgres, appends carry it, and a writer of an older epoch is refused. |
| Snapshots | Postcard of the state's serde form (the codec is in `shine-game`, the client decodes the same bytes), first writer wins (foreign key to the command row), hash stored as `i64` by bit cast. The cadence is a builder setting (`KindSettings::is_snapshot_version`). |
| Persistence | A worker appends its applied commands (id-checked: rows already stored are a no-op, a differing row is `Diverged`), then records the persisted version in Redis by content (version and command id). Every instance with the stream loaded persists the same commands; a jittered delay spreads the appends. |
| Redis window | Trimmed only below the oldest kept snapshot and never above what is persisted, so unpersisted commands always survive; `redisMaxUnpersisted` bounds them (ingest answers `busy`). |
| Stream id | `StreamId`: `^[A-Za-z0-9_.:-]{1,64}$`; Postgres `VARCHAR(64)` (the kind too), Redis key part, wire id. |
| Access | `StreamAccess` port (`can_read`, `can_write` per user and stream); `chat_access()` is an allow-list with `chat-global` only. |
| Own database | `builder.db.sqlCns`; the pool is created only when `commandStream.enabled`, and the command-log schema is applied at startup (`migrate("command_log", ...)`). Production needs the connection string in the key vault before enabling. |
| Logging | Lines carry `[{stream_id}]` or `[{user_id}]`. `trace`: command payloads; `debug`: per step and message; `info`: root-cause context; `warn`: infrastructure errors that heal by retrying; `error`: bugs and unexpected state, such as snapshot drift. |

<details data-agent="implementation">
<summary>Redis keys, scripts and limits</summary>

Keys live under a prefix (`cs` in production, `cs-test-<uuid>` in every test). Per stream: `{p}:{id}:meta` (hash `generation`, `version`, `persisted`), `{p}:{id}:log` (entry id `<version>-0`), `{p}:{id}:ids` (the command-id index) and `{p}:{id}:hydrating` (the hydration lock, 5 s). Global: `{p}:active` (the sorted set the sweep reads, scored by last write in ms) and `{p}:wake` (the wake channel). Every script checks first and writes last.

- `INGEST` dedupes by `commandId` (`duplicate` at the same version), answers `busy` above `redisMaxUnpersisted` and `NotReady` without a meta, then assigns the next version, appends and publishes the wake. It refreshes the TTL of the three stream keys and the active-set score at most once a second.
- `HYDRATE` refuses a same or newer generation and takes the stream off the active set. It deletes the old log before it writes the new one, so the Rust side validates the tail first (contiguous, ending at the head) and fails before any write.
- `MARK_PERSISTED` records a version only when the copy holds that command id there (`Refused` otherwise: the copy went back in time) and trims at `min(persisted + 1, keep_from)`, `keep_from` being the `keepSnapshots`-th newest snapshot (1 without one).
- `RESET` deletes only when the stream is still at the given generation, so a stale reset is a no-op. `UNLOCK` releases the hydration lock only for its own token. `IDLE_STREAMS` lists the streams idle for `orphanIdleSeconds`; `RETIRE_ACTIVE` unlists a stream only when nothing is unpersisted, and `REQUEUE_ACTIVE` puts back one the sweep could not retire.
- TTL and `redisMaxUnpersisted` are clamped to at least 1: `EXPIRE 0` deletes a key.
- Limits: the dedupe window is the commands still in Redis; multi-key scripts need a single-primary Redis; the scripts call `TIME`, which needs effect replication (always so on Redis 6.2+); `maxmemory-policy noeviction` is required.

</details>

<details data-agent="implementation">
<summary>StreamCore: what a worker does about each disagreement</summary>

- Faults: `Reload` (rebuild from Postgres), `Reset { generation }` (drop the Redis copy if it is still at that generation, then rebuild) and `Undecodable { version }` (the worker stops; rebuilding would meet the same command).
- Every Redis read starts at the replica's own version and checks that Redis still holds the command applied there. Another command id means Redis reassigned the version (restart from a dump, failover); a missing one that was not persisted means Redis lost it; an older generation or a lower version means it went back in time. All are `Reset`. Counting versions alone cannot see a reassignment that already caught up.
- A newer generation is adopted only when everything applied is proven persisted: the unpersisted commands are appended and must come back `UpToDate` (`NeedsProof`), otherwise `Reload`.
- Holes (trimmed from Redis, so persisted) are filled from Postgres without a false divergence. When they are missing there too (pruned), the worker fell behind the retained history and reloads from the newest snapshot; the Redis copy is fine.
- Frames of one step are shared by the subscribers. A send that fails removes the subscriber at once (a later frame would leave a gap). `Reset` is followed by exactly one `Sync`. A load turns every pending resume into a full `Sync`, and a resume is re-checked at delivery because a snapshot taken meanwhile can trim the commands it needs.
- A checkpoint is pushed after the commands it covers were delivered, at most one per step, always the latest.

</details>

<details data-agent="implementation">
<summary>StreamWorker, CommandStreamService and the sweep</summary>

- A step delivers (loads when needed, reads Redis in batches of 256 up to 1024 per step, applies, sends) and, on the persist timer and the periodic tick, persists (checks the copy, appends, records the mark, resolves the checkpoint, pushes it). Wake-driven steps are spaced at least 2 ms apart.
- A load runs `ensure_hydrated`, decodes the newest snapshot (an unreadable one means replaying from the start), applies the Postgres log, then reads the Redis meta. A failed load is retried with backoff from 200 ms to 10 s, and the backoff resets only after a full step, so a stream that reloads right after every load keeps backing off.
- Only the newest persisted checkpoint is resolved: a worker that is behind stores one snapshot, not each intermediate one. The store races between instances; a stored hash that differs is state drift (error log, `command_stream_drift`) and the worker rebases onto the stored snapshot. The worker that stored a snapshot prunes (unless `keepHistory`); a failure only logs.
- A stored command that does not decode ends the worker with `Unavailable`; the dispatcher answers its subscribers `streamSubscribeFailed{unavailable}` and refuses new ones until the stream is unloaded.
- Ingest order: size check, decode as the kind's command, `validate`, canonicalize, Redis `INGEST`. `NotReady` starts one background hydration per stream.
- `ensure_hydrated` runs under the Redis hydration lock and re-checks after taking it, so concurrent callers on any instance build the copy once: it drops a stale copy, bumps the generation, and copies the tail from the snapshot horizon (the last `snapshotEvery × keepSnapshots + snapshotEvery + 1` commands, cut at the horizon). A caller that finds the lock taken polls every 20 ms and gives up after 10 s with `NotReady` (`streamSubscribeFailed{failed}`, or `503` from the snapshot route).
- A snapshot store answers `Superseded` when a newer snapshot already exists; that checkpoint is skipped (not pushed, no rebase).
- The sweep handles up to 100 idle streams per run: it persists each tail and unlists the stream, and requeues the ones it could not retire. A Redis copy that disagrees with Postgres (another generation, a diverged or gapped append, a refused mark) is dropped: Postgres wins.

</details>

<details data-agent="implementation">
<summary>Retention and keeping a stream whole</summary>

Postgres prunes in one statement: the `keepSnapshots`-th newest snapshot is the horizon, older snapshots and the commands below it go, so Redis and Postgres keep the same window. The horizon always comes from the kept snapshots, never from the caller (`PgCommandLogDb::prune`). Pruned commands cannot be recovered, and a worker cannot fall back to replaying from the start when its newest snapshot does not decode.

A kind with `keepHistory` is never pruned, which is the way to keep a stream whole.

</details>

<details data-agent="test">
<summary>What the tests pin, and test conventions</summary>

| Risk | Test |
|---|---|
| A retried `commandId` | `ingest_dedupes_command_id`; the retry gets `duplicate` |
| Redis lost or back in time | `redis_loss_continues_from_the_postgres_head`, `redis_rolled_back_within_a_generation_is_repaired` |
| Slow client | `a_failed_send_stops_all_later_frames`, `a_slow_subscriber_never_sees_a_version_gap` |
| Subscribe churn, unregistered connection | `subscriptions_are_published_for_registered_connections_only`, `subscribe_unsubscribe_churn_leaves_no_subscription` |
| Resume at the edges | the four `resume_*` socket tests: tail, other generation, above the head, below the base |
| Pruning | `the_worker_prunes_after_storing_a_snapshot`, `a_worker_behind_the_pruned_history_starts_over_from_the_newest_snapshot`, `a_kind_that_keeps_its_history_is_never_pruned` |
| Scale | `heartbeat_enabled_stress_with_300_connections` |

- Database tests skip without `SHINE_TEST_REDIS_CNS` / `SHINE_TEST_PG_CNS` and fail with `CI` set; connection strings come from `shine_test::db::{get_redis_cns, get_pg_cns}`. Every test uses its own Redis prefix. Builder test files are declared at the bottom of the owning file with `#[path]`.
- `command_stream/test_utils.rs`: `setup_with` (a service, shared lock), `Test::instance` (a second process on the same stores), `Recorder` (an egress that records), `start_worker`, `eventually`, `setup_for_outage` (takes the lock exclusively, so a toxiproxy outage cannot hit other tests).
- A panic in a background thread aborts the whole test run, so a crashing worker is simulated by a worker that ends.
- A snapshot is stored when its version is persisted, and a worker that is behind skips intermediate ones: tests that need each snapshot write in rounds of `snapshotEvery` and wait for every snapshot.
- `*.sql` files are checked out with CRLF (`.gitattributes`), and the line endings are part of the migration checksum. A database migrated from a copy with other line endings reports "applied migration is different": drop the `cl_*` tables and `__migration__command_log` of the dev database.
- On Windows use PowerShell for environment variables, and run shell commands one at a time (parallel Git Bash starts can crash).

</details>

<details data-agent="implementation">
<summary>Open questions</summary>

- Decode errors: a stored command that does not decode stops the worker and the stream answers `unavailable`. Skipping the command would break the replay, which needs every version.
- `generation` is the history epoch; renaming it to `epoch` would match its role.

</details>

## Client engine

*`ClientStream<S>` (`shine-game::replay`) is the whole stream logic of a client, as a pure state machine.* The caller owns the socket, the timers and the randomness: it feeds the socket events and the server frames, sends the frames it gets back, and supplies the `commandId`s (wasm has no random source). The server's `StreamCore` is not reused: it is persistence, Redis checks and fan-out.

```mermaid
stateDiagram-v2
    [*] --> Connecting
    Connecting --> Syncing: socket open, subscribe sent
    Syncing --> Live: streamSync, or a resume answer that continues
    Syncing --> Resyncing: reset, or a resume answer with a gap or another generation
    Live --> Resyncing: reset, gap, other generation, checkpoint mismatch, notSubscribed
    Resyncing --> Live: streamSync
    Syncing --> Failed: subscribe refused, bad sync
    Resyncing --> Failed: subscribe refused, bad sync
    Live --> Failed: undecodable command
    Syncing --> Offline: socket closed, no answer in time
    Resyncing --> Offline: socket closed, no answer in time
    Live --> Offline: socket closed
    Failed --> Offline: socket closed
    Offline --> Syncing: reconnect, resume
```

| Concern | Rule |
|---|---|
| View | `local` is the authoritative replica with the pending writes applied on top, rebuilt after every frame; a pending write is applied with a provisional version, so a state must not depend on it. |
| Submit | The caller passes the `commandId`. The frame is returned only while `Live`; otherwise the write goes out after the next sync. |
| Sync | Replaces the state: the snapshot is decoded and checked against its hash, then the commands are applied. A sync that does not decode, does not match its hash or is not contiguous fails the stream. Pending writes whose id is in the sync are confirmed; the others are sent again with the same ids, which the server's dedupe makes safe. |
| Commands | Must be contiguous and of the same generation, else the client asks for a fresh sync. While `Resyncing` they are ignored (the sync supersedes them). |
| Resume | After a reconnect the subscribe carries `{generation, version}`. A sync, or a tail (possibly empty) that continues from the position, makes the stream `Live`; a tail with a gap or of another generation asks for a fresh sync. |
| Nack | `duplicate`, `invalid`, `forbidden` drop the pending write; `failed`, `busy`, `notReady` keep it (the caller calls `retry_pending` after a delay); `notSubscribed` subscribes again. |
| Checkpoint | Compared with the hash the client recorded at that version (the last eight multiples of `snapshotEvery`, so a checkpoint arriving after later commands still compares); a mismatch asks for a fresh sync, an unknown version is ignored. |
| Timeouts | The caller times the answer to a subscribe (about 10 s) and calls `on_sync_timeout`, then reconnects. A server that cannot finish loading is otherwise silent. |

## Client contract

- **One live connection per user, held by the active tab.** A tab opens its socket when it becomes the active one and closes it when it loses focus.
- **After a drop** the client reconnects with jittered backoff if its tab is active, and subscribes again to every stream it follows, resuming from its position; the answer is the tail after that position or a `streamSync` that replaces its state.
- **A write is retried** with the same `commandId` until it appears in `streamCommands`.
- **The engine is shared**: a Rust client uses `ClientStream` (see *Client engine*); other clients follow the same rules.

## Drop reasons and close codes

| Reason | Trigger | Close frame |
|---|---|---|
| `replaced` | same user connected again on this instance | 4000 `replaced` |
| `registry_inactive` | another instance took the user (registry change) | 4000 `registry_inactive` |
| `heartbeat_inactive` | registry row expired or replaced, found by the heartbeat | 4000 `heartbeat_inactive` |
| `session_invalid` | the session no longer exists (expired, logged out, revoked) | 4000 `session_invalid` |
| `slow_consumer` | egress channel full | none (notice does not fit), plain close after the buffer |
| `dead_channel` | egress channel closed without a disconnect | – (socket already gone) |
| `shutdown` | hub shutdown | 1001 `shutdown` |
| `client` | client disconnected | plain close; not counted as a drop |

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
| `ws.allowedHosts` | none (the `ws` section is required) | regular expressions for the accepted target host of a WebSocket upgrade (`X-Forwarded-Host`, `Forwarded`, then `Host`); an empty list refuses every upgrade |
| `service.allowedOrigins` (service section) | none | regular expressions for the accepted `Origin` of a WebSocket upgrade; the cross-site WebSocket hijacking defence; an empty list refuses every upgrade |
| `commandStream.enabled` | false (true in the shipped configs) | switches the command streams on; needs `db.sqlCns` |
| `commandStream.snapshotEvery`, `keepSnapshots` | 100, 3 | defaults for every stream kind |
| `commandStream.kinds.<kind>.snapshotEvery`, `keepSnapshots` | none (the prod config sets chat: 50, 5; the test config 5, 2) | per-kind override; Redis retention follows from the two |
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

Constants: ws egress capacity 1000, client-close drain 5 s (`connect.rs`), session check batch 256 (`session_checker.rs`). Clamps at startup: `hubHeartbeatSeconds` and `authCheckInterval` at least 1 (the TTL is a saturating 3 ×), `redisTtlSeconds` at least 60, `syncIntervalMs` at least 10, `persistDelayMs`, `orphanSweepSeconds`, `maxCommandBytes` and `redisMaxUnpersisted` at least 1; a `snapshotEvery` or `keepSnapshots` below 1 fails the start. Stream settings (snapshot cadence, Redis retention, persistence cadence) belong to the command-stream configuration.

## Observability

Status (`/info/status`, provider `hub`): `connections`, `subscribers`, and the completed ticks `heartbeatTicks`, `sessionCheckTicks` (a counter that stops is a stuck loop). With command streams enabled, a `PostgresPoolStatus` provider reports the builder's Postgres pool. Drops by reason are the `hub_connections_dropped` metric and the `reason` of `UserDisconnected`.

| Metric | Labels |
|---|---|
| `hub_connections` (up/down) | – |
| `hub_connections_dropped` | `reason` |
| `hub_commands`, `hub_command_duration`, `hub_command_queue_delay` | `command` |
| `hub_messages_sent` | `kind` (hub/stream/stream_event), `outcome` (ok/full/closed) |
| `hub_tick_duration` | `consumer` |
| `ws_messages_received` | `kind`, `outcome` |
| `command_stream_ingested` | `outcome` (accepted/duplicate/invalid/notReady/busy/error) |
| `command_stream_snapshot_reads` | `outcome` (ok/not_found/forbidden/error); an invalid id or disabled streams are not counted |
| `command_stream_drift`, `command_stream_decode_errors` | – |

Labels are fixed small sets, never user or connection ids. Each command runs in a root `hub command` span linked to the request that queued it. `ws open` is a root span linked to the upgrade request, so a socket does not keep its upgrade request open; `ws message` and `ws close` are root spans without a link. Drops log at info: `[user] [connection] Connection dropped: <reason>`.

## Testing

| File | Needs | Covers |
|---|---|---|
| `hub_connections_test.rs`, `hub_subscribers_test.rs`, `tasks/connection_tracker_test.rs` | – | send outcomes, topic filter, CAS removal, drop event, tracker starvation |
| `hub/tests/test_utils.rs` | – | harness: FIFO `barrier`, `wait_ticks`, `drops` (drops by reason from the events), `replay` invariant checker, egress inspection, seeded `Rng` |
| `hub/tests/lifecycle.rs` | Redis | every lifecycle edge case with exact events / egress / drops / registry |
| `hub/tests/contention.rs` | Redis | parallel users, same-user races, churn under traffic, two instances |
| `hub/tests/fault.rs` | Redis + toxiproxy | lost registry reply, failed write, outages |
| `hub/tests/stress.rs` | Redis + Postgres | 300 connections with every consumer ticking, writers and reconnects, then shutdown |
| `hub/tests/stream.rs`, `stream_dispatcher.rs` | Redis (+ Postgres) | subscription events, worker residency, restarts, slow subscriber, subscribe churn |
| `command_stream` tests (`stream_core`, service, worker, integration, chat) | Redis + Postgres | core state machine, ordering, dedupe, resume, Redis loss and rollback, snapshot disagreement, two instances, Postgres outage, chat semantics |
| `repositories/command_stream/redis/redis_command_stream_test.rs` | Redis | the Lua scripts: ingest, dedupe, hydrate, mark persisted, trim, reset, the active set |
| `crates/shine-infra/tests/pg_command_log.rs` | Postgres | the command log: generations, appends, snapshots, pruning |
| `routes/ws/connect_test.rs`, `routes/command_stream_test.rs` | Redis + Postgres | close frames, the stream frames over a real hub, resume, snapshot route statuses |
| `app_state_test.rs`, `settings/command_stream_test.rs` | – | configuration to heartbeat, session check and registry TTL; validated stream settings |
| `repositories/db_test.rs` | Postgres | the builder's pool and migration |
| `crates/shine-game/tests/replay/` | – | replay core, hashing, snapshot codec, wire frames, the client engine |
| `tests/api-tests/builder/` | running service | gating, close codes, the chat stream over a real socket and the snapshot route |

```bash
export CI=true
export SHINE_TEST_REDIS_CNS="redis://localhost:6379"
export SHINE_TEST_PG_CNS="postgres://username:password@localhost:5432/database-test?sslmode=disable"
export SHINE_TEST_TOXIPROXY_URL="http://localhost:8474"   # fault tests skip without it
cargo test -p shine-builder
cargo test -p shine-infra --test pg_command_log
cargo test -p shine-game --test replay
cd tests && pnpm test:local   # API tests, with the service running
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
| `services/command_stream/` | `CommandStreamService`, `StreamWorker`, `StreamCore`, the egress port and the metrics. |
| `services/command_stream/stream_kind.rs`, `stream_access.rs` | `StreamKindRegistry` and the `StreamAccess` port. |
| `services/hub/stream_egress.rs` | The hub's implementation of the stream egress port. |
| `routes/ws/router.rs` | The `/api/connect` route. |
| `routes/ws/connect.rs`, `routes/ws/message.rs` | Socket lifecycle, close codes, conversion of stream messages to wire frames. |
| `routes/ws/stream_socket.rs` | The stream side of a socket: opens streams, checks and ingests commands, answers its own failures through a weak sender. |
| `routes/command_stream.rs` | HTTP snapshot read of a stream. |
| `models/messages/hub.rs`, `models/messages/stream.rs` | `HubEvent`, `HubMessage`, `TopicKey`, stream frames and events. |
| `models/hub_error.rs` | `HubError`. |
| `repositories/hub_registry/` | `HubRegistry` trait and Redis implementation: SET / EXPIRE / DEL, CAS, pub/sub, registry-change listener. |
| `repositories/command_stream/redis/` | Redis stream log, meta, dedupe ids, the Lua scripts, the hydration lock, the active set and the wake listener. |
| `repositories/db.rs` | The builder's Postgres pool and the command-log migration. |
| `crates/shine-infra/src/db/command_log/` | Postgres command log and snapshots. |
| `crates/shine-game/src/replay/` | Pure replay core, chat state, the wire frames (`protocol.rs`) and the client engine (`client_stream.rs`), shared by server and client. |
| `services/command_stream/chat.rs` | The chat stream id and its access rule. |
| `app_state.rs`, `settings/command_stream.rs` | Wiring of the services and the hub cadences; validated stream settings. |

<details data-agent="implementation">
<summary>The registry write precedes the local teardown in ConnectUser</summary>

In `process(ConnectUser)` the order is: (1) `create_registry_connection`, (2) supersede the old local connection, (3) register the new one. If the registry write fails the loop returns before touching local state, so the previously active connection stays tracked. Reversing the order would let a transient Redis error leave the user with no connection and an orphaned, untracked new socket. The registry removal on the disconnect paths is best effort by contrast: a failure is logged and left to TTL, because the local removal and `UserDisconnected` must still happen so sockets and trackers never wedge.

</details>

<details data-agent="implementation">
<summary>Pub/sub is lossy by design; the heartbeat is the reconciler</summary>

The registry-change listener treats a `None` payload (Redis reconnect) as "notices during the outage were lost" and does nothing. Correctness does not depend on any single notification: the periodic heartbeat independently detects a connection the registry no longer holds. Pub/sub only shortens the window between a cross-instance takeover and the local teardown. Stream wakes follow the same rule with the periodic tick as the backstop, but the wake listener answers a `None` by waking every resident stream, which costs one idempotent pass each.

</details>

<details data-agent="implementation">
<summary>Hub tests wait on the FIFO barrier and tick counters, never on sleeps</summary>

Commands are FIFO, so a sentinel connect + disconnect whose events arrived proves every earlier command was processed; assertions after it are exact. To assert that a periodic consumer did not act, wait for its tick counter (`wait_ticks`, backed by `HubService::ticks`). Hub tests share the `serial = "hub-service"` group.

</details>

<details data-agent="implementation">
<summary>Working on the hub</summary>

- **Changing connection state:** add or extend a `ControlCommand` and handle it in `HubService::process`. Never mutate `Connections` from a consumer, route or test helper.
- **Removing a connection:** use the `remove_*` methods (CAS), then `record_drop(removed, reason)` for a hub-initiated removal (counts, logs, notifies the socket), then remove the registry row and publish `UserDisconnected`. Drop reasons are the `DisconnectReason` variants (metric labels via `as_str()`); adding a variant makes the compiler point at the close-code mapping in `connect.rs`.
- **New background consumer:** implement `ConnectionConsumer` and start it in `HubService::new` before the dispatchers, via `spawn_connection_loop`, which subscribes before the loop runs.
- **New egress message:** add a `HubMessage` variant; the socket forwards only what `event_to_wire_message` maps, and `try_send` labels its `kind`.
- **New stream kind:** register it in `StreamKindRegistry` (validator, state type, snapshot encoding); the hub and the protocol stay unchanged.
- **Sending to clients** happens off the loop (`send_to_connection`, used by the stream egress); a dead target is reported, not removed, by the sender.
- Keep `HubEvent` for broadcast lifecycle facts; addressed messages belong on `HubMessage`.
- Per-tick logs at debug or below; user and connection ids are fine in logs, never as metric labels.

</details>
