# Hub subsystem audit (builder service)

Scope: `services/builder/src/services/hub/*`, `routes/ws/*`, `repositories/hub_registry/*`, and the hub-to-`StreamRegistry` interface (`command_stream/stream_registry.rs` `open/close/disconnect/push`, `stream_authorize`). Read as new code, no history. All paths below are relative to `services/builder/src/`.

## Summary

| # | Sev | Area | Finding |
|---|-----|------|---------|
| 1 | High | authz / DoS | Stream authorization is allow-all and nothing caps streams per connection; one client can create unlimited streams (a worker each) and read/write any stream |
| 2 | Medium | DoS | No per-connection rate limit; every ws request feeds an unbounded control queue consumed by a single loop |
| 3 | Medium | DoS / availability | The single command loop does Redis I/O inline (up to the pool's 30 s checkout wait), so one slow Redis call stalls every connect/disconnect/drop |
| 4 | Medium | resource leak | No write timeout, idle timeout or ws keepalive; a peer that stops reading leaves a zombie socket after the hub has dropped it, and half-open peers stay registered |
| 5 | Medium | authz / amplification | `Ping` lets any user broadcast arbitrary strings to all users or target any user; default on (prod config turns it off) |
| 6 | Medium | shutdown | Nothing awaits hub shutdown, so stream persistence and registry cleanup race process exit |
| 7 | Medium | task lifecycle | Hub background tasks and the dispatcher are unsupervised; a panic silently disables session enforcement or kills the hub |
| 8 | Medium | scalability / revocation | `SessionChecker` validates sessions sequentially, so revocation lag grows with connection count and the tracker backlog is unbounded meanwhile |
| 9 | Low | perf | `remove_connection_by_id` is an O(n) scan under the write lock; misses are also O(n) |
| 10 | Low | perf | Every connect/disconnect is broadcast to every socket, which discards it (O(N) per event, O(N^2) in a storm) |
| 11 | Low | correctness | Registry pub/sub listener failure is not retried; fallback is the 300 s heartbeat |
| 12 | Low | correctness | `Connected` is sent before the hub registers the connection; registration failure closes the socket silently |
| 13 | Low | correctness | Close reason is lost when the socket queue is full; every drop reason except `Shutdown` maps to code 4000 |
| 14 | Low | robustness | `wait_shutdown_complete` panics if the command loop died; shutdown deregisters connections serially |

Checked and found sound: see the end.

---

## 1. High: stream authorization is allow-all, stream creation is uncapped

> Note: the allow-all `is_authorized` is a known placeholder. The per-connection/total stream cap and the missing re-check after subscribe remain valid regardless of the final authorization rules.

- `services/stream_authorize/stream_authorize_service.rs:14-16`: `is_authorized` returns `Ok(true)` for every user and stream ("for now").
- `routes/ws/connect.rs:260-274` parses `streamId` from the client (`kind:id`, `id` up to 64 chars of `[A-Za-z0-9_.:-]`, `shine-game` `stream_id.rs:4,61`).
- `hub_service.rs:264-289` then calls `registry.open`, which (`command_stream/stream_registry.rs:145-162`) inserts a new `StreamState` for any unseen id and adds it to the connection's set with no limit. `StreamDispatcher` (`tasks/stream_dispatcher.rs:~50`) calls `registry.subscribe`, which spawns a worker for each new stream (`stream_registry.rs:~255-262`).

Failure scenario: one authenticated user (free account) loops `{"type":"streamSubscribe","streamId":"chat:<random>"}` over one socket. Each message adds a `StreamState`, a backpressure object and a worker task (plus its Redis inbox and PG log activity), with no cap per connection or in total. Memory and tasks grow until the node degrades. Because any id is authorized, the same user can also read and write any other user's `chat:*` / `pixel-map:*` stream and push 16 KB commands that are fanned out to every subscriber (per-subscriber JSON and base64 serialization in `forward_egress`).

Fix:
- Implement real `is_authorized` (ownership or membership) before this ships beyond a demo; deny by default.
- Cap streams per connection (for example 8 to 32) in `StreamRegistry::open` and return `SubscribeFailure::Failed`/new `LimitExceeded`.
- Cap total open streams or workers per node.
- Add a per-connection command-rate token bucket in `send_stream_command`.

## 2. Medium: no per-connection rate limiting; unbounded control queue

- `hub_service.rs:93,104` (`HubServiceBuilder`) and `:325-330`: `control_tx` is an `UnboundedSender`. Every `subscribe`/`unsubscribe` (`:287-296`), every `request_connection`, every drop and every registry-change notification enqueues a `ControlCommand`.
- `routes/ws/connect.rs:170-195`: the receive loop does no throttling and no per-connection quota. Invalid JSON and unsubscribes cost nearly nothing to send.
- `StreamUnsubscribe` for any valid id enqueues `UnsubscribeStream` (`hub_service.rs:648-653`) and calls `publish_internal`, which takes the subscribers write lock and pushes into the unbounded `StreamDispatcher` queue.
- Also a user can reconnect in a tight loop (`request_connection`, `:352-368`). Each connect does a Redis `SET`+`PUBLISH` in the loop (`:662-665`) and triggers a registry-change round trip on every other node.

Failure scenario: a client spams subscribe/unsubscribe, or one account reconnects continuously. The producers outrun the loop whenever the loop is slowed (finding 3). `control_rx` and the per-consumer subscription queues (`hub_subscribers.rs`, unbounded) grow without bound until OOM.

Fix: per-connection token bucket in the receive loop (close with 1008 on repeated violation); per-user connect-rate limit at the upgrade; make the control channel bounded with `try_send` + drop-connection on overflow for client-originated commands (keep an unbounded or reserved lane for internal commands); expose queue depth as a metric.

## 3. Medium: command loop performs Redis I/O inline

- `hub_service.rs:542,546` (ConnectUser: `create_registry_connection`, `is_registry_connection`), `:605` (DisconnectUser), `:625` (DropConnection), `:712` (registry change heartbeat) all await Redis from the one loop that serializes every state mutation.
- Pool checkout wait defaults to 30 s (`crates/shine-infra/src/db/redis/redis_connection.rs:256`, `max_size` default 10) and the pool is shared with the HTTP session lookups, the heartbeat and the session checker. Per-command timeout is 500 ms by default.

Failure scenario: the pool is exhausted (traffic burst or Redis latency). Each loop command can block up to 30 s, so connects, disconnects and slow-consumer drops queue (finding 2), stale sockets stay in the map, and `Connections` keeps broadcasting to dead channels.

Fix: use a short dedicated timeout (for example 1 to 2 s) around registry calls in the loop; or move registry writes off the loop (the in-memory map is authoritative locally and the Redis write can be async with retry), or give the hub its own small Redis pool so it is not starved by HTTP traffic.

## 4. Medium: no write timeout, idle timeout or keepalive on the socket

- `routes/ws/connect.rs:366`: `ws_sender.send(...).await` has no timeout. A peer that stops reading (full TCP window) blocks `forward_egress` forever inside `send`.
- The hub, meanwhile, removes the connection once its 1000-slot queue fills (`hub_connections.rs:58`, SlowConsumer) and drops the sender. But the send task never reaches `rx.recv()` again, so `serve_socket` (`connect.rs:201-231`) never sees `send_task` finish. The receive task stays blocked on a silent peer, so the select never fires. The socket, two tasks and up to 1000 queued messages persist until TCP gives up (possibly never with a non-reading, ACKing peer).
- No server-initiated ws `Ping` and no idle deadline anywhere. A half-open client (NAT loss, power off) stays "connected": `Heartbeat` (`tasks/heartbeat.rs`) refreshes the Redis TTL of whatever the hub map holds and checks nothing about the socket, and `SessionChecker` only checks the session.

Failure scenario: a single account opens a socket, subscribes to a busy stream, stops reading, then reconnects (Replaced) and repeats. Each iteration leaves a zombie socket that the hub considers gone, so it is invisible to hub metrics.

Fix: wrap sends in `tokio::time::timeout` (for example 10 s) and exit the send task on failure; send periodic ws `Ping` frames and treat a missing `Pong` or any traffic within N intervals as dead; add an idle timeout; abort `recv_task` when the hub removes the connection (for example have `forward_egress` return when `rx` closes and select on it with a timeout).

## 5. Medium: `Ping` is an unauthenticated-target broadcast and push channel

- `routes/ws/connect.rs:305-322` and `hub_service.rs:427-443`: any logged-in user may send `{"type":"ping","id":"<255 chars>"}` and either `userId: <any uuid>` (`send_to_user`) or no `userId` (`broadcast` to every connection with `TopicKey::Ping`, which is every connection, `connect.rs:393`). There is no role check and no rate limit. User UUIDs are visible to other users in stream commands (`message.rs` `WireStreamCommand.user_id`).
- `ping_enabled` defaults to `true` (`hub_service.rs:62`); `server_config.prod.json:16` sets it false, so this is a fail-open default and a risk on any environment that omits the key.

Failure scenario (where enabled): one socket sends a few thousand broadcast pings. Each fans out into N queue slots at `hub_connections.rs:175-197`. Any victim whose socket drains slower than the burst overflows its 1000-slot queue and is evicted as `SlowConsumer` (`:58`); a targeted `send_to_user` does the same to one victim. It also lets users push attacker-chosen strings (255 B) to all clients.

Fix: default `ping_enabled` to `false`; restrict ping to an operator role; reject `userId` for non-admins; rate-limit; or delete the feature if unused.

## 6. Medium: shutdown is not awaited

- `main.rs:51-55`: the shutdown hook only calls `request_shutdown()` (fire and forget). `wait_shutdown_complete` is not called in production code (only in tests).
- `hub_service.rs:193`: the `StreamDispatcher` join handle is discarded. On `HubEvent::Shutdown` it runs `registry.shutdown(shutdown_timeout)` (default 10 s, `tasks/stream_dispatcher.rs:~60`) to persist pending work, but `shutdown_complete` (`:505`) is signalled without waiting for it.
- `crates/shine-infra/src/web/web_app.rs:94-110,150-155`: `run_web_app` returns once axum finishes serving. The hub closes every socket during shutdown, so the server finishes quickly and the runtime is dropped, aborting the dispatcher mid-persist. Plain-HTTP mode has no 10 s cap either.

Impact: buffered stream commands not yet persisted to the log can be lost at deploy or restart (the Redis inbox may still hold them; the log append is what is at risk), and registry rows are left until the 900 s TTL.

Fix: keep the dispatcher handle, join it (bounded) before `shutdown_complete.send_replace(true)`, and have the shutdown hook block (`block_in_place` or an async hook) on `wait_shutdown_complete` with a timeout before the server exits.

## 7. Medium: unsupervised background tasks

- `hub_service.rs:186-195`: `Heartbeat`, `SessionChecker`, the registry listener and `StreamDispatcher` are bare `tokio::spawn`s. Handles are joined only at shutdown (`:497-503`) and the dispatcher's handle is dropped.
- If `SessionChecker` panics, its subscription receiver drops; `Subscribers::publish` (`hub_subscribers.rs:36-46`) prunes it with only an `error!` log. Session revocation then stops being enforced for all live sockets with no alarm.
- If the command loop (`:466-506`) panics, `control_rx` is dropped, every `request_*` returns `SendCommandFailed`, existing sockets stay open and un-cleaned, and `wait_shutdown_complete` panics (finding 14).

Fix: spawn through a supervisor (restart consumers, or trip the health check and exit the process on loop failure); expose `HubStatus` liveness of each task.

## 8. Medium: sequential session checks

- `tasks/session_checker.rs:34-54`: `get_current_user` is awaited for every tracked connection one after another every 60 s. With N connections and per-lookup latency L, a tick takes N*L (10k * 2 ms = 20 s; 100k exceeds the interval and `MissedTickBehavior::Delay` stretches it).
- While `on_tick` runs, the `tokio::select!` in `connection_tracker.rs:~125` does not drain the subscription, so lifecycle events pile up in the unbounded queue.
- Revoked sessions keep their sockets for at least `session_check` (now 900 s in prod, was 60 s) plus tick time; a logged-out user keeps live stream access. Stream authorization is not re-evaluated at all after subscribe.
- `get_current_user` also extends the session TTL (`current_user_service.rs:125-131`). For a WebSocket-only user the checker is the only keepalive, so `session_check` must stay well below `sessionTtl` (1800 s).

Fix: bounded-concurrency checks (`buffer_unordered(32)` or a batched session lookup); revoke by event (publish on logout) rather than polling only.

Cheaper check (allows a short interval again without the cost or the TTL side effect): add a read-only existence check to `CurrentUserService` that only tests the sentinel key (`EXISTS`, or one `MGET`/pipeline for all tracked connections per tick) and does not read the session data or run the `EXPIRE` pipeline. Keep the TTL refresh on the normal request path (and, if needed, a rare refresh in the checker, e.g. every 15 min), so the frequent check stays a single cheap Redis round-trip per tick.

## 9. Low: O(n) `remove_connection_by_id`

`hub_connections.rs:110-119` scans `users` for a connection id while holding the write lock. It is hit on every dead-connection prune (`hub_service.rs:623,788`) and, since `HubStreamEgress::send` enqueues a `DropConnection` per failed send (`:216-224`), repeated drops for an already-removed connection each perform a full scan that returns `None`. Mass eviction of a slow cohort costs O(n^2) with writers blocking all stream sends.
Fix: store `user_id` in `Connection` and remove directly by `connection_id`.

## 10. Low: lifecycle events fan out to every socket

`connect.rs:393` subscribes each socket to `TopicKey::Hub`, but `event_to_wire_message` (`:109-115`) drops hub events, and the socket only needs its own disconnect, which `RemovedConnection::notify` (`hub_connections.rs:44-46`) already delivers. Every connect/disconnect therefore costs a read-lock scan and N `try_send`s (`:179-190`, `hub_service.rs:786`), during which the loop's writers wait. These events also consume the same 1000-slot queue that decides SlowConsumer eviction. A connect storm at N sockets is O(N^2) channel sends.
Fix: do not subscribe sockets to `TopicKey::Hub`; keep it for internal subscribers.

## 11. Low: registry listener not retried

`hub_service.rs:509-525`: if `listen_to_registry_changes` returns `Err` the task logs and exits; the node then misses cross-node takeover notifications and relies on the 300 s heartbeat (`HubConfig`), during which the old connection stays live although another node owns the user. (The listener handles reconnect internally via the `None` payload, but the initial subscribe failure is final.)
Fix: retry with backoff.

## 12. Low: `Connected` precedes registration

`connect.rs:149-165` sends `Connected` immediately after queueing `ConnectUser`. If `create_registry_connection` fails and the connection was not written (`hub_service.rs:542-549`), the command is dropped, `tx` closes, and `forward_egress` ends with `Close(None)`. The client saw `Connected` and then an unexplained close. Fix: reply `Connected` through the egress channel once registered (for example have the hub send it), or send a reasoned close frame on registration failure.

## 13. Low: lost close reason

`hub_connections.rs:44-46` uses `try_send` for the final `UserDisconnected`; for `SlowConsumer` (queue full by definition) it always fails, so the client gets `Close(None)` (`connect.rs:339,372`) and cannot tell why. All reasons except `Shutdown` share code 4000 (`connect.rs:45-58`), so a client auto-reconnect cannot distinguish `Replaced` from an error, which risks a reconnect ping-pong between two tabs. Fix: carry the reason in a side channel (`oneshot`/watch) that does not depend on queue space; use distinct 4xxx codes.

## 14. Low: shutdown robustness

- `hub_service.rs:402-406`: `.expect("hub dispatcher dropped its shutdown signal")` panics a caller if the loop task died (finding 7).
- `shutdown()` (`:758-763`) deregisters connections one at a time, each a pool checkout, WATCH transaction and PUBLISH; at 10k connections this is tens of seconds, beyond the 10 s grace window, leaving stale rows for 900 s and publishing N change events to other nodes. Fix: pipeline or batch removal, bounded concurrency.

---

## Verified as sound (no action)

- Origin and Host: `Origin` header is required (`shine-infra extracts/origin.rs:46-50`); an empty allow-list denies everything (`settings/mod.rs:9-15`); regexes are anchored (`app_state.rs:37-48`). Cross-site websocket hijack is mitigated; the session is authenticated at upgrade (`CheckedCurrentUser`, fingerprint-bound).
- Size limits: `max_message_size` and `max_frame_size` are 32 KB (`connect.rs:62,104`); commands are re-checked against 16 KB after base64 decode and `is_valid_command` (`stream_registry.rs:218`); ping id is capped at 255. Binary frames are ignored.
- Identity: `user_id` and `connection_id` always come from the server-side session and `request_connection`, never from the client payload (`connect.rs:149`, `hub_service.rs:257-263`).
- Map invariants: `users` and `connections` are mutated only on the loop; replacement removes the old entry before registering (`hub_service.rs:556-578`); stale disconnects are keyed by connection id and cannot remove a fresh reconnect (`hub_connections.rs:98-107`); lock scopes in `Connections` contain no awaits and only non-blocking `try_send`.
- Ordering: `ConnectUser`, `SubscribeStream` and `DisconnectUser` are FIFO on one channel; a subscribe that lands after the connection is gone is undone (`hub_service.rs:637-646`). `open` and `request_stream_subscribe` have no await between them (`:284-288`), so aborting the receive task cannot leave an orphaned open stream.
- Slow consumers: egress to sockets is bounded with `try_send` and never awaits under a lock; internal consumers are unbounded by design (see 2 and 8 for the consequence).
- Logging: `SessionKey` Debug is redacted (`session_key.rs:16-23`); client payloads appear only at `trace`.
