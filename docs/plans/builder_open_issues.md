# Builder hub and streams: open issues

Non-SDP investigation notes. Not kept in sync with the code.

What earlier reviews of the hub, the command streams and their clients found that a fresh read of the current code still confirms. Items that are fixed or whose code is gone are not listed. Method: static reading only, nothing compiled or run; each earlier finding was re-checked against the current tree by a separate reader, and a few key claims were spot-checked a second time. Paths are repo-relative.

> [!NOTE]
> An earlier review described a fix round (Lua registry operations, a hub timeout, close codes 4001–4003, a `hello` frame, a connect throttle, a drop guard). None of it is in this tree: the registry still uses `WATCH`/`MULTI`, close code 4000 is the only drop code, and there is no `hello` frame. If that round exists on another branch, merge it before redoing the items below.

## Fix before a prod deploy

| Finding | Where | Fix |
|---|---|---|
| **Prod TLS check rejects pool params, so prod start-up aborts**. `check_postgres_tls` parses the raw string, and tokio-postgres refuses `pool_timeout` and `max_size`. Dev and test skip the check, so only the first prod deploy shows it. | `crates/shine-infra/src/db/tls_policy.rs:12`; callers `services/builder/src/main.rs:42`, `services/identity/src/main.rs:83` | Strip the pool params with `ConnectionString` before parsing. Add a test for `stage=prod` with `sslmode=require&pool_timeout=5&max_size=3`. |
| **TLS policy fails open** for any stage other than the literal `prod`. `Prod`, `staging` or a typo accepts `sslmode=disable` and `redis://`. | `tls_policy.rs:5,9,23` | Allowlist the non-TLS stages (`test`, `local`, `dev`) and require TLS for the rest. |
| **`panic = "abort"` on the plain release profile** turns any runtime panic into a process outage. `assert_not_blocking` panics on any pooled `XREAD`/`B*` command; identity has request-path `assert!`s. | `Cargo.toml:152-153`; `services/Dockerfile:36,58`; `redis_commands.rs:91-96`; `auth_handler.rs:461`, `external_login_handler.rs:63` | Scope `abort` to `release-lto` or the wasm build. Make `assert_not_blocking` return an error. |
| **Pre-deploy config checks**. `sql_cns` has no default and `server_config.prod.json` has no `db` block; Redis below 6.2 is only noted in a comment (`XRANGE (id`). | `services/builder/src/repositories/db.rs:13`; `services/builder/server_config.prod.json`; `repositories/command_inbox/redis/inbox.rs:67-70`; `redis_check.rs:6` | Confirm Key Vault supplies `sqlCns` and `redisCns` (`rediss://`, `sslmode=require`, no pool params until H1 is fixed). Confirm the Azure Redis version is at least 6.2. |
| **Migration replaced instead of added**. `V1__commands.sql` deleted, `V1__command_log.sql` added, and V1 was also edited in place. | `services/builder/sql_migrations/` | If any shared database ran main's V1, add a `V2` that drops the old table; otherwise reset those databases. |
| **Service migrations run under the 10 s `statement_timeout`**. `PgConnection::migrate` lifts it, but both services call `runner().run_async` directly. A failed `RESET` also masks the migration error. | `services/builder/src/repositories/db.rs:37-42`; `services/identity/src/repositories/db.rs:42-47`; `pg_connection.rs:146-151,313` | Route service migrations through `PgConnection::migrate`. Report the migration error before the `RESET` error. |
| **Config is not validated**. A huge heartbeat wraps to a negative TTL and `EXPIRE` with it deletes the key; zero is accepted. | `services/builder/src/app_state.rs:56`; `repositories/hub_registry/redis/redis_hub_registry.rs:81,134` | Clamp and validate at start-up; cross-check session check and heartbeat against the TTL. |
| **Agent git-deny hook removed**. The `PreToolUse` deny for `git add/commit/push` is gone from `.claude/settings.json` and nothing in `CLAUDE.md` replaces it. | `.claude/settings.json`; `.claude/CLAUDE.md:104-105` | Restore the hook, or state the removal is intended and put the rule in `CLAUDE.md`. |

## Correctness of the stream log

| Finding | Where | Fix |
|---|---|---|
| **Chain hash omits `command_id` and `user_id`**. A stale instance whose batch has the same content at the same version is marked `Stored` while the log holds another author's row; replicas can diverge until a snapshot disagrees. | `crates/shine-game/src/replica/stable_hash.rs:54-59`; `stream_log_service.rs:124`; `repositories/command_log/pg/append.rs` (~100-125) | Compare the stored `command_id` and `user_id` and go through catch-up. Longer term, fold both ids into the chain hash (needs a migration). |
| **Resent pending commands become duplicates**. The only unique key is `inbox_id`; the client resends pending commands after a sync, and chat `Add` uses the command id as the message id. | `command_stream/stream_relay.rs:153-190`; `crates/shine-game/src/replica/client_stream.rs:198-205`, `chat.rs:85` | Dedupe on `(stream, command_id)` in a window that survives reload and other instances, or put recently folded ids in the sync so the client drops them. |
| **Inbox cursor never detects a regression**. `read_index` is reset only on reload, so a stream recreated with lower ids is never read. Rare (failover, clock step). | `command_stream/stream_relay.rs:155,186-187,326` | Compare the stream's newest id with the cursor and reset on regression. |
| **Inbox trim only after commands are stored**. A malformed entry waits for a later valid store; a failed trim is only warned. The stored-position filter makes a missed trim harmless. | `stream_persistence.rs:296-301`; `stream_log_service.rs:157-163` | Trim on every successful pass. |
| **`ensure_stream` writes on every worker load**. `ON CONFLICT DO UPDATE` makes a dead tuple and a row lock. | `repositories/command_log/pg/stream.rs` (~27-35) | `DO NOTHING` plus a select. |

## Abuse and capacity

| Finding | Where | Fix |
|---|---|---|
| **Authorization allows everything and nothing is capped**. No limit on subscriptions per connection, workers or `cl_streams` rows; the worker message channel is unbounded. One socket can subscribe to random ids until the Postgres pool is exhausted. | `stream_authorize_service.rs:15`; `command_stream/stream_registry.rs:145-162`; `stream_worker.rs:50` | Cap subscriptions per connection and workers globally. Authorize stream ids (allowlist, or a prefix tied to user or room). Create the `cl_streams` row on the first accepted command. |
| **No rate limit and no hard inbox cap**. Socket and frame sizes are limited and pushes wait on back-pressure, but there is no per-connection rate limit and no `MAXLEN`; `EXPIRE` is refreshed on every push, so under load the key never expires. | `repositories/command_inbox/redis/inbox.rs:33`; `command_stream/stream_registry.rs:218,222` | Token bucket per connection; `XADD … MAXLEN ~ N` or a length check in `push`. |
| **No connect throttle, connection cap or resume point**. Every reconnect costs an upgrade, two tasks, a channel, a registry write and broadcasts; a deploy or load-balancer flip amplifies it. | `routes/ws/connect.rs:76-107`; `hub_service.rs:93,104` | Per-user throttle before the upgrade, per-instance connection cap, bounded control queue, jittered backoff for clients. |
| **Reconnect storm after a Redis flush**. A missing registry row is treated as inactive, so the heartbeat drops every healthy connection. | `repositories/hub_registry/redis/redis_hub_registry.rs:87-89,117-139`; `hub_service.rs:712-736` | Re-create the row (`SET NX` plus TTL); drop only when it holds a different connection id. |

## Hub scale and robustness

| Finding | Where | Fix |
|---|---|---|
| **Lifecycle events go to every socket**. Each connect and disconnect is broadcast to all sockets on the `Hub` topic, then dropped by the socket; O(N) per event and O(N²) in a storm. `UserConnected` also carries the session key. | `routes/ws/connect.rs:393`; `hub_service.rs:579-584,777-790`; `hub_connections.rs:175-190` | Drop `TopicKey::Hub` from sockets; keep the session key out of the broadcast. |
| **Serial command loop, no timeouts**. Redis round trips run one at a time with `WATCH`/`MULTI`, an unbounded control queue, a shared pool and an unchunked `MGET`. A Redis stall stalls the hub; shutdown pays it per connection. | `hub_service.rs:467-484,662-686,758-763`; `redis_hub_registry.rs:117,184-208` | Compare-and-set as Lua, chunked heartbeat, hub-level timeout per registry call, bounded queue, dedicated pool. |
| **Aborted `EXEC` becomes an error**. The `(usize,)` parse fails on a nil reply, so takeover detection can be skipped. | `redis_hub_registry.rs:99,203` | Same Lua rewrite. |
| **Tracker tick starves lifecycle events**. `biased` polls the ticker first. | `services/hub/tasks/connection_tracker.rs:129-136` | Drain events before ticking, or bound tick work. |
| **Unsupervised tasks**. A panic in the command loop, heartbeat, session checker or dispatcher leaves the rest running and the status healthy; a relay-task panic sends no `streamUnsynced`. A dead session checker keeps revoked sessions connected. | `hub_service.rs:466-506`; `tasks/connection_tracker.rs:72,85`; `stream_worker.rs:99-107,147-152` | Supervise handles, fail fast, expose liveness. |
| **No drop guard**. A cancelled or panicking socket task leaves its entry registered. | `routes/ws/connect.rs:117-232` | A guard that requests disconnection. |
| **No write timeout, ping or idle timeout**. Half-open peers pin their tasks and queued batches. | `routes/ws/connect.rs:366-372,170` | Timeout around sends, server ping with a pong deadline. |
| **Session check extends the session TTL** and runs sequentially. | `tasks/session_checker.rs:33`; `crates/shine-infra/src/session/current_user_service.rs:125-131` | A read-only existence check, batched. |
| **One close code for every drop**. Replaced, session invalid and slow consumer all use 4000. Registration failure closes without a code. | `routes/ws/connect.rs:43-57,149-155`; `hub_service.rs:542-551` | Distinct codes: replaced (do not reconnect), session invalid (re-authenticate), transient (back off). |
| **Registry listener**. Spawned and not awaited, a failed listen is only logged, a pub/sub reconnect is ignored. | `hub_service.rs:186-187,509-525`; `redis_hub_connection_db.rs:47-52` | Await the subscribe, retry with backoff, recheck local users on reconnect. |
| **Shutdown is not awaited**. The hook only enqueues `Shutdown`; the process can exit before the registry cleanup finishes. A hub dropped without `Shutdown` leaks its tasks. | `main.rs:50-56`; `hub_service.rs:754,758-763`; `web_app.rs:88-96` | Await shutdown completion in the hook; weak handles in the loops. |
| **Topics ignored for targeted sends**. Only `broadcast` checks them, and sockets lack the `Stream` topic yet receive stream messages. | `hub_connections.rs:151-172` | Check the topic in targeted paths, or drop the topic concept for stream messages. |
| **`remove_connection_by_id` is O(N)** under the write lock. | `hub_connections.rs:110-119` | A reverse map. |
| **Redis pool keeps dead connections**. `test_on_check_out(false)` and timeouts do not mark a connection broken, so reused sockets time out for minutes after a failover or idle drop. | `crates/shine-infra/src/db/redis/redis_connection.rs:67,297` | Flag timeouts and I/O errors as broken, or restore an idle PING. |
| **Grace timer can starve**. It is the last arm of a `biased` select. | `command_stream/stream_worker.rs:235-267` | Check the deadline at the top of each iteration. |
| **Host check trusts client headers**. The Origin check is the real defence. | `routes/ws/connect.rs:84`; `crates/shine-infra/src/web/extracts/target_host.rs:31-37` | Trust forwarded headers only from the proxy; do not treat the host check as a control. |
| **Unversioned Redis keys and channel**. A mixed-version deploy makes instances drop each other's users. | `redis_hub_registry.rs:8-9,91,195`; `redis_command_inbox_db.rs:33` | Version the key and channel; treat an unknown format as foreign. |
| **Single-primary Redis assumed, undocumented**. `MGET` over many keys and a keyspace `SCAN`. | `redis_hub_registry.rs:35-42,117,157` | Document it, or make the calls cluster-safe and drop the `SCAN`. |
| **Dead code**. `list_connections` and its `SCAN` are test-only; `UserAlreadyConnected` is never built; `request_drop` has no `is_drop` assert; a listener error is only logged. | `hub_registry.rs:42-44`; `redis_hub_registry.rs:35,151`; `models/hub_error.rs:9`; `hub_service.rs:381,522` | Delete or `cfg(test)`, assert, return the error. |
| **Per-connection serialization**. The shared message is base64 and JSON encoded once per connection; CPU only. | `routes/ws/message.rs:126-160`; `connect.rs:357-366` | Cache the encoded frame, only if profiling shows it. |
| **Design decisions to make consciously**. Every command carries the sender's raw `user_id`; keys such as `hub-connection:` and `cmd-inbox:` have no environment prefix. | `routes/ws/message.rs:46`; `redis_hub_registry.rs:8`; `redis_command_inbox_db.rs:33` | Decide, then document. |

## Observability and logging

| Finding | Where | Fix |
|---|---|---|
| **`Problem::log` dumps internals at info and error**, including Postgres and Redis errors and attacker-controlled `Host`/`Origin`. Every 4xx logs a multi-line dump. | `crates/shine-infra/src/web/responses/problem_detail.rs:150-159` | Log status, type and a short cause at info; the `sensitive` dump only at trace. |
| **Info-level spans and logs on hot paths**. A per-command hub span with the user id, registry spans, and a session-checker warning per connection per tick. | `hub_service.rs:475`; `redis_hub_registry.rs:53,78,150,174,183`; `tasks/session_checker.rs:51` | `debug`; log once per tick with a count. |
| **Start-up self-test lines are counted**. Every restart adds two warn and two error events, so alerts on the metric fire on every deploy. | `crates/shine-infra/src/web/web_app.rs:202-213`; `telemetry_service.rs:109-115` | Exclude them from the counter. |
| **Metrics gaps**. No queue depth, rejected connects or registry-error metrics; `notify` bypasses the metrics helper; a full connection gets a repeated `DropConnection` per stream send. | `services/hub/hub_metrics.rs`; `hub_status.rs:25-28`; `hub_connections.rs:44-46` | Add the metrics; dedupe in-flight drops. |
| **Registry notices enqueue a command for users the instance does not hold**. | `redis_hub_connection_db.rs:55-69`; `hub_service.rs:475-481` | Filter in the listener. |
| **Protocol gaps**. No version or connection id in `connected`; bad JSON, binary frames and bad stream ids are dropped silently; no close after repeated invalid frames; OpenAPI documents only `OK`. | `routes/ws/message.rs:55,78-86`; `connect.rs:67-75,188-193,261-263` | Document the schema; consider a close after repeated refusals. |

## Web client and replica

| Finding | Where | Fix |
|---|---|---|
| **No watchdog on `waiting` and `syncing`**. A lost `streamSync` strands the client; only `unsynced` has a retry timer, re-armed only by another frame. | `client/web/src/experiments/pixel-map/index.ts:98-103`; `crates/shine-game/src/replica/client_stream.rs:173-176` | Re-send the subscribe with backoff for any non-live status. |
| **Backoff resets on every open**. If the server accepts the upgrade and then closes, the client retries every second forever. | `client/web/src/engine/net/websocket-stream-hub.ts:129-131,166` | Reset after the first sync or a minimum open time. |
| **A `failed` nack drops the pending command**, although the server says it may have been queued. | `client_stream.rs:253-257` | Keep it pending and resend, or mark it unknown. |
| **A persistent checkpoint mismatch loops forever**. Every mismatch resubscribes with no counter. Checkpoints are now sent, so this is live. | `client_stream.rs:235-251` | A counter, then a move to `Unsynced`. |
| **Stale state after a reconnect**. The stream is never reset to `Waiting`; paints are not gated by the connection state; `replaced` is never set although the type has it. | `pixel-map/index.ts:46-51,164-184`; `stream-hub.ts:1`; `pixel-map-ui.ts:14` | Reset or resync on reconnect; gate paints; handle `replaced` (no reconnect). |
| **Overlapping `Sync` tail records a bad hash**. `on_sync` still calls `apply_wire` and ignores a skipped outcome. The server sends non-overlapping tails today. | `client_stream.rs:189-193,220,305-308` | Honour `ApplyOutcome::Skipped`. |
| **Snapshot decode accepts trailing bytes**. `decode_snapshot` uses `postcard::from_bytes`; state and chain hashes come from the same frame. | `crates/shine-game/src/replica/codec.rs:15-17,35-41`; `client_stream.rs:280-286` | Canonical check for snapshots; verify the chain hash against a checkpoint. |
| **`StreamSubscribeFailed` is retried every 5 s forever**, including permanent reasons; `new WebSocket` is unguarded. | `client_stream.rs:155-159`; `websocket-stream-hub.ts:50` | Stop on permanent reasons; try/catch into the backoff path. |
| **`net/` is missing from the `client-web-architecture` skill's layout table**. | `.claude/skills/client-web-architecture/SKILL.md:14-25` | Add a row. |

## Tests

| Gap | Where |
|---|---|
| Stress tests are small (12 to 48 users) and run with heartbeat, session check and dispatcher idle at 3600 s, so scale, cascades, loop panic and Redis flush are untested. | `testing/hub/test_hub.rs:30,44`; `services/hub/tests/contention.rs:128,199,310,524` |
| Missing fault and race cases: listener failure, `WATCH` conflict, half-open or oversized frames, id regression; fixed sleeps and `tick * N` waits remain. | `services/hub/tests/fault.rs`; `contention.rs:513,582`; `lifecycle.rs:440,479`; `routes/ws/connect_test.rs:294`; `stream_worker_test.rs:120,271` |
| Mutation blind spots: the close-code test compares the constant to itself, no test drives `AppState` wiring, the `SEND_DRAIN_TIMEOUT` abort is untested. | `routes/ws/connect_test.rs:104-112`; `app_state.rs:56`; `connect.rs:65,228` |
| Rust, `shine-infra`: no tests for `tls_policy.rs` (prod with pool params, odd stage names). | `crates/shine-infra/src/db/tls_policy.rs` |
| Rust, builder: the stale-instance identical-content race, a duplicate resent command, `ConnectUser` registry failure, and `ensure_stream` plus migration against a database that has main's V1. | `services/builder/src/services/command_stream/` |
| Rust, `shine-game` `client_stream`: mid-batch gap, a command that fails to decode, a checkpoint mismatch while `Waiting`, overlapping `Sync` tails, trailing snapshot bytes, `HASH_HISTORY` eviction at 256, a `failed` nack on a known pending command, a command folded into the snapshot but absent from the tail. | `crates/shine-game/tests/replica/client_stream.rs` |
| TypeScript: only two tests exist for the hub. Missing: stale live after reconnect, a hang in waiting, an immediate close after open with growing delay, `connect()` twice, the `unsynced` timer re-arming, `resync()` while closed or replaced. Nothing tests the pixel map experiment's stream handling. | `client/web/src/engine/net/websocket-stream-hub.test.ts` |
| `shine-test` panic hook exits the whole run on a background-thread panic; a test that expects a spawned-task panic cannot exist. The fuzz CI job uses an unpinned nightly and an uncached `cargo install`. | `crates/shine-test/src/lib.rs` (~65-84); `.github/workflows/fuzz_ci.yml` |
