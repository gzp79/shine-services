# Hub Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (inline, in one session; no subagents). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the open findings of `docs/plans/hub_issues.md` in the existing builder hub without changing its design: one live connection per user across instances, Redis as the ground truth, a single command loop as sole writer.

**Architecture:** Registry operations become atomic Lua scripts with hub-level timeouts; the command loop, its consumers and the socket get supervision, fairness, timeouts and input limits; shutdown closes sockets before touching Redis. The stream platform is a separate plan (`docs/plans/command-streams-impl-plan.md`); the two are independent until the chat switch.

**Tech Stack:** Rust 2021, axum (ws), tokio, redis-rs 1.x (`redis::Script`), TypeScript API tests.

**Spec:** `docs/services/builder/hub.md` (design), `docs/plans/hub_issues.md` (findings).

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
- Redis >= 6.2, `maxmemory-policy noeviction`, single-primary Redis (multi-key scripts, `MGET`, pipelines).
- Not deployed yet: key and channel renames need no migration.

**Tests**
- `shine_test::test`; skip with `#[test(skip = "reason")]`, serialize with `#[test(serial = "key")]`. Connection strings: `shine_test::db::get_redis_cns` (with `CI` set a missing string fails the test). A panic in a background thread aborts the whole test run.
- Run DB tests with `SHINE_TEST_REDIS_CNS` set and check there is no "skipping" line. Windows: PowerShell for env vars. Each registry test uses a unique key prefix.

## Review Focus

1. **Redis flush or failover with live clients**: every connection is dropped at the next heartbeat (by design) with the reconnectable code. Test: Task H1 `a_missing_row_is_reported_missing_not_other`, Task H3 `redis_flush_drops_every_connection_with_the_transient_code`.
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
| H-D4 | Frames: first server frame `hello{version:1, connectionId}`; `error{reason}` with reasons `invalid`, `too_long`, `rate_limited`; 10 consecutive refused frames close with 1008. Limits: frame 8 KiB, token bucket 5 burst + 1 per second per connection, write timeout 10 s, ping every 30 s, idle timeout 90 s, per-user connect throttle 500 ms (429), instance cap 20 000 connections (503). All configurable under `builder.ws`. | proposed, confirm before Task H3 |
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

## Task H1: Registry as Lua (items 2, 5, 18, 27, 35, 47)

**Files:**
- Create: `services/builder/src/repositories/hub_registry/redis/scripts.rs`
- Modify: `repositories/hub_registry/redis/redis_hub_registry.rs`, `redis_hub_connection_db.rs`, `repositories/hub_registry/hub_registry.rs`, `services/hub/hub_service.rs`, `app_state.rs`, `app_config.rs`, `server_config.*.json`
- Test: `services/hub/tests/lifecycle.rs`, `tests/fault.rs`, unit tests next to the repository

**Interfaces:**
- Produces: `enum RegistryCheck { Active, Missing, Other }`; `HubRegistry::heartbeat_connection(user, conn) -> Result<RegistryCheck, HubError>`; `HubRegistry::heartbeat_connections(&[HubConnection]) -> Result<Vec<(HubConnection, RegistryCheck)>, HubError>` (only non-`Active`); `remove_connection_if_active` deletes and publishes in one script; `RedisHubConnectionDb::new(&pool, ttl_seconds: u64, prefix: &str)`; `const REGISTRY_TIMEOUT: Duration = 2 s`.

- [ ] **Step 1: Failing tests.** `remove_only_deletes_an_entry_owned_by_the_connection`; `a_concurrent_takeover_is_never_an_error` (50 interleaved `create_connection`/`heartbeat_connection`: no `Err`, the final row is the last writer); `batch_heartbeat_chunks_cover_the_edges` (255, 256, 257 connections); `a_missing_row_is_reported_missing_not_other`; `registry_keys_are_prefixed_and_versioned`; `config_is_clamped` (heartbeat 0 -> 1, `u64::MAX` -> 86 400 s, TTL never negative); `a_registry_operation_times_out_in_two_seconds` (fault test through toxiproxy).
- [ ] **Step 2:** Three `redis::Script` statics in `scripts.rs`: `HEARTBEAT` (0 missing / 1 refreshed / 2 other), `HEARTBEAT_MANY` (keys + ids, array of the same codes), `REMOVE_IF` (compare, `DEL`, `PUBLISH`, returns 0/1). Remove every WATCH/MULTI use and `transaction_async`.
- [ ] **Step 3:** Chunk the batch at 256 keys per call. Wrap every registry call of the hub in `tokio::time::timeout(REGISTRY_TIMEOUT)` (a timeout is a Redis error and follows the existing failure table). Give the registry its own pool from `AppState::new`. Add `builder.hub.prefix` (default `hub`). Clamp `hubHeartbeatSeconds` to `1..=86_400`, compute the TTL with `saturating_mul`.
- [ ] **Step 4:** Delete `list_connections` and `find_redis_keys` (or move them under `#[cfg(test)]` if a test needs them).
- [ ] **Step 5: Verify.** Run `cargo test -p shine-builder hub` with the env set. Expected: all pass, clippy clean. **Step 6: Checkpoint.**

## Task H2: Command loop, consumers, supervision (items 6, 9, 13, 17, 22, 25, 26, 32, 33, 39, 43, 44, 50)

**Files:** Modify `services/hub/hub_service.rs`, `hub_connections.rs`, `hub_command.rs`, `hub_metrics.rs`, `hub_status.rs`, `tasks/connection_tracker.rs`, `tasks/heartbeat.rs`, `models/hub_error.rs`, `app_state.rs`. Tests: `hub_connections_test.rs`, `tasks/connection_tracker_test.rs`, `tests/lifecycle.rs`, `tests/fault.rs`.

**Interfaces:**
- Produces: `HubService::new(..., fatal: Arc<dyn Fn(&str) + Send + Sync>) -> Result<HubService, HubError>`; `ControlCommand::ResyncRegistry`; `HubError::Overloaded`; `HubStatus { connections, subscribers, running: bool, pending_commands: usize, stopped_tasks: Vec<String> }`; control queue bounded at `CONTROL_QUEUE = 10_000` (H-D6); `Connections` stores `user_id` per connection plus a `connection_id -> user_id` index so `remove_connection_by_id` is O(1).

- [ ] **Step 1: Failing tests.** `ticks_longer_than_the_interval_still_apply_lifecycle_events`; `wake_ticks_are_spaced_at_least_50_ms`; `a_stopped_consumer_calls_fatal_and_marks_the_status` (a consumer that returns early; real panics abort the test run); `a_redis_error_in_a_consumer_does_not_call_fatal`; `a_connect_after_the_listener_started_takes_over`; `a_listener_reconnect_resyncs_local_connections` (`ResyncRegistry` drops only the connections the registry no longer holds); `remove_by_id_is_constant_time` (10 000 connections, 1 000 removals under 50 ms); `notices_for_unknown_users_create_no_command`; `a_full_control_queue_refuses_new_connects`; `duplicate_drop_requests_for_a_full_connection_are_deduplicated`; `the_close_notice_goes_through_the_metrics_helper`.
- [ ] **Step 2:** Tracker loop: apply pending lifecycle events (at most 1 024 per round) before a due tick; `WAKE_SPACING = 50 ms` between wake-driven ticks.
- [ ] **Step 3:** Supervision: keep the consumer handles in a `JoinSet`; when one ends unexpectedly while the hub runs, set `running = false`, record its name in `stopped_tasks`, log `error!` and call `fatal`. The same for the command loop task. Redis errors inside a consumer are logged and retried on the next tick; they never reach `fatal`.
- [ ] **Step 4:** Registry listener: await the first successful `listen` inside `new` (retry with backoff 200 ms to 10 s); a pub/sub reconnect (`None`) enqueues `ResyncRegistry`; `AlreadyListening` is `new`'s error; release the listener in `shutdown`. The listener drops notices for users not held locally with no command and no span. Registry and per-command spans are `debug`.
- [ ] **Step 5:** Metrics: gauge `hub_control_queue_depth`, counters for rejected connects, supervisor stops and registry failures, an `outcome` label on `hub_commands`. `RemovedConnection::notify` uses the metrics helper. Dedupe `DropConnection` per connection while one is in flight. Remove `HubError::UserAlreadyConnected`; `request_drop` asserts `reason.is_drop()`.
- [ ] **Step 6: Verify** as in H1. **Step 7: Checkpoint.**

## Task H3: Socket hardening and the client contract (items 1, 4, 8, 12, 19, 20, 21, 28, 30, 36, 37, 38, 48)

**Files:**
- Create: `services/builder/src/routes/ws/rate_limiter.rs`
- Modify: `routes/ws/connect.rs`, `routes/ws/message.rs`, `services/hub/hub_service.rs`, `models/messages/hub.rs`, `app_config.rs`, `tests/api-tests/builder/ws_connect.ts`
- Test: `routes/ws/connect_test.rs`, `services/hub/tests/lifecycle.rs`, `rate_limiter.rs`

**Interfaces:**
- Produces: `fn close_code(reason: &DisconnectReason) -> u16` per H-D3; `DisconnectReason::RegistryMissing` (label `registry_missing`); `struct TokenBucket { fn new(burst: u32, per_second: u32) -> Self; fn try_take(&mut self, now: Instant) -> bool }`; `struct ConnectionGuard` (drop requests a disconnection); frames `hello`, `error`; config `builder.ws.{maxConnections, maxFrameBytes, connectThrottleMs}`.

- [ ] **Step 1 (H3a):** Failing tests `a_socket_task_that_is_cancelled_still_disconnects`; `a_silent_client_is_closed_after_the_idle_timeout` (paused clock, 90 s); `a_client_that_does_not_read_is_closed_after_the_write_timeout`; `the_server_pings_every_30_seconds`; `sockets_do_not_receive_hub_events` (a connected socket never gets another user's `UserConnected`; the session key no longer reaches any socket).
- [ ] **Step 2 (H3a):** Sockets register on `TopicKey::Chat` only (until the stream switch); their own end notice already arrives through `RemovedConnection::notify`. Add `ConnectionGuard`. Egress: `WRITE_TIMEOUT = 10 s` per send, a failed or timed-out send ends the socket, ping every 30 s, read-idle timeout 90 s; keep `SEND_DRAIN_TIMEOUT`. Move the "Connected" log into the command loop after the registry write.
- [ ] **Step 3 (H3a):** Verify `cargo test -p shine-builder routes::ws`. Checkpoint.
- [ ] **Step 4 (H3b):** Failing tests `the_first_frame_is_hello` (`version` 1, `connectionId` equals the registry value); `oversized_frames_are_refused` (> 8 KiB -> `error too_long`); `ten_consecutive_refused_frames_close_with_1008`; `a_client_faster_than_the_rate_limit_gets_rate_limited`; `a_user_reconnecting_too_fast_is_throttled` (429 before the upgrade); `connects_above_the_instance_cap_get_503`; token bucket unit tests `burst_then_refill`.
- [ ] **Step 5 (H3b):** Pre-upgrade guards (per-user throttle, instance cap) before `ws.on_upgrade`; `max_message_size`/`max_frame_size` = `maxFrameBytes`; ingress token bucket; `error` frames for refused input; the counter of consecutive refused frames closes with 1008. Checkpoint.
- [ ] **Step 6 (H3c):** Failing tests `every_drop_reason_maps_to_its_close_code` (replaced 4001, registry_inactive 4001, registry_missing 4003, session_invalid 4002, slow_consumer/dead_channel 1013, shutdown 1001); `registration_failure_closes_with_1013`.
- [ ] **Step 7 (H3c):** `Other` -> `registry_inactive`/4001, `Missing` -> `registry_missing`/4003 on both the heartbeat and the registry-change path. Update the close-code constants in `ws_connect.ts` and run `cd tests && pnpm test:local` with the service up. Add to `hub.md`: the host allow-list is not a security control (Origin is the CSWSH defence). Checkpoint.

## Task H4: Shutdown and startup guards (items 11, 41, 42)

**Files:** Modify `services/hub/hub_service.rs`, `crates/shine-infra/src/web/web_app.rs:91-100` (shutdown hook), `services/builder/src/main.rs`, `app_state.rs`, `docs/services/builder/hub.md`. Tests: `tests/lifecycle.rs`, `tests/fault.rs`.

**Interfaces:** `HubService::wait_stopped() -> impl Future<Output = ()>`; `async fn check_redis(pool: &RedisConnectionPool) -> Result<(), AnyError>` (version >= 6.2; warns when `maxmemory-policy` is not `noeviction`).

- [ ] **Step 1: Failing tests.** `a_shutdown_of_many_connections_closes_every_socket_before_touching_redis` (300 connections); `shutdown_cleans_the_registry_within_ten_seconds`; `shutdown_during_a_redis_outage_still_closes_sockets` (toxiproxy); `redis_older_than_6_2_fails_the_start` (unit test on the version parser); `redis_flush_drops_every_connection_with_the_transient_code`.
- [ ] **Step 2:** Shutdown: first notify and drop every connection, then remove the registry rows concurrently (chunks of 256, 32 in flight) under a 10 s budget; `wait_stopped` resolves when the loop has finished. If `web_app.rs` hooks cannot await, add an async hook variant awaited with a 10 s deadline and use it from `main.rs`.
- [ ] **Step 3:** `check_redis` at startup: parse `INFO server` `redis_version`, fail below 6.2; `CONFIG GET maxmemory-policy` -> warn when not `noeviction` (ignore a denied command). `hub.md` Failure behaviour already states the single-primary and `noeviction` requirement; confirm it matches.
- [ ] **Step 4: Verify. Step 5: Checkpoint.**

## Final verification

- [ ] `cargo fmt --all --check`; `cargo clippy --workspace --all-targets`; `CI=true cargo test --workspace` with the connection strings set; `cd tests && pnpm test:local` with the service running. Expected: all green, no "skipping" lines.
- [ ] Update `docs/plans/hub_issues.md` statuses for every fixed item and re-point locations. Checkpoint.

## Coverage of `hub_issues.md`

| Items | Task |
|---|---|
| 2, 5, 18, 27, 35, 47 | H1 |
| 6, 9, 13, 17, 22, 25, 26, 32, 33, 39, 43, 44, 50 | H2 |
| 1, 4 (frame/rate part), 8, 12, 19, 20, 21, 28, 30, 36, 37, 38, 48 | H3 |
| 11, 41, 42 | H4 |
| 10, 16, 29, 31, 40, 45 (chat part), 49, 4 (text limit) | stream plan |
| 14, 15, 34, 51 | stream plan, final task |
| 3 (by design), 7, 23, 24, 46 | settled |
