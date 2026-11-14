# Command stream audit

Scope: `services/builder` command stream (Redis inbox, relay, persistence, Postgres log, worker, registry, stream authorize, ws routes/hub glue) plus the small bits of `shine-game` / `shine-core` / `shine-infra` they depend on. Read as new code; every finding below was checked against the code. Paths are relative to `services/builder/src` unless noted.

Design assumption that matters for most findings: several builder instances may serve the same stream, each with its own worker reading the same Redis inbox and appending to the same Postgres log (`stream_registry.rs:240-273`, `stream_persistence.rs`). Redis pub/sub wakes are best effort.

## Summary

| # | Sev | Area | Finding |
|---|-----|------|---------|
| 1 | High | authz / DoS | Every stream id is authorized and unbounded in number: one user can create unlimited workers, Redis pollers, Postgres rows |
| 2 | High | soundness | Chain hash ignores `user_id`/`command_id`/`inbox_id`; a false "already stored" verdict silently drops a command, trims it from the inbox, and can persist a wrong snapshot |
| 3 | Medium | multi-instance | Persisted-version wakes are ignored; an instance that missed an entry (trimmed by a peer) never reconciles and its subscribers stay stale |
| 4 | Medium | DoS | No rate limit on ws commands/subscribes; stream-wide backpressure lets one abuser get other users dropped as `Busy`; subscribe spam amplifies into full `Sync` messages |
| 5 | Medium | idempotency | No dedup by `command_id`; retries after `Failed`, and shutdown/drain leftovers, duplicate commands; client-chosen ids can collide in chat |
| 6 | Medium | availability | Any transient Redis/PG error kills the worker and unsyncs all subscribers (reload storm); nothing restarts a dead worker |
| 7 | Medium | resource leak | ws sink has no write timeout: a stalled reader leaves zombie sockets/tasks after the hub dropped the connection, and the user can reconnect for more |
| 11 | Medium | resource growth | Inbox `XADD` has no hard limit (soft backpressure is sampled); `MAXLEN` would drop unprocessed commands, so reject atomically with a Lua check-and-add returning `Busy` |
| 8 | Low | integrity | Replay trusts stored `snapshot.hash` and `command_hash`; neither is recomputed on load |
| 9 | Low | lifecycle | `is_finished()` turns true while a worker still drains; replacement worker overlaps, and the draining one is invisible to `shutdown` |
| 10 | Low | misc | Inbox cursor vs Redis id regression; load/prune race; no PG statement timeout; unwrap on poisoned registry mutex; dispatcher handle dropped; `ping_enabled` defaults to true; `ensure_stream` writes on every load |

## High

### 1. No stream authorization, unbounded stream creation

> Note: the allow-all `is_authorized` is a known placeholder. The per-connection/total stream cap, the Redis poller/worker cost per stream and the never-deleted `cl_streams` rows remain valid regardless of the final authorization rules.

- `services/stream_authorize/stream_authorize_service.rs:16` returns `Ok(true)` for every user and stream. Any authenticated user can read and write any `chat:<id>` / `pixel-map:<id>` (id: 1-64 chars of `[A-Za-z0-9_.:-]`, `StreamId::new`).
- Nothing bounds how many streams one connection opens. `HubService::subscribe_stream` (`services/hub/hub_service.rs:264`) calls `registry.open` (`stream_registry.rs:145`), which for each new id creates a `StreamState` with a `Backpressure` (`stream_registry.rs:152,348`), and that spawns a task polling Redis `XLEN` every 250 ms (`shine-core/src/tokio/backpressure.rs:42`). The dispatcher then starts a worker (2 tasks, `stream_worker.rs:56,59`) whose preflight does `ensure_stream`, an `INSERT` into `cl_streams` (`pg/stream.rs:27-35`) and reload reads; the recovery poll adds one more Redis read per stream every 10 s.
- `cl_streams` rows are never deleted (only the commands/snapshots are pruned, `pg/prune.rs`).

Failure scenario: a client loops `StreamSubscribe{streamId:"chat:x<N>"}` on one socket (one message is a few dozen bytes, handled sequentially without limit in `routes/ws/connect.rs:259-274`). After a minute: ~10^5 backpressure pollers (~4x10^5 `XLEN`/s), 2x10^5 worker tasks, 10^5 permanent Postgres rows, Postgres pool (2 s checkout, `pg_command_log_db.rs:14`) exhausted so legitimate streams fail to persist and give up (finding 6). Memory in `State.streams/connections` grows until the connection closes (and the rows stay forever).

Fix:
- Implement real authorization: an allow-list or a registry of creatable streams per kind (e.g. fixed ids for global chat/maps, or a membership table) and reject unknown ids before `open`.
- Cap streams per connection (e.g. 8-16) in `StreamRegistry::open`, return `SubscribeFailure::Failed`.
- Create the `Backpressure` lazily (on first push) or share one poller per instance (`SCAN`/pipelined `XLEN`).
- GC `cl_streams` rows with no commands/snapshots; stop doing `ensure_stream` on read-only loads.

### 2. Chain hash does not bind command identity: false "stored" verdict loses a command

- `Command::content_hash` is the stable hash of the command payload only (`crates/shine-game/src/replica/state.rs:9`; e.g. `ChatCommand::stable_hash` hashes text, `PixelMapCommand` hashes x/y/color). `fold_chain_hash(parent, version, content_hash)` (`stable_hash.rs:54`) adds only the version. `user_id`, `command_id` and `inbox_id` are not in any hash.
- `pg/append.rs:112-131` decides an overlap is "covered" purely by `chain_hash` equality at `head.version` and returns `Ok(head)` without writing; `StreamLogService::append` turns that into `AppendOutcome::Stored` when `head.version == last.version && head.chain_hash == last.chain_hash` (`stream_log_service.rs:124`). The only full-field comparison (`stream_persistence.rs:133-138`) runs on the `Behind` path, not on `Stored`.
- On `Stored`, the persistence advances its replica with its own commands, and `finish` trims the inbox through the batch's max `inbox_id` (`stream_persistence.rs:297-302`, `stream_log_service.rs:157-164`).

Failure scenario (two instances A and C, both loaded for chat:global, log at v10):
1. User U1 sends "gg" (entry e11), user U2 sends "gg" (e12).
2. A reads both, appends e11 as v11, trims through e11 (`XTRIM MINID`).
3. C missed the wake, or was busy (see finding 3), and polls only after the trim: it sees only e12 and, from its stale head v10, versions it v11. Payload hashes are equal, so the chain hash equals the log's v11. The append returns "covered", C reports `Stored`, applies e12 on its replica and trims the inbox through e12.
4. U2's message exists nowhere durable. C keeps appending on top of the log (its baseline hash equals the head hash). C's replica now has U2 as author of v11 while the log has U1, so C and A compute different state hashes. Whichever instance stores the next snapshot first wins `try_store_snapshot`; it only checks the snapshot's `chain_hash` against the command row (`pg/snapshot.rs:109-118`), never the state hash against the commands. If C's snapshot lands first the stored snapshot contradicts the log, and after `prune` removes the older commands (`pg/prune.rs`) nobody can detect it.

For `pixel-map` the collision probability is high (16x32x32 distinct payloads, same payload from many users); for chat it needs the same text. Also applies to one user repeating the same payload (double click).

Fix:
- Fold `command_id` and `user_id` into the per-command hash that feeds the chain (breaking change to the hash definition: version it, mirror in the web client `crates/shine-game` wasm).
- In `PgCommandLogDb::append`, when the batch overlaps stored rows, verify the stored rows equal the batch (`command_id`, `inbox_id`) rather than only the hash, or have the service compare `commands_after(baseline)` to the batch before returning `Stored`.
- On load, verify `snapshot.hash` (finding 8) and make `try_store_snapshot` reject a snapshot whose state hash differs from an independent replay if cheap.

## Medium

### 3. Persisted wakes are ignored: stale instances never reconcile

- `notify_persisted` publishes `<stream>@<version>` (`redis/wake.rs:15-29`, `stream_persistence.rs:299-301`), but the registry collapses `InboxWake::Stream{persisted}` to `registry.wake()` (`stream_registry.rs:323`), and a wake only runs `process_inbox` (`stream_worker.rs:263`): poll the inbox after `read_index`. `Reconcile` (compare with the durable head) runs only in `preflight` (`stream_worker.rs:206`).
- Peers trim the inbox as soon as they persist (`stream_log_service.rs:157`). An instance that polls late (lost pub/sub wake, `room == 0` early return in `stream_relay.rs:150-153`, busy loop, or the 10 s recovery poll) finds nothing and keeps serving a stale tail. The recovery poll cannot recover entries that were already trimmed.

Failure scenario: instance C misses the wake of e11 (pub/sub is lossy, `command_inbox_types.rs:76`); A persists and trims it. C's subscribers never receive v11, and a new subscriber on C gets a `Sync` without it. It heals only when the next command reaches C and its append mismatches (reload), so an idle stream stays stale indefinitely. Also feeds finding 2.

Fix: on `persisted: Some(v)` with `v` beyond the worker's confirmed version, schedule a `Reconcile` (cheap `head` read) in the relay loop; also reconcile on the recovery tick when nothing was read.

### 4. No rate limiting; stream-wide backpressure is a collateral-damage lever

- `connect.rs:170-195` processes every text frame; there is no per-connection or per-user limit on commands, subscribes or unsubscribes. Control commands go through an unbounded channel into the single hub loop (`hub_service.rs: send_control`), so a flood slows connect/disconnect for everyone.
- Backpressure is per stream, polled every 250 ms (`stream_config.rs`, `backpressure.rs`): between polls a connection can push thousands of entries (each push is one Redis round trip) past the 1024 high mark. Once blocked, every push to that stream waits 2 s then returns `Busy`, and `send_stream_command` drops the pusher's connection (`hub_service.rs:301-323`). One abuser flooding `chat:global` therefore gets honest users disconnected.
- Each `StreamSubscribe` for an already-subscribed stream resets that connection's cursor and causes a full `Sync` (snapshot up to ~400 KB for chat plus the tail, `stream_relay.rs:332-341`, base64-encoded in `message.rs`). A few-dozen-byte request yields hundreds of KB of egress and relay CPU; the worker message queue is unbounded (`stream_worker.rs:50`).

Fix: token bucket per connection (commands, subscribes) with disconnect on abuse; apply backpressure/penalty to the offender (per-user in-flight/quota), not whoever pushes next; coalesce repeated subscribes (ignore a resync request within N ms, or send a `Sync` at most once per interval per connection); bound the worker message queue.

### 5. No command dedup: retries and leftovers duplicate commands

- `CommandInboxDb::push` explicitly performs no check; `stream_relay.rs:266-267` keeps repeats as separate commands; no unique constraint on `(stream, command_id)` in `V1__command_log.sql` (only `(kind,id,inbox_id)`).
- `CommandFailure::Failed` means "may or may not have been queued" (`models/messages/stream.rs`), and `drain()` reads one inbox page only (`stream_worker.rs:319`), leaving the rest in Redis for up to the 1 h TTL (`app_state.rs:17`) with no signal to the client. A client that resends gets two commands: chat `Add` is shown twice with the same message id (`ChatMessage.id = command_id`, `chat.rs:166`); `Remove`/`Replace` then touch only the first.
- `command_id` is client chosen and not checked for uniqueness per stream, so a user can reuse another message's id; clients that key messages by id will merge or overwrite them. Server-side `find_own` prevents unauthorized edits, so impact is presentation/consistency.

Fix: reject or ignore a second command with the same `(user_id, command_id)` at sequencing time (keep a bounded recent-id set per worker, seeded from the log tail on reload) or enforce a partial unique key in the log; make `drain` loop until the page is short; tell clients when a command is dropped (ack by echo).

### 6. Transient errors escalate to worker death and reload storms

- `ingest` errors (a Redis pool timeout) and any `PersistResult::Failed` (PG pool timeout 2 s, `Contention` after 8 attempts, a single failed read) call `give_up` (`stream_worker.rs:287,309,356`): all subscribers get `Unsynced` and are cleared, the worker ends, and the optimistic tail is discarded. There is no retry/backoff before giving up.
- Nothing restarts a dead worker: `registry.wake` only wakes an existing worker, and `push` still accepts commands because it checks only `is_open` (`stream_registry.rs:203-236`). Until clients resubscribe, commands pile up in the inbox up to the high mark, then pushers are dropped `Busy`.
- After a Redis/PG blip, every stream on the instance gives up at once and all clients resubscribe together, each worker reloading snapshot plus tail from Postgres (thundering herd, and amplifies finding 1).

Fix: retry transient errors (bounded, jittered) inside the relay/persistence loops and give up only on structural errors (`Corrupt`, `SnapshotDiverged`, repeated failure); have `push`/wake restart a worker for streams with open connections; add jitter to client resubscribe.

### 7. WebSocket sink has no write timeout (zombie connections)

`forward_egress` (`connect.rs:332-375`) does `ws_sender.send(...).await` with no timeout. A client that stops reading blocks that await once the TCP window fills. The hub later drops it as `SlowConsumer` and releases the user's slot, but `serve_socket` keeps waiting on `send_task`/`recv_task` (`connect.rs:201-214`); the drain timeout/abort only applies when the receive side ends first. The same user can reconnect (one hub connection per user is replaced, not capped by sockets), so sockets, two tasks and the queued `Arc<StreamMessage>` backlog (up to 1000 messages, the big `Sync`s pinned) leak per attacker until TCP gives up.

Fix: wrap sends in `tokio::time::timeout` (and abort the receive task when the hub drops the sender or on timeout); optionally set TCP keepalive/user timeout.

### 11. Inbox has no hard size limit (backpressure is only a sampled soft limit)

`push` (`repositories/command_inbox/redis/inbox.rs:24`) is a MULTI/EXEC pipeline of `XADD`, `EXPIRE`, `PUBLISH`. Nothing bounds the stream length at write time. Backpressure is a cached `XLEN` sampled every 250 ms (`stream_registry.rs:152,348`) with no reservation, so many connections pushing while the cached state is stale can go far past `inbox_backpressure_high`. A stream with no running worker is never drained, and every push re-arms the `EXPIRE`, so the key can grow without bound (16 KB per entry).

`XADD ... MAXLEN` is not the fix: it does not reject, it evicts the oldest entries after appending. That silently drops commands that clients already saw optimistically.

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
- If finding 5 (dedup by `command_id`) is implemented, do the dedup check in the same script so a retry of a stored command is not rejected as `Full`.
- Add a test: push up to the hard limit, assert the next push returns `Full` and the stream length is unchanged.

## Low

### 8. Replay trusts stored hashes

- `StreamPersistence::rebuild` (`stream_persistence.rs:195-214`) restores a snapshot with `Replica::from_snapshot(version, state, snapshot.chain_hash)` without checking that `stable_hash(state) == snapshot.hash`.
- `StoredCommand::to_envelope` (`command_log_types.rs:26-39`) uses the stored `command_hash`; `apply_next` checks only that the chain folds from that stored value, so a row whose `data` does not match its `command_hash` replays silently.

Fix: recompute `content_hash` from the decoded command and compare; compare `replica.hash()` with `snapshot.hash` on load; treat mismatch as `Corrupt`/reload from an older snapshot. Pairs with finding 2.

### 9. Worker replacement overlap and shutdown blind spot

`run()` calls `receiver.close()` before `drain()` (`stream_worker.rs:271-274`), and `is_finished()` is `messages.is_closed()` (`stream_worker.rs:94`), so the registry replaces the worker while it still drains (`stream_registry.rs:263-270`). Two workers for one stream then run in one process; correctness relies on the CAS and hash checks (and finding 2 applies to them). The replaced `Arc<StreamWorker>` is gone from `State.streams`, so `shutdown` (`stream_registry.rs:334-344`) never waits for its drain, and the process may exit with commands taken from the inbox but not yet persisted (they stay in Redis, but see finding 5).

Fix: report finished only when both tasks ended; keep draining workers in a side list that `shutdown` awaits.

### 10. Smaller items

- Inbox cursor vs id regression: `ingest` skips every entry with `inbox_id <= stored_inbox` (`stream_relay.rs:158-164`). If Redis ids ever move backwards (failover to a node with a slower clock, restored older dump with a later `last-id` after manual reset), new valid commands are skipped silently until ids pass the stored value. Mitigate by treating a poll that returns only skipped entries repeatedly as an alert, or storing the Redis epoch.
- Load/prune race: `StreamLogService::load` reads head, latest snapshot, then the tail in three queries (`stream_log_service.rs:68-82`). Two newer snapshots plus a prune in between yields a tail starting above the snapshot, so `apply_next` fails with `Gap` (`Corrupt`) and the worker gives up. Rare; read in one snapshot-isolated transaction.
- Postgres statements have no `statement_timeout` (only the append transaction sets `lock_timeout`/`idle_in_transaction`, `pg/append.rs:20`); a hung query stalls the persistence task, `in_flight` never resolves, the optimistic tail fills to `relay_max_pending` and pushes go `Busy`. `drain` would also wait forever (bounded only by the registry shutdown timeout).
- `StreamRegistry` uses `std::sync::Mutex` with `.lock().unwrap()` everywhere and runs `spawn` under it (`stream_registry.rs:267`); a panic while held poisons it and every later call panics, including the Redis wake handler.
- `StreamDispatcher::start` returns a `JoinHandle` that `HubService` drops (`hub_service.rs:193`); if the task dies, stream subscriptions silently stop while the hub keeps buffering events for it in an unbounded channel.
- `HubConfig` default is `ping_enabled: true` (`hub_service.rs:72`); prod config overrides it, but with it on, any user can broadcast arbitrary ping ids or target another user (`connect.rs:305-322`).
- `ensure_stream` is an upsert (`DO UPDATE SET id = cl_streams.id`), so every reload takes a write lock on the stream row and contends with append CAS; use `SELECT` first.
- Rolling deploys: an older binary decoding a command variant added by a newer one fails `to_envelope` with `Corrupt` and `give_up`s repeatedly for that stream (ingest drops it, but canonical replay cannot). Version the command codec and keep new variants gated.
- Chat text is stored and relayed raw (`is_well_formed` checks length/emptiness only); clients must escape.

## Checked and found sound

- Inbox push is one atomic `MULTI` (XADD+EXPIRE+PUBLISH) (`redis/inbox.rs:24-45`); stream keys and wake payloads only contain validated `StreamId` characters (no injection; `@` is excluded); `XRANGE` exclusive ranges, `MINID` trim and the numeric `InboxId` ordering are right.
- Append: dense-run assumptions are guaranteed by the relay (`reversion_optimistic`), index arithmetic is guarded (`pg/append.rs:106-131`), `ON CONFLICT DO NOTHING` + CAS on `(version, chain_hash)` serialize concurrent writers and roll back losing inserts; no orphan rows; `i64`/`usize` casts are range-checked by preceding branches.
- `user_id` always comes from the authenticated session, never from the client; WS host/origin allow-lists fail closed; command size is bounded (16 KiB, canonical postcard re-encode check, per-kind well-formedness); postcard decode failures never panic.
- Worker/registry lifecycle ordering (`open` -> hub event -> `subscribe`, late subscribe after disconnect, queued subscribes answered with `Unsynced` on exit) holds up; the select loop's branches are cancellation safe and no std lock is held across an await.
- Reload resets `read_index` to the last stored inbox id, so entries sequenced optimistically but not persisted are re-read, not lost (except via finding 2).

## Not verified

Redis pool checkout timeout (affects how long a `push` can park with Redis down), client-side handling of `Unsynced`/duplicate ids, and the `shine_infra` pub/sub listener internals beyond reading `redis_listener.rs`.
