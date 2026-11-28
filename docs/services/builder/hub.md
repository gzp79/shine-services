# Hub

*The builder's WebSocket hub: locally serialized connection lifecycle, cross-instance registry reconciliation, and topic-based delivery to WebSocket connections.*

## Problem

The hub tracks connections and routes messages to them. Within one instance, a serial command loop is the sole writer of connection state; a `connection_id` makes stale disconnects harmless. Redis applies last-write-wins across instances, which notify and reconcile with one another rather than sharing a global command loop. This is a resource-abuse mitigation, not a global single-connection guarantee: two instances can briefly serve the same user until the other instance observes the registry change. Session and registry checks are periodic, so invalidation is eventually observed, not instantaneous.

Message delivery does not pass through the command loop: the gateway writes directly to each connection's bounded socket queue. The loop serializes lifecycle changes and stream-subscription events. The bus has three topics: `Hub` for connection lifecycle, `Stream` for stream events and worker messages, and `Ping` for the optional ping path. Stream behavior is detailed in [Command Streams](command-stream.md); admission and per-socket budgets are in [Hub Load Protection](hub-load-protection.md).

## Architecture

Host and origin checks, authentication, and hub admission happen before the WebSocket upgrade. The permit is held until the socket ends. The first server frame is `connected`; the receive task sends it after queuing `ConnectUser`, before the hub loop has necessarily completed Redis and local registration. The user id and connection id come from the authenticated connection, never from a frame.

Two tasks serve a socket. The receive task reads client frames and hands requests to their owning path: stream commands go straight to the registry, stream subscribe/unsubscribe operations combine registry changes with ordered loop events, ping uses topic delivery, and request replies can write directly to the socket sender. The send task reads `SocketReceiver`; hub-side producers enqueue through a `SocketSender` registered with the connection. This keeps stream data, ping delivery, and lifecycle control distinct without duplicating their details here.

```mermaid
flowchart TB
    classDef indigo fill:#6366f122,stroke:#6366f1
    classDef teal fill:#14b8a622,stroke:#14b8a6
    classDef blue fill:#0ea5e922,stroke:#0ea5e9
    classDef green fill:#22c55e22,stroke:#22c55e
    classDef orange fill:#fb923c22,stroke:#fb923c
    classDef violet fill:#a855f722,stroke:#a855f7

    C["WebSocket client"]:::indigo
    A["WS route<br/>host, origin, auth, admission"]:::teal

    subgraph Socket["Socket tasks"]
        R["Receive task<br/>SocketBudget"]:::teal
        S["Send task<br/>SocketReceiver::next<br/>Close has priority"]:::teal
        TX["SocketSender<br/>bounded data + close slot"]:::green
    end

    subgraph Hub["Hub"]
        CQ["Control queue<br/>unbounded FIFO"]:::green
        L["HubLoop<br/>serializes state changes"]:::blue
        CN["Local Connections"]:::green
        SE["HubGateway<br/>direct delivery and control"]:::teal
        BUS["Topic subscribers"]:::orange
        BG["Heartbeat, session checker,<br/>registry listener"]:::orange
        HR[("Redis hub registry")]:::violet
        DISP["Stream dispatcher"]:::blue
    end

    subgraph Streams["Command streams"]
        REG["Stream registry<br/>open, close, push"]:::blue
        IB[("Redis inbox")]:::violet
        W["Stream worker"]:::blue
        PG[("Postgres<br/>canonical log + snapshots")]:::violet
    end
    C -- "GET /api/connect" --> A
    A -- "upgrade accepted" --> R
    C -- "frames" --> R
    CN -- "SocketMessage" --> S
    R -- "lifecycle commands" --> CQ
    R -- "stream command: push<br/>subscribe: open, unsubscribe: close" --> REG
    R -- "direct request reply" --> TX
    CQ -- "ControlCommand" --> L
    SE -- "direct delivery" --> CN
    CN -- "try_send" --> TX
    TX -- "SocketMessage" --> S
    L -- "register, unregister" --> CN
    L -- "create, remove" --> HR
    BG -- "heartbeat CAS" --> HR
    HR -. "registry change" .-> BG
    BG -- "registry change, tick results" --> CQ
    L -- "HubEvent" --> BUS
    BUS -- "Hub topic events" --> BG
    L -- "broadcast HubEvent to matching socket topics" --> CN
    L -- "StreamEvent" --> BUS
    BUS -- "Stream topic events" --> DISP
    DISP -- "subscribe / start" --> REG
    REG --> IB --> W
    W -- "durable append, replay, snapshots" --> PG
    W -- "StreamEgress" --> SE
    SE -- "dead send: DisconnectUser" --> CQ
    S -- "text frames, close frame" --> C
```

`HubGateway` delivers directly to bounded socket queues; ordinary messages are not serialized through the command loop. The loop is the sole writer of local connection state and orders lifecycle and stream events. Redis stores the cross-instance active id; pub/sub accelerates reconciliation and heartbeat is the backstop. Receive-task replies may use the connection's weak sender directly. A worker uses `StreamEgress` through the gateway and cannot close a socket itself; failed delivery requests a disconnect, which the loop processes. Stream workers persist to Postgres. The diagram focuses on runtime paths; limit behavior is in [Hub Load Protection](hub-load-protection.md), and stream internals are in [Command Streams](command-stream.md).

### Message paths

| Direction | Message | Path | When overloaded |
|---|---|---|---|
| to hub | `streamCommand` | Receive task → registry `push` → Redis inbox → worker. Does not touch the control queue or the loop. | Inbox back-pressure: nack `busy`, the connection stays. |
| to hub | `streamSubscribe` | Registry `open` (authorize) → control queue → loop publishes `Subscribed` → dispatcher → worker. | Subscribe bucket of the socket; refusal is `streamSubscribeFailed`. |
| to hub | `streamUnsubscribe` | Registry `close` → control queue → loop publishes `Unsubscribed`. | Frame bucket of the socket. |
| to hub | `ping` | Receive task → `send_to_user` or `broadcast` → queue of the target. Off unless `ping_enabled`. | Frame bucket of the socket. |
| to hub | connect, disconnect, drop | Control queue → loop → `Connections` and the Redis registry. | Local connects are limited by admission; remote registry notifications are not covered by this instance's cap. See [Hub Load Protection](hub-load-protection.md). |
| to client | `streamSync`, `streamCommands`, `streamCheckpoint`, `streamDesynced` | Worker → `StreamEgress::send` → `Connections` → queue. Ordered per connection. | A full queue drops the connection as `slow_consumer`. |
| to client | `streamNack`, `streamSubscribeFailed` | The answer of a refused request goes from the receive task into the queue; a failed stream is answered by the hub with `streamDesynced`. | Lost with a full queue; that client is dropped anyway. |
| to client | `ping` | Topic delivery → direct queue of topic-matching connection(s). | Bounded queue; failed send closes the connection. |
| to client | close frame | The close slot, see [Ending a connection](#ending-a-connection). | Cannot be full. |

A subscribe has two steps. The registry `open` runs at once on the receive task: it authorizes the stream and refuses with `streamSubscribeFailed`. The `Subscribed` event follows through the control queue, so the loop orders it after the connection's own `ConnectUser`, and drops it when the connection is already gone; the dispatcher then starts or joins the worker. An unsubscribe is the same pair, `close` and `Unsubscribed`.

Lifecycle events are published to internal consumers and broadcast to sockets subscribed to the `Hub` topic, but are not converted into client protocol frames.

### Stream delivery

The `HubGateway` holds the connections and the sender of the control queue. A delivery is one call: it never waits, and a failure is answered by ending the connection, not by retrying. Ordinary delivery goes directly through `Connections`; the loop is only involved if the send reports a dead connection.

```mermaid
sequenceDiagram
    participant W as Stream worker
    participant SE as HubGateway
    participant CN as Connections
    participant TX as SocketSender<br/>of the connection
    participant CQ as Control queue
    participant L as Hub loop

    W->>SE: send(connection_id, message)
    SE->>CN: send_to_connection
    CN->>TX: try_send(message)
    alt queued
        TX-->>CN: ok
        CN-->>SE: queued
        SE-->>W: true
    else queue full or closed
        TX-->>CN: full or closed
        CN->>TX: close(SlowConsumer or DeadChannel)
        alt first to close
            CN-->>SE: FailedSend(user_id, connection_id, reason)
            SE->>CQ: DisconnectUser
            CQ->>L: DisconnectUser
            L->>CN: Connections::unregister, count the drop
        else already closed
            CN-->>SE: not queued
        end
        SE-->>W: false, the worker forgets the subscriber
    end
```

A connection with a full or closed queue is found by every send to it. The sender closes the connection's slot, and `close` tells whether the caller was first: only the first sender queues the disconnect, the others find the connection closed and queue nothing. The close slot is the deduplication; the control queue keeps no state for it. The socket is told at once, and the loop counts and publishes the reason the slot holds, so the metric and the event are what the client was told.

`notify_failed` queues `StreamFailed` on the same control queue. The loop closes the stream for every connection that has it open and answers each with `streamDesynced` through this same `send`.

### Next event of the send task

`SocketReceiver::next` is the only way the send task reads its endpoint and it decides by priority: a close reason beats queued data, so a slow consumer hears why it is dropped instead of draining a full queue first.

```mermaid
flowchart TD
    classDef green fill:#22c55e22,stroke:#22c55e
    classDef red fill:#ef444422,stroke:#ef4444
    classDef orange fill:#fb923c22,stroke:#fb923c

    N["SocketReceiver::next"] --> K{"close reason set?"}
    K -- "yes" --> CL["Close(reason)<br/>queued data is dropped"]:::orange
    K -- "no" --> M{"message queued?"}
    M -- "yes" --> MS["Data(message)"]:::green
    M -- "no" --> G{"every sender dropped?"}
    G -- "yes" --> GO["Gone<br/>ended without a reason, logged"]:::red
    G -- "no" --> W["wait, then ask again"]
    W --> N
```

### Topics

A message on the bus has exactly one topic. A connection declares its topics when it connects (the WS route uses `Hub` and `Ping`) and receives broadcast messages only for those topics. Internal tasks subscribe to topics in the same way. Stream worker egress is addressed by `connection_id` and does not rely on the socket subscribing to the `Stream` topic.

| Topic | Message | Carries |
|---|---|---|
| `Hub` | `HubEvent` | Connection lifecycle events. `Shutdown` is published to internal subscribers only; sockets close directly. |
| `Stream` | `StreamEvent`, `StreamMessage` | Internal subscribe/unsubscribe events use this topic. `StreamMessage` carries the topic tag but worker egress targets a connection directly. See [Command Streams](command-stream.md). |
| `Ping` | `PingEvent` | A client-chosen `id`, delivered to one user or to every connection subscribed to the topic. |

| Delivery | Recipients |
|---|---|
| `send_to_connection` | One connection by id. |
| `send_to_user` | The user's active connection. |
| `broadcast` | Every connection whose topics include the message's topic. |
| `subscribe` | An internal task, on its own channel. |

### Ping

A client sends `{"type":"ping","id":"…","userId":"<uuid>?"}`. With a `userId` the ping goes to that user's active connection, without one it goes to every connection subscribed to `Ping`, the sender included. Recipients get `{"type":"ping","id":"…"}`. `id` is at most 255 characters; an invalid ping is ignored. With `ping_enabled` off the request is counted as `disabled` and nothing is delivered.

### Wire protocol

| Direction | Frame | Fields and behavior |
|---|---|---|
| to server | `ping` | `id`, optional `userId`. |
| to client | `connected` | `userId`; first frame of a connection. |
| to client | `ping` | `id`. |
| to client | close | Sent when a connection ends: the `DisconnectReason` name as reason, and a code that tells the client whether to reconnect, back off or sign in again. See [Ending a connection](#ending-a-connection). |

Non-text, unparseable, or invalid request frames are ignored without an answer; the socket stays open. A message exceeding twice `MAX_COMMAND_BYTES` ends the socket. Frames over budget are refused and, if denials persist, the socket is dropped; connects over the rate or capacity limit are refused before upgrade. See [Hub Load Protection](hub-load-protection.md).

## Connection lifecycle

The unbounded control channel carries more than connection lifecycle commands. The loop serializes connection-state changes and lifecycle publication; stream subscription commands publish internal stream events, registry-change commands reconcile one user's entry, and shutdown closes the hub. It does not mediate ordinary message delivery. Periodic consumers track lifecycle events independently and enqueue disconnects when their checks find stale connections.

```mermaid
flowchart LR
    classDef indigo fill:#6366f122,stroke:#6366f1
    classDef teal fill:#14b8a622,stroke:#14b8a6
    classDef blue fill:#0ea5e922,stroke:#0ea5e9
    classDef green fill:#22c55e22,stroke:#22c55e
    classDef orange fill:#fb923c22,stroke:#fb923c

    P["Producer<br/>API, consumers, registry listener"]:::indigo
    Q["Control channel<br/>unbounded"]:::teal
    L["HubLoop<br/>one command at a time"]:::blue
    S["Connection state<br/>local Connections, Redis registry"]:::green
    T["Periodic consumer<br/>heartbeat, session checker"]:::orange
    D["Stream dispatcher"]:::blue

    P -- "enqueue ControlCommand" --> Q
    Q --> L
    L -- "lifecycle commands" --> S
    L -- "HubEvent" --> T
    T -. "DisconnectUser" .-> Q
    L -- "StreamEvent" --> D
    P -. "messages delivered directly,<br/>not through the loop" .-> S
```

| Command | Enqueued by | Loop action |
|---|---|---|
| `ConnectUser` | `HubService::request_connect` | Write the Redis registry (verify ambiguous failure), then supersede and register locally. |
| `DisconnectUser` | API, heartbeat, session checker, dead-channel send | Remove only if `connection_id` still matches; update Redis best-effort and publish the lifecycle event. |
| `HubRegistryChanged` | Redis registry-change listener | Recheck the user's active connection against the registry. |
| `SubscribeStream`, `UnsubscribeStream` | Stream API | Publish an internal `StreamEvent` if the connection still exists. |
| `StreamFailed` | Stream worker through `StreamEgress::notify_failed` | Close the stream for its open connections, then deliver `streamDesynced`. |
| `Shutdown` | `HubService::shutdown` | Publish shutdown, close sockets, remove registry rows concurrently (32 at a time, up to 5 s), stop consumers and loop; stream dispatcher drains workers.

### Lifecycle events

The loop emits lifecycle `HubEvent`s (defined in `models/messages/hub.rs`) for connection changes. Stream subscription and stream failure paths use internal `StreamEvent`s or direct stream messages instead; they are not lifecycle events. Lifecycle events carry the connection identity that lets a consumer distinguish a supersede from a same-connection renewal.

| Event | Payload | Emitted when |
|---|---|---|
| `UserConnected` | `user_id`, `connection_id`, `session_key` | A new connection is registered (after any prior one is superseded). |
| `UserDisconnected` | `user_id`, `connection_id`, `reason` | A connection is removed — superseded, disconnected, dropped-dead, or found inactive in the registry. |
| `Shutdown` | — | The hub is shutting down; delivered to internal consumers before they stop. |

`reason` is a `DisconnectReason`. `Client` is a disconnect the client asked for; every other reason is a *drop*, counted in `hub_connections_dropped`. The reason of every end is also told to the socket, see [Ending a connection](#ending-a-connection). Shutdown is different: the loop publishes `Shutdown`, closes sockets, and clears subscribers rather than publishing one `UserDisconnected` per socket.

Lifecycle events are published to the `Hub` topic. Internal subscribers receive them losslessly; connections receive topic-matching events through their bounded socket queues. `Shutdown` is internal-only, because sockets are closed directly. The bus has two classes of recipient:

| Recipient class | Channel | On lag | Why |
|---|---|---|---|
| `Connections` (addressable clients) | bounded queue of a `SocketReceiver`, handed in at connect time | reported dead → pruned | A slow client must not stall the hub; dropping it is correct. |
| `Subscribers` (internal tasks) | unbounded `mpsc`, lossless | never dropped | A dropped `UserConnected`/`UserDisconnected` would corrupt a task's connection tracker forever. |

### Event fan-out to tasks

Each periodic task subscribes to the `Hub` topic and event-sources its *own* `ConnectionTracker` from the events — there is no shared connection map between tasks and no lock. The tracker applies each event to a private `user_id → (connection_id, session_key)` map.

```mermaid
flowchart TD
    classDef orange fill:#fb923c22,stroke:#fb923c
    classDef green fill:#22c55e22,stroke:#22c55e

    E["HubEvent (Hub topic)<br/>UserConnected / UserDisconnected / Shutdown"]:::orange
    HB["Heartbeat"]:::green
    SC["SessionChecker"]:::green

    E -- "refreshes each tracked connection's Redis TTL;<br/>disconnects any the registry dropped" --> HB
    E -- "re-validates each tracked session;<br/>disconnects on expiry" --> SC
```

All subscriptions are registered synchronously before the command loop starts dispatching, so no task can miss an event emitted during startup — see `hub_service.rs · HubServiceBuilder::build`. The tracker's tick is prioritized over the event stream (`biased` select in `run_connection_loop`) so a heavy event burst cannot starve TTL refreshes and let a live connection expire.

### Last connection wins

`HubService::request_connect` mints a fresh `connection_id` and enqueues `ConnectUser`. The socket sends `connected` after that enqueue, before the loop has necessarily registered it. The loop then runs the sequence below. The registry write comes first; if its response is ambiguous, the loop checks whether Redis holds the new id before deciding to abort or continue.

```mermaid
sequenceDiagram
    participant RC as request_connect<br/>enqueue
    participant S as Socket task
    participant P as process()<br/>sole writer
    participant R as Redis registry<br/>cross-instance
    participant C as Connections<br/>in-memory
    participant B as Lifecycle bus<br/>HubEvent

    RC->>P: enqueue ConnectUser (new id)
    RC->>S: return connection_id
    S->>S: send connected frame
    P->>R: SET id EX ttl + PUBLISH
    alt write succeeds
        R-->>P: ok
    else write reports error
        P->>R: is_active(user, new id)
        alt registry holds new id
            R-->>P: active, continue
        else registry does not hold new id
            R-->>P: inactive, abort; old connection remains
        end
    end
    P->>C: remove old (if old ≠ new)
    P->>B: UserDisconnected(old)
    P->>C: insert new
    P->>B: UserConnected(new)
```

A same-`connection_id` re-registration is a renewal, not a supersede: the old-vs-new check skips teardown. Each connect mints a new id. Last-write-wins is enforced by the local loop and Redis registry writes; other instances learn of a change through pub/sub and heartbeat reconciliation, so cross-instance disconnect is eventual rather than globally serialized. Overlap across instances is tolerated: this limits persistent duplicate connections but does not guarantee there is only one live socket at every instant.

### Keeping connection info current

Three independent mechanisms detect a connection that should no longer be live and converge on the same CAS'd `DisconnectUser` path. Because the target carries a `connection_id`, a removal aimed at a connection that has since been replaced is a no-op — a stale check can never tear down a fresh reconnect.

| Mechanism | Trigger | Action |
|---|---|---|
| `Heartbeat` | Every `heartbeat` interval. | Batched `heartbeat_connections`: one script call per 256 tracked entries refreshes the TTL of those holding this id and returns the rest (entry gone or held by another connection) as stale → `request_drop` each. |
| `SessionChecker` | Every `session_check` interval. | Re-validates each tracked `(user_id, session_key)`. On `SessionExpired`/`SessionCompromised` → `request_disconnect`; transient errors skip the tick. |
| `HubRegistryChanged` | Redis pub/sub from another instance. | Loop rechecks the user's active connection with a CAS'd `heartbeat_connection`; if the registry does not hold this id, removes it locally and emits `UserDisconnected`. |

### Cross-instance registry

The Redis registry (`repositories/hub_registry`) extends last-wins beyond a single process. The key `hub-connection:{user_id}` holds the user's current `connection_id` with a TTL; the local heartbeat is what keeps that TTL alive, so a crashed instance's entries expire on their own. Mutations publish to `hub-registry-changed` as `{instance_id}:{user_id}`, and the listener drops self-originated notifications (its local view is already current).

| Operation | Redis | Semantics |
|---|---|---|
| `create_connection` | `SET key id EX ttl` + `PUBLISH` | Overwrites the active id for the user (last write wins) and notifies other instances. |
| `heartbeat_connection` | Lua script (`GET`, `SET EX` / `EXPIRE`) | Atomic CAS: extends the TTL when the stored id matches, writes nothing and returns `false` otherwise (row gone or held by another id). |
| `heartbeat_connections` | The same Lua script over many rows | Batched in chunks of 256 rows, one round trip per chunk; returns the entries that do not hold this id. A row lost to a Redis restart drops the node's connections, which reconnect (accepted, R3). |
| `remove_connection_if_active` | Lua script (`GET`, `DEL`, `PUBLISH`) | Atomic CAS: deletes only when both user and id match, and notifies in the same step. |

## Ending a connection

Every end of a connection is one `DisconnectReason`, set through `SocketSender::close` by the hub loop, the receive task, or a sender that finds the queue full or closed. The data queue carries no end-of-connection event. `connect.rs · close_code` maps the reason to the code of the close frame; the frame's reason text is `DisconnectReason::as_str`.

| Reason | Set by | Code | Client reaction |
|---|---|---|---|
| `Client` | Receive task, the peer closed or went away | 1000 | None; the client is gone. Not a drop. |
| `Shutdown` | Hub loop, the hub stops | 1001 | Reconnect with backoff, possibly to another instance. |
| `RateLimited` | Receive task, the socket budget is exhausted | 1008 | Back off, then reconnect. |
| `Replaced` | Hub loop, a newer connection of the user | 4001 | Do not reconnect; show that another tab took over. |
| `RegistryInactive` | Hub loop, another instance took the user over | 4001 | As `Replaced`. |
| `SessionInvalid` | Session checker, through the hub | 4002 | Do not reconnect; the user signs in again. |
| `HeartbeatInactive` | Heartbeat, the registry row is gone | 4000 | Reconnect with backoff. |
| `SlowConsumer` | The first send that finds the queue full | 4000 | Reconnect with backoff. |
| `DeadChannel` | The first send that finds the queue closed | 4000 | Reconnect with backoff. |

A close that no reason explains (a dropped connection, a failed write) carries no code; the client sees 1006 and backs off like for 4000.

A socket over its frame budget sets its own reason before it asks the hub to remove the connection, so the close frame does not depend on the hub loop. The hub's own close for the same drop finds the slot taken.

```mermaid
sequenceDiagram
    participant C as WebSocket client
    participant R as Receive task
    participant K as Close slot
    participant S as Send task
    participant H as Hub loop

    C->>R: frames over the budget
    R->>K: close(RateLimited)
    K-->>S: Close(RateLimited)
    S->>C: close frame 1008, rate_limited
    R->>H: request_disconnect(RateLimited)
    Note over H: may be busy or stalled, the close frame did not wait
    H->>K: close(RateLimited), ignored (first wins)
    H->>H: remove the connection, count the drop
```

## Client contract

The development `WebSocketStreamHub` owns one reconnecting socket and notifies its listeners when the connection opens or closes.

| Concern | Rule |
|---|---|
| Reconnect | On close it reconnects after a deterministic exponential delay, from 1 s up to 15 s. It adds no jitter and does not distinguish close codes. The attempt counter resets on WebSocket `open`, so a socket that opens and then closes again retries after 1 s. |
| Activation | On `connected`, it activates every stream subscription. Stream-specific state and resync behavior belong to the client replica. |
| Stream failure | `ClientStream` marks `streamDesynced` and any subscribe failure as `Desynced`; a `notSubscribed` nack triggers resubscribe. The pixel-map experiment retries a desynced stream after 5 s. |
| Pending commands | `ClientStream` retains pending commands across a sync and resends those not yet present in the server's command batch; any nack removes that command from pending. |
| Socket construction | An exception from `new WebSocket` is not caught by this development client; the close-driven reconnect path is not reached. |

## Metrics

The hub works asynchronously — control commands queue for the loop, messages are pushed to clients, consumers tick on their own schedule — so request-style metrics do not apply. Metrics are split by scope, each a process-wide set of OpenTelemetry instruments with its own meter and prefix; without a configured meter provider they are no-ops. Labels take a small fixed set of values; user and connection ids appear only in traces.

| Scope | Meter | Prefix | Covers |
|---|---|---|---|
| Hub | `shine-builder-hub` (`hub_metrics()`) | `hub_` | Connections, control commands, pushes to clients, background ticks. |
| WebSocket | `shine-builder-ws` (`ws_metrics()`) | `ws_` | What the clients send. |

A *control command* is the hub's own input (connect, disconnect, registry change…).

| Metric | Measures |
|---|---|
| `hub_connections` | Live connections. |
| `hub_connections_dropped` | Connections the hub removed, by reason. |
| `hub_connects_rejected` | Connects refused before the upgrade, by reason (`host`, `origin`, `rate_limited`, `capacity`, `hub_stopped`). |
| `hub_control_commands` | Control commands processed, by command. |
| `hub_control_command_duration` | Processing time of a control command in the loop. |
| `hub_control_command_queue_delay` | Time a control command waited before the loop picked it up. |
| `hub_messages_sent` | Messages pushed to connections, by kind and outcome. |
| `hub_tick_duration` | Tick time of the background consumers, by consumer. |
| `ws_messages_received` | Messages received from clients, by kind and outcome (`rate_limited` for the ones over the budget of the socket). |

## Key rules

| Concern | Rule |
|---|---|
| Connection replacement | The local loop replaces a different active connection. Across instances, Redis last-write-wins and notifications/heartbeats eventually remove stale local connections; temporary overlap is allowed because this is resource-abuse mitigation, not a strict single-connection guarantee. |
| Stale removal | Every removal path checks `connection_id` — locally (`Connections::unregister`) and in Redis (`remove_connection_if_active`) — so a stale removal cannot tear down a fresh reconnect. |
| Session bound | `SessionChecker` disconnects on definitive session loss when the periodic check observes it; transient validation errors defer the check. |
| Crashed instance | Registry entries carry a TTL kept alive only by the live instance's heartbeat, so its connections self-clean. |
| Task supervision | The command loop, heartbeat, session checker, stream dispatcher and the registry listener (when its subscribe fails) live as long as the hub. One that ends before a shutdown is recorded by the service-wide `TaskSupervisor` (`shine-core`, in `CoreServices`; the hub starts its tasks with its `spawn`, named `hub ...`). The web app then runs the shutdown as for a SIGTERM (hooks, then the server drains), so the platform restarts the service instead of leaving it half alive. |
| Lifecycle delivery | Lossless unbounded subscriber channels, with all subscriptions registered before the loop starts, so internal tasks never miss an event. |
| Topics | A connection receives only the topics it declared at connect time; a message has exactly one topic. |
| Identity | The user id and connection id come from the authenticated connection, never from a frame. |
| Slow connection | Egress never waits: a connection whose queue is full is dropped as `slow_consumer`, and its socket closes at once. |
| One endpoint | Socket task replies and hub-side producers enqueue through the connection's `SocketSender`; the close reason uses its separate slot. `socket_channel(capacity)` creates the sender and `SocketReceiver` together. |
| Data and end | The queue is bounded and carries data only. The reason of the end is a separate set-once slot that is not part of the queue, so it cannot be full. |
| First wins | The slot is set once; later calls are ignored, and `close` returns whether the caller was first. The receive task, the hub loop and the senders may all end the same connection; the first reason is the one the client gets, and the one the loop counts and publishes (`UnregisteredConnection::close` returns it). |
| Dead connection | A send that finds the queue full or closed ends the connection as `SlowConsumer` or `DeadChannel`. Only the first sender to end it queues a `DisconnectUser`, so a mass eviction queues one command per connection. |
| Priority | `next` returns `Close` before queued data. Queued data is not flushed: the peer of a `Client` close has stopped reading, and after `Shutdown` or any drop the client resubscribes and gets a fresh sync; its unconfirmed commands stay pending and go out again with it. |
| Silent end | The queue closing with no reason is `Gone`: the socket still ends, a warning is logged, and the close frame has no code. It marks a bug, not a case. |
| Commands skip the loop | A stream command goes from the receive task to the registry; it never waits for the control queue. Subscribe and unsubscribe queue ordered stream events; lifecycle commands preserve the connection's connect-before-disconnect ordering. |
| Drop bookkeeping | Local removal, best-effort Redis removal, publishing `UserDisconnected` and counting `hub_connections_dropped` happen as the loop processes the disconnect. `Client` is not a drop and is not counted. |
| Workers do not close | A worker that finds a connection full or dead gets `false` from `StreamEgress::send`; the hub drops the connection with `SlowConsumer` or `DeadChannel`. `StreamEgress` has no close. |
| Other ends | The idle timeout and the message size limit end the receive task, and the socket ends the connection as `Client` (1000). A write or drain timeout ends the send task without a close frame. |
| Ping | Gated by `ping_enabled`; `id` is at most 255 characters. |
| Load | A connect is admitted by the per-user rate and instance cap before upgrade; frames draw on the socket budget. The unbounded control queue also accepts remote registry notifications. See [Hub Load Protection](hub-load-protection.md). |

## File map

| File | Role |
|---|---|
| `services/builder/src/routes/ws/connect.rs` | Upgrade (after `admit`), `connected` frame, receive and send tasks over the connection's endpoint, ping request, `close_code`, the close frame, the receive task's own close and answers. |
| `services/builder/src/routes/ws/socket_budget.rs` | `SocketBudget`: the frame budget of a socket. |
| `services/builder/src/routes/ws/message.rs` | Wire request and response shapes. |
| `services/builder/src/services/hub/hub_service.rs` | The public handle: builder, admission, stream entry points, `request_*`, direct delivery through the gateway, registry listener. |
| `services/builder/src/services/hub/control_command.rs` | `ControlCommand` enum — lifecycle and stream commands handled by the loop. |
| `services/builder/src/services/hub/hub_gateway.rs` | `HubGateway`: direct delivery to connections, `send_control` to the loop, `StreamEgress` for workers. |
| `services/builder/src/services/hub/hub_loop.rs` | `HubLoop`: command processing, local connection lifecycle, registry reconciliation, stream failure handling, shutdown, event publication. |
| `services/builder/src/services/hub/hub_registry.rs` | `HubRegistry`: the hub's calls to the Redis connection registry. |
| `crates/shine-infra/src/sync/admission.rs` | Per-user connect rate and instance-cap permit held for the socket lifetime. |
| `services/builder/src/services/hub/hub_connections.rs` | Addressable connection registry (`Connections::register`, `unregister`); keeps each connection's `SocketSender`; dead-connection detection. |
| `services/builder/src/services/hub/hub_subscribers.rs` | Internal lossless fan-out subscribers. |
| `services/builder/src/services/hub/hub_status.rs` | Status provider exposing connection and subscriber counts from `HubService::stats`. |
| `services/builder/src/services/hub/hub_metrics.rs` | OpenTelemetry instruments of the hub. |
| `services/builder/src/routes/ws/ws_metrics.rs` | OpenTelemetry instruments of the websocket endpoint. |
| `services/builder/src/services/hub/tasks/background_task.rs` | Event-sourced `ConnectionTracker` and periodic background-task loop driver. |
| `services/builder/src/services/hub/tasks/heartbeat.rs` | Batched registry TTL refresh; disconnects registry-dropped connections. |
| `services/builder/src/services/hub/tasks/session_checker.rs` | Per-tick session validation; disconnect on expiry. |
| `services/builder/src/models/messages/hub.rs` | `HubEvent`, `DisconnectReason`, `HubMessage`, `TopicKey`, `ToTopic`. |
| `services/builder/src/models/messages/ping.rs` | `PingEvent`. |
| `services/builder/src/services/hub/socket_channel.rs` | `socket_channel`, `SocketSender`, `SocketReceiver`, `SocketMessage`: the data queue and the close slot of a connection. |
| `services/builder/src/services/hub/tasks/stream_dispatcher.rs` | Subscribes to `Hub` and `Stream` topics, applies stream subscription events, purges ended workers and drains the registry on shutdown. |
| `services/builder/src/services/command_stream/stream_egress.rs` | `StreamEgress`: worker-to-connection delivery. |
| `client/web/src/engine/net/stream-hub.ts` | `StreamListener.disconnected(stopped?)`, `HubStopReason`. |
| `client/web/src/engine/net/websocket-stream-hub.ts` | Development reconnecting socket; deterministic exponential reconnect delay and stream subscription routing. |
| `client/web/src/experiments/pixel-map/index.ts` | Pixel-map stream subscription, live-state gating, and fixed-delay resubscribe after desync. |
| `crates/shine-game/src/replica/client_stream.rs` | Client replica state, pending commands, sync/recovery, checkpoint verification and nack behavior. |
| `services/builder/src/repositories/hub_registry/hub_connections.rs` | `HubConnections` trait — cross-instance connection contract. |
| `services/builder/src/repositories/hub_registry/redis/redis_hub_connections.rs` | Redis implementation: SET/EXPIRE/DEL, CAS transactions, pub/sub. |
| `services/builder/src/repositories/hub_registry/redis/redis_hub_connection_db.rs` | Pooled context factory; registry-change listener with self-origin filtering. |
| `services/builder/src/app_config.rs`, `services/builder/src/app_state.rs` | Hub configuration and application wiring; constructs the Redis registry/inbox and Postgres command log. |

<details data-agent="implementation">
<summary>Why the registry write precedes local teardown in ConnectUser</summary>

In `HubLoop::process(ConnectUser)` the order is: write the new registry id, remove any different local connection, then register the new one. If the write call reports an error, the loop checks `is_active`: when Redis holds the new id despite a lost response it continues; otherwise it returns before changing local state. Disconnect-side registry removal is best-effort; local removal and `UserDisconnected` still happen so sockets and trackers do not wedge, with TTL cleanup handling a stale registry entry.

</details>

<details data-agent="implementation">
<summary>Pub/sub is lossy by design; the heartbeat is the reconciler</summary>

The `hub-registry-changed` listener treats a `None` payload (Redis reconnect) as "messages during the outage were lost" and does nothing — it does not attempt a resync. Correctness does not depend on any single notification arriving: the periodic `heartbeat_connections` reconciliation independently detects a connection the registry no longer holds and disconnects it. The pub/sub path is only a latency optimization that shortens the window between a cross-instance supersede and the local teardown; the heartbeat is the backstop.

</details>

<details data-agent="implementation">
<summary>Why a close slot and not an event on the queue</summary>

The reason has to arrive exactly when the queue is the problem: a slow consumer has a full queue by definition, and an event queued behind it either does not fit or arrives after the stale data. A slot kept inside the same bounded queue works but needs every sender to leave room for it, which is arithmetic every producer must know. A separate slot, owned by `SocketReceiver`, keeps the rule in one place: data is data, the reason is the reason, and `next` is the only reader.

`SocketReceiver` holds an `mpsc::Receiver` for the data and a `watch::Receiver<Option<DisconnectReason>>` for the reason. The sender sets it with `send_if_modified` so only the first writer changes it, and returns whether that was the caller. `next` has to stay cancel safe because the send task selects on it next to the ping timer: both `recv` and the watch wait are, and a change must keep that true.

A sender that finds a dead queue ends the connection itself, so its reason can win against a decision the loop was about to take. The one case that matters is a stuck client that is replaced at the same moment: it is told `slow_consumer` (4000), reconnects once, and takes the connection back from the newer tab, which is told `replaced` and stays down until it is reloaded. The fresh connection is not slow, so it settles after that one reconnect.

A watch whose senders are all dropped without a reason is the `Gone` case; the hub keeps its sender until the connection is removed, so `Gone` after a registered connection means a path closed a connection without telling it why.

</details>

<details data-agent="test">
<summary>Where the hub is covered</summary>

`services/builder/src/services/hub/tests/` covers the loop: `lifecycle.rs` (last-wins, disconnect, topic delivery, heartbeat, session checks), `contention.rs` (concurrent connects and broadcasts, across instances) and `fault.rs` (registry write failures; the faults of the streams are in `stream_fault.rs`). Unit tests sit beside `hub_connections.rs`, `hub_subscribers.rs` and `tasks/background_task.rs`, and `routes/ws/connect_test.rs` covers the socket. `tests/api-tests/builder/ws_connect.ts` covers the real service: origin and auth rejection and the close code of a replaced socket (last-wins). The rest of the hub (ping delivery, session checks, drop reasons) is covered in Rust only.

</details>

<details data-agent="test">
<summary>Coverage map of the egress, close and client rules</summary>

| # | Case | Kind |
|---|---|---|
| 1 | `next`: close beats queued messages; a message when no reason; `Gone` when every sender dropped without a reason | unit |
| 2 | The first reason wins, also across clones of the sender | unit |
| 3 | A full queue still delivers the reason (`SlowConsumer`) | unit, socket test |
| 4 | `close_code` of every reason, compared with the literal codes | unit |
| 5 | A socket over its budget closes 1008 while the hub loop is not serving | socket test |
| 6 | A client that closes gets 1000 and no drop is counted | socket test, hub test |
| 7 | The replaced socket of the real service closes 4001 `replaced` | Playwright (`ws_connect.ts`) |
| 8 | WebSocket stream hub reconnects after a close with capped exponential delay and routes subscriptions by stream id | vitest |
| 9 | `ClientStream` applies sync and command batches, clears nacked commands, resubscribes on `notSubscribed`, and verifies checkpoints | Rust unit |
| 10 | Many sends to one full or closed queue queue one `DisconnectUser`; a replaced connection is not removed by a late one | unit, hub test |

</details>
