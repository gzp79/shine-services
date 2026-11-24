# Hub gateway and command loop split

Non-SDP plan. Not kept in sync with the code.

Cleanup of the builder hub (`services/builder/src/services/hub`) after the close-reason work ([Hub](../services/builder/hub.md)): one place that delivers to connections and reports dead ones, one removal path for dead connections, and `HubService` that does not store the same state twice. Nothing here is implemented.

**Paths** are relative to `services/builder/src/services/hub/` unless they start with `services/`, `docs/` or `crates/`.

## Starting point

- `socket_channel.rs`: `SocketSender` (hub side) and `SocketReceiver` (send task side) with a set-once close slot; the first `close` wins and tells the caller it was first.
- A send that finds a full or closed queue closes the connection as `SlowConsumer` or `DeadChannel`; only the first closer reports it as a `DisconnectUser`. `ControlCommand::DropConnection` is merged into `DisconnectUser`.
- `HubStreamEgress` (`hub_stream_egress.rs`) holds `Arc<Connections>` and the `ControlQueue`; `HubService` implements `StreamEgress` a second time.
- `ControlQueue` (`hub_control_queue.rs`) wraps an unbounded sender: it stamps the span context and the enqueue time, and counts the queue depth.

## Problem

| Smell | Where |
|---|---|
| Delivery to a connection and the report of a dead one exist twice | `HubStreamEgress::send` and `HubService::send_to_connection`, `send_to_user`, `broadcast`, `drop_dead` |
| `StreamEgress` is implemented twice | `HubStreamEgress`, `HubService` |
| A dead connection found while broadcasting a lifecycle event is removed inline, a copy of the `DisconnectUser` handler | `HubService::publish` (the `pending` queue, `record_drop`, the registry removal) |
| `ControlQueue` is a newtype whose job is the stamping and the metric, shared by two holders | `hub_control_queue.rs` |
| `Inner` would hold a gateway and `Arc<Connections>` for the same state | the planned gateway, if added naively |
| `start_dispatcher` is the command loop, next to `StreamDispatcher` | `hub_service.rs` |

## Who touches `Connections`

Only two kinds of code use it.

| User | Uses |
|---|---|
| The command loop | writes: `register_connection`, `unregister_connection`, `unregister_all`, `find_active`, `contains` |
| Delivery (`HubStreamEgress`, `HubService::send_to_*`, `broadcast`) | sends: `send_to_connection`, `send_to_user`, `broadcast`, and `len()` for the stats |

`subscribe_stream` and `send_stream_command` use the stream registry and the control queue, not `Connections`. `heartbeat_registry_connections` works on the Redis registry rows, a different kind of "connection". Unit tests of `Connections` create it alone, with no control sender.

## Target

### `HubGateway`

A cloneable handle that replaces `HubStreamEgress` and `ControlQueue`. It breaks the same cycle `HubStreamEgress` does: the stream registry is built before `HubService`, so it must hold a handle that does not own the hub.

```rust
#[derive(Clone)]
pub struct HubGateway {
    connections: Arc<Connections>,
    control_sender: mpsc::UnboundedSender<ControlMessage>,
}

impl HubGateway {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<ControlMessage>);

    // Delivery. Never waits; a dead connection is reported once, as a DisconnectUser.
    pub async fn send_to_connection(&self, id: Uuid, message: HubMessage) -> Result<bool, HubError>;
    pub async fn send_to_user(&self, user_id: Uuid, message: HubMessage) -> Result<(), HubError>;
    pub async fn broadcast(&self, message: HubMessage) -> Result<(), HubError>;
    pub fn connection_count(&self) -> usize;

    // The one place that stamps the span and the time and counts the queue depth.
    pub fn request(&self, command: ControlCommand) -> Result<(), HubError>;
}

impl StreamEgress for HubGateway { /* send -> send_to_connection, notify_failed -> request(StreamFailed) */ }
```

- `HubService::send_to_*`, `broadcast` and `send_control` delegate to it; the `request_*` methods only build their `ControlCommand`.
- `Inner` holds the gateway, not `Arc<Connections>` next to it. The loop reaches `Connections` through a crate-private accessor on the gateway.
- It stays small: delivery and requests to the loop. No admission, no Redis registry, no subscribers.
- Deleted: `HubStreamEgress`, the `StreamEgress` impl of `HubService` (or it forwards), `drop_dead`, `ControlQueue` and `ControlQueue::disconnect`.

### `publish`

```text
publish(event):
    UserDisconnected  -> stream_registry.disconnect(connection_id)
    subscribers.publish(message)
    gateway.broadcast(message)        // a dead connection is queued as DisconnectUser by the first closer
```

The inline pruning and the `pending` queue go. A dead connection found while broadcasting is removed by its own queued command, a little after the current one, so its `UserDisconnected` comes later than it does inline today. The loop then has one removal path for dead connections, `DisconnectUser`, plus the decisions that are its own: replaced, registry inactive, shutdown.

### `HubLoop` (second step)

The command loop moves out of `HubService`, which is large (`process` and its helpers).

| Holds | Notes |
|---|---|
| The control receiver, the shutdown signalling, `process` and its helpers | exclusive to the loop |
| The writer side of `Connections` | the gateway holds the other reference, for sends; `HubService` holds neither |
| `subscribers`, `hub_registry`, `stream_registry` | shared with `HubService` (`subscribe`, the heartbeat task, `subscribe_stream` and `send_stream_command`) |

`HubService` keeps the gateway, admission, the shared registries and `subscribers`, the supervisor and the ping flag. `start_dispatcher` becomes the loop's start (`start_command_loop`). The value of this step is mostly file size and clarity; the double storage is already gone after the gateway step.

## Steps

1. Gateway: add `HubGateway`, move delivery and `request` into it, delete `HubStreamEgress`, `drop_dead`, `ControlQueue` and the second `StreamEgress` impl. Shrink `publish`.
2. Rename `start_dispatcher` to `start_command_loop`.
3. `HubLoop`, if `hub_service.rs` is still hard to navigate.
4. Naming, any time (each is a rename): `Connections` to `LocalConnections`, `DeadConnection` to `FailedSend`, `hub_command.rs` to `control_command.rs`, `request_disconnection` and `request_drop` to one `request_disconnect(reason)`, `RemovedConnection` to `UnregisteredConnection`.

## Checks and open points

- **Event order.** The lifecycle and contention tests assert event sequences; run them after step 1. The events come from the same handler, so I expect no change except timing.
- **Write rule by convention.** The gateway holds the same `Connections` as the loop, so "only the loop writes" is enforced by who calls the write methods. A split into a writer type and a send-only view would make it a compile-time rule; worth it only if the rule is broken once.
- **The loop sends to its own queue** from `publish` (through `gateway.broadcast`). The queue is unbounded, so it cannot block; a failed send means the hub is shutting down.
- **`RemovedConnection`** warns when dropped without a `close`; a `debug_assert!` in `Drop` (not while panicking) would fail the tests on a forgotten close, and the two unit tests that drop one on purpose would need an explicit discard.
- **Scope of the gateway.** If registry, admission or subscribers creep into it, it becomes a second `HubService`.
