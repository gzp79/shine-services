# Builder hub and command streams: audit and open issues

Non-SDP investigation notes. Not kept in sync with the code.

Consolidated findings for the builder service hub (`services/hub`, `routes/ws`, `repositories/hub_registry`), the command stream (Redis inbox, relay, persistence, Postgres log, worker, registry, stream authorize) and their clients (`shine-game` replica, web client), plus the prod-deploy blockers found in the surrounding infrastructure. Several earlier audits were merged here and duplicates collapsed; a finding that was reported by more than one of them appears once.

Method: static reading only, nothing compiled or run. Each finding was checked against the code at the time of its audit; the open-issue rows were re-checked against the then-current tree by a separate reader. Line numbers drift, so treat them as pointers.

**Paths**: relative to `services/builder/src/` unless they start with `crates/`, `client/`, `docs/`, `.claude/`, `.github/`, `Cargo.toml` or `services/builder/` / `services/identity/` (those are repo-relative).

**Design assumption that matters for many findings**: several builder instances may serve the same stream, each with its own worker reading the same Redis inbox and appending to the same Postgres log (`stream_registry.rs:240-273`, `stream_persistence.rs`). Redis pub/sub wakes are best effort.

> [!NOTE]
> The allow-all `is_authorized` (`services/stream_authorize/stream_authorize_service.rs:14-16`) is a known placeholder. The per-connection/total stream caps, the per-stream Redis/worker cost, the never-deleted `cl_streams` rows and the missing re-check after subscribe remain valid regardless of the final authorization rules.

> [!NOTE]
> An earlier review described a fix round (Lua registry operations, a hub timeout, close codes 4001-4003, a `hello` frame, a connect throttle, a drop guard). None of it is in this tree: the registry still uses `WATCH`/`MULTI`, close code 4000 is the only drop code, and there is no `hello` frame. If that round exists on another branch, merge it before redoing the items below.

## Summary

Severity: **Blocker** = fix before a prod deploy; High / Medium / Low as assessed by the audits (rows originally listed without severity were assigned one when merged).

| ID | Sev | Area | Finding |
|----|-----|------|---------|
| **Prod-deploy blockers** | | | |
| D1 | Fixed | config | Prod TLS check rejected pool params, prod start-up would abort |
| D2 | Blocker | config | TLS policy fails open for any stage other than literal `prod` |
| D3 | Blocker | availability | `panic = "abort"` on the plain release profile |
| D4 | Blocker | config | Pre-deploy config checks (`sql_cns`, `redisCns`, Redis >= 6.2) |
| D5 | Blocker | migration | V1 migration replaced instead of added |
| D6 | Blocker | migration | Service migrations run under the 10 s `statement_timeout` |
| D7 | High | config | Config is not validated (heartbeat wrap to negative TTL) |
| D8 | Medium | tooling | Agent git-deny hook removed |
| **Stream log correctness** | | | |
| L1 | High | soundness | Chain hash ignores `user_id`/`command_id`/`inbox_id`; false "stored" verdict loses a command |
| L2 | Medium | multi-instance | Persisted-version wakes are ignored; stale instances never reconcile |
| L3 | Medium | idempotency | No dedup by `command_id`; resends, retries and drain leftovers duplicate commands |
| L4 | Low | integrity | Replay trusts stored `snapshot.hash` and `command_hash` |
| L5 | Low | lifecycle | `is_finished()` true while a worker still drains; replacement overlap; draining worker invisible to `shutdown` |
| L6 | Low | correctness | Inbox cursor never detects a Redis id regression |
| L7 | Low | correctness | Inbox trim only after commands are stored; failed trim only warned |
| L8 | Low | perf | `ensure_stream` writes on every worker load |
| L9 | Low | correctness | Load/prune race yields a `Gap` |
| L10 | Low | availability | No PG `statement_timeout` on reads/appends |
| L11 | Low | robustness | Poisoned registry mutex panics every later call |
| L12 | Low | compat | Rolling deploy: old binary cannot decode new command variants |
| L13 | Low | correctness | Grace timer can starve (last arm of a `biased` select) |
| L14 | Low | misc | Chat text stored and relayed raw |
| **Abuse and capacity** | | | |
| A1 | High | authz / DoS | Allow-all authorization and uncapped stream creation |
| A2 | Medium | DoS | No rate limit on ws commands/subscribes; stream-wide backpressure punishes honest users; subscribe spam amplifies into `Sync` |
| A3 | Medium | DoS | No per-connection throttle on the hub control queue; no connect throttle or connection cap |
| A4 | Medium | resource growth | Inbox has no hard size limit (soft backpressure is sampled) |
| A5 | Medium | resource leak | ws sink has no write timeout, idle timeout or keepalive (zombie and half-open sockets) |
| A6 | Medium | authz / amplification | `Ping` broadcast/target; default on |
| **Availability and robustness** | | | |
| R1 | Medium | availability | Transient Redis/PG errors kill the worker and unsync all subscribers (reload storm); nothing restarts a dead worker |
| R2 | Medium | availability | Hub command loop does Redis I/O inline; serial, no timeouts, unbounded queue |
| R3 | Medium | availability | Reconnect storm after a Redis flush (missing registry row = inactive) |
| R4 | Medium | correctness | Aborted `EXEC` becomes an error; takeover detection can be skipped |
| R5 | Medium | shutdown | Shutdown is not awaited; deregistration is serial |
| R6 | Medium | task lifecycle | Hub background tasks and dispatcher are unsupervised |
| R7 | Medium | scalability / revocation | `SessionChecker` is sequential and extends the session TTL |
| R8 | Medium | availability | Redis pool keeps dead connections |
| R9 | Low | correctness | Registry listener not awaited, not retried, reconnect ignored |
| R10 | Low | correctness | `Connected` sent before registration; registration failure closes silently |
| R11 | Low | correctness | One close code (4000) for every drop; reason lost when the queue is full |
| R12 | Low | robustness | No drop guard for a cancelled or panicking socket task |
| R13 | Low | perf | Lifecycle events broadcast to every socket (O(N) per event); `UserConnected` carries the session key |
| R14 | Low | perf | `remove_connection_by_id` is O(N) under the write lock |
| R15 | Low | correctness | Tracker tick starves lifecycle events |
| R16 | Low | correctness | Topics ignored for targeted sends |
| R17 | Low | security | Host check trusts client headers |
| R18 | Low | compat | Unversioned Redis keys and channel; no environment prefix |
| R19 | Low | docs | Single-primary Redis assumed, undocumented |
| R20 | Low | cleanup | Dead code in hub registry/errors |
| R21 | Low | perf | Per-connection JSON/base64 serialization |
| R22 | Low | design | Decisions to make consciously (raw `user_id` in commands) |
| **Observability** | | | |
| O1 | Medium | logging | `Problem::log` dumps internals at info/error |
| O2 | Low | logging | Info-level spans/logs on hot paths |
| O3 | Low | metrics | Start-up self-test lines are counted |
| O4 | Low | metrics | Metrics gaps; repeated `DropConnection` per stream send |
| O5 | Low | perf | Registry notices enqueue commands for users the instance does not hold |
| O6 | Low | protocol | Protocol gaps (no version/connection id, silent drops, OpenAPI) |
| **Web client and replica** | | | |
| C1 | Medium | client | No watchdog on `waiting` and `syncing` |
| C2 | Medium | client | Backoff resets on every open |
| C3 | Medium | client | A `failed` nack drops the pending command |
| C4 | Medium | client | A persistent checkpoint mismatch loops forever |
| C5 | Medium | client | Stale state after a reconnect; `replaced` never set |
| C6 | Low | client | Overlapping `Sync` tail records a bad hash |
| C7 | Low | client | Snapshot decode accepts trailing bytes |
| C8 | Low | client | `StreamSubscribeFailed` retried every 5 s forever; `new WebSocket` unguarded |
| C9 | Low | docs | `net/` missing from the `client-web-architecture` skill |

Checked and found sound: see [Verified as sound](#verified-as-sound). Test gaps: see [Tests](#tests).

---

## Prod-deploy blockers

| ID | Finding | Where | Fix |
|---|---|---|---|
| D1 | **Fixed.** The prod TLS check used to parse the raw connection string, and tokio-postgres refuses `pool_timeout` and `max_size`, so prod start-up would abort. The check now lives in `create_postgres_pool(stage, cns)`, runs after the pool params are taken, and tests `sslmode` with `ConnectionString::check_property`. `check_postgres_tls` is deleted. | `crates/shine-infra/src/db/postgres/pg_connection.rs` (`create_postgres_pool`), `crates/shine-infra/src/db/cns_param.rs` (`check_property`) | None. |
| D2 | **TLS policy fails open** for any stage other than the literal `prod`. `Prod`, `staging` or a typo accepts `sslmode=disable`. Redis has no TLS check at all for now (fly.io reaches it over WireGuard, which does not support `rediss://`); `create_redis_pool` takes the stage but ignores it. | `pg_connection.rs` (`stage == PROD_STAGE`); `redis_connection.rs` (`create_redis_pool`); `PROD_STAGE`/`TEST_STAGE` in `crates/shine-infra/src/web/config/core_config.rs` | Allowlist the non-TLS stages (`TEST_STAGE`, `local`, `dev`) and require TLS for the rest. |
| D3 | **`panic = "abort"` on the plain release profile** turns any runtime panic into a process outage. `assert_not_blocking` panics on any pooled `XREAD`/`B*` command; identity has request-path `assert!`s. | `Cargo.toml:152-153`; `services/Dockerfile:36,58`; `redis_commands.rs:91-96`; `auth_handler.rs:461`, `external_login_handler.rs:63` | Scope `abort` to `release-lto` or the wasm build. Make `assert_not_blocking` return an error. |
| D4 | **Pre-deploy config checks.** `sql_cns` has no default and `server_config.prod.json` has no `db` block; Redis below 6.2 is only noted in a comment (`XRANGE (id`). | `services/builder/src/repositories/db.rs:13`; `services/builder/server_config.prod.json`; `repositories/command_inbox/redis/inbox.rs:67-70`; `redis_check.rs:6` | Confirm Key Vault supplies `sqlCns` (with `sslmode=require`) and `redisCns` (a `rediss://` URL is not required: the Redis TLS check is disabled because fly.io uses WireGuard). Confirm the Azure Redis version is at least 6.2. |
| D5 | **Migration replaced instead of added.** `V1__commands.sql` deleted, `V1__command_log.sql` added, and V1 was also edited in place. | `services/builder/sql_migrations/` | If any shared database ran main's V1, add a `V2` that drops the old table; otherwise reset those databases. |
| D6 | **Service migrations run under the 10 s `statement_timeout`.** `PgConnection::migrate` lifts it, but both services call `runner().run_async` directly. A failed `RESET` also masks the migration error. | `services/builder/src/repositories/db.rs:37-42`; `services/identity/src/repositories/db.rs:42-47`; `pg_connection.rs:146-151,313` | Route service migrations through `PgConnection::migrate`. Report the migration error before the `RESET` error. |
| D7 | **Config is not validated.** A huge heartbeat wraps to a negative TTL and `EXPIRE` with it deletes the key; zero is accepted. | `app_state.rs:56`; `repositories/hub_registry/redis/redis_hub_registry.rs:81,134` | Clamp and validate at start-up; cross-check session check and heartbeat against the TTL. |
| D8 | **Agent git-deny hook removed.** The `PreToolUse` deny for `git add/commit/push` is gone from `.claude/settings.json` and nothing in `CLAUDE.md` replaces it. | `.claude/settings.json`; `.claude/CLAUDE.md:104-105` | Restore the hook, or state the removal is intended and put the rule in `CLAUDE.md`. |

---

## Stream log correctness

### L1. Chain hash does not bind command identity: false "stored" verdict loses a command (High)

- `Command::content_hash` is the stable hash of the command payload only (`crates/shine-game/src/replica/state.rs:9`; e.g. `ChatCommand::stable_hash` hashes text, `PixelMapCommand` hashes x/y/color). `fold_chain_hash(parent, version, content_hash)` (`crates/shine-game/src/replica/stable_hash.rs:54`) adds only the version. `user_id`, `command_id` and `inbox_id` are not in any hash.
- `repositories/command_log/pg/append.rs:112-131` decides an overlap is "covered" purely by `chain_hash` equality at `head.version` and returns `Ok(head)` without writing; `StreamLogService::append` turns that into `AppendOutcome::Stored` when `head.version == last.version && head.chain_hash == last.chain_hash` (`stream_log_service.rs:124`). The only full-field comparison (`stream_persistence.rs:133-138`) runs on the `Behind` path, not on `Stored`.
- On `Stored`, the persistence advances its replica with its own commands, and `finish` trims the inbox through the batch's max `inbox_id` (`stream_persistence.rs:297-302`, `stream_log_service.rs:157-164`).

Failure scenario (two instances A and C, both loaded for `chat:global`, log at v10):
1. User U1 sends "gg" (entry e11), user U2 sends "gg" (e12).
2. A reads both, appends e11 as v11, trims through e11 (`XTRIM MINID`).
3. C missed the wake, or was busy (see L2), and polls only after the trim: it sees only e12 and, from its stale head v10, versions it v11. Payload hashes are equal, so the chain hash equals the log's v11. The append returns "covered", C reports `Stored`, applies e12 on its replica and trims the inbox through e12.
4. U2's message exists nowhere durable. C keeps appending on top of the log (its baseline hash equals the head hash). C's replica now has U2 as author of v11 while the log has U1, so C and A compute different state hashes. Whichever instance stores the next snapshot first wins `try_store_snapshot`; it only checks the snapshot's `chain_hash` against the command row (`pg/snapshot.rs:109-118`), never the state hash against the commands. If C's snapshot lands first the stored snapshot contradicts the log, and after `prune` removes the older commands (`pg/prune.rs`) nobody can detect it.

For `pixel-map` the collision probability is high (16x32x32 distinct payloads, same payload from many users); for chat it needs the same text. Also applies to one user repeating the same payload (double click).

Fix:
- Short term: in `PgCommandLogDb::append`, when the batch overlaps stored rows, compare the stored `command_id` and `user_id` (and `inbox_id`) to the batch rather than only the hash, or have the service compare `commands_after(baseline)` to the batch before returning `Stored`; on mismatch go through catch-up.
- Longer term: fold `command_id` and `user_id` into the per-command hash that feeds the chain (breaking change to the hash definition and needs a migration: version it, mirror in the web client `crates/shine-game` wasm).
- On load, verify `snapshot.hash` (L4) and make `try_store_snapshot` reject a snapshot whose state hash differs from an independent replay if cheap.

### L2. Persisted wakes are ignored: stale instances never reconcile (Medium)

- `notify_persisted` publishes `<stream>@<version>` (`repositories/command_inbox/redis/wake.rs:15-29`, `stream_persistence.rs:299-301`), but the registry collapses `InboxWake::Stream{persisted}` to `registry.wake()` (`stream_registry.rs:323`), and a wake only runs `process_inbox` (`stream_worker.rs:263`): poll the inbox after `read_index`. `Reconcile` (compare with the durable head) runs only in `preflight` (`stream_worker.rs:206`).
- Peers trim the inbox as soon as they persist (`stream_log_service.rs:157`). An instance that polls late (lost pub/sub wake, `room == 0` early return in `stream_relay.rs:150-153`, busy loop, or the 10 s recovery poll) finds nothing and keeps serving a stale tail. The recovery poll cannot recover entries that were already trimmed.

Failure scenario: instance C misses the wake of e11 (pub/sub is lossy, `command_inbox_types.rs:76`); A persists and trims it. C's subscribers never receive v11, and a new subscriber on C gets a `Sync` without it. It heals only when the next command reaches C and its append mismatches (reload), so an idle stream stays stale indefinitely. Also feeds L1.

Fix: on `persisted: Some(v)` with `v` beyond the worker's confirmed version, schedule a `Reconcile` (cheap `head` read) in the relay loop; also reconcile on the recovery tick when nothing was read.

### L3. No command dedup: retries, resends and leftovers duplicate commands (Medium)

- `CommandInboxDb::push` explicitly performs no check; `stream_relay.rs:153-190,266-267` keeps repeats as separate commands; no unique constraint on `(stream, command_id)` in `V1__command_log.sql` (only `(kind,id,inbox_id)`).
- `CommandFailure::Failed` means "may or may not have been queued" (`models/messages/stream.rs`), and `drain()` reads one inbox page only (`stream_worker.rs:319`), leaving the rest in Redis for up to the 1 h TTL (`app_state.rs:17`) with no signal to the client. The client also resends pending commands after a sync (`crates/shine-game/src/replica/client_stream.rs:198-205`). A client that resends gets two commands: chat `Add` is shown twice with the same message id (`ChatMessage.id = command_id`, `chat.rs:85,166`); `Remove`/`Replace` then touch only the first.
- `command_id` is client chosen and not checked for uniqueness per stream, so a user can reuse another message's id; clients that key messages by id will merge or overwrite them. Server-side `find_own` prevents unauthorized edits, so impact is presentation/consistency.

Fix: reject or ignore a second command with the same `(user_id, command_id)` at sequencing time (keep a bounded recent-id set per worker, seeded from the log tail on reload, and surviving other instances) or enforce a partial unique key in the log; alternatively put recently folded ids in the sync so the client drops them. Make `drain` loop until the page is short; tell clients when a command is dropped (ack by echo). If A4 (Lua hard limit) is implemented, do the dedup check in the same script so a retry of a stored command is not rejected as `Full`.

### L4. Replay trusts stored hashes (Low)

- `StreamPersistence::rebuild` (`stream_persistence.rs:195-214`) restores a snapshot with `Replica::from_snapshot(version, state, snapshot.chain_hash)` without checking that `stable_hash(state) == snapshot.hash`.
- `StoredCommand::to_envelope` (`command_log_types.rs:26-39`) uses the stored `command_hash`; `apply_next` checks only that the chain folds from that stored value, so a row whose `data` does not match its `command_hash` replays silently.

Fix: recompute `content_hash` from the decoded command and compare; compare `replica.hash()` with `snapshot.hash` on load; treat mismatch as `Corrupt`/reload from an older snapshot. Pairs with L1.

### L5. Worker replacement overlap and shutdown blind spot (Low)

`run()` calls `receiver.close()` before `drain()` (`stream_worker.rs:271-274`), and `is_finished()` is `messages.is_closed()` (`stream_worker.rs:94`), so the registry replaces the worker while it still drains (`stream_registry.rs:263-270`). Two workers for one stream then run in one process; correctness relies on the CAS and hash checks (and L1 applies to them). The replaced `Arc<StreamWorker>` is gone from `State.streams`, so `shutdown` (`stream_registry.rs:334-344`) never waits for its drain, and the process may exit with commands taken from the inbox but not yet persisted (they stay in Redis, but see L3).

Fix: report finished only when both tasks ended; keep draining workers in a side list that `shutdown` awaits.

### L6. Inbox cursor never detects a regression (Low)

`ingest` skips every entry with `inbox_id <= stored_inbox` (`stream_relay.rs:155-164,186-187,326`), and `read_index` is reset only on reload. If Redis ids ever move backwards (failover to a node with a slower clock, restored older dump with a later `last-id` after manual reset, stream recreated with lower ids), new valid commands are skipped silently until ids pass the stored value. Rare.

Fix: compare the stream's newest id with the cursor and reset on regression; treat a poll that returns only skipped entries repeatedly as an alert, or store the Redis epoch.

### L7. Inbox trim only after commands are stored (Low)

A malformed entry waits for a later valid store; a failed trim is only warned (`stream_persistence.rs:296-301`, `stream_log_service.rs:157-163`). The stored-position filter makes a missed trim harmless. Fix: trim on every successful pass.

### L8. `ensure_stream` writes on every worker load (Low)

`repositories/command_log/pg/stream.rs:~27-35` is an upsert (`ON CONFLICT DO UPDATE SET id = cl_streams.id`), so every reload makes a dead tuple, takes a write lock on the stream row and contends with append CAS. Fix: `DO NOTHING` plus a `SELECT` first; do not run it on read-only loads. See also A1 (create the row on the first accepted command, GC rows without commands/snapshots).

### L9. Load/prune race (Low)

`StreamLogService::load` reads head, latest snapshot, then the tail in three queries (`stream_log_service.rs:68-82`). Two newer snapshots plus a prune in between yields a tail starting above the snapshot, so `apply_next` fails with `Gap` (`Corrupt`) and the worker gives up. Rare. Fix: read in one snapshot-isolated transaction.

### L10. No PG statement timeout (Low)

Only the append transaction sets `lock_timeout`/`idle_in_transaction` (`pg/append.rs:20`). A hung query stalls the persistence task, `in_flight` never resolves, the optimistic tail fills to `relay_max_pending` and pushes go `Busy`. `drain` would also wait forever (bounded only by the registry shutdown timeout). Fix: `statement_timeout` for the stream log connections (see D6 for the migration side).

### L11. Poisoned registry mutex (Low)

`StreamRegistry` uses `std::sync::Mutex` with `.lock().unwrap()` everywhere and runs `spawn` under it (`stream_registry.rs:267`); a panic while held poisons it and every later call panics, including the Redis wake handler. Fix: do not spawn under the lock; recover from poison or use a non-poisoning lock.

### L12. Rolling deploys and command codec (Low)

An older binary decoding a command variant added by a newer one fails `to_envelope` with `Corrupt` and `give_up`s repeatedly for that stream (ingest drops it, but canonical replay cannot). Fix: version the command codec and keep new variants gated. Related: R18.

### L13. Grace timer can starve (Low)

The grace timer is the last arm of a `biased` select in `stream_worker.rs:235-267`. Fix: check the deadline at the top of each iteration.

### L14. Chat text is raw (Low)

Chat text is stored and relayed raw (`is_well_formed` checks length/emptiness only); clients must escape.

---

## Abuse and capacity

### A1. Allow-all authorization and uncapped stream creation (High)

- `services/stream_authorize/stream_authorize_service.rs:14-16` returns `Ok(true)` for every user and stream. Any authenticated user can read and write any `chat:<id>` / `pixel-map:<id>` (id: 1-64 chars of `[A-Za-z0-9_.:-]`, `StreamId::new`, `shine-game` `stream_id.rs:4,61`), and push 16 KB commands that are fanned out to every subscriber (per-subscriber JSON and base64 serialization in `forward_egress`).
- Nothing bounds how many streams one connection opens. `routes/ws/connect.rs:259-274` parses `streamId` and handles frames sequentially without limit; `HubService::subscribe_stream` (`services/hub/hub_service.rs:264-289`) calls `registry.open` (`stream_registry.rs:145-162`), which for each new id creates a `StreamState` with a `Backpressure` (`stream_registry.rs:152,348`), and that spawns a task polling Redis `XLEN` every 250 ms (`crates/shine-core/src/tokio/backpressure.rs:42`). `StreamDispatcher` (`tasks/stream_dispatcher.rs`) then calls `registry.subscribe`, which starts a worker (2 tasks, `stream_worker.rs:56,59`) whose preflight does `ensure_stream` (an `INSERT` into `cl_streams`, `pg/stream.rs:27-35`) and reload reads; the recovery poll adds one more Redis read per stream every 10 s. The worker message channel is unbounded (`stream_worker.rs:50`).
- `cl_streams` rows are never deleted (only commands/snapshots are pruned, `pg/prune.rs`).
- Authorization is not re-evaluated after subscribe (see R7).

Failure scenario: a client loops `{"type":"streamSubscribe","streamId":"chat:x<N>"}` on one socket. After a minute: ~10^5 backpressure pollers (~4x10^5 `XLEN`/s), 2x10^5 worker tasks, 10^5 permanent Postgres rows, Postgres pool (2 s checkout, `pg_command_log_db.rs:14`) exhausted so legitimate streams fail to persist and give up (R1). Memory in `State.streams/connections` grows until the connection closes (the rows stay forever).

Fix:
- Implement real authorization before this ships beyond a demo: deny by default, an allow-list or registry of creatable streams per kind (fixed ids for global chat/maps, a prefix tied to user or room, or a membership table); reject unknown ids before `open`.
- Cap streams per connection (8-32) in `StreamRegistry::open`, return `SubscribeFailure::Failed`/new `LimitExceeded`. Cap total open streams/workers per node.
- Create the `Backpressure` lazily (on first push) or share one poller per instance (`SCAN`/pipelined `XLEN`). Bound the worker message queue.
- Create the `cl_streams` row on the first accepted command; GC rows with no commands/snapshots; stop doing `ensure_stream` on read-only loads (L8).

### A2. No rate limit on commands/subscribes; backpressure punishes honest users (Medium)

- `connect.rs:170-195` processes every text frame; there is no per-connection or per-user limit on commands, subscribes or unsubscribes.
- Backpressure is per stream, polled every 250 ms (`stream_config.rs`, `backpressure.rs`): between polls a connection can push thousands of entries (each push is one Redis round trip) past the 1024 high mark. Once blocked, every push to that stream waits 2 s then returns `Busy`, and `send_stream_command` drops the pusher's connection (`hub_service.rs:301-323`). One abuser flooding `chat:global` therefore gets honest users disconnected.
- Each `StreamSubscribe` for an already-subscribed stream resets that connection's cursor and causes a full `Sync` (snapshot up to ~400 KB for chat plus the tail, `stream_relay.rs:332-341`, base64-encoded in `message.rs`). A few-dozen-byte request yields hundreds of KB of egress and relay CPU.

Fix: token bucket per connection (commands, subscribes), disconnect on repeated violation (close 1008); apply backpressure/penalty to the offender (per-user in-flight/quota), not whoever pushes next; coalesce repeated subscribes (ignore a resync request within N ms, or send a `Sync` at most once per interval per connection).

### A3. Hub control queue unthrottled; no connect throttle or connection cap (Medium)

- `hub_service.rs:93,104` (`HubServiceBuilder`) and `:325-330`: `control_tx` is an `UnboundedSender`. Every `subscribe`/`unsubscribe` (`:287-296`), every `request_connection`, every drop and every registry-change notification enqueues a `ControlCommand`. Invalid JSON and unsubscribes cost nearly nothing to send.
- `StreamUnsubscribe` for any valid id enqueues `UnsubscribeStream` (`hub_service.rs:648-653`) and calls `publish_internal`, which takes the subscribers write lock and pushes into the unbounded `StreamDispatcher` queue.
- A user can reconnect in a tight loop (`request_connection`, `:352-368`, `routes/ws/connect.rs:76-107`). Each connect costs an upgrade, two tasks, a channel, a Redis `SET`+`PUBLISH` (`:662-665`), a registry-change round trip on every other node and broadcasts (R13); a deploy or load-balancer flip amplifies it. There is no resume point.

Failure scenario: a client spams subscribe/unsubscribe, or one account reconnects continuously. Producers outrun the loop whenever it is slowed (R2). `control_rx` and the per-consumer subscription queues (`hub_subscribers.rs`, unbounded) grow without bound until OOM.

Fix: per-connection token bucket in the receive loop (shared with A2); per-user connect-rate limit before the upgrade; per-instance connection cap; bounded control channel with `try_send` + drop-connection on overflow for client-originated commands (unbounded or reserved lane for internal commands); jittered backoff for clients; expose queue depth as a metric (O4).

### A4. Inbox has no hard size limit (Medium)

`push` (`repositories/command_inbox/redis/inbox.rs:24-45`) is a MULTI/EXEC pipeline of `XADD`, `EXPIRE`, `PUBLISH`. Nothing bounds the stream length at write time. Backpressure is a cached `XLEN` sampled every 250 ms (`stream_registry.rs:152,348`, `:218,222`) with no reservation, so many connections pushing while the cached state is stale can go far past `inbox_backpressure_high`. A stream with no running worker is never drained, and every push re-arms the `EXPIRE` (so under load the key never expires), so the key can grow without bound (16 KB per entry).

`XADD ... MAXLEN` is not the fix (an earlier note suggested it): it does not reject, it evicts the oldest entries after appending. That silently drops commands that clients already saw optimistically.

Fix: reject at write time, atomically, with a Lua script (or Redis function) replacing the pipeline, keeping the soft limit as the cheap early path:

```lua
-- KEYS[1]=inbox key; ARGV: hard_limit, ttl, wake_channel, stream_id, command_id, user_id, data
if redis.call('XLEN', KEYS[1]) >= tonumber(ARGV[1]) then
  return false                      -- push() maps to RedisCommandInboxError::Full -> Busy
end
local id = redis.call('XADD', KEYS[1], '*', 'command_id', ARGV[5], 'user_id', ARGV[6], 'data', ARGV[7])
redis.call('EXPIRE', KEYS[1], ARGV[2])
redis.call('PUBLISH', ARGV[3], ARGV[4])
return id
```

- Hard limit = `inbox_backpressure_high * 1.1`; soft limit (poller) unchanged, so clients get `Busy` early without a Redis call per push.
- Add a `Full` variant to `RedisCommandInboxError` and map it to the same `Busy` result as backpressure. The command is not stored, so the client follows the existing `Busy` path.
- One round trip and O(1) `XLEN`, same cost as today. The script touches a single key, so it is Redis Cluster safe.
- Do the L3 dedup check in the same script if implemented.
- Test: push up to the hard limit, assert the next push returns `Full` and the stream length is unchanged.

### A5. No write timeout, idle timeout or keepalive on the socket (Medium)

- `routes/ws/connect.rs:332-375` (`forward_egress`): `ws_sender.send(...).await` (`:366`) has no timeout. A peer that stops reading (full TCP window) blocks `forward_egress` forever inside `send`.
- The hub meanwhile removes the connection once its 1000-slot queue fills (`hub_connections.rs:58`, `SlowConsumer`) and drops the sender, releasing the user's slot. But the send task never reaches `rx.recv()` again, so `serve_socket` (`connect.rs:201-231`) never sees `send_task` finish; the drain timeout/abort (`SEND_DRAIN_TIMEOUT`) only applies when the receive side ends first. The receive task stays blocked on a silent peer, so the select never fires. The socket, two tasks and up to 1000 queued messages (big `Sync`s pinned) persist until TCP gives up (possibly never with a non-reading, ACKing peer).
- No server-initiated ws `Ping` and no idle deadline anywhere. A half-open client (NAT loss, power off) stays "connected": `Heartbeat` (`tasks/heartbeat.rs`) refreshes the Redis TTL of whatever the hub map holds and checks nothing about the socket, and `SessionChecker` only checks the session.

Failure scenario: one account opens a socket, subscribes to a busy stream, stops reading, then reconnects (`Replaced`) and repeats. Each iteration leaves a zombie socket that the hub considers gone, so it is invisible to hub metrics.

Fix: wrap sends in `tokio::time::timeout` (about 10 s) and exit the send task on failure; abort `recv_task` when the hub removes the connection (have `forward_egress` return when `rx` closes); send periodic ws `Ping` frames and treat a missing `Pong` or no traffic within N intervals as dead; add an idle timeout; optionally set TCP keepalive/user timeout.

### A6. `Ping` is a broadcast and push channel (Medium)

- `routes/ws/connect.rs:305-322` and `hub_service.rs:427-443`: any logged-in user may send `{"type":"ping","id":"<255 chars>"}` and either `userId: <any uuid>` (`send_to_user`) or no `userId` (`broadcast` to every connection with `TopicKey::Ping`, which is every connection, `connect.rs:393`). No role check and no rate limit. User UUIDs are visible to other users in stream commands (`message.rs` `WireStreamCommand.user_id`).
- `HubConfig` defaults `ping_enabled: true` (`hub_service.rs:62,72`); `server_config.prod.json:16` sets it false, so this is a fail-open default and a risk on any environment that omits the key.

Failure scenario (where enabled): one socket sends a few thousand broadcast pings. Each fans out into N queue slots at `hub_connections.rs:175-197`. Any victim whose socket drains slower than the burst overflows its 1000-slot queue and is evicted as `SlowConsumer`; a targeted `send_to_user` does the same to one victim. It also lets users push attacker-chosen strings (255 B) to all clients.

Fix: default `ping_enabled` to `false`; restrict ping to an operator role; reject `userId` for non-admins; rate-limit; or delete the feature if unused.

---

## Availability and robustness

### R1. Transient errors escalate to worker death and reload storms (Medium)

- `ingest` errors (a Redis pool timeout) and any `PersistResult::Failed` (PG pool timeout 2 s, `Contention` after 8 attempts, a single failed read) call `give_up` (`stream_worker.rs:287,309,356`): all subscribers get `Unsynced` and are cleared, the worker ends, and the optimistic tail is discarded. No retry/backoff before giving up. A relay-task panic sends no `streamUnsynced` (`stream_worker.rs:99-107,147-152`).
- Nothing restarts a dead worker: `registry.wake` only wakes an existing worker, and `push` still accepts commands because it checks only `is_open` (`stream_registry.rs:203-236`). Until clients resubscribe, commands pile up in the inbox up to the high mark, then pushers are dropped `Busy`.
- After a Redis/PG blip, every stream on the instance gives up at once and all clients resubscribe together, each worker reloading snapshot plus tail from Postgres (thundering herd; amplifies A1).

Fix: retry transient errors (bounded, jittered) inside the relay/persistence loops and give up only on structural errors (`Corrupt`, `SnapshotDiverged`, repeated failure); have `push`/wake restart a worker for streams with open connections; add jitter to client resubscribe.

### R2. Hub command loop: inline Redis I/O, serial, no timeouts (Medium)

- `hub_service.rs:542,546` (ConnectUser: `create_registry_connection`, `is_registry_connection`), `:605` (DisconnectUser), `:625` (DropConnection), `:712` (registry change heartbeat), `:467-484,662-686,758-763` all await Redis from the one loop that serializes every state mutation, one round trip at a time with `WATCH`/`MULTI` (`redis_hub_registry.rs:117,184-208`, unchunked `MGET`).
- Pool checkout wait defaults to 30 s (`crates/shine-infra/src/db/redis/redis_connection.rs:256`, `max_size` default 10) and the pool is shared with the HTTP session lookups, the heartbeat and the session checker. Per-command timeout is 500 ms by default.

Failure scenario: the pool is exhausted (traffic burst or Redis latency). Each loop command can block up to 30 s, so connects, disconnects and slow-consumer drops queue (A3), stale sockets stay in the map, and `Connections` keeps broadcasting to dead channels. Shutdown pays the stall per connection (R5).

Fix: compare-and-set as Lua (also fixes R4), chunked heartbeat, a short hub-level timeout (1-2 s) around every registry call, bounded queue, and either move registry writes off the loop (the in-memory map is authoritative locally; async write with retry) or give the hub a dedicated small Redis pool.

### R3. Reconnect storm after a Redis flush (Medium)

A missing registry row is treated as inactive, so the heartbeat drops every healthy connection (`redis_hub_registry.rs:87-89,117-139`, `hub_service.rs:712-736`). Fix: re-create the row (`SET NX` plus TTL); drop only when it holds a different connection id.

### R4. Aborted `EXEC` becomes an error (Medium)

The `(usize,)` parse fails on a nil reply (`redis_hub_registry.rs:99,203`), so takeover detection can be skipped. Fix: the same Lua rewrite as R2.

### R5. Shutdown is not awaited (Medium)

- `main.rs:50-56`: the shutdown hook only calls `request_shutdown()` (fire and forget). `wait_shutdown_complete` is not called in production code (only in tests).
- `hub_service.rs:193`: the `StreamDispatcher` join handle is discarded. On `HubEvent::Shutdown` it runs `registry.shutdown(shutdown_timeout)` (default 10 s, `tasks/stream_dispatcher.rs`) to persist pending work, but `shutdown_complete` (`:505`) is signalled without waiting for it.
- `crates/shine-infra/src/web/web_app.rs:88-110,150-155`: `run_web_app` returns once axum finishes serving. The hub closes every socket during shutdown, so the server finishes quickly and the runtime is dropped, aborting the dispatcher mid-persist. Plain-HTTP mode has no 10 s cap either.
- `shutdown()` (`hub_service.rs:758-763`) deregisters connections one at a time, each a pool checkout, WATCH transaction and PUBLISH; at 10k connections this is tens of seconds, beyond the 10 s grace window, leaving stale rows for 900 s and publishing N change events to other nodes.
- `.expect("hub dispatcher dropped its shutdown signal")` (`hub_service.rs:402-406`) panics a caller if the loop task died (R6). A hub dropped without `Shutdown` leaks its tasks.

Impact: buffered stream commands not yet persisted to the log can be lost at deploy or restart (the Redis inbox may still hold them; the log append is what is at risk), and registry rows are left until the 900 s TTL.

Fix: keep the dispatcher handle and join it (bounded) before `shutdown_complete.send_replace(true)`; have the shutdown hook block (`block_in_place` or an async hook) on `wait_shutdown_complete` with a timeout before the server exits; pipeline or batch deregistration with bounded concurrency; return an error instead of `expect`; weak handles in the loops.

### R6. Unsupervised background tasks (Medium)

- `hub_service.rs:186-195`: `Heartbeat`, `SessionChecker`, the registry listener and `StreamDispatcher` are bare `tokio::spawn`s. Handles are joined only at shutdown (`:497-503`) and the dispatcher's handle is dropped (if it dies, stream subscriptions silently stop while the hub keeps buffering events for it in an unbounded channel).
- If `SessionChecker` panics, its subscription receiver drops; `Subscribers::publish` (`hub_subscribers.rs:36-46`) prunes it with only an `error!` log. Session revocation then stops being enforced for all live sockets with no alarm (`tasks/connection_tracker.rs:72,85`).
- If the command loop (`:466-506`) panics, `control_rx` is dropped, every `request_*` returns `SendCommandFailed`, existing sockets stay open and un-cleaned, status stays healthy, and `wait_shutdown_complete` panics.

Fix: spawn through a supervisor (restart consumers, or trip the health check and exit the process on loop failure); expose `HubStatus` liveness of each task.

### R7. Session checker is sequential and extends the session TTL (Medium)

- `tasks/session_checker.rs:33-54`: `get_current_user` is awaited for every tracked connection one after another every tick. With N connections and per-lookup latency L, a tick takes N*L (10k * 2 ms = 20 s; 100k exceeds the interval and `MissedTickBehavior::Delay` stretches it).
- While `on_tick` runs, the `tokio::select!` in `tasks/connection_tracker.rs:~125-136` does not drain the subscription, so lifecycle events pile up in the unbounded queue (see also R15).
- Revoked sessions keep their sockets for at least `session_check` (now 900 s in prod, was 60 s) plus tick time; a logged-out user keeps live stream access. Stream authorization is not re-evaluated after subscribe.
- `get_current_user` also extends the session TTL (`crates/shine-infra/src/session/current_user_service.rs:125-131`). For a WebSocket-only user the checker is the only keepalive, so `session_check` must stay well below `sessionTtl` (1800 s).
- A per-connection warning is logged each tick (O2).

Fix: bounded-concurrency checks (`buffer_unordered(32)`) or a batched lookup; revoke by event (publish on logout) rather than polling only. Cheaper check (allows a short interval again without the cost or the TTL side effect): add a read-only existence check to `CurrentUserService` that only tests the sentinel key (`EXISTS`, or one `MGET`/pipeline for all tracked connections per tick) and does not read the session data or run the `EXPIRE` pipeline. Keep the TTL refresh on the normal request path (and, if needed, a rare refresh in the checker, e.g. every 15 min).

### R8. Redis pool keeps dead connections (Medium)

`test_on_check_out(false)` and timeouts do not mark a connection broken (`crates/shine-infra/src/db/redis/redis_connection.rs:67,297`), so reused sockets time out for minutes after a failover or idle drop. Fix: flag timeouts and I/O errors as broken, or restore an idle PING.

### R9. Registry listener (Low)

`hub_service.rs:186-187,509-525`: the listener is spawned and not awaited; if `listen_to_registry_changes` returns `Err` the task logs and exits (a listener error is only logged); the node then misses cross-node takeover notifications and relies on the 300 s heartbeat (`HubConfig`), during which the old connection stays live although another node owns the user. The listener handles pub/sub reconnect internally via the `None` payload (`redis_hub_connection_db.rs:47-52`), but the initial subscribe failure is final and a reconnect does not recheck local users. Fix: await the subscribe, retry with backoff, recheck local users on reconnect, return the error.

### R10. `Connected` precedes registration (Low)

`connect.rs:149-165` sends `Connected` immediately after queueing `ConnectUser`. If `create_registry_connection` fails and the connection was not written (`hub_service.rs:542-551`), the command is dropped, `tx` closes, and `forward_egress` ends with `Close(None)`. The client saw `Connected` and then an unexplained close. Fix: reply `Connected` through the egress channel once registered (have the hub send it), or send a reasoned close frame on registration failure.

### R11. One close code for every drop; close reason lost (Low)

`hub_connections.rs:44-46` uses `try_send` for the final `UserDisconnected`; for `SlowConsumer` (queue full by definition) it always fails, so the client gets `Close(None)` (`connect.rs:339,372`) and cannot tell why. All reasons except `Shutdown` share code 4000 (`connect.rs:43-58`), so a client auto-reconnect cannot distinguish `Replaced` from an error, which risks a reconnect ping-pong between two tabs. Fix: carry the reason in a side channel (`oneshot`/watch) that does not depend on queue space; use distinct 4xxx codes: replaced (do not reconnect), session invalid (re-authenticate), transient (back off). Also relevant to C5.

### R12. No drop guard (Low)

A cancelled or panicking socket task leaves its entry registered (`routes/ws/connect.rs:117-232`). Fix: a guard that requests disconnection.

### R13. Lifecycle events fan out to every socket (Low)

`connect.rs:393` subscribes each socket to `TopicKey::Hub`, but `event_to_wire_message` (`:109-115`) drops hub events, and the socket only needs its own disconnect, which `RemovedConnection::notify` (`hub_connections.rs:44-46`) already delivers. Every connect/disconnect therefore costs a read-lock scan and N `try_send`s (`:179-190`, `hub_service.rs:579-584,777-790`, `hub_connections.rs:175-190`), during which the loop's writers wait. These events also consume the same 1000-slot queue that decides `SlowConsumer` eviction. A connect storm at N sockets is O(N^2) channel sends. `UserConnected` also carries the session key. Fix: do not subscribe sockets to `TopicKey::Hub`; keep it for internal subscribers; keep the session key out of the broadcast.

### R14. O(N) `remove_connection_by_id` (Low)

`hub_connections.rs:110-119` scans `users` for a connection id while holding the write lock. It is hit on every dead-connection prune (`hub_service.rs:623,788`) and, since `HubStreamEgress::send` enqueues a `DropConnection` per failed send (`:216-224`), repeated drops for an already-removed connection each perform a full scan that returns `None`. Mass eviction of a slow cohort costs O(N^2) with writers blocking all stream sends. Fix: store `user_id` in `Connection` (a reverse map) and remove directly by `connection_id`; dedupe in-flight drops (O4).

### R15. Tracker tick starves lifecycle events (Low)

`biased` polls the ticker first in `services/hub/tasks/connection_tracker.rs:129-136`. Fix: drain events before ticking, or bound tick work.

### R16. Topics ignored for targeted sends (Low)

Only `broadcast` checks topics; sockets lack the `Stream` topic yet receive stream messages (`hub_connections.rs:151-172`). Fix: check the topic in targeted paths, or drop the topic concept for stream messages.

### R17. Host check trusts client headers (Low)

`routes/ws/connect.rs:84`; `crates/shine-infra/src/web/extracts/target_host.rs:31-37`. The Origin check is the real defence. Fix: trust forwarded headers only from the proxy; do not treat the host check as a control.

### R18. Unversioned Redis keys and channel (Low)

A mixed-version deploy makes instances drop each other's users (`redis_hub_registry.rs:8-9,91,195`; `redis_command_inbox_db.rs:33`). Keys such as `hub-connection:` and `cmd-inbox:` also have no environment prefix. Fix: version the key and channel (and prefix by environment); treat an unknown format as foreign. Related: L12.

### R19. Single-primary Redis assumed, undocumented (Low)

`MGET` over many keys and a keyspace `SCAN` (`redis_hub_registry.rs:35-42,117,157`). Fix: document it, or make the calls cluster-safe and drop the `SCAN`.

### R20. Dead code (Low)

`list_connections` and its `SCAN` are test-only; `UserAlreadyConnected` is never built; `request_drop` has no `is_drop` assert (`hub_registry.rs:42-44`; `redis_hub_registry.rs:35,151`; `models/hub_error.rs:9`; `hub_service.rs:381,522`). Fix: delete or `cfg(test)`, assert.

### R21. Per-connection serialization (Low)

The shared message is base64 and JSON encoded once per connection (`routes/ws/message.rs:126-160`; `connect.rs:357-366`); CPU only. Fix: cache the encoded frame, only if profiling shows it.

### R22. Design decisions to make consciously (Low)

Every command carries the sender's raw `user_id` (`routes/ws/message.rs:46`); user UUIDs are therefore visible to all subscribers (relevant to A6). Decide, then document.

---

## Observability and logging

| ID | Finding | Where | Fix |
|---|---|---|---|
| O1 | **`Problem::log` dumps internals at info and error**, including Postgres and Redis errors and attacker-controlled `Host`/`Origin`. Every 4xx logs a multi-line dump. | `crates/shine-infra/src/web/responses/problem_detail.rs:150-159` | Log status, type and a short cause at info; the `sensitive` dump only at trace. |
| O2 | **Info-level spans and logs on hot paths.** A per-command hub span with the user id, registry spans, and a session-checker warning per connection per tick. | `hub_service.rs:475`; `redis_hub_registry.rs:53,78,150,174,183`; `tasks/session_checker.rs:51` | `debug`; log once per tick with a count. |
| O3 | **Start-up self-test lines are counted.** Every restart adds two warn and two error events, so alerts on the metric fire on every deploy. | `crates/shine-infra/src/web/web_app.rs:202-213`; `telemetry_service.rs:109-115` | Exclude them from the counter. |
| O4 | **Metrics gaps.** No queue depth, rejected connects or registry-error metrics; `notify` bypasses the metrics helper; a full connection gets a repeated `DropConnection` per stream send. | `services/hub/hub_metrics.rs`; `hub_status.rs:25-28`; `hub_connections.rs:44-46` | Add the metrics; dedupe in-flight drops. |
| O5 | **Registry notices enqueue a command for users the instance does not hold.** | `redis_hub_connection_db.rs:55-69`; `hub_service.rs:475-481` | Filter in the listener. |
| O6 | **Protocol gaps.** No version or connection id in `connected`; bad JSON, binary frames and bad stream ids are dropped silently; no close after repeated invalid frames; OpenAPI documents only `OK`. | `routes/ws/message.rs:55,78-86`; `connect.rs:67-75,188-193,261-263` | Document the schema; consider a close after repeated refusals. |

---

## Web client and replica

| ID | Finding | Where | Fix |
|---|---|---|---|
| C1 | **No watchdog on `waiting` and `syncing`.** A lost `streamSync` strands the client; only `unsynced` has a retry timer, re-armed only by another frame. | `client/web/src/experiments/pixel-map/index.ts:98-103`; `crates/shine-game/src/replica/client_stream.rs:173-176` | Re-send the subscribe with backoff for any non-live status. |
| C2 | **Backoff resets on every open.** If the server accepts the upgrade and then closes, the client retries every second forever. | `client/web/src/engine/net/websocket-stream-hub.ts:129-131,166` | Reset after the first sync or a minimum open time. |
| C3 | **A `failed` nack drops the pending command**, although the server says it may have been queued (see L3). | `client_stream.rs:253-257` | Keep it pending and resend (relies on server dedup, L3), or mark it unknown. |
| C4 | **A persistent checkpoint mismatch loops forever.** Every mismatch resubscribes with no counter. Checkpoints are now sent, so this is live. | `client_stream.rs:235-251` | A counter, then a move to `Unsynced`. |
| C5 | **Stale state after a reconnect.** The stream is never reset to `Waiting`; paints are not gated by the connection state; `replaced` is never set although the type has it. | `pixel-map/index.ts:46-51,164-184`; `stream-hub.ts:1`; `pixel-map-ui.ts:14` | Reset or resync on reconnect; gate paints; handle `replaced` (no reconnect; needs distinct close codes, R11). |
| C6 | **Overlapping `Sync` tail records a bad hash.** `on_sync` still calls `apply_wire` and ignores a skipped outcome. The server sends non-overlapping tails today. | `client_stream.rs:189-193,220,305-308` | Honour `ApplyOutcome::Skipped`. |
| C7 | **Snapshot decode accepts trailing bytes.** `decode_snapshot` uses `postcard::from_bytes`; state and chain hashes come from the same frame. | `crates/shine-game/src/replica/codec.rs:15-17,35-41`; `client_stream.rs:280-286` | Canonical check for snapshots; verify the chain hash against a checkpoint. |
| C8 | **`StreamSubscribeFailed` is retried every 5 s forever**, including permanent reasons; `new WebSocket` is unguarded. | `client_stream.rs:155-159`; `websocket-stream-hub.ts:50` | Stop on permanent reasons; try/catch into the backoff path. |
| C9 | **`net/` is missing from the `client-web-architecture` skill's layout table.** | `.claude/skills/client-web-architecture/SKILL.md:14-25` | Add a row. |

---

## Tests

| Gap | Where |
|---|---|
| Stress tests are small (12 to 48 users) and run with heartbeat, session check and dispatcher idle at 3600 s, so scale, cascades, loop panic and Redis flush are untested. | `testing/hub/test_hub.rs:30,44`; `services/hub/tests/contention.rs:128,199,310,524` |
| Missing fault and race cases: listener failure, `WATCH` conflict, half-open or oversized frames, id regression; fixed sleeps and `tick * N` waits remain. | `services/hub/tests/fault.rs`; `contention.rs:513,582`; `lifecycle.rs:440,479`; `routes/ws/connect_test.rs:294`; `stream_worker_test.rs:120,271` |
| Mutation blind spots: the close-code test compares the constant to itself, no test drives `AppState` wiring, the `SEND_DRAIN_TIMEOUT` abort is untested. | `routes/ws/connect_test.rs:104-112`; `app_state.rs:56`; `connect.rs:65,228` |
| Rust, `shine-infra`: the prod TLS check is tested for rejections only (no database needed); there is no test for odd stage names (D2) or for accepting `sslmode=require` through the pool. | `crates/shine-infra/src/db/postgres/pg_connection.rs` |
| Rust, builder: the stale-instance identical-content race (L1), a duplicate resent command (L3), `ConnectUser` registry failure (R10), inbox push at the hard limit returns `Full` (A4), and `ensure_stream` plus migration against a database that has main's V1 (D5). | `services/builder/src/services/command_stream/` |
| Rust, `shine-game` `client_stream`: mid-batch gap, a command that fails to decode, a checkpoint mismatch while `Waiting`, overlapping `Sync` tails, trailing snapshot bytes, `HASH_HISTORY` eviction at 256, a `failed` nack on a known pending command, a command folded into the snapshot but absent from the tail. | `crates/shine-game/tests/replica/client_stream.rs` |
| TypeScript: only two tests exist for the hub. Missing: stale live after reconnect, a hang in waiting, an immediate close after open with growing delay, `connect()` twice, the `unsynced` timer re-arming, `resync()` while closed or replaced. Nothing tests the pixel map experiment's stream handling. | `client/web/src/engine/net/websocket-stream-hub.test.ts` |
| `shine-test` panic hook exits the whole run on a background-thread panic; a test that expects a spawned-task panic cannot exist. The fuzz CI job uses an unpinned nightly and an uncached `cargo install`. | `crates/shine-test/src/lib.rs` (~65-84); `.github/workflows/fuzz_ci.yml` |

---

## Verified as sound

No action needed.

**Command stream**
- Inbox push is one atomic `MULTI` (XADD+EXPIRE+PUBLISH) (`redis/inbox.rs:24-45`); stream keys and wake payloads only contain validated `StreamId` characters (no injection; `@` is excluded); `XRANGE` exclusive ranges, `MINID` trim and the numeric `InboxId` ordering are right.
- Append: dense-run assumptions are guaranteed by the relay (`reversion_optimistic`), index arithmetic is guarded (`pg/append.rs:106-131`), `ON CONFLICT DO NOTHING` + CAS on `(version, chain_hash)` serialize concurrent writers and roll back losing inserts; no orphan rows; `i64`/`usize` casts are range-checked by preceding branches.
- Worker/registry lifecycle ordering (`open` -> hub event -> `subscribe`, late subscribe after disconnect, queued subscribes answered with `Unsynced` on exit) holds up; the select loop's branches are cancellation safe and no std lock is held across an await.
- Reload resets `read_index` to the last stored inbox id, so entries sequenced optimistically but not persisted are re-read, not lost (except via L1).

**Hub and websocket**
- Origin and Host: `Origin` header is required (`crates/shine-infra/src/web/extracts/origin.rs:46-50`); an empty allow-list denies everything (`settings/mod.rs:9-15`); regexes are anchored (`app_state.rs:37-48`). Cross-site websocket hijack is mitigated; the session is authenticated at upgrade (`CheckedCurrentUser`, fingerprint-bound). WS host/origin allow-lists fail closed.
- Size limits: `max_message_size` and `max_frame_size` are 32 KB (`connect.rs:62,104`); commands are re-checked against 16 KB after base64 decode, a canonical postcard re-encode check and `is_valid_command` / per-kind well-formedness (`stream_registry.rs:218`); ping id is capped at 255. Binary frames are ignored; postcard decode failures never panic.
- Identity: `user_id` and `connection_id` always come from the server-side session and `request_connection`, never from the client payload (`connect.rs:149`, `hub_service.rs:257-263`).
- Map invariants: `users` and `connections` are mutated only on the loop; replacement removes the old entry before registering (`hub_service.rs:556-578`); stale disconnects are keyed by connection id and cannot remove a fresh reconnect (`hub_connections.rs:98-107`); lock scopes in `Connections` contain no awaits and only non-blocking `try_send`.
- Ordering: `ConnectUser`, `SubscribeStream` and `DisconnectUser` are FIFO on one channel; a subscribe that lands after the connection is gone is undone (`hub_service.rs:637-646`). `open` and `request_stream_subscribe` have no await between them (`:284-288`), so aborting the receive task cannot leave an orphaned open stream.
- Slow consumers: egress to sockets is bounded with `try_send` and never awaits under a lock; internal consumers are unbounded by design (see A3 and R7 for the consequence).
- Logging: `SessionKey` Debug is redacted (`session_key.rs:16-23`); client payloads appear only at `trace`.

## Not verified

Redis pool checkout timeout (affects how long a `push` can park with Redis down), client-side handling of `Unsynced`/duplicate ids beyond what is listed above, and the `shine_infra` pub/sub listener internals beyond reading `redis_listener.rs`.
