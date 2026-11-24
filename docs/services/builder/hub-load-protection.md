# Hub Load Protection

*Per-instance limits that keep one misbehaving client from exhausting the hub: admission at connect, a budget per socket, a depth limit on the command queue, and a time limit on the loop's Redis calls.*

## Problem

A logged-in client can reconnect in a tight loop, flood a socket with frames, or spam subscribes that each cost the server a full `Sync`. Without limits, producers outrun the hub command loop, its queue grows until the process runs out of memory, and a slow Redis stalls every connect and disconnect behind it. Limits must stop the offender, not whoever happens to push next, and must stay simple: in memory, per instance, no shared state.

## Architecture

A connection passes two gates before it can load the hub: admission when it connects, then a budget on every frame it sends. Everything reaches the command loop through one FIFO queue, so a socket's connect, subscribes and disconnect keep their order. Requests that originate at a client are admitted only while the queue is shallower than `control_queue_capacity`; the hub's own commands are never refused.

```mermaid
flowchart LR
    classDef indigo fill:#6366f122,stroke:#6366f1
    classDef teal fill:#14b8a622,stroke:#14b8a6
    classDef blue fill:#0ea5e922,stroke:#0ea5e9
    classDef green fill:#22c55e22,stroke:#22c55e
    classDef orange fill:#fb923c22,stroke:#fb923c
    classDef red fill:#ef444422,stroke:#ef4444

    C["WebSocket client"]:::indigo
    A["HubService::admit<br/>per-user connect rate, instance cap, queue depth"]:::teal
    F["SocketBudget<br/>per socket"]:::teal
    D{"queue depth<br/>under capacity?"}
    S["HubService::send_stream_command<br/>stream registry write path"]:::blue
    B{"inbox back-pressure<br/>waits above backpressure_high,<br/>up to backpressure_timeout"}
    I["Redis inbox"]:::green
    Q["Command queue<br/>FIFO"]:::green
    IN["Hub internal commands<br/>disconnect, drop, registry change"]:::orange
    L["Command loop<br/>registry calls with timeout"]:::blue
    X["Rejected<br/>429, 503, nack, close"]:::red

    C -- "GET /api/connect: connect()" --> A
    A -- "Ok(ConnectionPermit): on_upgrade, serve_socket" --> F
    A -- "Err(AdmitError): 429 or 503" --> X
    F -- "Verdict::Allow, frame is a subscribe or unsubscribe" --> D
    F -- "Verdict::Allow, frame is a stream command" --> S
    S -- "StreamRegistry::push" --> B
    B -- "room: queued" --> I
    B -- "timeout: Err(Busy), nack busy, connection stays" --> X
    F -- "Verdict::Deny, Verdict::Close" --> X
    D -- "yes: ControlQueue::send_client" --> Q
    D -- "no: request_drop(Overloaded)" --> X
    IN -- "ControlQueue::send" --> Q
    Q -- "process()" --> L
```

`Verdict::Allow` is one verdict for every granted frame; what happens next depends on the frame. A stream command does not use the queue: it goes straight to the stream registry's write path ([Command Streams](command-stream.md)). A ping, when enabled, goes to the hub's topic delivery and is not drawn.

### Verdict

`SocketBudget::check` returns a `Verdict` for every frame. The overview above draws its three outcomes as the arrows leaving `SocketBudget`; this diagram is the inside of that box, showing how `check` reaches them.

A text frame is parsed first (bounded by the 32 KB message limit), then draws tokens. A binary frame is not parsed and draws one frame token. The violation counter turns a sustained flood into a close.

```mermaid
flowchart TD
    classDef green fill:#22c55e22,stroke:#22c55e
    classDef red fill:#ef444422,stroke:#ef4444
    classDef orange fill:#fb923c22,stroke:#fb923c

    P["text frame parsed, or binary frame"] --> T{"frame token<br/>and, for subscribe or unsubscribe,<br/>subscribe token"}
    T -- "granted" --> OK["Verdict::Allow<br/>handle the frame, violations = 0"]:::green
    T -- "denied" --> V{"violations over the limit?"}
    V -- "no" --> N["Verdict::Deny<br/>command nack or subscribe failure, reason rateLimited<br/>invalid or binary frame: no answer"]:::orange
    V -- "yes" --> D["Verdict::Close<br/>drop connection, reason RateLimited, close 1008"]:::red
```

## Key rules

| Concern | Rule |
|---|---|
| Scope | Every limit is per instance and in memory. Nothing is shared through Redis. A user has one live connection per instance (last wins), so a per-socket budget is the user's budget across all streams. |
| Frame budget | One `TokenBucket` per socket, owned by its receive task (no lock). Every text or binary frame costs a frame token, parseable or not (WebSocket ping, pong and close frames are not charged). A granted binary frame is ignored. Subscribe and unsubscribe also cost a subscribe token. |
| Denied frame | Command: `streamNack` reason `rateLimited`. Subscribe: `streamSubscribeFailed` reason `rateLimited`. Invalid, binary or non-stream frame: no answer. The connection stays open. |
| Violations | Consecutive denied frames are counted; any granted frame resets the count. At `violation_limit` the socket is dropped as `RateLimited` (close 1008). |
| Subscribe coalescing | The subscribe bucket is the coalescing rule: repeated subscribes to a stream (each a full `Sync`) are bounded by its refill rate. |
| Busy inbox | The inbox back-pressure is kept as it is (`inbox.backpressure_high`, `backpressure_low`, `backpressure_poll_period`, `backpressure_timeout`): pushes wait above the high mark and fail `busy` after the timeout. A `busy` push is nacked; the connection stays open and there is no `Busy` disconnect reason. The sender's frame budget limits how fast it can retry. |
| Connect rate | `KeyedRateLimiter<Uuid>` keyed by `user_id`, before the upgrade. Every authenticated attempt that passes the host and origin checks draws a token, including ones the cap then refuses. It outlives the socket, so a reconnect loop does not reset its budget. Over budget: 429 with `Retry-After`. |
| Connection cap | A semaphore of `max_connections` permits, owned by `HubService`. `admit(user_id)` checks the rate, then the cap, then the queue depth, and returns a `ConnectionPermit` that the socket holds until it ends. Exhausted: 503. |
| Client backoff | Clients are expected to back off on a refused connect and on close 1008 and 1013; that work is the audit items C2 and C8 and is not part of this design. |
| Command queue | One FIFO, so a connection's commands run in the order they were sent. The queue counts its depth. Client-originated `SubscribeStream` and `UnsubscribeStream` are accepted only while the depth is under `control_queue_capacity`; otherwise the connection is dropped as `Overloaded` (close 1013). The capacity is a memory bound for a flood, sized far above steady state, not a fairness tool: in a mass reconnect honest clients can hit it too, and back off. |
| Queue depth at connect | Admission refuses a connect (503, counted `overloaded`) while the queue is at capacity, so the client sees the refusal on the upgrade and not a close after it. |
| Hub commands | Disconnects, drops, registry changes, stream failures and shutdown skip the depth check, so they are never refused. They are bounded by what produces them: one disconnect per connection, drops deduplicated per connection, one registry notice per remote connect, one tick of the tasks. |
| Registry timeout | Every call to the hub connection registry (create, find, remove, heartbeat, from the loop and the tasks) is wrapped in `registry_timeout`. The stream registry has its own limits (`backpressure_timeout`) and is untouched. A timeout is an error like any other registry error (counted as `timeout`); `ConnectUser` already re-reads the registry after a failed write, so a timed-out write that landed is accepted. |
| Ping | A ping frame is an ordinary frame: it draws a frame token. |
| Close codes | `RateLimited` 1008, `Overloaded` 1013, the rest as in [Hub](hub.md#wire-protocol). |
| Metrics | Refused connects count in `hub_connect_rejected` (`rate_limited`, `capacity`); drops in `hub_connections_dropped` by reason; denied frames in `ws_messages_received` with outcome `rate_limited`; timed-out registry calls in the registry error counter. |

### Limits

All values are configuration; the defaults below are the starting point. The frame and subscribe limits are per socket (`WsConfig.limits`); admission, lane capacity and timeout are hub settings (`HubConfig`).

| Setting | Default | Meaning |
|---|---|---|
| `frames` | burst 60, 30 / s | Frame bucket of a socket. |
| `subscribes` | burst 10, 2 / s | Subscribe and unsubscribe bucket of a socket. |
| `violation_limit` | 20 | Consecutive denied frames before the drop. |
| `connect_rate` | burst 5, 1 per 10 s | Connects per user. |
| `max_connections` | 10 000 | Sockets per instance, from upgrade to close. |
| `control_queue_capacity` | 4096 | Queue depth at which client requests are refused. |
| `registry_timeout` | 2 s | Per registry call in the hub. |

## Reusable pieces

Two types in `shine-infra::sync`, next to `ExponentialBackoff`; neither knows about the hub.

| Type | Contract |
|---|---|
| `TokenBucket` | `new(capacity, refill_per_second)`; `try_acquire(&mut self) -> bool`, one token per call, lazily refilled from a monotonic clock; `try_acquire_at(now)` and `is_full_at(now)` take the time as a parameter so tests need no sleeps. |
| `KeyedRateLimiter<K>` | Cloneable handle over a mutex-guarded map of buckets with one shared configuration. `check(key) -> bool` draws a token from the key's bucket, creating it full. Buckets that have refilled to full are idle and are swept lazily, so memory follows the keys active within one refill period. |

Another key shape is another `K` (`(Uuid, Group)`); another rule is another limiter instance with its own configuration. `SocketBudget` is builder-local (`ws_limits.rs`): two `TokenBucket`s and the violation counter behind a `check(frame_kind) -> Verdict` of `Allow`, `Deny` or `Close`, one instance per socket. Admission is not a type: it is the method `HubService::admit`, which combines the `KeyedRateLimiter`, the connection semaphore and the queue depth check.

## File map

| File | Role |
|---|---|
| `crates/shine-infra/src/sync/token_bucket.rs` | `TokenBucket`. |
| `crates/shine-infra/src/sync/keyed_rate_limiter.rs` | `KeyedRateLimiter<K>`. |
| `crates/shine-infra/src/sync/mod.rs` | Exports the two types. |
| `services/builder/src/routes/ws/ws_limits.rs` | `SocketBudget` and `Verdict`; `WsLimits` configuration. |
| `services/builder/src/routes/ws/ws_config.rs` | `WsConfig.limits`. |
| `services/builder/src/routes/ws/connect.rs` | `admit` call before the upgrade; `SocketBudget` in the receive loop; answers for denied frames; close codes 1008 and 1013. |
| `services/builder/src/routes/ws/message.rs` | Wire reason `rateLimited` for nacks and subscribe failures. |
| `services/builder/src/models/messages/stream.rs` | `CommandFailure::RateLimited`, `SubscribeFailure::RateLimited`. |
| `services/builder/src/models/messages/hub.rs` | `DisconnectReason::RateLimited`, `Overloaded`; no `Busy`. |
| `services/builder/src/services/hub/hub_control_queue.rs` | Single FIFO with a depth counter; `send_client` (depth-gated) and `send` (always accepted). |
| `services/builder/src/services/hub/hub_service.rs` | `admit` and `ConnectionPermit` (connect limiter, semaphore, depth check); registry calls with timeout; overflow drop; busy nack without a drop; `HubConfig` fields. |
| `services/builder/src/services/hub/hub_metrics.rs` | New refusal reasons. |
| `crates/shine-game/src/replica/client_stream.rs` | A `rateLimited` nack removes the pending command like any other nack; a `rateLimited` subscribe failure takes the existing subscribe-failure path. No retry of the command. |

<details data-agent="implementation">
<summary>Why parse before charging, and why there is one queue</summary>

A denied command must be answered with its `commandId`, and a denied subscribe with its stream id, so the frame has to be parsed before it is charged. The parse cost per frame is bounded by the 32 KB message limit, and `violation_limit` closes a sustained flood after a few dozen frames.

The queue stays one FIFO because a socket's `ConnectUser` must be processed before its `DisconnectUser`; two channels would let a quick close overtake its own connect and leave a registered connection with no socket. The depth gate gives the bound without a second channel: client requests check the counter, hub commands do not. The counter is checked and incremented without a lock, so the depth may overshoot the capacity by the number of producers racing it, which is fine for a memory bound. An overflow drops the connection through `request_drop`, so the socket sees a normal hub close.

</details>

<details data-agent="security">
<summary>What this does not cover</summary>

Limits are per user: an attacker with many accounts has one budget per account, and `max_connections` is the backstop. There is no per-IP limit (it needs trusted proxy headers, see the audit item R17), no budget shared between instances, and no per-user quota inside the stream inbox. The loop still awaits registry calls inline; the timeout and the depth gate bound the damage of a slow Redis, they do not remove it.

</details>

<details data-agent="test">
<summary>Coverage map</summary>

| # | Case | Kind |
|---|---|---|
| 1 | `TokenBucket`: burst, refill over time, capacity clamp, `is_full_at` | unit, injected time |
| 2 | `KeyedRateLimiter`: keys independent, idle keys swept, clone shares state | unit |
| 3 | `SocketBudget`: grant, deny, violation count resets on a granted frame, `Close` at the limit | unit |
| 4 | Denied command nacks `rateLimited` with its `commandId`; denied subscribe fails `rateLimited` | socket test |
| 5 | Sustained flood closes the socket 1008; honest traffic beside it is untouched | socket test |
| 6 | Connect loop of one user gets 429; another user is unaffected | route test |
| 7 | Cap reached gives 503; closing a socket frees its permit | route test |
| 8 | Subscribe at a full queue drops the producer as `Overloaded`; connect at a full queue is refused at admission with 503 | hub test |
| 9 | Disconnect and drop are accepted at a full queue, and a quick connect then close leaves nothing registered | hub test |
| 10 | Registry call that never answers times out; a `ConnectUser` whose write timed out but landed is kept | hub fault test |
| 11 | `busy` nack leaves the connection registered (replaces the tests that expect the busy drop) | hub stream test |

</details>
