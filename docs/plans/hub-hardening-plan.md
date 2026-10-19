# Hub Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (inline, in one session; no subagents). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the open findings (listed per task below) in the existing builder hub without changing its design: one live connection per user across instances, Redis as the ground truth, a single command loop as sole writer.

**Architecture:** Registry operations become atomic Lua scripts with hub-level timeouts; the command loop, its consumers and the socket get supervision, fairness, timeouts and input limits; shutdown closes sockets before touching Redis. The command streams (chat is one of them) are built and documented in `hub.md`; this plan does not change them.

**Tech Stack:** Rust 2021, axum (ws), tokio, redis-rs 1.x (`redis::Script`), TypeScript API tests.

**Spec:** `docs/services/builder/hub.md` (design, command streams, test conventions).

## Progress

| Task | State |
|---|---|
| H0 contract in `hub.md` | waits for H-D3 / H-D4 |
| H1 registry as Lua | open |
| H2 command loop, consumers | open |
| H3 socket hardening | open |
| H4 shutdown and startup guards | open |

The findings come from three review passes of the whole hub (a static read of code and tests; nothing was run to reproduce them, item 5 was confirmed by reading redis-rs 1.7.1). Each task lists its findings by number, with severity. Settled elsewhere: the session check is read-only and batched, the hub tests fail without a database on `CI`, the old chat and its findings (unbounded chat bursts, cursors, poison entries, unatomic append, topic-less sends) are gone with the move to command streams, and the fixed-sleep tests wait for tick counters.

**Checked and found clean** (do not re-review): no nested locks and no lock held across an await on another lock; connection ids are v4 UUIDs (no ABA between a stale drop and a reconnect); the command FIFO keeps a connect before a later disconnect; a stale registry-change notice is harmless (the handler re-reads the registry); the stream author comes from the session and topics are server-chosen; nothing sensitive is logged above trace; all expiry uses the Redis clock; a crashed instance's rows expire by TTL; every Redis key has a TTL or trim; concurrent connects of one user on two instances converge; topics are fixed at connect, so there is no subscribe race. A missing registry row is a drop by design (Redis is the ground truth), and the earlier suspicion that `transaction_async` leaves a pooled connection WATCHed was checked in redis-rs and is not an issue. There is no in-repo ws client except `tests/api-tests/builder/ws_connect.ts` and the stream engine, so the reconnect contract is a server concern.

## Global Constraints

**Process**
- The user reviews and commits. Never run git write commands (`git add`, `commit`, `push`, `stash`, branch or tag changes). Every task ends with a **Checkpoint**: stop, report what changed and the test output, wait. The user may ask for changes or renames at a checkpoint; apply them before the next task.
- Run inline in one session; no subagents.
- Before editing builder routes, services or repositories invoke `service-architecture`; before API tests `api-test-writing`; before running the service `local-development`.
- Re-read each hub/ws file right before editing it; the user edits these files between tasks. Never revert their changes.

**Style and logging**
- Comments say what, not how; concise; match the surrounding density. `tracing::*!` only. `trace`: payloads and text; `debug`: per-step and per-message spans; `info`: root-cause context in prod; `warn`: infrastructure errors that heal by retrying; `error`: bugs.
- `cargo fmt --all` before every checkpoint; `cargo clippy --workspace --all-targets` stays clean; after TS changes `cd tests && pnpm run format && pnpm run lint`.

**Behaviour rules**
- Registry key `hub-connection:<user>` holds the active connection id (TTL = 3 × `hubHeartbeatSeconds`). A missing row or another id means "not registered" and the connection is dropped at the next heartbeat. Nothing restores a row (Redis is the ground truth).
- Fail fast only on server errors (a hub task or the command loop dying unexpectedly): log at `error` and exit the process. Redis, Postgres and network errors never stop the process: retry with backoff and recover.
- Redis >= 6.2, `maxmemory-policy noeviction`, single-primary Redis (multi-key scripts, `MGET`, pipelines, the stream scripts).
- Not deployed yet: key and channel renames need no migration.

**Tests**
- `shine_test::test`; skip with `#[test(skip = "reason")]`, serialize with `#[test(serial = "key")]`. Connection strings: `shine_test::db::{get_redis_cns, get_pg_cns}` (with `CI` set a missing string fails the test). A panic in a background thread aborts the whole test run. Hub tests use the `serial = "hub-service"` group, the FIFO `barrier` and `wait_ticks` instead of sleeps (see *Testing* in `hub.md`).
- Run DB tests with the connection strings set and check there is no "skipping" line. Windows: PowerShell for env vars. Each registry test uses a unique key prefix.

## Review Focus

1. **Redis flush or failover with live clients**: every connection is dropped at the next heartbeat (by design) with the reconnectable code. Test: Task H1 `a_missing_row_is_reported_missing_not_other`, Task H4 `redis_flush_drops_every_connection_with_the_transient_code`.
2. **Takeover race** (two instances write the row at once): no error, the last writer wins, the loser drops. Test: Task H1 `a_concurrent_takeover_is_never_an_error`.
3. **Half-open or silent socket**: closed by the idle and write timeouts, never pinned forever. Tests: Task H3 `a_silent_client_is_closed_after_the_idle_timeout`, `a_client_that_does_not_read_is_closed_after_the_write_timeout`.
4. **Reconnect storm from one user** (focus switching): throttled before the upgrade, no task or row leak. Test: Task H3 `a_user_reconnecting_too_fast_is_throttled`.
5. **Redis stall during shutdown**: every socket still gets its close frame, the process exits within the budget. Test: Task H4 `shutdown_during_a_redis_outage_still_closes_sockets`.

---

## Decisions

| # | Decision | Status |
|---|---|---|
| H-D1 | Registry keys and channel are prefixed and versioned: `{prefix}:v1:conn:<user>`, `{prefix}:v1:registry-changed`, default prefix `hub`. | decided (no rollout concern) |
| H-D2 | The registry check reports `Active`, `Missing` or `Other`; the two drop causes get distinct reasons so a client can tell a takeover from a lost row. | decided |
| H-D3 | Close codes: 4001 replaced / registry takeover (stay down until the tab is active again), 4002 session invalid (re-authenticate), 4003 registry row missing or registration failure (reconnect with jitter), 1013 slow consumer or dead channel, 1001 shutdown. | proposed, confirm before Task H3 |
| H-D4 | Frames: first server frame `hello{version:1, connectionId}`; `error{reason}` with reasons `invalid`, `too_long`, `rate_limited` for input the socket cannot parse or accept (stream commands keep their own `streamNack`); 10 consecutive refused frames close with 1008. Limits: frame 8 KiB, token bucket 5 burst + 1 per second per connection, write timeout 10 s, ping every 30 s, idle timeout 90 s, per-user connect throttle 500 ms (429), instance cap 20 000 connections (503). All configurable under `builder.ws`. | proposed, confirm before Task H3 |
| H-D5 | The registry write stays inline in the command loop; the loop calls it through one function so it can move off the loop later. Connect throughput is not optimized now. | decided (postponed) |
| H-D6 | The control queue is bounded at 10 000; a full queue refuses new connects (`HubError::Overloaded`, 503). Internal producers use `try_send` and rely on their next tick. | decided |

## Milestones

| M | Slice | Task | Independent test | Depends on |
|---|---|---|---|---|
| H1 | Atomic registry | H1 | Redis unit + fault tests | - |
| H2 | Command loop, consumers, supervision | H2 | unit + lifecycle tests | - |
| H3a | Socket lifecycle: guard, timeouts, sockets off the Hub topic | H3 steps 1-3 | `connect_test.rs` | - |
| H3b | Socket input: limits, frames, throttle, cap | H3 steps 4-5 | rate limiter + connect tests | H3a |
| H3c | Close-code classes | H3 steps 6-7 | code table + TS tests | H1, H3a |
| H4 | Shutdown and startup guards | H4 | lifecycle + fault tests | H1, H2 |

---

## Task H0: Document the proposed contract (after confirming H-D3 and H-D4)

**Files:** Modify `docs/services/builder/hub.md`.

- [ ] **Step 1:** In *Drop reasons and close codes* replace the 4000 column with the H-D3 classes and add the `Missing`/`Other` split to the heartbeat description.
- [ ] **Step 2:** Describe the socket frames (`hello`, `error`), the limits and timeouts of H-D4; remove "(and Hub-topic connections)" from the `HubEvent` row; extend *Client contract* with the close-code classes and the jittered reconnect.
- [ ] **Step 3: Checkpoint.**

## Task H1: Registry as Lua

**Findings.**
- **2 (High)** The command loop makes serial Redis round trips (about 2 per connect, 6 per disconnect: PING on checkout, WATCH, GET, MULTI/DEL/EXEC, UNWATCH, PUBLISH) with no hub-level timeout, and the registry shares the default 10-connection pool (30 s pool timeout) with the stream logs. The heartbeat is one unchunked `MGET` plus a pipeline of N `EXPIRE`s, which the 500 ms default response timeout fails at large N. Fix: a dedicated pool, Lua compare-and-set (one round trip), a chunked heartbeat.
- **5 (High)** WATCH/MULTI destructures `(usize,)` with `?`: an aborted EXEC (nil) becomes `Err` instead of a retry (the closure always returns `Ok(Some(..))`, so `transaction_async` never retries), so takeover detection can be skipped and two connections stay live until the next heartbeat; `process_registry_change` only logs the error. `remove_connection_if_active` has the same flaw. Fix: Lua compare-and-expire and compare-and-delete.
- **18 (Med)** A Redis stall blocks the single command loop: the response timeout is the redis-rs default of 500 ms unless `timeout` is set, and the 30 s `pool_timeout` bounds checkout in a hard outage. Fix: a 1-2 s hub-level timeout per registry operation, explicit `timeout`/`pool_timeout` in the production connection string. The Lua rewrite (5) comes first: a timeout cancels a WATCH sequence mid-flight.
- **27 (Low)** Unchecked numeric config: a huge heartbeat wraps to a negative TTL (`EXPIRE` with a negative TTL deletes the key; `SET EX` rejects it), huge intervals may overflow tokio `Instant` arithmetic. Fix: clamp and validate at startup.
- **35 (Info)** The registry keys and channel have no per-environment prefix, so a shared Redis means cross-environment takeover. Fix: H-D1. (Stream messages carry the author's `userId` by design.)
- **47 (Low)** No format versioning of keys, values or channel: a mixed-version deploy makes instances drop each other's users (an unparsable value is "not mine"). Fix: H-D1; treat an unknown format as foreign and skip it.
- **45 (Med, remaining)** Hub-level timeouts are unsafe with the current WATCH code until the Lua rewrite; a dropped `listen` can leave a subscribed channel with no handler (`redis_listener.rs`, subscribe before the handler is inserted).

**Files:**
- Create: `services/builder/src/repositories/hub_registry/redis/scripts.rs`
- Modify: `repositories/hub_registry/redis/redis_hub_registry.rs`, `redis_hub_connection_db.rs`, `repositories/hub_registry/hub_registry.rs`, `services/hub/hub_service.rs`, `app_state.rs`, `app_config.rs`, `server_config.*.json`
- Test: `services/hub/tests/lifecycle.rs`, `tests/fault.rs`, unit tests next to the repository

**Interfaces:**
- Produces: `enum RegistryCheck { Active, Missing, Other }`; `HubRegistry::heartbeat_connection(user, conn) -> Result<RegistryCheck, HubError>`; `HubRegistry::heartbeat_connections(&[HubConnection]) -> Result<Vec<(HubConnection, RegistryCheck)>, HubError>` (only non-`Active`); `remove_connection_if_active` deletes and publishes in one script; `RedisHubConnectionDb::new(&pool, ttl_seconds: u64, prefix: &str)`; `const REGISTRY_TIMEOUT: Duration = 2 s`.

- [ ] **Step 1: Failing tests.** `remove_only_deletes_an_entry_owned_by_the_connection`; `a_concurrent_takeover_is_never_an_error` (50 interleaved `create_connection`/`heartbeat_connection`: no `Err`, the final row is the last writer); `batch_heartbeat_chunks_cover_the_edges` (255, 256, 257 connections); `a_missing_row_is_reported_missing_not_other`; `registry_keys_are_prefixed_and_versioned`; `config_is_clamped` (heartbeat 0 -> 1, `u64::MAX` -> 86 400 s, TTL never negative); `a_registry_operation_times_out_in_two_seconds` (fault test through toxiproxy).
- [ ] **Step 2:** Three `redis::Script` statics in `scripts.rs`: `HEARTBEAT` (0 missing / 1 refreshed / 2 other), `HEARTBEAT_MANY` (keys + ids, array of the same codes), `REMOVE_IF` (compare, `DEL`, `PUBLISH`, returns 0/1). Remove every WATCH/MULTI use and `transaction_async`.
- [ ] **Step 3:** Chunk the batch at 256 keys per call. Wrap every registry call of the hub in `tokio::time::timeout(REGISTRY_TIMEOUT)` (a timeout is a Redis error and follows the failure table of `hub.md`). Give the registry its own pool from `AppState::new`. Add `builder.hub.prefix` (default `hub`). Clamp `hubHeartbeatSeconds` to `1..=86_400`, compute the TTL with `saturating_mul`.
- [ ] **Step 4:** Delete `list_connections` and `find_redis_keys` (or move them under `#[cfg(test)]` if a test needs them).
- [ ] **Step 5: Verify.** Run `cargo test -p shine-builder hub` with the env set. Expected: all pass, clippy clean. **Step 6: Checkpoint.**

## Task H2: Command loop, consumers, supervision

**Findings.**
- **6 (High)** The tracker loop is `biased` towards the ticker: a tick that lasts at least one interval is always due again (`MissedTickBehavior::Delay`), so lifecycle events are never read. Realistic for the heartbeat at scale (the session check is batched). Fix: drain events before ticking, or bound the tick work.
- **9 (Med), 25 (Low)** The registry listener task is not supervised and subscribes inside a spawned task nothing awaits: a failed first `listen` is logged and never retried, and a takeover right after startup can be missed. A pub/sub reconnect (`None`) is ignored, so notices lost in the gap wait for the next heartbeat. Fix: await the first subscribe in `new` with retry, recheck all local users on reconnect.
- **13 (Med), 17 (High)** The command loop and the consumers (heartbeat, session check, stream dispatcher) are unsupervised: handles are only aborted when the loop ends. A panic leaves the hub dead or a consumer silent while `HubStatus` looks healthy (a dead session checker keeps revoked sessions on their sockets). After a loop panic `send_control` fails and new sockets are closed. Fix: supervise, fail fast, expose liveness.
- **22 (Med)** Wake-driven ticks have no minimum spacing. The chat dispatcher, the only user of the wake, is gone, so the wake support of the tracker has no production caller: remove it with its tests, or keep the 50 ms spacing if a consumer needs it again.
- **26 (Low)** A hub dropped without `Shutdown` leaks its tasks (they hold `HubService` clones) and listeners; every test hub does. Fix: a weak handle in the loop.
- **32 (Low)** Registry per-event spans are `info`, against the log-level rules. Fix: `debug`.
- **33 (Low)** Metric gaps: control-queue depth (only the `hub_command_queue_delay` histogram), failed connects, registry and heartbeat errors, subscriber prunes, no `outcome` label on `hub_commands`; `HubStatus` has the tick counters but no liveness or queue depth.
- **39 (Med)** `remove_connection_by_id` is an O(N) scan under the write lock (k drops over N connections cost k × N and block all readers). Fix: store `user_id` per connection and a reverse index.
- **43 (Med)** Every cluster-wide connect/disconnect from another instance makes this instance enqueue a command and an `info` root span, even for users it does not hold. Fix: pre-filter in the listener with a cheap local lookup, `debug` for notice-only spans.
- **44 (Med)** Backpressure is applied inconsistently: `RemovedConnection::notify` bypasses the metrics helper (no log or metric on a lost close notice), lifecycle noise shares the 1000 slots, a full connection gets a duplicate `DropConnection` from every producer, and `Subscribers` and the control queue have no depth gauge. Fix: route `notify` through `try_send`, dedupe in-flight drops, document the matrix.
- **50 (Low)** Dead code and API misuse: `list_connections` (no callers) and `find_redis_keys` (whole-keyspace `SCAN`) are reachable only from tests, `HubError::UserAlreadyConnected` is never constructed, `request_drop` accepts `DisconnectReason::Client`, and a second `HubService` on one pool loses takeover (`AlreadyListening` only logged). Fix: delete or `#[cfg(test)]` the scan, remove the variant, assert `reason.is_drop()`, make `new` return the listener error.

**Files:** Modify `services/hub/hub_service.rs`, `hub_connections.rs`, `hub_command.rs`, `hub_metrics.rs`, `hub_status.rs`, `tasks/connection_tracker.rs`, `tasks/heartbeat.rs`, `models/hub_error.rs`, `app_state.rs`. Tests: `hub_connections_test.rs`, `tasks/connection_tracker_test.rs`, `tests/lifecycle.rs`, `tests/fault.rs`.

**Interfaces:**
- Produces: `HubService::new(..., fatal: Arc<dyn Fn(&str) + Send + Sync>) -> Result<HubService, HubError>`; `ControlCommand::ResyncRegistry`; `HubError::Overloaded`; `HubStatus { connections, subscribers, running: bool, pending_commands: usize, stopped_tasks: Vec<String>, .. }` (the tick counters stay); control queue bounded at `CONTROL_QUEUE = 10_000` (H-D6); `Connections` stores `user_id` per connection plus a `connection_id -> user_id` index so `remove_connection_by_id` is O(1).

- [ ] **Step 1: Failing tests.** `ticks_longer_than_the_interval_still_apply_lifecycle_events`; `a_stopped_consumer_calls_fatal_and_marks_the_status` (a consumer that returns early; real panics abort the test run); `a_redis_error_in_a_consumer_does_not_call_fatal`; `a_connect_after_the_listener_started_takes_over`; `a_listener_reconnect_resyncs_local_connections` (`ResyncRegistry` drops only the connections the registry no longer holds); `remove_by_id_is_constant_time` (10 000 connections, 1 000 removals under 50 ms); `notices_for_unknown_users_create_no_command`; `a_full_control_queue_refuses_new_connects`; `duplicate_drop_requests_for_a_full_connection_are_deduplicated`; `the_close_notice_goes_through_the_metrics_helper`.
- [ ] **Step 2:** Tracker loop: apply pending lifecycle events (at most 1 024 per round) before a due tick. Remove the unused wake support (or give it `WAKE_SPACING = 50 ms` if kept).
- [ ] **Step 3:** Supervision: keep the consumer handles in a `JoinSet`; when one ends unexpectedly while the hub runs, set `running = false`, record its name in `stopped_tasks`, log `error!` and call `fatal`. The same for the command loop task. Redis errors inside a consumer are logged and retried on the next tick; they never reach `fatal`.
- [ ] **Step 4:** Registry listener: await the first successful `listen` inside `new` (retry with backoff 200 ms to 10 s); a pub/sub reconnect (`None`) enqueues `ResyncRegistry`; `AlreadyListening` is `new`'s error; release the listener in `shutdown`. The listener drops notices for users not held locally with no command and no span. Registry and per-command spans are `debug`.
- [ ] **Step 5:** Metrics: gauge `hub_control_queue_depth`, counters for rejected connects, supervisor stops and registry failures, an `outcome` label on `hub_commands`. `RemovedConnection::notify` uses the metrics helper. Dedupe `DropConnection` per connection while one is in flight. Remove `HubError::UserAlreadyConnected`; `request_drop` asserts `reason.is_drop()`.
- [ ] **Step 6: Verify** as in H1. **Step 7: Checkpoint.**

## Task H3: Socket hardening and the client contract

**Findings.**
- **1 (High)** Every socket subscribes to the Hub topic, so each connect and disconnect is pushed to every socket's 1000-slot channel: O(N²), cascading into slow-consumer drops (each drop publishes another `UserDisconnected`). Sockets do not need the events (`RemovedConnection::notify` already tells a socket its own end). Fix: register sockets on no topic.
- **12 (Med)** `UserConnected` carries the `SessionKey` and is broadcast to every Hub-topic socket; not on the wire today, a latent leak. Fix: sockets off the Hub topic (1), and keep the key out of any broadcast payload.
- **4 (High, remaining)** Nothing overrides the tungstenite default `max_message_size` (64 MiB), and there is no per-connection rate limit on frames (a command is limited by `maxCommandBytes` only after it is read). Fix: `maxFrameBytes`, a token bucket.
- **8 (Med), 38 (Med)** `forward_egress` logs send errors and keeps looping; there is no server ping or idle timeout, and `ws_sender.send` has no timeout, so a half-open or non-reading client pins its socket, two tasks and up to 1000 queued frames for hours. `SEND_DRAIN_TIMEOUT` covers only the client-close path. Fix: break on a send error, a write timeout, ping and a read-idle timeout.
- **19 (Med)** No per-user or per-IP connect throttle, though every tab focus switch is a disconnect plus a connect; each reconnect costs an upgrade, two tasks, a channel, a registry SET+PUBLISH, a `Replaced` drop and events. Fix: throttle before `ws.on_upgrade`, bound the control queue (H2).
- **20 (Med)** No drop guard around the connection lifetime: the two socket tasks are detached `spawn`s and the receiver lives in the send task, so if `serve_socket` is cancelled the entry stays registered until the channel fills or the heartbeat notices. Fix: a guard that requests the disconnection.
- **21 (Med, remaining)** A frame that does not parse is dropped silently and never disconnects (a failed command already gets `streamNack failed`). Fix: `error` frames for refused input and a close after repeated invalid frames.
- **28 (Low)** The host allow-list trusts the client-sent `X-Forwarded-Host`/`Forwarded`; the Origin check is the real cross-site WebSocket hijacking defence. Fix: document that the host check is not a security control (or trust the headers only from the proxy).
- **30 (Low)** A registration failure gives the client a plain close with no code, and "Connected" is logged before the hub registers anything; a registry write that lands but whose read-back fails leaves the registry naming a discarded connection. Fix: close with 1013, log "Connected" from the command loop.
- **36 (High)** Every hub drop uses close code 4000 with only a free-text reason; a registration failure has no code, and a slow-consumer drop is never delivered (the channel is full). The client must tell a takeover (stay down) from a lost registry row after a Redis flush (reconnect with backoff), and both are `heartbeat_inactive`/4000 today. Fix: H-D3 close codes, split the reasons, extend the *Client contract* of `hub.md`, update `HUB_DROPPED_CLOSE_CODE` in `ws_connect.ts`.
- **37 (High)** A reconnect storm after a deploy or LB flip is amplified: jittered backoff is only a documented client rule, there is no connection cap, shutdown closes sockets sequentially with no staggering. (A resume position now exists for streams: `streamSubscribe` with `resume`.) Fix: a per-instance connection cap (503/1013), jittered backoff in the contract, a staggered 1001 on shutdown (H4).
- **48 (Low)** The socket protocol has no version or hello, no error frames for refused input, binary frames are ignored without a metric, and the utoipa path documents only `OK`. Fix: `hello{version, connectionId}`, `error` frames, document the schema.

**Files:**
- Create: `services/builder/src/routes/ws/rate_limiter.rs`
- Modify: `routes/ws/connect.rs`, `routes/ws/stream_socket.rs`, `routes/ws/message.rs`, `services/hub/hub_service.rs`, `models/messages/hub.rs`, `app_config.rs`, `tests/api-tests/builder/ws_connect.ts`
- Test: `routes/ws/connect_test.rs`, `services/hub/tests/lifecycle.rs`, `rate_limiter.rs`

**Interfaces:**
- Produces: `fn close_code(reason: &DisconnectReason) -> u16` per H-D3; `DisconnectReason::RegistryMissing` (label `registry_missing`); `struct TokenBucket { fn new(burst: u32, per_second: u32) -> Self; fn try_take(&mut self, now: Instant) -> bool }`; `struct ConnectionGuard` (drop requests a disconnection); frames `hello`, `error`; config `builder.ws.{maxConnections, maxFrameBytes, connectThrottleMs}`.

- [ ] **Step 1 (H3a):** Failing tests `a_socket_task_that_is_cancelled_still_disconnects`; `a_silent_client_is_closed_after_the_idle_timeout` (paused clock, 90 s); `a_client_that_does_not_read_is_closed_after_the_write_timeout`; `the_server_pings_every_30_seconds`; `sockets_do_not_receive_hub_events` (a connected socket never gets another user's `UserConnected`; the session key no longer reaches any socket).
- [ ] **Step 2 (H3a):** Sockets register on no topic (their own end notice already arrives through `RemovedConnection::notify`). Add `ConnectionGuard`. Egress: `WRITE_TIMEOUT = 10 s` per send, a failed or timed-out send ends the socket, ping every 30 s, read-idle timeout 90 s; keep `SEND_DRAIN_TIMEOUT`. Move the "Connected" log into the command loop after the registry write.
- [ ] **Step 3 (H3a):** Verify `cargo test -p shine-builder routes::ws`. Checkpoint.
- [ ] **Step 4 (H3b):** Failing tests `the_first_frame_is_hello` (`version` 1, `connectionId` equals the registry value); `oversized_frames_are_refused` (> 8 KiB -> `error too_long`); `ten_consecutive_refused_frames_close_with_1008`; `a_client_faster_than_the_rate_limit_gets_rate_limited`; `a_user_reconnecting_too_fast_is_throttled` (429 before the upgrade); `connects_above_the_instance_cap_get_503`; token bucket unit tests `burst_then_refill`.
- [ ] **Step 5 (H3b):** Pre-upgrade guards (per-user throttle, instance cap) before `ws.on_upgrade`; `max_message_size`/`max_frame_size` = `maxFrameBytes`; ingress token bucket; `error` frames for refused input; the counter of consecutive refused frames closes with 1008. Checkpoint.
- [ ] **Step 6 (H3c):** Failing tests `every_drop_reason_maps_to_its_close_code` (replaced 4001, registry_inactive 4001, registry_missing 4003, session_invalid 4002, slow_consumer/dead_channel 1013, shutdown 1001); `registration_failure_closes_with_1013`.
- [ ] **Step 7 (H3c):** `Other` -> `registry_inactive`/4001, `Missing` -> `registry_missing`/4003 on both the heartbeat and the registry-change path. Update the close-code constants in `ws_connect.ts` and run `cd tests && pnpm test:local` with the service up. Add to `hub.md`: the host allow-list is not a security control (Origin is the CSWSH defence). Checkpoint.

## Task H4: Shutdown and startup guards

**Findings.**
- **11 (Med)** Shutdown removes connections one at a time with a Redis call each, and the shutdown hook only enqueues `request_shutdown` (hooks are synchronous, nothing awaits completion), after which the loop aborts the background tasks. A socket is notified just before its own registry removal, so later sockets wait behind earlier Redis calls and may miss the 1001 frame, and rows can be left behind. Fix: close the sockets first, clean the registry in chunks, await with a timeout.
- **41 (Med)** The stream log and the registry need Redis >= 6.2 (exclusive `XRANGE (id`), and `noeviction`; nothing probes the version at startup. Fix: probe at startup, fail fast below 6.2, warn on another eviction policy.
- **42 (Med)** `MGET` over many keys, the `EXPIRE` pipeline, `SCAN`, the session check's multi-key `EXISTS` and the stream scripts are not Redis Cluster compatible (plain `redis::Client`). Fix: document "single-primary Redis required" (done in `hub.md`) or make the calls per-key.

**Files:** Modify `services/hub/hub_service.rs`, `crates/shine-infra/src/web/web_app.rs` (shutdown hook), `services/builder/src/main.rs`, `app_state.rs`, `docs/services/builder/hub.md`. Tests: `tests/lifecycle.rs`, `tests/fault.rs`.

**Interfaces:** `HubService::wait_stopped() -> impl Future<Output = ()>`; `async fn check_redis(pool: &RedisConnectionPool) -> Result<(), AnyError>` (version >= 6.2; warns when `maxmemory-policy` is not `noeviction`).

- [ ] **Step 1: Failing tests.** `a_shutdown_of_many_connections_closes_every_socket_before_touching_redis` (300 connections); `shutdown_cleans_the_registry_within_ten_seconds`; `shutdown_during_a_redis_outage_still_closes_sockets` (toxiproxy); `redis_older_than_6_2_fails_the_start` (unit test on the version parser); `redis_flush_drops_every_connection_with_the_transient_code`.
- [ ] **Step 2:** Shutdown: first notify and drop every connection, then remove the registry rows concurrently (chunks of 256, 32 in flight) under a 10 s budget; `wait_stopped` resolves when the loop has finished. If `web_app.rs` hooks cannot await, add an async hook variant awaited with a 10 s deadline and use it from `main.rs`.
- [ ] **Step 3:** `check_redis` at startup: parse `INFO server` `redis_version`, fail below 6.2; `CONFIG GET maxmemory-policy` -> warn when not `noeviction` (ignore a denied command). `hub.md` *Failure behaviour* already states the single-primary and `noeviction` requirement; confirm it matches.
- [ ] **Step 4: Verify. Step 5: Checkpoint.**

## Final verification

- [ ] `cargo fmt --all --check`; `cargo clippy --workspace --all-targets`; `CI=true cargo test --workspace` with the connection strings set; `cd tests && pnpm test:local` with the service running. Expected: all green, no "skipping" lines.
- [ ] Sync `docs/services/builder/hub.md` with the code (the `shine-skills:sdp` Sync). Checkpoint.
