# Hub issues (builder service)

Merged from three review passes of the whole hub (`services/builder`). Static read of code and tests only: nothing was compiled or run, and no claim was reproduced. Item 5 is from reading redis-rs 1.7.1; item 23 is explicitly unverified.

Paths are relative to `services/builder/src` unless noted. Source notes with the longer write-ups: `temp/hub-review.md` (1-15), `temp/hub-review-2.md` (16-35), `temp/hub-review-3.md` (36-51).

**Verdict:** no major rewrite needed. The design holds; fixes are local or moderate.

**Checked and found clean:** no nested locks or lock held across an await on another lock (no deadlock); connection ids are v4 UUIDs (no ABA between stale drop and reconnect); command FIFO keeps connect before a later disconnect; stale registry-change notices are harmless (handler re-reads the registry); chat sender comes from the session, topics are server-chosen; nothing sensitive is logged above trace; all expiry uses the Redis clock (no clock-skew issue); crashed-instance rows expire by TTL; every Redis key has a TTL or trim; concurrent connects of one user on two instances converge; topics are fixed at connect, so no subscribe/unsubscribe race. There is no in-repo ws client (`client/web/src` has none; only `tests/api-tests/builder/ws_connect.ts` consumes the protocol), so the reconnect contract is purely a server concern today.

## Status

First fix round done. `cargo test -p shine-builder` against a local Valkey: 79 passed (was 41 hub tests before); `cargo clippy --all-targets` clean. Not run: the API tests under `tests/` (the TS close-code constants were updated but not executed), and the toxiproxy fault tests (skipped without `SHINE_TEST_TOXIPROXY_URL`).

**Fixed** (item: what changed, tests):

| # | What changed | Tests |
|---|---|---|
| 1 | Sockets subscribe to `Chat` only; the end notice reaches them directly. | `connect_test`, topic tests |
| 2 | Registry ops are single-round-trip Lua scripts with a 2 s hub timeout (batch heartbeat chunked by 256); queued-command gauge; new connects refused above 10 000 queued commands. | `registry.rs` (chunk edges), `resilience.rs` |
| 3 | A missing registry row is restored, never treated as a takeover; only a different owner drops the connection. | `heartbeat_refreshes_live_rows_restores_missing...`, `heartbeat_restores_a_missing_entry...` |
| 4 | Chat text capped at 2000 bytes, frames at 8 KiB, token bucket of 5 + 1/s per connection; one `Arc` batch shared by all connections at the same cursor. | `oversized_chat...`, `chat_faster_than_the_rate_limit...`, `rate_limiter.rs` |
| 5, 23 | CAS expire/delete as Lua (remove also publishes atomically); no WATCH/MULTI left, so no aborted-EXEC error or stale WATCH. | `remove_only_deletes_an_entry_owned...` |
| 6, 22 | The tracker applies queued lifecycle events before ticking (bounded), and wake ticks are spaced 50 ms apart. | `ticks_longer_than_the_interval...`, `steady_wakes_are_spaced_out` |
| 8, 38 | Writes have a 10 s timeout and a failed write ends the socket; server ping every 30 s and a 90 s idle timeout. | `a_silent_client_is_closed...`, `a_client_that_does_not_read...` |
| 9, 25 | The registry listener is subscribed before `new` returns, retried with backoff on failure, and a pub/sub reconnect triggers a resync of all local connections. | `a_resync_drops_only...`, `a_connect_after_the_registry_listener_started...` |
| 10, 16, 31 | Connections are grouped by cursor, so a newcomer replaying history no longer holds back the rest; each group reads up to 8 pages per tick; unreadable entries are skipped. | `a_joining_connection...`, `a_backlog_beyond_one_page...`, `unreadable_entries_are_skipped...` |
| 11 (partly) | Shutdown notifies and closes every socket first, then cleans the registry concurrently within a 10 s budget; `wait_stopped` added. | `a_shutdown_of_many_connections...` (300) |
| 13, 17 | A panic in a command or a consumer tick is caught, logged and counted, and the loop continues; `HubStatus` reports `running`, `pendingCommands` and `stoppedTasks`. | `the_status_reports...` (a panic itself can't be tested: the test harness aborts the run on any panic) |
| 19 | Per-user connect throttle (500 ms) and the overload check run before the upgrade (429/503). | `a_user_reconnecting_too_fast...` |
| 20 | A drop guard removes the connection however the socket task ends. | `a_socket_task_that_is_cancelled...` |
| 21, 48 (partly) | Protocol v1: `hello` first frame, `ack` with the stream id, `error` frames (`invalid`, `too_long`, `rate_limited`, `append_failed`); 10 consecutive refused frames close the socket (1008); append failures are logged with exponential spacing. | `client_chat_is_appended_and_delivered_back`, `a_client_that_keeps_sending_refused_frames...` |
| 24 | With `CI` set, a missing `SHINE_TEST_REDIS_CNS` fails the hub tests instead of skipping them. | - |
| 26 (partly) | The chat append listener is released on shutdown. | - |
| 29, 45a | Append is one Lua script (XADD + XTRIM + EXPIRE + PUBLISH), so a cancelled caller can't leave an immortal or unannounced stream. | `an_append_stores_trims_and_expires...` |
| 32 (partly) | Registry and chat-append spans are `debug`. | - |
| 33 (partly) | New metrics: command queue depth, rejected connects, caught panics, registry failures. | - |
| 36 | Distinct close codes: 4001 replaced (don't reconnect), 4002 session invalid (re-authenticate), 4003 transient, 1013 slow consumer / dropped without notice, 1001 shutdown. | `every_drop_reason_maps_to_its_close_code` |
| 39 | `remove_connection_by_id` is O(1). | `remove_by_id_finds_the_connection...` |
| 40 | Targeted sends respect the connection's topics. | `directed_sends_respect_the_topics...` |
| 43 | Registry notices for users not held locally are dropped in the listener, with no command or span. | - |
| 44 (partly) | The close notice goes through the metrics helper (`kind=close`); duplicate `DropConnection`s are deduplicated. | - |
| 49 | Cursors ahead of the stream's newest id are reset (checked every 30 ticks). | `a_stream_restarting_behind_the_cursors...` |
| 50 (partly) | `request_drop` asserts the reason is a drop. | - |

**Not fixed yet:**

- 7 (the session check still extends the session TTL), 12 (the session key is still in the internal lifecycle events; sockets no longer receive them), 14/15/34/51 (more edge tests were added, but there is no heartbeat-enabled scale/stress test and fixed sleeps remain), 18 (no dedicated Redis pool, no explicit pool/response timeouts in the connection string), 27 (config clamping), 28, 30, 35, 37 (resume cursor, per-instance connection cap), 41 (Redis 6.2 probe), 42 (cluster doc), 46 (HUB.md and `chat-data-flow.html` still describe the old behaviour), 47 (key versioning), 50 (the dead `list_connections` scan and `UserAlreadyConnected` remain).
- 11: the shutdown hook in `main.rs` still only enqueues the command, because hooks are synchronous, so the process can exit before the registry cleanup finishes; the rows then expire by TTL, and sockets are already closed.
- 16: there is no gap marker for comments trimmed before delivery. Paging makes this unlikely, but a burst above `streamMaxMessages` between two reads still loses comments.
- 26: a hub dropped without `Shutdown` still leaks its tasks (they hold strong references to each other).
- Behaviour change to check with clients: close code 4000 no longer exists, and frames other than chat now get `error` replies.

Effort: **S** small/local, **M** moderate (a few files, design decisions).

## Critical path: scale and correctness

| # | Sev | Location | Issue and fix | Effort |
|---|---|---|---|---|
| 1 | High | `routes/ws/connect.rs:301` | Every socket subscribes to the Hub topic, so each connect/disconnect is pushed to every socket's 1000-slot channel: O(N²), cascades into SlowConsumer drops (each drop publishes another disconnect). Sockets don't need these events (`RemovedConnection::notify` already covers it). Drop `TopicKey::Hub` from sockets, index `connection_id → user_id`. HUB.md "low frequency" claim is wrong at scale. | M |
| 2 | High | `services/hub/hub_service.rs:275`; `redis_hub_registry.rs:105-143` | Single command loop does serial Redis round trips (~2 per connect, ~6 per disconnect: PING, WATCH, GET, MULTI/DEL/EXEC, UNWATCH, PUBLISH), no hub-level timeouts, unbounded control queue, shares the default 10-connection pool (30 s pool timeout) with chat appends. Heartbeat is one unchunked MGET plus N EXPIREs; the 500 ms default response timeout fails it at large N. Fix: dedicated pool, Lua compare-and-set (one round trip), chunked heartbeat, bounded queue. | M |
| 3 | High | `redis_hub_registry.rs:132` | Missing registry row is treated as takeover. After Redis flush/failover/outage longer than the row TTL, the heartbeat drops every healthy connection (4000), causing a reconnect storm. Re-create with SET NX + TTL; drop only when the row holds a different connection id. | S |
| 4 | High | `routes/ws/connect.rs:164`; `chat_dispatcher.rs:103-112` | No chat size/rate limit (axum default ~64 MiB per message); each message is stored then deep-cloned per connected socket. Add max length, per-connection rate limit, `max_message_size`, share the batch via `Arc`. | S |
| 5 | High | `redis_hub_registry.rs:94,198` | WATCH/MULTI destructures `(usize,)` with `?`; an aborted EXEC (nil) becomes `Err` instead of retry, so takeover detection can be skipped (two live connections until the next 300 s heartbeat). `remove_connection_if_active` has the same flaw. Replace both with Lua compare-and-expire / compare-and-delete. | M |
| 6 | High | `tasks/connection_tracker.rs:128` | `biased` select polls the ticker first; a tick lasting at least one interval is always due again, so lifecycle events are never read (session checker at ~30k connections, chat dispatcher much sooner). Drain events before ticking or bound tick work. | S |
| 10 | High | `tasks/chat_dispatcher.rs:75` | New connections start at cursor `"0"` and pin the shared `oldest`, so one query of 128 oldest entries serves everyone and all other users' chat stalls ~H/128 ticks (~80 s at `streamMaxMessages=10000`). Bootstrap new connections separately. | M |
| 16 | High | `tasks/chat_dispatcher.rs:18,79`; `app_config.rs:31-33` | Silent chat loss under bursts: dispatcher reads at most 128 entries/tick, stream is trimmed to ~200 on every append, trimmed entries are never delivered and nothing signals the gap. Size retention above max backlog, loop reads until caught up, detect cursor older than first retained id and send a resync marker. | M |
| 17 | High | `tasks/connection_tracker.rs:71-84,125`; `hub_service.rs:55-59` | Heartbeat, session-checker and chat-dispatcher loops are unsupervised; a panic leaves them dead and `HubStatus` healthy. A dead session checker means revoked sessions keep their socket and can keep posting chat. Join/monitor handles, restart or fail fast, expose per-consumer liveness. | S |
| 36 | High | `routes/ws/connect.rs:39-53,256-260`; `HUB.md:344-353` | Every hub drop uses close code 4000; only free-text reason distinguishes replace/session/heartbeat. An auto-reconnecting client with two tabs ping-pongs forever; slow-consumer and registration failures give no usable code. Use distinct codes (replaced: don't reconnect; session invalid: re-auth; transient: backoff; 1013 try-again/slow consumer) and add a client-contract section to HUB.md. | S |
| 37 | High | `chat_dispatcher.rs:22,61,103-111`; `redis_hub_registry.rs:56-69`; `connect.rs` | Reconnect storm after deploy/LB flip is amplified: no jitter guidance, no connection cap, no resume point, every new connection replays full history with per-connection clones (~4 GB transient at 30k sockets x 1 KB). Add optional resume cursor on upgrade, `Arc` the batch, per-instance connection cap (503/1013), jittered backoff docs, staggered 1001 on shutdown. | M |

## Reliability and robustness

| # | Sev | Location | Issue and fix | Effort |
|---|---|---|---|---|
| 7 | Med | `tasks/session_checker.rs:33`; `current_user_service.rs:118-125` | Session check uses `get_current_user`, which extends the session TTL, so any open socket keeps its session alive forever; sequential (~4 round trips per connection). Use a read-only existence check; run concurrently/batched. | S |
| 8 | Med | `connect.rs:273` | `forward_egress` logs send errors and keeps looping; no server ping or idle timeout. Half-open sockets live forever and lose chat. Break on send error; add ping + read-idle timeout. | S |
| 9 | Med | `hub_service.rs:242`; `redis_hub_connection_db.rs:47-52` | Registry listener is spawned and forgotten: failed subscribe is not retried, pub/sub reconnect (`None`) is ignored. Retry the listen; recheck all local users on reconnect. | S |
| 11 | Med | `hub_service.rs:462`; `main.rs:34-38` | Shutdown removes connections one at a time with a Redis transaction each, hook doesn't await completion; sockets may miss the 1001 frame and leave rows behind. Close sockets first, clean registry in chunks, await with a timeout. | M |
| 12 | Med | `hub_service.rs:312` | `UserConnected` carries the `SessionKey` and is broadcast to every Hub-topic socket. Not on the wire today; latent leak. Keep the key out of the broadcast payload. | S |
| 13 | Med | `hub_service.rs:208` | Command loop is unsupervised: a panic in `process()` leaves the hub dead while other tasks run and `HubStatus` looks fine. Add supervisor/fail-fast, expose liveness. | S |
| 18 | Med | `hub_service.rs:462-471`; `redis_hub_connection_db.rs:56-61`; `crates/shine-infra/src/db/redis/redis_connection.rs` | Redis stall becomes a 30 s per-command stall of the single loop; shutdown pays it per connection (no close frame at all). Hub-level timeout per registry op (~1-2 s), close sockets before touching Redis, ship explicit `timeout`/`pool_timeout` in the prod connection string. Do the Lua rewrite (5) before adding timeouts (see 45). | S |
| 19 | Med | `connect.rs:72-100,165-230`; `hub_service.rs:312-326` | No per-user/IP connect throttle; each reconnect costs an upgrade, two tasks, a channel, registry SET+PUBLISH, a `Replaced` drop and broadcasts; control queue is unbounded. Throttle before `ws.on_upgrade`, bound the queue. | S |
| 20 | Med | `connect.rs:136-246` | No drop guard around connection lifetime: if `handle_socket` is cancelled/panics the entry stays registered and heartbeat keeps its TTL alive. Today only the item-1 broadcast's failed send cleans it up; fixing 1 removes that. Add a drop guard that requests disconnection. | S |
| 21 | Med | `connect.rs:158-197` | Failed `append_comment` is only logged (one `error!` per message during an outage); sender never learns the message was lost; invalid frames never disconnect. Return error/ack frame, rate-limit the log, close on repeated invalid frames. | S |
| 22 | Med | `connection_tracker.rs:129-155`; `chat_dispatcher.rs:36-40` | Wake-driven ticks have no minimum spacing; one fast writer keeps the dispatcher at 100% duty and keeps item 6's starvation permanent. Floor of one wake-tick per 50-100 ms, keep coalescing. | S |
| 23 | Med | `redis_hub_registry.rs:77-103,184-205` | **Unverified.** `transaction_async` only UNWATCHes on the `Some` path; on `Err` the key may stay WATCHed on a pooled connection (bb8 `has_broken` is false), aborting later unrelated EXECs. Verify in redis-rs source; mitigated by the Lua rewrite. | S |
| 38 | Med | `connect.rs:205-218,239-282` | `ws_sender.send` has no timeout: a hub-dropped slow client with a closed TCP window pins its socket, two tasks and up to 1000 queued batches for hours. Wrap sends in a timeout, abort the send task after a grace period. | S |
| 39 | Med | `hub_connections.rs:109-118`; `hub_service.rs:356,492` | `remove_connection_by_id` is an O(N) scan under the write lock; k drops over N connections cost k x N and block all readers (~1.5e8 steps for 5k drops over 30k). Store `user_id` in `Connection` / keep a reverse map. | S |
| 40 | Med | `hub_connections.rs:150-171`; `chat_dispatcher.rs:54-65,118-121`; `HUB.md:86` | `Chat` topic is decorative: targeted sends ignore topics, so a connection with `topics=[]` still gets chat; the test harness uses `[Chat]`, prod uses `[Chat, Hub]`. Check topic in targeted paths (or track only Chat-subscribed connections) and test it; state that topics are immutable. | S |
| 41 | Med | `redis_chat_comments_store.rs:80-86`; `redis_chat_comments_db.rs:23-47` | Needs Redis >= 6.2 (exclusive `XRANGE (id`); on 6.0 chat never delivers and the service reports healthy. Probe at startup, fail fast, document minimum version. | S |
| 42 | Med | `redis_hub_registry.rs:30-44,105-167` | MGET over many slots, EXPIRE pipeline and `SCAN` are not Redis Cluster compatible (plain `redis::Client`); undocumented. Document "single-primary Redis required" or make per-key/cluster-routable and drop `SCAN`. | S doc / M cluster |
| 43 | Med | `hub_service.rs:217-223,248-251`; `redis_hub_registry.rs:179-215`; `redis_hub_connection_db.rs:62-69` | Every cluster-wide connect/disconnect makes every instance enqueue a command and an info-level root span even for users it doesn't hold. Pre-filter in the listener with a cheap local lookup; use `debug` for notice-only spans. | S |
| 44 | Med | `hub_connections.rs:44-64`; `hub_service.rs:107-112,483-503`; `chat_dispatcher.rs:117-131`; `HUB.md:88-89,390` | Backpressure applied inconsistently: `RemovedConnection::notify` bypasses the metrics helper (no log/metric on lost close notice; HUB.md lists a nonexistent `kind=close`); Full is a drop on first failure with lifecycle noise sharing the slots; a Full connection gets a duplicate `DropConnection` every chat tick; `Subscribers` and the control queue have no depth metric. Route `notify` through `try_send`, dedupe in-flight drops, document the matrix. | S |
| 45 | Med | `connect.rs:226-235`; `redis_chat_comments_store.rs:33-63`; `redis_hub_registry.rs:79-98` | Cancellation hazards: `recv_task.abort()` can cut `append_comment` between XADD and its follow-up (no PUBLISH/XTRIM, and on a fresh stream no EXPIRE, so an immortal key); hub-level timeouts are unsafe with current WATCH code until the Lua rewrite; a dropped `listen` can leave a subscribed channel with no handler. Make append atomic (Lua/MULTI, PUBLISH best effort), `spawn` it. | S |

## Hardening, docs, observability

| # | Sev | Location | Issue and fix | Effort |
|---|---|---|---|---|
| 25 | Low | `hub_service.rs:242-258,55-59` | Registry listener subscribes asynchronously, so a takeover right after startup can be missed. Await the subscribe before `new` returns. | S |
| 26 | Low | `hub_service.rs:253-257,204-232`; `chat_dispatcher.rs:36-39` | Chat listener is never unlistened; a hub dropped without `Shutdown` leaks its tasks and subscriptions (every test hub does). Use a weak handle in the loop; unlisten both channels in `shutdown`. | S |
| 27 | Low | `redis_hub_registry.rs:76,129`; `app_state.rs:50-54` | Unchecked numeric config: a huge heartbeat wraps to a negative TTL and `EXPIRE` with a negative TTL deletes the key; huge intervals can panic tokio `Instant` arithmetic. Clamp and validate at startup, cross-check session_check/heartbeat vs TTL. | S |
| 28 | Low | `connect.rs:83-91`; `crates/shine-infra/src/web/extracts/target_host.rs:35-37` | Host allow-list trusts client-sent `X-Forwarded-Host`/`Forwarded`. The Origin check is the real CSWSH defence; don't treat the host check as a security control, or trust forwarded headers only from the proxy. | S |
| 29 | Low | `redis_chat_comments_store.rs:31-60` | Append is XADD then a separate pipeline; if the second fails the caller sees `Err` (client may resend and duplicate) though the message is stored. Treat XADD as the success point, follow-ups best-effort (or Lua). | S |
| 30 | Low | `connect.rs:133-139,248-267`; `hub_service.rs:125-145,327-338` | Registration failure gives the client a plain close with no code; "Connected" is logged before the hub registers anything; a registry write that lands but whose read-back fails leaves the registry naming a discarded connection. Send 1013, log connected from the loop. | S |
| 31 | Low | `chat_dispatcher.rs:68-83` | A batch of unparsable stream entries yields an empty batch, cursors never advance, and the same 128 poison entries are re-read forever. Advance the cursor to the last raw id read. | S |
| 32 | Low | `redis_hub_registry.rs:48,73,169,178`; `redis_chat_comments_store.rs:30`; `session_checker.rs:51` | Per-event registry spans and per-message `append_comment` spans are `info`, against the log-level rules; session checker warns once per connection per tick during an outage. Use `debug`; log once per tick with a count. | S |
| 33 | Low | `services/hub/hub_metrics.rs`, `hub_status.rs` | Metrics gaps: control-queue depth, failed connects, registry/heartbeat errors, session-check outcomes, subscriber prunes, chat read failures, chat lag; no outcome label on `commands`; `HubStatus` has only two counts. | S |
| 34 | Low | `tests/chat.rs:98-108,203`; `tests/lifecycle.rs:494,548`; `tests/fault.rs:224`; `routes/ws/connect_test.rs:300-304`; `tests/contention.rs:565` | More fixed-sleep tests ("exactly once" after 200-500 ms, 5 ms race against `leave()`, `tick * 3`). Use deterministic barriers / paused clock. Missing cases: connect-rate flood, consumer task death, burst > `BATCH_LIMIT`, poison entries, listener startup window, shutdown during Redis outage. | S |
| 35 | Info | `connect.rs:124-127`; `models/messages/chat.rs` | By design but decide consciously: global chat room exposes every sender's internal `user_id`, and reconnects replay history (clients must dedupe). Redis keys/channel have no per-environment prefix (shared Redis means cross-environment takeover). | - |
| 46 | Med | `HUB.md:86-87,290-291,307-309,364,366-377,390`; `docs/services/builder/chat-data-flow.html` | Docs drift: "exactly once" (HUB.md) vs "at-least-once" (html) and both wrong; "low frequency" lifecycle events; "Redis recovery is automatic" (false beyond the row TTL, see 3); append drawn as one step; config table omits `ws.allowedHosts`, `service.allowedOrigins`, Redis `timeout`/`pool_timeout`/`max_size`; no `close` metric; html says "registered on the Hub topic" (code: `[Chat, Hub]`). Correct each and state the delivery guarantee once. | S |
| 47 | Low | `redis_hub_registry.rs:8,56-61,86-88,123-126,190-192`; `redis_hub_connection_db.rs:55-58` | No format versioning of Redis keys/values/channel; a mixed-version deploy makes instances drop each other's users (unparsable value = "not mine"). Version the key/channel; treat unknown format as foreign, skip. | S |
| 48 | Low | `connect.rs:58-66,98-114,164-176`; `routes/ws/message.rs` | Wire protocol has no version/hello, no ack or error frames; binary frames, bad JSON and unknown types are silently discarded; OpenAPI documents neither 101 nor the schemas. Send `hello {version, connectionId}`, add `error`/`ack` frames, document the schema. | S |
| 49 | Low | `chat_dispatcher.rs:75-105,139-151` | Cursors have no regression detection: after a flush/failover/stream recreate, new ids compare lower than live cursors and are never delivered until reconnect. Compare the stream's last id with the max cursor and reset on regression. | S |
| 50 | Low | `hub_registry.rs:42-44`; `redis_hub_registry.rs:30-44,145-167`; `models/hub_error.rs:9,53`; `hub_service.rs:144-146,168-186,242-258`; `hub_connections.rs:161` | Dead code/API misuse: `list_connections` + `find_redis_keys` (whole-keyspace `SCAN`) only used by tests; `HubError::UserAlreadyConnected` never constructed; `request_drop` accepts `DisconnectReason::Client` (silent); a second `HubService` on one pool loses takeover (`AlreadyListening` only logged). Delete or `#[cfg(test)]` the scan, remove the variant, assert `reason.is_drop()`, make `new` return the listener error. | S |

## Tests

| # | Sev | Location | Issue and fix | Effort |
|---|---|---|---|---|
| 14 | Med | `services/hub/tests/contention.rs:119`; `test_utils.rs:43-64` | Stress tests are small (48/12/16 users, 2 instances) and run with heartbeat/session/chat at 3600 s, so background-consumer interplay, scale, cascades, large shutdown, Redis stall, loop panic and Redis flush are untested. Items 1, 2, 3, 6, 10 would not be caught. Add a heartbeat-enabled stress test. | S |
| 15 | Med | `services/hub/tests/fault.rs:52`; `contention.rs:494` | Missing: listener subscribe failure/reconnect, WATCH conflict, heartbeat timeout, late-joiner cursor pin, Full chat channel, oversized/flooding client, stream expiry/ID regression, half-open ws, oversized/Close/ping frames; `contention.rs:494` asserts a negative after a fixed 200 ms sleep. | S |
| 24 | Med | `services/hub/tests/*.rs`; `test_utils.rs:41-48` | Every hub test returns early when `SHINE_TEST_REDIS_CNS` is unset, so CI can be green with zero assertions. Fail or `#[ignore]` instead; panic when `CI` is set. | S |
| 51 | Low | `test_utils.rs:83-84,370`; `app_state.rs:50-72`; `tests/chat.rs`; `connect_test.rs:99-107` | Mutation blind spots: app wiring (`saturating_mul(3)`, `.max(1)`, interval mapping) never tested; no `XLEN`/stream-TTL assertions (removing `XTRIM`/`EXPIRE` is invisible); production topic set never exercised; `CLOSE_CODE_DROPPED` asserted against itself; `SEND_DRAIN_TIMEOUT`/abort branch untested; no stalled-send test. Add tests driving real `AppState::new`, `XLEN`/`TTL` checks, the prod topic set, a never-completing send sink. | S |

## Suggested order

1. **Registry change** (one Lua-based change): 3, 5, the registry part of 2, plus 23. Items 2, 5, 11 and 18 share the registry and command-loop code. Add timeouts (18, 45) only after the Lua rewrite.
2. **Supervision and chat integrity:** 13, 17, 16, 31, 20 (20 must land with or before the fix for 1).
3. **Scale:** 1, 39, 6, 22, 10, then 37 together with 4 (resume cursor, shared batch, admission cap).
4. **Client contract:** 36, 38, 8, 44, 19, 21, 48.
5. **Deployment guards:** 41, 42, 47, 27, 25, 9.
6. **Everything else**, with tests (14, 15, 24, 34, 51) and doc corrections (46) added alongside the code each one describes. Item 24 is cheap and worth doing first, since until then a green run proves little.
