# Builder hub and command streams: open issues

Non-SDP investigation notes. Not kept in sync with the code.

Open findings for the builder hub (`services/hub`, `routes/ws`, `repositories/hub_registry`), the command stream (Redis inbox, relay, persistence, Postgres log, worker, registry, stream authorize) and their clients (`shine-game` replica, web client). Fixed, intentional and accepted items were removed; see git history of this file for their write-ups.

Method: static reading only. Line numbers drift, treat them as pointers.

**Paths**: relative to `services/builder/src/` unless they start with `crates/`, `client/`, `docs/`, `.claude/`, `.github/`, `Cargo.toml` or `services/`.

**Design assumption**: several builder instances may serve the same stream, each with its own worker reading the same Redis inbox and appending to the same Postgres log (`stream_registry.rs`, `stream_persistence.rs`). Redis pub/sub wakes are best effort.

**Conventions that stay from closed items**
- Only the literal `prod` stage requires TLS; a new stage on a hosted database needs its own TLS check.
- `panic = "abort"` on release: a panic is a critical bug, the platform restarts the service. Findings that only matter when a panic is survived are moot.
- Once a V1 migration has run on a database that is not purged, change it only with a new migration.
- `command_id` is a client-owned placeholder, the server does not dedup by it.
- A missing hub registry row means inactive (reconnect storm after a Redis flush is accepted).
- The inbox soft backpressure limit is the only size bound (no hard limit).
- `Problem::log` dumping internals at `info`/`error` is a documented exception to "sensitive data: trace only".

## Summary

| ID | Sev | Area | Finding |
|----|-----|------|---------|
| L2 | Low / Medium | multi-instance | Persisted-version wakes are ignored; a peer that missed a wake stays stale |
| L4 | Low | integrity | A `Corrupt` load gives the worker up; snapshot store checks only `chain_hash` |
| L5 | Low | lifecycle | `is_finished()` true while a worker drains; draining worker invisible to `shutdown` |
| L6 | Low | correctness | Inbox cursor never detects a Redis id regression |
| L7 | Low | correctness | Inbox trim only after commands are stored |
| L8 | Low | perf | `ensure_stream` writes on every worker load |
| L9 | Low | correctness | Load/prune race yields a `Gap` |
| L10 | Low | availability | No PG `statement_timeout` on reads/appends |
| L12 | Low | compat | Rolling deploy: old binary cannot decode new command variants |
| L13 | Low | correctness | Grace timer can starve |
| L14 | Low | misc | Chat text stored and relayed raw |
| A1 | Low | authz | Access is by stream id only; no per-user rule, no re-check after subscribe, `any` is valid in prod config |
| A2 | Medium | DoS | No rate limit on commands/subscribes; stream-wide backpressure punishes honest users |
| A3 | Medium | DoS | Unbounded hub control queue; no connect throttle or connection cap |
| A6 | Low | authz | `Ping` has no role check or rate limit (default is off) |
| R2 | Medium | availability | Hub command loop does Redis I/O inline, serial, unbounded queue |
| R7 | Medium | scalability | `SessionChecker` is sequential and extends the session TTL |
| R9 | Low | correctness | Registry listener not awaited, not retried |
| R10 | Low | correctness | `Connected` sent before registration |
| R11 | Low | correctness | One close code (4000) for every drop; reason lost when queue is full |
| R12 | Low | robustness | No drop guard for a cancelled socket task |
| R13 | Low | perf | Lifecycle events broadcast to every socket |
| R14 | Low | perf | `remove_connection_by_id` is O(N) under the write lock |
| R15 | Low | correctness | Tracker tick starves lifecycle events |
| R16 | Low | correctness | Topics ignored for targeted sends |
| R17 | Low | security | Host check trusts client headers |
| R18 | Low | compat | Unversioned Redis keys and channel; no environment prefix |
| R19 | Low | docs | Single-primary Redis assumed, undocumented |
| R20 | Low | cleanup | Dead code in hub registry/errors |
| R21 | Low | perf | Per-connection JSON/base64 serialization |
| R22 | Low | design | Raw `user_id` in commands |
| R23 | Low | resource leak | No TCP keepalive / user timeout on the ws listener |
| O2 | Low | logging | Info-level spans/logs on hot paths |
| O5 | Low | perf | Registry notices enqueue commands for users the instance does not hold |
| O6 | Low | protocol | Protocol gaps (no version/connection id, silent drops, OpenAPI) |
| C1 | Medium | client | No watchdog on `waiting` and `syncing` |
| C2 | Medium | client | Backoff resets on every open |
| C3 | Medium | client | A `failed` nack drops the pending command |
| C4 | Medium | client | A persistent checkpoint mismatch loops forever |
| C5 | Medium | client | Stale state after a reconnect; `replaced` never set |
| C6 | Low | client | Overlapping `Sync` tail records a bad hash |
| C7 | Low | client | Snapshot decode accepts trailing bytes |
| C8 | Low | client | `StreamSubscribeFailed` / `unsynced` retried every 5 s forever, no jitter |
| C9 | Low | docs | `net/` missing from the `client-web-architecture` skill |

---

## Stream log correctness

### L2. Persisted wakes are ignored: stale instances never reconcile (Low with one instance, Medium with several)

Open, analysed, no code changed. With one instance it cannot happen. With several, the clients of an instance that missed a wake silently stop receiving commands (no data loss: the log append identity check rejects stale appends). The instance's own worker sends each command to its clients on the inbox wake, before it is persisted; persistence is the backup, not the delivery path. After a missed wake nothing tells those clients they are behind (checkpoints follow only the instance's own persists, the web client has no watchdog, C1). They recover on the next command reaching that instance or on resubscribe: seconds on a busy stream, indefinitely on a quiet one.

**Today**
- A push publishes `<stream>` on `cmd-inbox-wake`; after a persist the instance publishes `<stream>@<version>` (`notify_persisted`, `repositories/command_inbox/redis/wake.rs`). Every instance with a worker for the stream hears both.
- The registry `listen` handler (`stream_registry.rs`) throws the version away: it only calls `Notify::notify_one` on the worker. A listener reconnect (`InboxWake::All`) wakes every worker the same way.
- A woken worker runs `process_inbox`: read the inbox after its `read_index`, version, submit to persistence. It never compares its head with the log. `PersistenceWork::Reconcile` is sent once, from `preflight` at start.
- Peers trim the inbox through the entries they stored. If C misses the wake, A persists and trims; C finds an empty inbox. The 10 s recovery poll runs the same `process_inbox`, so it does not help. The same hole exists after a reconnect.

Failure: C misses the wake of e11; A persists v11 and trims. C's subscribers never get v11; a new subscriber on C gets a `Sync` of stale state. When the next command reaches C it is versioned from the stale head and sent at the wrong version, its append fails (`Behind`), C reloads and resyncs: a wrong frame, then a correction.

Not causes: the `room == 0` early return in `StreamRelay::ingest` (`settle` notifies again) and a busy loop (`notify_one` keeps one permit).

**Proposed fix**
1. The worker keeps the highest persisted version heard (`AtomicI64::fetch_max`, set by the registry handler, `wake` carries it). In the relay loop, when it is above `confirmed_version` and nothing is in flight, send `PersistenceWork::Reconcile` (one cheap `head` read per peer persist). Compare after `settle`, since an instance's own announcement arrives before its state updates.
2. On `InboxWake::All`, flag every worker for a reconcile.
3. No periodic reconcile (a `head` read per stream per instance every 10 s costs more than it adds).

**Tests**: two workers on one inbox and log, A persists and trims, C gets only the persisted wake and its subscriber must get the commands; `All` after a peer persisted and trimmed gives a reconcile and a resync; a persisted version not above `confirmed_version` sends no `Reconcile`. Open question: whether a reconcile may be sent while a persistence is in flight (the proposal waits).

### L4. A corrupt load gives the worker up (Low)

Replay recomputes `command_hash` and checks the restored state hash against `snapshot.hash`; a mismatch is `CommandStreamError::Corrupt`. Not done: no fallback to an older snapshot (the worker gives up), and `try_store_snapshot` checks only the snapshot's `chain_hash`.

### L5. Worker replacement overlap and shutdown blind spot (Low)

`run()` calls `receiver.close()` before `drain()` and `is_finished()` is `messages.is_closed()` (`stream_worker.rs`), so the registry replaces the worker while it still drains. Two workers for one stream then run in one process; correctness relies on the CAS and hash checks. The replaced worker is gone from `State.streams`, so `shutdown` never waits for its drain and the process may exit with commands taken from the inbox but not persisted (they stay in Redis up to the 1 h TTL, no signal to the client). `drain()` also reads one inbox page only.

Fix: report finished only when both tasks ended; keep draining workers in a list that `shutdown` awaits; make `drain` loop until the page is short.

### L6. Inbox cursor never detects a regression (Low)

`ingest` skips every entry with `inbox_id <= stored_inbox` (`stream_relay.rs`), `read_index` is reset only on reload. If Redis ids move backwards (failover to a node with a slower clock, restored older dump, recreated stream), new commands are skipped silently until ids pass the stored value. Fix: compare the stream's newest id with the cursor and reset on regression, or alert on repeated skipped-only polls.

### L7. Inbox trim only after commands are stored (Low)

A malformed entry waits for a later valid store; a failed trim is only warned (`stream_persistence.rs`, `stream_log_service.rs`). The stored-position filter makes a missed trim harmless. Fix: trim on every successful pass.

### L8. `ensure_stream` writes on every worker load (Low)

`repositories/command_log/pg/stream.rs` is an upsert (`ON CONFLICT DO UPDATE SET id = cl_streams.id`): a dead tuple and a row write lock per reload, contending with append CAS. Fix: `DO NOTHING` plus a `SELECT` first, not on read-only loads.

### L9. Load/prune race (Low)

`StreamLogService::load` reads head, latest snapshot and tail in three queries. Two newer snapshots plus a prune in between give a tail above the snapshot, `apply_next` fails with `Gap` (`Corrupt`) and the worker gives up. Fix: one snapshot-isolated transaction.

### L10. No PG statement timeout (Low)

Only the append transaction sets `lock_timeout`/`idle_in_transaction` (`pg/append.rs`). A hung query stalls the persistence task, the optimistic tail fills to `relay_max_pending` and pushes go `Busy`; `drain` waits until the registry shutdown timeout. Fix: `statement_timeout` for the stream log connections.

### L12. Rolling deploys and command codec (Low)

An older binary decoding a command variant added by a newer one fails `to_envelope` with `Corrupt` and gives up repeatedly for that stream. Fix: version the command codec, gate new variants. Related: R18.

### L13. Grace timer can starve (Low)

The grace timer is the last arm of a `biased` select (`stream_worker.rs`). Fix: check the deadline at the top of each iteration.

### L14. Chat text is raw (Low)

Stored and relayed raw (`is_well_formed` checks length/emptiness only); clients must escape.

---

## Abuse and capacity

### A1. Access is by stream id only (Low)

Which streams may be opened is configured per kind (`streams.<kind>.access`, see [command-stream.md](../services/builder/command-stream.md#stream-access)); production allows a fixed list, so the number of streams is bounded and stream creation is not an abuse vector. What remains:

- Every authenticated user may read and write the allowed streams: no per-user rule (`StreamAccess::allows` already receives the user id).
- Authorization is not re-evaluated after subscribe (R7).
- `mode: any` is accepted in every stage; only the config layers keep it out of prod. A start-up check that rejects it for the `prod` stage would close that.

### A2. No rate limit on commands/subscribes; backpressure punishes honest users (Medium)

- `connect.rs` processes every text frame; no per-connection or per-user limit on commands, subscribes or unsubscribes.
- Backpressure is per stream, polled every 250 ms: between polls a connection can push thousands of entries past the 1024 high mark. Once blocked, every push waits 2 s then returns `Busy`, and `send_stream_command` drops the pusher's connection (`hub_service.rs`). One abuser flooding `chat:global` gets honest users disconnected.
- Each `StreamSubscribe` for an already-subscribed stream resets the cursor and causes a full `Sync` (up to ~400 KB for chat plus tail, base64). A few dozen bytes in, hundreds of KB out.

Fix: token bucket per connection (commands, subscribes), disconnect on repeated violation (close 1008); penalize the offender (per-user in-flight/quota), not whoever pushes next; coalesce repeated subscribes (at most one `Sync` per interval per connection).

### A3. Hub control queue unthrottled; no connect throttle or connection cap (Medium)

- `control_tx` is an `UnboundedSender` (`hub_service.rs`). Every subscribe/unsubscribe, `request_connection`, drop and registry-change notice enqueues a `ControlCommand`; invalid JSON and unsubscribes cost almost nothing to send. `StreamUnsubscribe` for any valid id takes the subscribers write lock and pushes into the unbounded `StreamDispatcher` queue.
- A user can reconnect in a tight loop: each connect costs an upgrade, two tasks, a channel, a Redis `SET`+`PUBLISH`, a registry-change round trip on every other node and broadcasts (R13). No resume point.

Failure: a client spams subscribe/unsubscribe or one account reconnects continuously; producers outrun the loop whenever it is slowed (R2); `control_rx` and the unbounded per-consumer queues grow until OOM.

Fix: per-connection token bucket (shared with A2); per-user connect-rate limit before the upgrade; per-instance connection cap; bounded control channel with `try_send` + drop-connection on overflow for client-originated commands (reserved lane for internal ones); jittered client backoff.

### A6. `Ping` has no role check or rate limit (Low while disabled)

`HubConfig::ping_enabled` now defaults to `false` and prod sets it false. Where it is enabled, any logged-in user may send `{"type":"ping","id":"<255 chars>"}` with `userId: <any uuid>` or broadcast to every connection (`connect.rs`, `hub_service.rs`), which can evict slow victims as `SlowConsumer` and push attacker-chosen strings to all clients. Fix: operator-only role, reject `userId` for non-admins, rate limit (A2/A3), or delete the feature.

---

## Availability and robustness

### R2. Hub command loop: inline Redis I/O, serial, no timeouts (Medium)

`ConnectUser`, `DisconnectUser`, `DropConnection`, the registry change heartbeat and others (`hub_service.rs`) await Redis from the one loop that serializes every state mutation. Pool checkout wait defaults to 30 s (`redis_connection.rs`, `max_size` 10) and the pool is shared with HTTP session lookups, the heartbeat and the session checker.

Failure: pool exhausted; each loop command can block up to 30 s, so connects, disconnects and slow-consumer drops queue (A3), stale sockets stay in the map and `Connections` broadcasts to dead channels.

Fix: a short hub-level timeout (1-2 s) around every registry call, a bounded queue, and either move registry writes off the loop (the in-memory map is authoritative locally; async write with retry) or give the hub a small dedicated Redis pool.

### R7. Session checker is sequential and extends the session TTL (Medium)

- `tasks/session_checker.rs`: `get_current_user` is awaited per tracked connection, one after another. N connections at latency L take N*L per tick (10k * 2 ms = 20 s; 100k exceeds the interval).
- While `on_tick` runs the `select!` in `connection_tracker.rs` does not drain the subscription, so lifecycle events pile up in the unbounded queue (R15).
- A revoked session keeps its socket for at least `session_check` (900 s in prod) plus tick time, and stream authorization is not re-evaluated.
- `get_current_user` also extends the session TTL (`crates/shine-infra/src/session/current_user_service.rs`); for a WebSocket-only user the checker is the only keepalive, so `session_check` must stay well below `sessionTtl` (1800 s).
- A warning is logged per connection per tick (O2).

Fix: bounded concurrency (`buffer_unordered(32)`) or a batched lookup; revoke by event (publish on logout). Cheaper check: a read-only existence check in `CurrentUserService` (`EXISTS` on the sentinel key, one pipeline per tick) without the data read or `EXPIRE`; keep the TTL refresh on the normal request path.

### R9. Registry listener (Low)

`hub_service.rs`: the listener is spawned and not awaited; an initial subscribe failure is final (logged, task exits), so the node misses cross-node takeover notifications and relies on the 300 s heartbeat; a reconnect does not recheck local users. Fix: await the subscribe, retry with backoff, recheck local users on reconnect.

### R10. `Connected` precedes registration (Low)

`connect.rs` sends `Connected` right after queueing `ConnectUser`. If `create_registry_connection` fails, the command is dropped, `tx` closes and the client sees `Connected` then an unexplained close. Fix: have the hub send `Connected` once registered, or a reasoned close on failure.

### R11. One close code for every drop; close reason lost (Low)

`hub_connections.rs` sends the final `UserDisconnected` with `try_send`; for `SlowConsumer` (queue full by definition) it always fails, so the client gets `Close(None)`. All reasons except `Shutdown` share code 4000, so a client cannot tell `Replaced` from an error (reconnect ping-pong between two tabs). Fix: carry the reason in a side channel (`oneshot`/watch); distinct 4xxx codes: replaced (do not reconnect), session invalid (re-authenticate), transient (back off). See C5.

### R12. No drop guard (Low)

A cancelled socket task (the `serve_socket` future dropped) leaves its entry registered (`connect.rs`). Fix: a guard that requests disconnection.

### R13. Lifecycle events fan out to every socket (Low)

`connect.rs` subscribes each socket to `TopicKey::Hub` but `event_to_wire_message` drops hub events; the socket only needs its own disconnect, already delivered by `RemovedConnection::notify`. Every connect/disconnect costs a read-lock scan and N `try_send`s and uses the same 1000-slot queue that decides `SlowConsumer` eviction; a connect storm is O(N^2). `UserConnected` also carries the session key. Fix: do not subscribe sockets to `TopicKey::Hub`; keep the session key out of the broadcast.

### R14. O(N) `remove_connection_by_id` (Low)

`hub_connections.rs` scans `users` for a connection id under the write lock, on every dead-connection prune. Drops are now deduplicated per connection, but each real removal is still a full scan; a mass eviction is O(N^2) with writers blocking all stream sends. Fix: store `user_id` in `Connection` (reverse map).

### R15. Tracker tick starves lifecycle events (Low)

`biased` polls the ticker first (`connection_tracker.rs`). Fix: drain events before ticking, or bound tick work.

### R16. Topics ignored for targeted sends (Low)

Only `broadcast` checks topics; sockets lack the `Stream` topic yet receive stream messages (`hub_connections.rs`). Fix: check the topic in targeted paths, or drop the topic concept for stream messages.

### R17. Host check trusts client headers (Low)

`connect.rs`; `crates/shine-infra/src/web/extracts/target_host.rs`. The Origin check is the real defence. Fix: trust forwarded headers only from the proxy; do not treat the host check as a control.

### R18. Unversioned Redis keys and channel (Low)

A mixed-version deploy makes instances drop each other's users (`redis_hub_registry.rs`, `redis_command_inbox_db.rs`); keys such as `hub-connection:` and `cmd-inbox:` have no environment prefix. Fix: version the key and channel, prefix by environment, treat an unknown format as foreign. Related: L12.

### R19. Single-primary Redis assumed, undocumented (Low)

`MGET` over many keys and a keyspace `SCAN` (`redis_hub_registry.rs`). Fix: document it, or make the calls cluster-safe and drop the `SCAN`.

### R20. Dead code (Low)

`list_connections` and its `SCAN` are test-only; `UserAlreadyConnected` is never built; `request_drop` has no `is_drop` assert (`hub_registry.rs`, `redis_hub_registry.rs`, `models/hub_error.rs`, `hub_service.rs`). Fix: delete or `cfg(test)`, assert.

### R21. Per-connection serialization (Low)

The shared message is base64 and JSON encoded once per connection (`routes/ws/message.rs`, `connect.rs`); CPU only. Fix: cache the encoded frame, only if profiling shows it.

### R22. Design decision: raw `user_id` in commands (Low)

Every command carries the sender's raw `user_id` (`routes/ws/message.rs`); user UUIDs are visible to all subscribers (relevant to A6). Decide, then document.

### R23. No TCP keepalive / user timeout on the ws listener (Low)

The socket write, ping and idle timeouts exist; the listener itself sets no TCP keepalive or user timeout, and the `sendDrain` abort has no test. Fix: set them on the accepted socket.

---

## Observability and logging

| ID | Finding | Where | Fix |
|---|---|---|---|
| O2 | **Info-level spans and logs on hot paths.** A per-command hub span with the user id, registry spans, and a session-checker warning per connection per tick. | `hub_service.rs`; `redis_hub_registry.rs`; `tasks/session_checker.rs` | `debug`; log once per tick with a count. |
| O5 | **Registry notices enqueue a command for users the instance does not hold.** | `redis_hub_connection_db.rs`; `hub_service.rs` | Filter in the listener. |
| O6 | **Protocol gaps.** No version or connection id in `connected`; bad JSON, binary frames and bad stream ids are dropped silently; no close after repeated invalid frames; OpenAPI documents only `OK`. | `routes/ws/message.rs`; `connect.rs` | Document the schema; consider a close after repeated refusals. |

---

## Web client and replica

| ID | Finding | Where | Fix |
|---|---|---|---|
| C1 | **No watchdog on `waiting` and `syncing`.** A lost `streamSync` strands the client; only `unsynced` has a retry timer, re-armed only by another frame. | `client/web/src/experiments/pixel-map/index.ts:98-103`; `crates/shine-game/src/replica/client_stream.rs:173-176` | Re-send the subscribe with backoff for any non-live status. |
| C2 | **Backoff resets on every open.** If the server accepts the upgrade and then closes, the client retries every second forever. | `client/web/src/engine/net/websocket-stream-hub.ts:129-131,166` | Reset after the first sync or a minimum open time. |
| C3 | **A `failed` nack drops the pending command**, although the server says it may have been queued. | `client_stream.rs:253-257` | Keep it pending and resend (the server does not dedup, so a possible repeat is accepted), or mark it unknown. |
| C4 | **A persistent checkpoint mismatch loops forever.** Every mismatch resubscribes with no counter. | `client_stream.rs:235-251` | A counter, then a move to `Unsynced`. |
| C5 | **Stale state after a reconnect.** The stream is never reset to `Waiting`; paints are not gated by the connection state; `replaced` is never set although the type has it. | `pixel-map/index.ts:46-51,164-184`; `stream-hub.ts:1`; `pixel-map-ui.ts:14` | Reset or resync on reconnect; gate paints; handle `replaced` (no reconnect; needs distinct close codes, R11). |
| C6 | **Overlapping `Sync` tail records a bad hash.** `on_sync` still calls `apply_wire` and ignores a skipped outcome. The server sends non-overlapping tails today. | `client_stream.rs:189-193,220,305-308` | Honour `ApplyOutcome::Skipped`. |
| C7 | **Snapshot decode accepts trailing bytes.** `decode_snapshot` uses `postcard::from_bytes`; state and chain hashes come from the same frame. | `crates/shine-game/src/replica/codec.rs:15-17,35-41`; `client_stream.rs:280-286` | Canonical check for snapshots; verify the chain hash against a checkpoint. |
| C8 | **`StreamSubscribeFailed` is retried every 5 s forever**, including permanent reasons, and an `unsynced` resubscribe has no jitter, so after a server-side give-up every client of an instance comes back at once; `new WebSocket` is unguarded. | `client_stream.rs:155-159`; `websocket-stream-hub.ts:50` | Stop on permanent reasons; jittered backoff; try/catch into the backoff path. |
| C9 | **`net/` is missing from the `client-web-architecture` skill's layout table.** | `.claude/skills/client-web-architecture/SKILL.md:14-25` | Add a row. |

---

## Tests

| Gap | Where |
|---|---|
| Stress tests are small (12 to 48 users) and run with heartbeat, session check and dispatcher idle at 3600 s, so scale, cascades, a command loop that ends early and Redis flush are untested. | `testing/hub/test_hub.rs`; `services/hub/tests/contention.rs` |
| Missing fault and race cases: listener failure, half-open or oversized frames, id regression; fixed sleeps and `tick * N` waits remain. | `services/hub/tests/fault.rs`; `contention.rs`; `lifecycle.rs`; `routes/ws/connect_test.rs`; `stream_worker_test.rs` |
| Mutation blind spots: the close-code test compares the constant to itself, no test drives `AppState` wiring, the `SEND_DRAIN_TIMEOUT` abort is untested. | `routes/ws/connect_test.rs:104-112`; `app_state.rs`; `connect.rs` |
| Rust, `shine-infra`: the prod TLS check is tested for rejections only; no test for accepting `sslmode=require` through the pool. Migrations under the `statement_timeout` need a database and have no test. | `crates/shine-infra/src/db/postgres/pg_connection.rs` |
| Rust, builder: `ConnectUser` registry failure (R10) and `ensure_stream` (L8). | `services/builder/src/services/command_stream/` |
| Rust, `shine-game` `client_stream`: mid-batch gap, a command that fails to decode, a checkpoint mismatch while `Waiting`, overlapping `Sync` tails, trailing snapshot bytes, `HASH_HISTORY` eviction at 256, a `failed` nack on a known pending command, a command folded into the snapshot but absent from the tail. | `crates/shine-game/tests/replica/client_stream.rs` |
| TypeScript: only two tests exist for the hub. Missing: stale live after reconnect, a hang in waiting, an immediate close after open with growing delay, `connect()` twice, the `unsynced` timer re-arming, `resync()` while closed or replaced. Nothing tests the pixel map experiment's stream handling. | `client/web/src/engine/net/websocket-stream-hub.test.ts` |
| `shine-test` panic hook aborts the whole run on a background-thread panic (a failing test then shows no summary, only exit code `0xffffffff`); a test that expects a spawned-task panic cannot exist. The fuzz CI job uses an unpinned nightly and an uncached `cargo install`. | `crates/shine-test/src/lib.rs`; `.github/workflows/fuzz_ci.yml` |

---

## Verified as sound

No action needed.

**Command stream**
- Inbox push is one atomic `MULTI` (XADD+EXPIRE+PUBLISH); stream keys and wake payloads contain only validated `StreamId` characters (no injection); `XRANGE` exclusive ranges, `MINID` trim and numeric `InboxId` ordering are right.
- Append: dense-run assumptions are guaranteed by the relay, index arithmetic is guarded (`pg/append.rs`), `ON CONFLICT DO NOTHING` + CAS on `(version, chain_hash)` serialize concurrent writers and roll back losing inserts; the chain hash binds command, user and inbox identity.
- Worker/registry lifecycle ordering holds up; the select loop's branches are cancellation safe and no std lock is held across an await.
- Reload resets `read_index` to the last stored inbox id, so entries sequenced optimistically but not persisted are re-read, not lost.

**Hub and websocket**
- Origin and Host: `Origin` is required, an empty allow-list denies everything, regexes are anchored; the session is authenticated at upgrade (`CheckedCurrentUser`, fingerprint-bound). Allow-lists fail closed.
- Size limits: `max_message_size` and `max_frame_size` are 32 KB; commands are re-checked against 16 KB after base64 decode, a canonical postcard re-encode check and per-kind well-formedness; ping id is capped at 255. Binary frames are ignored; postcard decode failures never panic.
- Identity: `user_id` and `connection_id` always come from the server-side session, never from the client payload.
- Map invariants: `users` and `connections` are mutated only on the loop; replacement removes the old entry before registering; stale disconnects are keyed by connection id and cannot remove a fresh reconnect; lock scopes in `Connections` contain no awaits and only non-blocking `try_send`.
- Ordering: `ConnectUser`, `SubscribeStream` and `DisconnectUser` are FIFO on one channel; a subscribe that lands after the connection is gone is undone.
- Slow consumers: egress to sockets is bounded with `try_send`; internal consumers are unbounded by design (A3, R7).
- Logging: `SessionKey` Debug is redacted; client payloads appear only at `trace`.

## Not verified

Redis pool checkout timeout (affects how long a `push` can park with Redis down), client-side handling of `Unsynced`/duplicate ids beyond what is listed above, and the `shine_infra` pub/sub listener internals beyond `redis_listener.rs`.
