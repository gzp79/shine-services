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
| D1 ✅ | Fixed | config | Prod TLS check rejected pool params, prod start-up would abort |
| D2 ✅ | Intentional | config | TLS is required only for the literal `prod` stage |
| D3 ✅ | Intentional | availability | `panic = "abort"` on the release profile; a panic is a critical bug, not a survivable condition |
| D4 ✅ | No action | config | Missing config or Redis < 6.2 already stops start-up |
| D5 ✅ | Intentional | migration | V1 migration replaced instead of added; databases were purged |
| D6 ✅ | Fixed | migration | Service migrations ran under the 10 s `statement_timeout` |
| D7 ✅ | No action | config | Config is trusted; a zero interval already stops start-up |
| **Stream log correctness** | | | |
| L1 ✅ | Fixed | soundness | Chain hash ignored `user_id`/`command_id`/`inbox_id`; false "stored" verdict lost a command |
| L2 | Low / Medium | multi-instance | Persisted-version wakes are ignored; after a missed wake an instance's clients stay stale (Medium with several instances, none with one). No data loss since L1 |
| L3 ✅ | Intentional | idempotency | No dedup by `command_id`: the id is a client-owned placeholder |
| L4 ✅ | Fixed | integrity | Replay trusted stored `snapshot.hash` and `command_hash`; both are now recomputed on load |
| L5 | Low | lifecycle | `is_finished()` true while a worker still drains; replacement overlap; draining worker invisible to `shutdown` |
| L6 | Low | correctness | Inbox cursor never detects a Redis id regression |
| L7 | Low | correctness | Inbox trim only after commands are stored; failed trim only warned |
| L8 | Low | perf | `ensure_stream` writes on every worker load |
| L9 | Low | correctness | Load/prune race yields a `Gap` |
| L10 | Low | availability | No PG `statement_timeout` on reads/appends |
| L11 ✅ | Moot | robustness | Poisoned registry mutex (cannot happen under `panic = "abort"`) |
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
| R1 ✅ | Fixed | availability | Transient Redis/PG errors killed the worker and unsynced all subscribers; the calls are now retried. Resubscribe jitter moved to C8 |
| R2 | Medium | availability | Hub command loop does Redis I/O inline; serial, no timeouts, unbounded queue |
| R3 ✅ | Intentional | availability | Reconnect storm after a Redis flush (missing registry row = inactive) |
| R4 ✅ | Fixed | correctness | Aborted `EXEC` becomes an error; takeover detection can be skipped |
| R5 ✅ | Fixed | shutdown | Shutdown was not awaited; deregistration was serial |
| R6 ✅ | Fixed | task lifecycle | Hub background tasks and dispatcher were unsupervised |
| R7 | Medium | scalability / revocation | `SessionChecker` is sequential and extends the session TTL |
| R8 | Medium | availability | Redis pool keeps dead connections |
| R9 | Low | correctness | Registry listener not awaited, not retried, reconnect ignored |
| R10 | Low | correctness | `Connected` sent before registration; registration failure closes silently |
| R11 | Low | correctness | One close code (4000) for every drop; reason lost when the queue is full |
| R12 | Low | robustness | No drop guard for a cancelled socket task |
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
| O1 ✅ | Accepted | logging | `Problem::log` dumps internals at info/error: a documented exception, the only root-cause info in prod |
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
| C8 | Low | client | `StreamSubscribeFailed` / `unsynced` retried every 5 s forever with no jitter; `new WebSocket` unguarded |
| C9 | Low | docs | `net/` missing from the `client-web-architecture` skill |

Checked and found sound: see [Verified as sound](#verified-as-sound). Test gaps: see [Tests](#tests).

---

## Prod-deploy blockers

All closed. Kept short: what is left is a convention to keep, not work.

| ID | Finding | Where |
|---|---|---|
| D1 ✅ | **Fixed.** The prod TLS check rejected the pool params and would abort start-up; it now runs in `create_postgres_pool` after the params are taken. | `crates/shine-infra/src/db/postgres/pg_connection.rs`, `db/cns_param.rs` |
| D2 ✅ | **Intentional.** Only the literal `prod` stage requires TLS (a stage typo fails config load). Redis has no TLS check (fly.io WireGuard). A new stage on a hosted database needs its own TLS check. | `pg_connection.rs`, `redis_connection.rs`, `core_config.rs` |
| D3 ✅ | **Intentional.** `panic = "abort"` on release: a panic is a critical bug, the platform restarts the service. Known panic sites are programmer-error guards. Findings that only matter when a panic is survived are moot (L11) or narrowed (R6, R12). | `Cargo.toml`, `services/Dockerfile` |
| D4 ✅ | **No action.** Missing config or Redis < 6.2 stops start-up (`check_redis`, `MIN_REDIS_VERSION`). | `builder/src/repositories/db.rs`, `redis_check.rs` |
| D5 ✅ | **Intentional.** V1 was replaced in place; the databases were purged. Convention: once a V1 has run on a database that is not purged, change it only with a new migration. | `services/builder/sql_migrations/` |
| D6 ✅ | **Fixed.** Migrations ran under the 10 s `statement_timeout`; both services now call `PgConnection::run_migrations`, which lifts it for the run. No test (needs a database). | `pg_connection.rs`, both `repositories/db.rs` |
| D7 ✅ | **No action.** Config is trusted; a zero interval panics at start-up, which D3 turns into a stop. | `app_state.rs`, `connection_tracker.rs` |

---

## Stream log correctness

### L1 ✅. Chain hash did not bind command identity (Fixed)

The chain hash covered only the payload and version, so an append of identical content from another user or id looked "covered", reported `Stored`, and lost the command. **Fixed:** `InsertCommands` also compares `command_id`, `user_id` and `inbox_id` (a mismatch is `LogDiverged`, taking the `Behind` path), and `fold_chain_hash` now folds `command_id` and `user_id` in. Breaking change of the hash: stored chain hashes and snapshots are invalid, purge the databases (`docs/plans/purge-databases.sql`) and deploy server and web client together. Tests: `a_batch_with_the_same_hashes_but_another_command_identity_is_diverged` (`pg_command_log_db_test.rs`), `the_same_content_from_another_author_or_command_id_has_another_chain_hash` and `a_command_of_another_author_with_the_chain_hash_of_the_original_is_rejected_as_diverged` (`crates/shine-game/tests/replica/replica.rs`).

### L2. Persisted wakes are ignored: stale instances never reconcile (Low with one instance, Medium with several)

Status: open, analysed, no code changed. Re-read against the code after the L1 fix; by reading only, nothing was run.

Severity depends on how many builder instances serve a stream. With one instance there is no peer to trim the inbox, so it cannot happen. With several it is **Medium**: the data is never lost (see "Impact after L1"), but the clients of an instance that missed a wake silently stop receiving commands. They are updated normally by their instance's own worker, which reads each new entry from the inbox on the wake and sends it before it is persisted; persistence is the backup, not the delivery path. After a missed wake nothing tells those clients they are behind: checkpoints are only sent after the instance's own persists, and the web client has no watchdog (C1). They recover when the next command reaches that instance, or when they resubscribe. On a busy stream that is seconds, on a quiet one it can be indefinitely.

#### How the wakes work today

- A push publishes `<stream>` on the wake channel `cmd-inbox-wake`; after an instance persists a batch it publishes `<stream>@<version>` (`notify_persisted`, `repositories/command_inbox/redis/wake.rs:15-29`, called from `stream_persistence.rs:299-301`). Every instance with a worker for the stream hears both.
- The registry's `listen` handler (`stream_registry.rs:323`) throws the version away: `InboxWake::Stream { stream_id, .. }` becomes `registry.wake(&stream_id)`, which is only `Notify::notify_one` on the worker. A listener reconnect gives `InboxWake::All`, which wakes every worker the same way.
- A woken worker runs `process_inbox` (`stream_worker.rs:263`): read the Redis inbox after its own `read_index`, version what it finds, submit it to the persistence. It never compares its head with the log.
- `PersistenceWork::Reconcile` (compare the confirmed head with the durable head, reload on mismatch) is sent from one place only, `preflight` (`stream_worker.rs:206`), once when the worker starts. Otherwise an instance finds out that it is behind only when its own append fails, and that needs a new command to reach it.

#### Why a missed wake does not heal

- Peers trim the inbox through the entries they stored as soon as they persist (`stream_log_service.rs:157`).
- In the normal case instances A and C both read the same entry and both try to append it. One wins, the other sees it covered, and both converge. That is the design.
- If C misses the wake, A persists and trims. When C looks, the entry is gone from the inbox, so there is nothing to read. The recovery poll (`inbox_recovery_poll_interval`, default 10 s) does not help: it runs the same `process_inbox`, which reads the now empty inbox.
- The reconnect case has the same hole. `wake_all` after a reconnect only re-reads the inbox, so whatever a peer persisted and trimmed during the outage is invisible.

Failure scenario: instance C misses the wake of entry e11 (Redis pub/sub is lossy by design, `command_inbox_types.rs:76`, for example around a failover or a restart); A persists e11 as v11 and trims it. C's subscribers never receive v11, and a new subscriber on C gets a `Sync` built from C's stale state. An idle stream stays stale indefinitely. When the next command reaches C, C versions it from its stale head and sends that frame to its subscribers at the wrong version; its append then fails (`Behind`), C reloads and resyncs the subscribers. So they see a wrong frame first, then a correction.

#### Impact after L1

Before the L1 fix this could also lose a command: C's stale append of identical content was accepted as `Stored`. With the identity check in `InsertCommands` and the ids in the chain hash, that append now fails and takes the reload path. What remains is **staleness and one wrong frame** on the lagging instance, not data loss..

#### Causes that do not hold

The audit named three ways to be late. Only the first is real:

- Lost pub/sub wake: real. This is the cause.
- `room == 0` early return in `StreamRelay::ingest` (`stream_relay.rs:150-153`): not a cause. While the optimistic tail is full the read is skipped, but the persistence answer (`settle`) calls `notify_one` again, so the entries are read afterwards and nothing is lost.
- Busy loop: not a cause. `Notify::notify_one` keeps one pending permit while the worker is busy, so a wake that arrives meanwhile is not dropped.

#### Proposed fix

1. **Use the persisted version.** The worker keeps the highest persisted version it has heard of (`AtomicI64::fetch_max`, set by the registry handler, so `wake` carries it). In the relay loop, when that version is above `confirmed_version` and no persistence is in flight, send `PersistenceWork::Reconcile`. That is one cheap `head` read per peer persist; the answer path (`PersistResult::Current` or `Reloaded`) already exists. Do the comparison after `settle`: an instance's own persist announcement arrives before its own state is updated, and would otherwise cause a pointless reconcile.
2. **Handle the reconnect.** On `InboxWake::All`, flag every worker as needing a reconcile, which closes the trimmed-during-the-outage hole directly.
3. **No periodic reconcile.** A `head` read every 10 s per stream per instance costs a lot and adds little once 1 and 2 exist. The earlier idea of reconciling on the recovery tick when nothing was read is dropped.

#### Test plan

- Two workers on one inbox and one log. Worker A persists and the inbox is trimmed; C gets only the persisted-version wake (not the command wake). Expect C's subscriber to receive the new commands. Existing worker tests simulate wakes with `worker.wake()` (`stream_worker_test.rs`); the wake would need an optional persisted version.
- Reconnect: wake every worker with `All` after a peer persisted and trimmed; expect a reconcile and a resync.
- No-op: a persisted version that is not above `confirmed_version` must not send a `Reconcile`.

#### Open questions

- Where the highest persisted version lives: on `StreamWorker` next to `wake`, read by the relay loop.
- Whether a reconcile may be sent while a persistence is in flight (the loop currently allows one in-flight request); the proposal waits for it to finish.

### L3 ✅. No command dedup (Intentional)

`command_id` is a client-owned placeholder; the server does not check it for uniqueness. A repeat is a new, ordered command (the chain hash binds the id, L1). Accepted: a resent pending command can be applied twice, and a user can reuse another message's id (`find_own` still blocks edits of other users' messages). No dedup in the relay, log or inbox script (A4). `drain()` reading one inbox page moved to L5.

### L4 ✅. Replay trusted stored hashes (Fixed)

`StoredCommand::to_envelope` now recomputes `command_hash`, and `StreamPersistence::rebuild` compares the restored state hash with `snapshot.hash`; a mismatch is `CommandStreamError::Corrupt`. Tests in `stream_persistence_test.rs`. Not done: a `Corrupt` load gives the worker up (no fallback to an older snapshot), and `try_store_snapshot` still checks only the snapshot's `chain_hash`.

### L5. Worker replacement overlap and shutdown blind spot (Low)

`run()` calls `receiver.close()` before `drain()` (`stream_worker.rs:271-274`), and `is_finished()` is `messages.is_closed()` (`stream_worker.rs:94`), so the registry replaces the worker while it still drains (`stream_registry.rs:263-270`). Two workers for one stream then run in one process; correctness relies on the CAS and hash checks (and L1 applies to them). The replaced `Arc<StreamWorker>` is gone from `State.streams`, so `shutdown` (`stream_registry.rs:334-344`) never waits for its drain, and the process may exit with commands taken from the inbox but not yet persisted (they stay in Redis for up to the 1 h TTL, `app_state.rs:17`, with no signal to the client). Also, `drain()` reads one inbox page only (`stream_worker.rs:319`), so a longer inbox is left unread on shutdown.

Fix: report finished only when both tasks ended; keep draining workers in a side list that `shutdown` awaits; make `drain` loop until the page is short.

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

### L11 ✅. Poisoned registry mutex (Moot)

`StreamRegistry` uses `std::sync::Mutex` with `.lock().unwrap()` everywhere and runs `spawn` under it (`stream_registry.rs:267`). Poisoning needs a panic while the lock is held; under `panic = "abort"` (D3) the process dies instead, so there is no poisoned state to handle. No action.

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

### R1 ✅. Transient errors escalate to worker death and reload storms (Fixed)

Redis inbox reads already retried (`poll_inbox`, `inbox_max_retries`). **Fixed:** `StreamLogService` now retries `head`, `load` and `append` on `ServiceUnavailable` (`log_max_retries` 8, `log_retry_min_delay` 100 ms, `log_retry_max_delay` 1 s, jittered); a repeated append is safe because the log finds an already stored batch covered. Snapshot store, prune and trim stay best effort; `Contention` is not retried. Tests in `stream_worker_test.rs` (`FlakyLog`): `a_log_that_fails_for_a_moment_does_not_end_the_stream`, `..._while_loading_...`, `a_log_that_keeps_failing_ends_the_stream_once_the_retries_are_spent`. Left to the client: a given-up worker is replaced on the next `subscribe`, and the resubscribe has no jitter (C8).

### R2. Hub command loop: inline Redis I/O, serial, no timeouts (Medium)

- `hub_service.rs:542,546` (ConnectUser: `create_registry_connection`, `is_registry_connection`), `:605` (DisconnectUser), `:625` (DropConnection), `:712` (registry change heartbeat), `:467-484,662-686,758-763` all await Redis from the one loop that serializes every state mutation, one round trip at a time with `WATCH`/`MULTI` (`redis_hub_registry.rs:117,184-208`, unchunked `MGET`).
- Pool checkout wait defaults to 30 s (`crates/shine-infra/src/db/redis/redis_connection.rs:256`, `max_size` default 10) and the pool is shared with the HTTP session lookups, the heartbeat and the session checker. Per-command timeout is 500 ms by default.

Failure scenario: the pool is exhausted (traffic burst or Redis latency). Each loop command can block up to 30 s, so connects, disconnects and slow-consumer drops queue (A3), stale sockets stay in the map, and `Connections` keeps broadcasting to dead channels. Shutdown pays the stall per connection (R5).

Fix: a short hub-level timeout (1-2 s) around every registry call (compare-and-set as Lua and the chunked heartbeat are done, R3/R4), bounded queue, and either move registry writes off the loop (the in-memory map is authoritative locally; async write with retry) or give the hub a dedicated small Redis pool.

### R3 ✅. Reconnect storm after a Redis flush (Intentional)

**Intentional, no action.** A missing registry row is inactive: after a Redis flush or failover the next heartbeat drops the node's connections and clients reconnect. Redis is the truth about who holds the user and only a connect writes a row. The proposed `SET NX` re-create was rejected: a heartbeat can write a row back for a connection the hub just removed (phantom row until TTL). A Redis restart is rare; if the storm ever matters, spread the reconnects on the client (C8). Pinned by `redis_hub_registry_test.rs` and the lifecycle heartbeat tests.

### R4 ✅. Aborted `EXEC` becomes an error (Fixed)

An aborted `EXEC` came back as `Err(Incompatible type ... value was nil)` (checked by forcing it), so the registry-change recheck was skipped. **Fixed:** the heartbeat and the removal are single Lua scripts (`HEARTBEAT`, `REMOVE`, `redis_hub_registry.rs`): atomic, no `WATCH`; the batch runs in chunks of 256 and the removal publishes in the same step. Tests in `redis_hub_registry_test.rs`; no race test (the window cannot be placed from the API; a 200-round attempt on the old code never hit it).

### R5 ✅. Shutdown is not awaited (Fixed)

Most of it was fixed already: the shutdown hook awaits `wait_shutdown_complete` (`main.rs`), the hub keeps and joins the stream dispatcher handle, which bounds the workers' last persist by `stream_shutdown_timeout`. **Fixed now:** `HubService::shutdown` told the sockets and removed the registry rows one at a time, so with a slow Redis every connection cost its own wait (30 connections at 300 ms took 9.4 s, and a Redis outage costs a pool wait each). It now closes every socket first, then removes the rows 32 at a time within `SHUTDOWN_REGISTRY_TIMEOUT` (5 s), the rest expires with the TTL. `wait_shutdown_complete` no longer panics (`expect`) if the loop task is gone, it returns. Test: `shutdown_with_a_slow_registry_takes_no_longer_than_one_round_of_removals` (`fault.rs`, toxiproxy latency), failed before with 9.4 s. Not done: the hook itself has no time limit, it relies on the two bounds above.

### R6 ✅. Unsupervised background tasks (Fixed)

The command loop, heartbeat, session checker, registry listener and stream dispatcher are spawned once and only joined at shutdown, so one that ended on its own (an early return, not a panic, D3) left the hub half alive with a healthy status: no session revocation, sockets never cleaned up, or stream subscriptions silently dead. **Fixed:** `TaskSupervisor` (`shine-core`, `tokio/task_supervisor.rs`, reusable by any service; its `spawn` reports a task that returns) records a task that ends while the hub is not shutting down (`Shutdown` sets the flag first), logging it at `error`. The supervisor is one per service, in `CoreServices` (`shine-infra`, `web_app.rs`), and the hub gets it through `with_task_supervisor` (tasks named `hub ...`); the web app's shutdown signal waits on it next to SIGTERM and `ShutdownHandle` requests, marks the supervisor stopping when any shutdown begins, then runs the hooks and drains the server, so the platform restarts the service. Nothing is needed in a service's `main.rs`. The registry listener counts only when its initial subscribe fails (R9: it returns after subscribing otherwise). Tests: `task_supervisor.rs` (first failure kept, none while stopping, a returned task is reported, an aborted one is not), `lifecycle.rs` (`a_shutdown_does_not_report_the_ending_tasks_as_failed`, with the stream dispatcher running, fails without the flag; `a_task_that_ends_while_the_hub_runs_is_reported_as_failed`), `web_app.rs` (`ShutdownHandle`, a failed task requests the shutdown, none once it started). No test makes a real task end by itself (nothing outside the hub can close its subscriptions).

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

A cancelled socket task (for example when the `serve_socket` future is dropped) leaves its entry registered (`routes/ws/connect.rs:117-232`). A panicking task is not a case here: it aborts the process (D3). Fix: a guard that requests disconnection.

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
| O1 ✅ | **Accepted.** `Problem::log` dumps the problem (Postgres/Redis errors, attacker-controlled `Host`/`Origin`) at `error`/`info`: a documented exception to "sensitive data: trace only", it is the only root-cause information in prod. Revisit when errors carry a safe cause label. | `crates/shine-infra/src/web/responses/problem_detail.rs:150-159` | None. |
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
| C3 | **A `failed` nack drops the pending command**, although the server says it may have been queued. | `client_stream.rs:253-257` | Keep it pending and resend (the server does not dedup, L3, so the client has to accept a possible repeat), or mark it unknown. |
| C4 | **A persistent checkpoint mismatch loops forever.** Every mismatch resubscribes with no counter. Checkpoints are now sent, so this is live. | `client_stream.rs:235-251` | A counter, then a move to `Unsynced`. |
| C5 | **Stale state after a reconnect.** The stream is never reset to `Waiting`; paints are not gated by the connection state; `replaced` is never set although the type has it. | `pixel-map/index.ts:46-51,164-184`; `stream-hub.ts:1`; `pixel-map-ui.ts:14` | Reset or resync on reconnect; gate paints; handle `replaced` (no reconnect; needs distinct close codes, R11). |
| C6 | **Overlapping `Sync` tail records a bad hash.** `on_sync` still calls `apply_wire` and ignores a skipped outcome. The server sends non-overlapping tails today. | `client_stream.rs:189-193,220,305-308` | Honour `ApplyOutcome::Skipped`. |
| C7 | **Snapshot decode accepts trailing bytes.** `decode_snapshot` uses `postcard::from_bytes`; state and chain hashes come from the same frame. | `crates/shine-game/src/replica/codec.rs:15-17,35-41`; `client_stream.rs:280-286` | Canonical check for snapshots; verify the chain hash against a checkpoint. |
| C8 | **`StreamSubscribeFailed` is retried every 5 s forever**, including permanent reasons, and an `unsynced` resubscribe has no jitter, so after a server-side give-up (R1) every client of an instance comes back at once; `new WebSocket` is unguarded. | `client_stream.rs:155-159`; `websocket-stream-hub.ts:50` | Stop on permanent reasons; jittered backoff; try/catch into the backoff path. |
| C9 | **`net/` is missing from the `client-web-architecture` skill's layout table.** | `.claude/skills/client-web-architecture/SKILL.md:14-25` | Add a row. |

---

## Tests

| Gap | Where |
|---|---|
| Stress tests are small (12 to 48 users) and run with heartbeat, session check and dispatcher idle at 3600 s, so scale, cascades, a command loop that ends early and Redis flush are untested. | `testing/hub/test_hub.rs:30,44`; `services/hub/tests/contention.rs:128,199,310,524` |
| Missing fault and race cases: listener failure, `WATCH` conflict, half-open or oversized frames, id regression; fixed sleeps and `tick * N` waits remain. | `services/hub/tests/fault.rs`; `contention.rs:513,582`; `lifecycle.rs:440,479`; `routes/ws/connect_test.rs:294`; `stream_worker_test.rs:120,271` |
| Mutation blind spots: the close-code test compares the constant to itself, no test drives `AppState` wiring, the `SEND_DRAIN_TIMEOUT` abort is untested. | `routes/ws/connect_test.rs:104-112`; `app_state.rs:56`; `connect.rs:65,228` |
| Rust, `shine-infra`: the prod TLS check is tested for rejections only (no database needed); there is no test for accepting `sslmode=require` through the pool. | `crates/shine-infra/src/db/postgres/pg_connection.rs` |
| Rust, builder: `ConnectUser` registry failure (R10), inbox push at the hard limit returns `Full` (A4), and `ensure_stream` (L8). | `services/builder/src/services/command_stream/` |
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
