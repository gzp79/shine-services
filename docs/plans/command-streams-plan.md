# Command Streams Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replicated, replayable per-stream command logs in the builder service. Clients write commands over the hub websocket. Redis orders them. Every interested server replays them into a state, persists them to Postgres, races to write snapshots, and pushes hash checkpoints to clients.

**Architecture:**
- **Replay core (`shine-game`).** Pure: the `ReplayState` trait, `Replica`, a deterministic `StableHash`, and the demo state `IntArray`.
- **Command log (`shine-infra`).** A state-agnostic Postgres command log that replaces the unused `db/event_source`.
- **Redis repository (builder).** Uses Lua scripts.
- **`CommandStreamService`.** Opens streams, hydrates Redis from Postgres, and runs the ingest hot path.
- **Hub.** Owns subscriptions: new control commands, plus `StreamEvent`s that only internal subscribers receive.
- **`StreamDispatcher` (hub task).** Runs one `StreamWorker<S>` per resident stream.
- **Worker.** A pure, unit-tested `StreamCore<S>` (replay, cursors, checkpoints, content checks) inside a thin I/O shell. Delivery runs on every Redis wake; persistence runs on a short, jittered cadence. Messages go out through a `StreamEgress` port that the hub implements.

**Tech Stack:** Rust 2021, tokio, axum ws, redis-rs 1.x (`redis::Script`), tokio-postgres + bb8 + refinery, serde/serde_json, postcard, Playwright (TS API tests).

**Spec:**
- The "Protocol rules" and "Soundness" sections below are normative.
- Appendix A at the end of this file is the design rationale. Where it differs from the tasks, the tasks and those two sections win.
- Task 0 copies the plan to `temp/command-streams/`.

## Global Constraints

**Process**
- **Never run git write commands** (`git add`, `git commit`, `git push`, `git stash`, …). The user handles git. Every task ends with a **Checkpoint** step: stop, report what changed and the test output, and wait.
- The hub and socket files have **uncommitted user changes in progress**, and they keep moving. At the time of writing:
  - a drop is signalled by the socket's own `HubEvent::UserDisconnected { reason }`;
  - sockets register on `TopicKey::Chat` and `TopicKey::Hub`;
  - `connect.rs` has `serve_socket`, `forward_egress` and `connect_test.rs`;
  - hub tests use the harness in `services/hub/test_utils.rs` and the `serial = "hub-service"` group;
  - the hub doc is `services/builder/HUB.md`.

  **Re-read each hub/ws file right before editing it**, insert the code from this plan into whatever is there now (adapting names that moved), and never revert those changes. If the code no longer matches an instruction, stop and ask.
- Before editing builder routes, services or repositories, invoke the `service-architecture` skill. Before writing API tests, invoke `api-test-writing`.

**Style and logging**
- Comments say *what*, not *how*; concise; no trivia. Match the comment density of the surrounding file. Prefix log lines with `[{stream_id}]` (or `[{user_id}]`) like the rest of the code.
- Logging: use `tracing::*!` only (no `log::`).
  - **trace:** command payloads, JSON data and emails are user data, so they may only be logged here.
  - **debug:** per-step and per-message logs/spans.
  - **info:** context for root-causing in prod.
  - **warn:** infrastructure errors that heal by retrying (Redis or Postgres down) and may need attention.
  - **error:** a bug or unexpected state (e.g. snapshot drift).

**Data and protocol**
- **Integers:** `version` and `generation` are `i64`. `hash` is `u64` in Rust, an `i64` in Postgres (bit cast with `as`), and a 16-digit lower-case hex string on the wire.
- **Commands** are JSON *strings* on the wire. They are stored in Redis and Postgres as **canonical** JSON text: the typed command, re-serialized. Postgres stores that text in a `TEXT` column, never `jsonb`, because `jsonb` rejects `\u0000`.

**Redis**
- Redis ≥ 6.2 is required (`XTRIM MINID`, exclusive `XRANGE (`).
- Redis must run with `maxmemory-policy noeviction`; the service warns at startup otherwise.
- All command-stream keys live under a prefix (`RedisStreamConfig::prefix`): `cs` in production, and a unique `cs-test-<uuid>` in every test.

**Formatting and lint** (CI runs both): run `cargo fmt --all` before every checkpoint. After any TS change, also run `cd tests && pnpm run format && pnpm run lint`.

**Tests**
- Use `shine_test::test`. A plain `#[ignore]` is silently dropped, so use `#[test(skip = "reason")]` to skip and `#[test(serial = "key")]` to serialize.
- DB-backed tests **skip silently** when `SHINE_TEST_REDIS_CNS` / `SHINE_TEST_PG_CNS` are missing. Always run them with the env vars set, and check that the output shows they ran (no "skipping" line).
- On Windows, set env vars with PowerShell, e.g.:
  ```powershell
  $env:SHINE_TEST_PG_CNS="postgres://username:password@postgres.mockbox.foo:5432/database-builder?sslmode=disable"
  $env:SHINE_TEST_REDIS_CNS="redis://redis.mockbox.foo:6379"
  ```
  If the hosts don't resolve, invoke the `local-development` skill, section "Run Rust crate tests".
- When something does not compile, look up the crate API on docs.rs (`https://docs.rs/<crate>/latest/<crate>/`) instead of guessing.

## Review Focus

1. **A client retries a command after a timeout** (same `commandId` sent twice). Exactly one version is assigned, and the retry gets `streamNack duplicate`. Tests: Task 4 `ingest_dedupes_command_id`, Task 10 `resending the same commandId…`.
2. **Valid JSON with the wrong shape, an oversized command, a nested batch, or a `\u0000` escape.** The command is canonicalized or refused with `streamNack invalid`, and it never blocks persistence. Tests: Task 1 `validate_rejects_nested_and_long_batches`, Task 2 `command_text_is_stored_verbatim`, Task 10 `invalid commands shall be nacked`.
3. **Redis is lost, or goes back in time** (a restart from its dump, or a failover) while clients are connected, including a copy that reassigned versions and has caught up past a worker. Versions continue from the Postgres head, no version is assigned twice, and clients get Reset then Sync. Tests: Task 8 `redis_loss_continues_from_postgres_head`, `redis_rolled_back_within_a_generation_is_repaired`, `a_reassigned_copy_that_caught_up_is_detected`, `redis_generation_behind_postgres_is_repaired`; Task 4 `mark_persisted_refuses_a_copy_that_disagrees`.
4. **A lagging worker whose Redis tail was trimmed** (holes filled from Postgres). No false `Diverged`, and no reset. Tests: Task 2 `append_matches_the_overlap_by_version`, Task 6 core tests.
5. **A slow client whose egress channel is full.** It is dropped cleanly, and never sees a version gap or a checkpoint ahead of its commands. Tests: Task 7 `a_failed_send_stops_all_later_frames`, Task 6 core tests for I10 and I12.

## Changes from the appendix (normative)

Format and protocol:
- Commands travel as JSON strings and are stored as canonical text (`TEXT` column).
- There is no success ack. `streamNack` reports failures, including `duplicate`.

Code structure:
- Redis keys use a prefix.
- The repositories are concrete `Arc<Inner>` handles; each has a single implementation.
- The command-log schema is a SQL file in `shine-infra`, applied with `PgConnection::migrate("command_log", …)`. Builder's never-applied `sql_migrations/V1__commands.sql` is removed.
- The worker is a pure `StreamCore` plus an I/O shell, and sends through `StreamEgress`.

Worker behaviour:
- Delivery runs on every wake.
- Persistence runs on a jittered cadence (`persistDelayMs`). Every resident instance appends; the appends are id-checked against Postgres.
- Every read checks that Redis still holds the command the worker applied last (content, not just a version count), and the persisted mark is recorded only for the command Redis holds at that version.
- Only the instance that stores a snapshot prunes.
- At most two captured checkpoints are kept, and only the newest ready one is resolved.

Hub:
- The hub keeps no per-connection stream set. A subscribe is accepted only while the connection is registered.
- There is no `publish_internal`: stream events go to `TopicKey::Stream`, which no socket subscribes to.
- There is no `HubService::ingest_stream_command`. The socket calls `CommandStreamService::ingest` directly, like chat.
- The socket's own replies (nacks, subscribe failures) go through the command loop (`HubService::request_reply`), so a reply to a frame sent right after connecting is not lost while the connection is still being registered.

Redis copies and the orphan sweep:
- A Redis copy behind Postgres is dropped and rebuilt from Postgres. That covers an older generation, and an older version in the same generation after a restart from a dump or a failover.
- The orphan sweep covers every idle stream, resident or not. A stream leaves the activity set only when its meta reports nothing unpersisted.

Docs: the `sdp` skill named in CLAUDE.md does not exist. Task 11 writes the docs by hand.

## Protocol rules (wire)

JSON text frames, `#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]`.

| Direction | Message | Fields |
|---|---|---|
| client → server | `streamSubscribe` | `streamId, kind` |
| client → server | `streamUnsubscribe` | `streamId` |
| client → server | `streamCommand` | `streamId, commandId, command` (a JSON **string**) |
| server → client | `streamSync` | `streamId, generation, snapshot: {version, hash, data} \| null, commands` |
| server → client | `streamCommands` | `streamId, generation, commands: [{version, commandId, userId, command}]` |
| server → client | `streamCheckpoint` | `streamId, version, hash` |
| server → client | `streamReset` | `streamId` |
| server → client | `streamNack` | `streamId, commandId, reason` |
| server → client | `streamSubscribeFailed` | `streamId, reason` |

**Encoding**
- `command` is JSON text inside a JSON string. Browsers must not run `JSON.parse` over command payloads: it rounds integers above 2^53. The client's wasm engine parses them instead.
- `hash` is 16 lower-case hex digits. `snapshot.data` is base64 postcard. Versions and generations stay below 2^53.

**State and ordering**
- Every `streamSync` replaces the client's state for that stream, whether or not a `streamReset` came first.
- Within one Sync epoch, versions are contiguous. A gap means a frame was lost, so the client subscribes again (resync).
- A large backlog arrives as one `streamSync` followed by `streamCommands` frames of at most 256 commands.
- At most one `streamCheckpoint` is sent per persist cycle, always the latest. It is never ahead of the commands already delivered.

**Acknowledgement and failure**
- There is no success ack: the writer sees its `commandId` in `streamCommands`.
- `streamNack{reason: "duplicate"}` means the command is already in the log, so the client drops it from pending.
- Nack reasons: `notReady`, `busy`, `notSubscribed`, `forbidden`, `invalid`, `failed`, `duplicate`, `unavailable`.
- `streamSubscribeFailed` reasons: `unknownKind`, `kindMismatch`, `forbidden`, `failed`, `unavailable`.
- Subscribing again is the client's resync request.

## Soundness

This section argues that the protocol in Tasks 2–9 is correct under concurrency. Do not change any mechanism named here without re-checking the argument.

### Model

- **Instances.** Any number of builder instances. Each runs a hub command loop, a `StreamDispatcher`, and at most one live `StreamWorker` per stream (briefly two while an old one flushes).
- **Redis.** A single Redis; each script runs atomically. Redis can lose a stream's keys, or all its data. It can also **go back in time**: a restart from its RDB/AOF dump, or a failover to a lagging replica, restores an older meta (same generation, lower version).
- **Postgres.** One Postgres, using READ COMMITTED with `SELECT … FOR UPDATE` on `cl_streams`.
- **Timing.** Any task can stall for any length of time, and locks can expire.
- **Ordering.** Messages to one connection go through a bounded FIFO channel. A send to a full channel fails, and the hub then drops that connection. Nothing is ordered across connections.

### Invariants

| # | Invariant | Why it holds |
|---|---|---|
| I1 | Within a generation, versions are dense and totally ordered. | Only `INGEST` assigns versions, atomically. `XADD "{v}-0"` runs before `HSET version`, so a failing `XADD` (and Lua does not roll back) leaves `version` unchanged. `HYDRATE` deletes the log before re-adding entries in ascending order. |
| I2 | The Postgres log is a gap-free prefix of the order, and its rows are never rewritten. | `append` holds the head row lock. Every row ≤ head in the input must match the stored row **by version** (`command_id`, `user_id`), so the input may have holes. New rows must start at `head + 1` and be contiguous. A trigger makes rows immutable. Under READ COMMITTED, a lock waiter re-reads the committed head. |
| I3 | Generation fencing: rows of generation g are only persisted before the bump to g+1. | `begin_generation` (an autocommit `UPDATE`) and `append` take the same row lock. New rows must carry the head's generation. |
| I4 | Redis never trims a command that isn't persisted. | Trimming cuts at `min(persisted + 1, version − keep + 1)`. `persisted` only rises, either to the head `HYDRATE` wrote, or to a version a committed `append` returned, and only if the copy holds the persisted command there (`MARK_PERSISTED` checks the command id and refuses a copy behind the version). A copy that went back in time and reassigned versions is refused and dropped (I7). |
| I5 | A worker never holds a version that a newer generation reassigns. | A worker only adopts a **newer** generation, and only once everything it applied is proven to be in Postgres: `unpersisted` is empty (every command left it through an id-checked `append`), or the adoption `append` returned `UpToDate` at or above the replica's version. By I3 those rows committed before the bump, so the new generation starts above them. An **older** generation is never adopted; that copy is dropped (I7). |
| I6 | Postgres wins every conflict. | `StaleGeneration` and `Diverged` drop the Redis copy (`RESET` is generation-guarded, so it can never drop a newer copy), then the worker rebuilds from Postgres. The orphan sweep does the same. |
| I7 | A Redis copy that went back in time is detected and dropped. | `ensure_hydrated` reads the **Postgres head first, then the Redis meta**: both only grow and head ≤ meta.version for a healthy copy, so `meta.generation < head.generation`, or an equal generation with `meta.version < head.version`, really is stale, and it is reset. Workers check every read, which starts at the replica's own version: an older generation, `meta.version` below the replica, **another command id at the replica's version**, or a missing unpersisted entry there means `Reset`. A copy restored from a dump or a replica is a prefix of the old history plus reassigned versions, so either its version is below the replica or the entry at the replica's version is a different command. |
| I8 | Concurrent hydrators converge. | Each instance runs one hydration per stream at a time. Across instances, a Redis lock serializes hydration. `HYDRATE` only writes over an older generation. A stale writer that lands after an expired lock is repaired by I7. Only the unpersisted commands of the losing generation are lost, which R13 accepts. |
| I9 | Snapshots match the log. Above the prune horizon, the first writer is authoritative. | A snapshot is captured at its exact version, from a replica that has applied exactly versions 1..v. It is stored only once v ≤ persisted (which makes it a Postgres prefix, by I2), and a foreign key requires the command row. The primary key `(stream, version)` makes the first writer authoritative. A loser whose hash differs rebases. A version below a newer stored snapshot returns `Superseded` and is never stored, so a lagging, drifted instance cannot certify itself. |
| I10 | Within one Sync epoch, a client receives every version exactly once and in order. | This is structural in `StreamCore`: every synced subscriber shares one cursor, `fanned_out`. `since_snapshot` is never trimmed above `fanned_out`, a rebase keeps the version, and the per-connection channel is FIFO. On the first failed send the subscriber is removed, so it gets no later frame and no gap. |
| I11 | Every `Reset` is followed by exactly one `Sync`, and nothing is sent between them. | `take_reset_frames` moves the synced subscribers to `pending_sync`, and nothing is delivered until the next successful load. |
| I12 | A checkpoint never runs ahead of the commands it covers. | Checkpoints only go to synced subscribers, and only when `version ≤ fanned_out`. |
| I13 | Subscription events are ordered, socket replies are never lost to a registration in flight, and dead connections never pin a worker. | `ConnectUser` is queued before the receive task starts, and the command loop is FIFO. Subscribes and the socket's own replies (`request_reply`) go through the loop, behind the registration. A `SubscribeStream` for a connection that isn't registered is ignored. The dispatcher reads hub and stream events from one channel, in loop order. `UserDisconnected` removes every subscription of that connection. |
| I14 | Each resident stream has one live worker. | The dispatcher owns the workers in a `JoinSet` and identifies them by task id. The exit of an unloaded or replaced worker is ignored. A crashed worker is restarted at most once per `RESPAWN_DELAY`. |
| I15 | Liveness: no stream stays stuck while Redis and Postgres are reachable. | Each mechanism below has a bounded retry, so a stream always recovers once both are reachable. |
| I16 | No command content can block persistence. | Commands are stored as canonical text, which was validated at ingest, in a `TEXT` column. A worker never skips its own id-checked `append` because of what Redis reports as persisted: Redis can go back in time within a generation. |

How I15 holds:
- Reloads back off from 200 ms up to 10 s.
- The persisted mark is retried on every persist cycle.
- The orphan sweep doesn't skip resident streams, and it requeues any stream it can't retire.
- Appends use `lock_timeout` and `idle_in_transaction_session_timeout`, so a dead connection releases the stream's row lock.
- Hydration runs once per stream per instance.
- Every unrecoverable mismatch ends in a generation-guarded `RESET` and a rehydrate from Postgres.

### What is deliberately not guaranteed

- **Durability before persistence.** A command delivered in `streamCommands` but not yet in Postgres can be lost when Redis loses data or goes back in time. Its clients then get `streamReset` (I11) and resend their pending commands. A client's "authoritative" state means accepted by the server, not durable.
- **Retry window.** Deduplication covers the last `redisKeepCommands` versions plus the unpersisted ones. A retry after that window, or after a Redis loss, is accepted as a new command.
- **Reusing a command id.** If a client sends the same `commandId` with different data at the same version after a Redis loss, the id check passes. The next checkpoint catches it, and the client heals through a Sync.
- **First writer wins.** If the instance that wins a snapshot race has a drifted state, its state becomes authoritative; this is the agreed semantics. The drift is logged at `error` and counted in `command_stream_drift`, because it means a determinism bug.
- **Code skew.** During a rolling deploy, a command one build can't decode is ignored by that build only. The snapshot race heals the difference, but command schemas must stay backward compatible.
- **Redis going back in time.** Detection is immediate on `open` and on load, and on the worker's next read while running (at the latest one `syncIntervalMs` later). Commands accepted at reused versions before then are lost, and their writers resend after the Reset.

## Throughput model (single stream)

These are estimates, not measurements. Task 8's `single_stream_throughput` probe measures the real numbers. Report them at the Task 8 checkpoint, and again after the optional Task 12.

Assumptions:
- Same availability zone: Redis round trip r ≈ 0.3 ms, Postgres round trip p ≈ 0.5 ms, commit flush f ≈ 1 ms.
- Commands are about 100 B, and the demo state is used.
- I is the number of instances with local subscribers.
- Default config.

Without Task 12, every pool checkout first sends a validation round trip: bb8's `test_on_check_out` defaults to `true`.

| Path | Expected (this plan) | After optional Task 12 | Limited by |
|---|---|---|---|
| Round trip (write until seen back), idle | ≈ 1.3 ms + client RTT (4 Redis round trips incl. checkout PINGs) | ≈ 0.8 ms | Redis round trips |
| Round trip p99 under load | ≈ 3–6 ms at any I: persistence no longer blocks delivery; steps are paced at ≥ 2 ms | ≈ 2–4 ms | step pacing, Redis |
| Ingest, one socket (writes awaited in order) | ≈ 1.6k/s | ≈ 3k/s | Redis round trip (+ PING) |
| Redis calls per instance | ≤ ≈ 16k/s (pool of 10 × 2 round trips) | ≥ 100k/s (pool 32, no PING) | pool size |
| Many light streams, whole cluster | ≈ (17 + 7·I) µs of Redis CPU per command: ≈ 26k/s at I=3, ≈ 11k/s at I=10 | same | Redis single thread; `PUBLISH` reaches every instance |
| One hot stream, sustained | ≈ 15–20k/s | ≈ 20–30k/s | Postgres row inserts (≈ 40 µs each) plus the other instances' overlap checks on the row lock |
| Postgres transactions | ≈ I per stream per persist cycle (I × 25–50/s for a hot stream), not I × wakes; the first one inserts, the others get `UpToDate` after a lock and an overlap check (≈ 3 p) | same | row lock, ≈ 5 instances per stream before it bites |
| Persist lag | between `persistDelayMs` and 2 × `persistDelayMs` (20–40 ms) | same | cadence |
| Fan-out CPU | ≈ 1.5 µs per command + 3 µs per frame, per subscriber (one shared frame per step; each socket serializes it) | same | per-socket serialization |
| Snapshots | rule: `snapshotEvery ≥ write rate / 5` (≈ ≤ 5 checkpoints/s per stream); losers skip the upload; only the winner prunes | same | Postgres round trips, state size |
| New subscriber `Sync` | snapshot + ≤ `snapshotEvery` + unpersisted commands, in frames of 256 | same | — |

Bottleneck per stream:

| Subscribers | Bottleneck |
|---|---|
| 1–10 | Per-socket Redis round trips and the pool (until Task 12), then Postgres inserts |
| 100+ | Per-socket serialization: about 0.15 ms of CPU per command per instance |

Deferred (not in this plan):
- A single-statement `unnest` insert: about 3× the persist ceiling.
- A separate persist job.
- Per-stream wake channels: past about 6 instances, `PUBLISH` fan-out costs about 30% of `INGEST`.
- Rendering each frame once for all sockets.

## File structure

| File | Responsibility |
|---|---|
| `crates/shine-game/src/replay/*` | replay core (Task 1) |
| `crates/shine-infra/src/db/command_log/{mod.rs, command_log_types.rs, pg_command_log.rs, sql/V1__command_log.sql}` | Postgres command log (Task 2) |
| `crates/shine-infra/tests/pg_command_log.rs` | DB tests (Task 2) |
| `services/builder/src/models/command_stream_error.rs` | `CommandStreamError` (Task 3) |
| `services/builder/src/repositories/command_stream/redis/{mod.rs, scripts.rs, redis_command_stream_db.rs, redis_command_stream_test.rs}` | Redis repository (Task 4) |
| `services/builder/src/models/messages/stream.rs` | stream messages and events (Task 5) |
| `services/builder/src/services/hub/hub_stream_test.rs` | hub subscription tests (Task 5) |
| `services/builder/src/services/command_stream/{mod.rs, command_stream_metrics.rs, stream_access.rs, stream_kind.rs, stream_egress.rs, command_stream_service.rs}` | service, kinds, ports (Task 6) |
| `services/builder/src/services/command_stream/{stream_core.rs, stream_core_test.rs}` | pure worker core + unit tests (Task 6) |
| `services/builder/src/services/command_stream/stream_worker.rs` | worker I/O shell (Task 7) |
| `services/builder/src/services/hub/{stream_egress.rs, tasks/stream_dispatcher.rs}` | `StreamEgress` for the hub; dispatcher (Task 7) |
| `services/builder/src/services/command_stream/command_stream_test.rs` | Redis + Postgres integration tests (Task 8) |
| `tests/src/api/builder_stream_ws.ts`, `tests/api-tests/builder/command_stream.ts` | API tests (Task 10) |

---

## Task 0: Stage the design

**Files:**
- Create: `temp/command-streams/design.md`, `temp/command-streams/plan.md`

- [ ] **Step 1:** Copy "Appendix A — Design spec" (everything below the `# Appendix A` heading at the end of this file) verbatim into `temp/command-streams/design.md`, and copy the whole plan into `temp/command-streams/plan.md`.
- [ ] **Step 2: Checkpoint.** Report the two paths. (The SDP HTML doc is written in Task 11 once the code exists.)

---

## Task 1: Replay core in `shine-game`

**Files:**
- Modify: `Cargo.toml` (workspace deps), `crates/shine-game/Cargo.toml`, `crates/shine-game/src/lib.rs`
- Create: `crates/shine-game/src/replay/{mod.rs, stable_hash.rs, state.rs, envelope.rs, replica.rs, policy.rs, codec.rs, demo.rs, tests.rs}`

**Interfaces — Produces (used by Tasks 6–8, 10):**
- `shine_game::replay::{StableHash, StableHasher, stable_hash_of, ReplayState, CommandEnvelope, Checkpoint, Replica, ApplyOutcome, Gap, CheckpointPolicy, encode_snapshot, decode_snapshot, SnapshotCodecError, IntArray, DemoCommand, DemoReject, DEMO_LEN, MAX_BATCH_LEN}`
- `ReplayState::KIND` for `IntArray` is `"demo"`.
- `Replica::apply_next(&CommandEnvelope<S::Command>) -> Result<ApplyOutcome, Gap>`; `ApplyOutcome::{Applied, Ignored, AlreadyApplied}`.

- [ ] **Step 1: Dependencies**

In the root `Cargo.toml`, under `[workspace.dependencies]` after `serde_json = "1.0"`, add:

```toml
postcard = { version = "1.1", default-features = false, features = ["use-std"] }
```

In `crates/shine-game/Cargo.toml` `[dependencies]`, after `serde_json = { workspace = true }`, add (uuid is declared directly, without the workspace `v4` feature, so the wasm build needs no random source):

```toml
postcard = { workspace = true }
uuid = { version = "1.22", default-features = false, features = ["serde"] }
```

In `crates/shine-game/src/lib.rs` add `pub mod replay;` after `pub mod mesh;`.

- [ ] **Step 2: Write the failing tests**

Create `crates/shine-game/src/replay/tests.rs`:

```rust
use super::*;
use shine_test::test;
use uuid::Uuid;

fn env(version: i64, command: DemoCommand) -> CommandEnvelope<DemoCommand> {
    CommandEnvelope {
        version,
        command_id: Uuid::from_u128(version as u128),
        user_id: Uuid::nil(),
        command: Some(command),
    }
}

fn set(idx: u32, value: i64) -> DemoCommand {
    DemoCommand::Set { idx, value }
}

#[test]
fn fnv_matches_reference_vectors() {
    let mut h = StableHasher::new();
    assert_eq!(h.finish(), 0xcbf2_9ce4_8422_2325);
    h.write(b"a");
    assert_eq!(h.finish(), 0xaf63_dc4c_8601_ec8c);
}

#[test]
fn int_array_hash_is_length_then_le_values() {
    // Mirrors the TS helper in tests/src/api/builder_stream_ws.ts.
    let mut state = IntArray::default();
    state.values[0] = -1;
    let mut h = StableHasher::new();
    h.write(&10u64.to_le_bytes());
    h.write(&(-1i64).to_le_bytes());
    for _ in 1..10 {
        h.write(&0i64.to_le_bytes());
    }
    assert_eq!(stable_hash_of(&state), h.finish());
}

#[test]
fn applied_and_ignored_commands_both_consume_a_version() {
    let mut replica = Replica::<IntArray>::new();
    assert_eq!(replica.apply_next(&env(1, set(0, 5))), Ok(ApplyOutcome::Applied));
    let before = replica.hash();
    assert_eq!(replica.apply_next(&env(2, set(10, 1))), Ok(ApplyOutcome::Ignored));
    assert_eq!(replica.version(), 2);
    assert_eq!(replica.hash(), before, "a rejected command leaves the state unchanged");
}

#[test]
fn undecodable_command_is_ignored() {
    let mut replica = Replica::<IntArray>::new();
    let envelope = CommandEnvelope {
        version: 1,
        command_id: Uuid::nil(),
        user_id: Uuid::nil(),
        command: None,
    };
    assert_eq!(replica.apply_next(&envelope), Ok(ApplyOutcome::Ignored));
    assert_eq!(replica.version(), 1);
}

#[test]
fn gap_and_already_applied_are_detected() {
    let mut replica = Replica::<IntArray>::new();
    assert_eq!(replica.apply_next(&env(2, set(0, 1))), Err(Gap { expected: 1, got: 2 }));
    replica.apply_next(&env(1, set(0, 1))).unwrap();
    assert_eq!(replica.apply_next(&env(1, set(0, 9))), Ok(ApplyOutcome::AlreadyApplied));
    assert_eq!(replica.state().values[0], 1);
}

#[test]
fn batch_is_all_or_nothing() {
    let mut replica = Replica::<IntArray>::new();
    let batch = DemoCommand::Batch {
        commands: vec![set(0, 1), set(1, 2), DemoCommand::Add { idx: 2, delta: 1 }, set(99, 0)],
    };
    assert_eq!(replica.apply_next(&env(1, batch)), Ok(ApplyOutcome::Ignored));
    assert_eq!(replica.state(), &IntArray::default());
}

#[test]
fn demo_commands_apply_and_reject() {
    let mut s = IntArray::default();
    s.apply(&set(3, 7)).unwrap();
    s.apply(&DemoCommand::Swap { a: 3, b: 4 }).unwrap();
    assert_eq!((s.values[3], s.values[4]), (0, 7));
    assert!(s
        .apply(&DemoCommand::CompareAndSet { idx: 4, expected: 1, value: 2 })
        .is_err());
    s.apply(&DemoCommand::CompareAndSet { idx: 4, expected: 7, value: 2 }).unwrap();
    assert_eq!(s.values[4], 2);
    s.apply(&set(0, i64::MAX)).unwrap();
    assert_eq!(s.apply(&DemoCommand::Add { idx: 0, delta: 1 }), Err(DemoReject::Overflow));
}

#[test]
fn validate_rejects_nested_and_long_batches() {
    let nested = DemoCommand::Batch {
        commands: vec![DemoCommand::Batch { commands: vec![] }],
    };
    assert!(!IntArray::validate(&nested));
    let long = DemoCommand::Batch {
        commands: (0..=MAX_BATCH_LEN as u32).map(|i| set(i % 10, 1)).collect(),
    };
    assert!(!IntArray::validate(&long));
    assert!(IntArray::validate(&DemoCommand::Batch { commands: vec![set(0, 1)] }));
}

#[test]
fn demo_command_json_shape() {
    let cmd: DemoCommand = serde_json::from_str(r#"{"op":"compareAndSet","idx":1,"expected":2,"value":3}"#).unwrap();
    assert_eq!(cmd, DemoCommand::CompareAndSet { idx: 1, expected: 2, value: 3 });
    assert!(serde_json::from_str::<DemoCommand>(r#"{"op":"set","idx":"x","value":1}"#).is_err());
}

#[test]
fn snapshot_round_trip_keeps_hash() {
    let mut state = IntArray::default();
    state.values = [1, -2, 3, -4, 5, i64::MIN, i64::MAX, 0, 9, 10];
    let bytes = encode_snapshot(&state).unwrap();
    let decoded: IntArray = decode_snapshot(&bytes).unwrap();
    assert_eq!(decoded, state);
    assert_eq!(stable_hash_of(&decoded), stable_hash_of(&state));
}

#[test]
fn replay_from_snapshot_equals_replay_from_zero() {
    let log: Vec<_> = (1..=20)
        .map(|v| env(v, DemoCommand::Add { idx: (v % 10) as u32, delta: v }))
        .collect();

    let mut full = Replica::<IntArray>::new();
    let mut other = Replica::<IntArray>::new();
    for e in &log {
        full.apply_next(e).unwrap();
        other.apply_next(e).unwrap();
    }
    assert_eq!(full.hash(), other.hash());

    let mut prefix = Replica::<IntArray>::new();
    for e in &log[..10] {
        prefix.apply_next(e).unwrap();
    }
    let snapshot = encode_snapshot(prefix.state()).unwrap();
    let mut resumed = Replica::from_snapshot(10, decode_snapshot::<IntArray>(&snapshot).unwrap());
    for e in &log[10..] {
        resumed.apply_next(e).unwrap();
    }
    assert_eq!(resumed.hash(), full.hash());
    assert_eq!(resumed.version(), 20);
}

#[test]
fn checkpoint_policy() {
    let policy = CheckpointPolicy::new(5);
    assert!(!policy.is_checkpoint(0));
    assert!(!policy.is_checkpoint(4));
    assert!(policy.is_checkpoint(5));
    assert!(policy.is_checkpoint(10));
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p shine-game replay`
Expected: compile errors (`replay` module items not found).

- [ ] **Step 4: Implement**

`crates/shine-game/src/replay/mod.rs`:

```rust
//! Deterministic command replay shared by the servers and the client.

mod codec;
mod demo;
mod envelope;
mod policy;
mod replica;
mod stable_hash;
mod state;

#[cfg(test)]
mod tests;

pub use self::{
    codec::{decode_snapshot, encode_snapshot, SnapshotCodecError},
    demo::{DemoCommand, DemoReject, IntArray, DEMO_LEN, MAX_BATCH_LEN},
    envelope::{Checkpoint, CommandEnvelope},
    policy::CheckpointPolicy,
    replica::{ApplyOutcome, Gap, Replica},
    stable_hash::{stable_hash_of, StableHash, StableHasher},
    state::ReplayState,
};
```

`stable_hash.rs`:

```rust
use std::collections::BTreeMap;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// 64-bit FNV-1a; identical output on native and wasm.
#[derive(Clone, Debug)]
pub struct StableHasher(u64);

impl Default for StableHasher {
    fn default() -> Self {
        Self(FNV_OFFSET)
    }
}

impl StableHasher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= *byte as u64;
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}

/// Platform independent hashing of replayed state. Not implemented for `usize`, floats or
/// `HashMap`: their encoding or iteration order differs between platforms.
pub trait StableHash {
    fn stable_hash(&self, hasher: &mut StableHasher);
}

/// Hash of a single value.
pub fn stable_hash_of<T: StableHash + ?Sized>(value: &T) -> u64 {
    let mut hasher = StableHasher::new();
    value.stable_hash(&mut hasher);
    hasher.finish()
}

macro_rules! impl_stable_hash_int {
    ($($ty:ty),*) => {
        $(impl StableHash for $ty {
            fn stable_hash(&self, hasher: &mut StableHasher) {
                hasher.write(&self.to_le_bytes());
            }
        })*
    };
}

impl_stable_hash_int!(u8, u16, u32, u64, i8, i16, i32, i64);

impl StableHash for bool {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        hasher.write(&[*self as u8]);
    }
}

impl<T: StableHash> StableHash for [T] {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        (self.len() as u64).stable_hash(hasher);
        for item in self {
            item.stable_hash(hasher);
        }
    }
}

impl<T: StableHash, const N: usize> StableHash for [T; N] {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.as_slice().stable_hash(hasher);
    }
}

impl<T: StableHash> StableHash for Vec<T> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.as_slice().stable_hash(hasher);
    }
}

impl<T: StableHash> StableHash for Option<T> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        match self {
            None => 0u8.stable_hash(hasher),
            Some(value) => {
                1u8.stable_hash(hasher);
                value.stable_hash(hasher);
            }
        }
    }
}

impl<K: StableHash, V: StableHash> StableHash for BTreeMap<K, V> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        (self.len() as u64).stable_hash(hasher);
        for (key, value) in self {
            key.stable_hash(hasher);
            value.stable_hash(hasher);
        }
    }
}
```

`state.rs`:

```rust
use crate::replay::StableHash;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt;

/// A state rebuilt by replaying an ordered command log. `apply` must be deterministic.
pub trait ReplayState: Clone + Default + Serialize + DeserializeOwned + StableHash + Send + Sync + 'static {
    type Command: Clone + fmt::Debug + Serialize + DeserializeOwned + Send + Sync + 'static;
    type Reject: fmt::Debug;

    /// Stream kind name, stored with the stream and sent by the clients.
    const KIND: &'static str;

    /// Applies atomically: on `Err` the state is unchanged.
    fn apply(&mut self, command: &Self::Command) -> Result<(), Self::Reject>;

    /// Ingest-time shape limits; replay never calls it.
    fn validate(_command: &Self::Command) -> bool {
        true
    }
}
```

`envelope.rs`:

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A command at its position in the log. `command` is `None` when it could not be decoded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandEnvelope<C> {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub command: Option<C>,
}

/// State hash at a version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub version: i64,
    pub hash: u64,
}
```

`replica.rs`:

```rust
use crate::replay::{stable_hash_of, Checkpoint, CommandEnvelope, ReplayState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    /// Rejected or undecodable; the version is still consumed.
    Ignored,
    /// The version was already applied; nothing changed.
    AlreadyApplied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("expected version {expected}, got {got}")]
pub struct Gap {
    pub expected: i64,
    pub got: i64,
}

/// A state with the version of the last command applied to it.
#[derive(Clone, Debug)]
pub struct Replica<S: ReplayState> {
    state: S,
    version: i64,
}

impl<S: ReplayState> Default for Replica<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: ReplayState> Replica<S> {
    pub fn new() -> Self {
        Self {
            state: S::default(),
            version: 0,
        }
    }

    pub fn from_snapshot(version: i64, state: S) -> Self {
        Self { state, version }
    }

    pub fn version(&self) -> i64 {
        self.version
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn hash(&self) -> u64 {
        stable_hash_of(&self.state)
    }

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            version: self.version,
            hash: self.hash(),
        }
    }

    /// Applies the next command of the log.
    pub fn apply_next(&mut self, envelope: &CommandEnvelope<S::Command>) -> Result<ApplyOutcome, Gap> {
        if envelope.version <= self.version {
            return Ok(ApplyOutcome::AlreadyApplied);
        }
        if envelope.version != self.version + 1 {
            return Err(Gap {
                expected: self.version + 1,
                got: envelope.version,
            });
        }
        self.version = envelope.version;
        match &envelope.command {
            Some(command) if self.state.apply(command).is_ok() => Ok(ApplyOutcome::Applied),
            _ => Ok(ApplyOutcome::Ignored),
        }
    }
}
```

`policy.rs`:

```rust
/// Versions at which every instance snapshots and pushes a checkpoint.
#[derive(Clone, Copy, Debug)]
pub struct CheckpointPolicy {
    every: i64,
}

impl CheckpointPolicy {
    pub fn new(every: i64) -> Self {
        Self { every: every.max(1) }
    }

    pub fn every(&self) -> i64 {
        self.every
    }

    pub fn is_checkpoint(&self, version: i64) -> bool {
        version > 0 && version % self.every == 0
    }
}
```

`codec.rs`:

```rust
use crate::replay::ReplayState;

#[derive(Debug, thiserror::Error)]
#[error("snapshot codec error")]
pub struct SnapshotCodecError(#[from] postcard::Error);

pub fn encode_snapshot<S: ReplayState>(state: &S) -> Result<Vec<u8>, SnapshotCodecError> {
    Ok(postcard::to_allocvec(state)?)
}

pub fn decode_snapshot<S: ReplayState>(bytes: &[u8]) -> Result<S, SnapshotCodecError> {
    Ok(postcard::from_bytes(bytes)?)
}
```

`demo.rs`:

```rust
use crate::replay::{ReplayState, StableHash, StableHasher};
use serde::{Deserialize, Serialize};

pub const DEMO_LEN: usize = 10;
pub const MAX_BATCH_LEN: usize = 32;

/// Demo state for tests and samples: ten integers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntArray {
    pub values: [i64; DEMO_LEN],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum DemoCommand {
    Set { idx: u32, value: i64 },
    Add { idx: u32, delta: i64 },
    Swap { a: u32, b: u32 },
    CompareAndSet { idx: u32, expected: i64, value: i64 },
    /// All-or-nothing.
    Batch { commands: Vec<DemoCommand> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DemoReject {
    IndexOutOfRange(u32),
    Overflow,
    Mismatch { idx: u32, expected: i64, actual: i64 },
}

impl IntArray {
    fn slot(idx: u32) -> Result<usize, DemoReject> {
        let slot = idx as usize;
        if slot < DEMO_LEN {
            Ok(slot)
        } else {
            Err(DemoReject::IndexOutOfRange(idx))
        }
    }

    fn apply_in_place(&mut self, command: &DemoCommand) -> Result<(), DemoReject> {
        match command {
            DemoCommand::Set { idx, value } => {
                self.values[Self::slot(*idx)?] = *value;
            }
            DemoCommand::Add { idx, delta } => {
                let slot = Self::slot(*idx)?;
                self.values[slot] = self.values[slot].checked_add(*delta).ok_or(DemoReject::Overflow)?;
            }
            DemoCommand::Swap { a, b } => {
                self.values.swap(Self::slot(*a)?, Self::slot(*b)?);
            }
            DemoCommand::CompareAndSet { idx, expected, value } => {
                let slot = Self::slot(*idx)?;
                if self.values[slot] != *expected {
                    return Err(DemoReject::Mismatch {
                        idx: *idx,
                        expected: *expected,
                        actual: self.values[slot],
                    });
                }
                self.values[slot] = *value;
            }
            DemoCommand::Batch { commands } => {
                for command in commands {
                    self.apply_in_place(command)?;
                }
            }
        }
        Ok(())
    }
}

impl StableHash for IntArray {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.values.stable_hash(hasher);
    }
}

impl ReplayState for IntArray {
    type Command = DemoCommand;
    type Reject = DemoReject;

    const KIND: &'static str = "demo";

    fn apply(&mut self, command: &DemoCommand) -> Result<(), DemoReject> {
        // Work on a copy so a failing batch leaves no partial change.
        let mut next = self.clone();
        next.apply_in_place(command)?;
        *self = next;
        Ok(())
    }

    fn validate(command: &DemoCommand) -> bool {
        match command {
            DemoCommand::Batch { commands } => {
                commands.len() <= MAX_BATCH_LEN && !commands.iter().any(|c| matches!(c, DemoCommand::Batch { .. }))
            }
            _ => true,
        }
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p shine-game replay`
Expected: all 12 tests PASS. Then run `cargo build -p shine-game --target wasm32-unknown-unknown` (if the target is installed; otherwise run `cd client/web && pnpm build:wasm`) — expected: builds (proves no wasm-incompatible deps).

- [ ] **Step 6: Checkpoint.** Report changed files and test output; wait for the user.

---

---

## Task 2: Postgres command log in `shine-infra` (replaces `event_source`)

**Files:**
- Delete: `crates/shine-infra/src/db/event_source/` (whole folder), `crates/shine-infra/tests/pg_event_source.rs`
- Modify: `crates/shine-infra/src/db/mod.rs`, `crates/shine-infra/src/db/postgres/pg_type.rs`, `crates/shine-infra/src/db/postgres/mod.rs`
- Create: `crates/shine-infra/src/db/command_log/{mod.rs, command_log_types.rs, pg_command_log.rs, sql/V1__command_log.sql}`, `crates/shine-infra/tests/pg_command_log.rs`

**Interfaces — Produces (used by Tasks 4–8):**
```rust
shine_infra::db::command_log::{
    PgCommandLogDb,          // Clone; Arc<Inner>
    StreamHead { kind: String, version: i64, generation: i64 },
    StoredCommand { version: i64, command_id: Uuid, user_id: Uuid, generation: i64, data: String },
    StoredSnapshot { version: i64, hash: u64, data: Vec<u8> },
    AppendOutcome::{Appended { version }, UpToDate { version }, StaleGeneration { head: StreamHead }, Gap { head: i64 }, Diverged { version: i64 }},
    SnapshotOutcome::{Stored, Exists { hash: u64 }, Superseded},
    CommandLogError::{StreamNotFound, KindMismatch { stored, requested }, Db(PgError)},
}
impl PgCommandLogDb {
    pub fn migrations() -> Vec<String>;
    pub async fn new(pool: &PgConnectionPool) -> Result<Self, CommandLogError>;
    pub async fn ensure_stream(&self, stream_id: Uuid, kind: &str) -> Result<StreamHead, CommandLogError>;
    pub async fn head(&self, stream_id: Uuid) -> Result<Option<StreamHead>, CommandLogError>;
    pub async fn begin_generation(&self, stream_id: Uuid) -> Result<StreamHead, CommandLogError>;
    pub async fn append(&self, stream_id: Uuid, commands: &[StoredCommand]) -> Result<AppendOutcome, CommandLogError>;
    pub async fn commands_after(&self, stream_id: Uuid, after: i64, limit: i64) -> Result<Vec<StoredCommand>, CommandLogError>;
    pub async fn last_commands(&self, stream_id: Uuid, count: i64) -> Result<Vec<StoredCommand>, CommandLogError>;
    pub async fn try_store_snapshot(&self, stream_id: Uuid, version: i64, hash: u64, data: &[u8]) -> Result<SnapshotOutcome, CommandLogError>;
    pub async fn latest_snapshot(&self, stream_id: Uuid, at_or_before: i64) -> Result<Option<StoredSnapshot>, CommandLogError>;
    pub async fn prune_snapshots(&self, stream_id: Uuid, keep: i64) -> Result<u64, CommandLogError>;
}
```

- [ ] **Step 1: Remove `event_source`**

Delete the folder and the test file listed above. In `crates/shine-infra/src/db/mod.rs`, replace `pub mod event_source;` with `pub mod command_log;`. Run `cargo build -p shine-infra`. Expected: the only error is the missing `command_log` module (nothing else used `event_source`).

- [ ] **Step 2: BYTEA parameter type**

In `crates/shine-infra/src/db/postgres/pg_type.rs`, add after the `PgValueTypeVarcharArray` impls:

```rust
#[derive(Debug)]
pub struct PgValueTypeBytea;
impl PgValue for PgValueTypeBytea {
    const PG_TYPE: PgType = PgType::BYTEA;
}
```

and after `impl ToPgType for &str`:

```rust
impl ToPgType for Vec<u8> {
    type PgValueType = PgValueTypeBytea;
}
```

In `crates/shine-infra/src/db/postgres/mod.rs`, add `PgValueTypeBytea` to the `pg_type::{…}` re-export list.

- [ ] **Step 3: Schema**

Create `crates/shine-infra/src/db/command_log/sql/V1__command_log.sql`. `data` is `TEXT`: the log never queries inside a command, and it must store the canonical text byte for byte. `jsonb` would also reformat the text and reject `\u0000`.

```sql
CREATE TABLE cl_streams (
    stream_id UUID NOT NULL PRIMARY KEY,
    kind VARCHAR(64) NOT NULL,
    version BIGINT NOT NULL DEFAULT 0 CHECK (version >= 0),
    generation BIGINT NOT NULL DEFAULT 0 CHECK (generation >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE cl_commands (
    stream_id UUID NOT NULL REFERENCES cl_streams (stream_id) ON DELETE CASCADE,
    version BIGINT NOT NULL CHECK (version > 0),
    command_id UUID NOT NULL,
    user_id UUID NOT NULL,
    generation BIGINT NOT NULL,
    data TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (stream_id, version)
);

CREATE OR REPLACE FUNCTION cl_commands_immutable()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'cl_commands rows are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER cl_commands_no_update
BEFORE UPDATE ON cl_commands
FOR EACH ROW
EXECUTE FUNCTION cl_commands_immutable();

CREATE TABLE cl_snapshots (
    stream_id UUID NOT NULL,
    version BIGINT NOT NULL,
    hash BIGINT NOT NULL,
    data BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (stream_id, version),
    FOREIGN KEY (stream_id, version) REFERENCES cl_commands (stream_id, version) ON DELETE CASCADE
);
```

- [ ] **Step 4: Write the failing tests**

Create `crates/shine-infra/tests/pg_command_log.rs`:

```rust
use shine_infra::db::{
    command_log::{AppendOutcome, CommandLogError, PgCommandLogDb, SnapshotOutcome, StoredCommand},
    postgres::{create_postgres_pool, PgConnectionPool},
};
use shine_test::test;
use std::env;
use tokio::sync::OnceCell;
use uuid::Uuid;

static INIT: OnceCell<()> = OnceCell::const_new();

async fn setup() -> Option<PgCommandLogDb> {
    let Ok(cns) = env::var("SHINE_TEST_PG_CNS") else {
        tracing::warn!("SHINE_TEST_PG_CNS not set, skipping test");
        return None;
    };
    let _ = rustls::crypto::ring::default_provider().install_default();
    let pool: PgConnectionPool = create_postgres_pool(&cns).await.unwrap();
    INIT.get_or_init(|| async {
        let mut client = pool.get().await.unwrap();
        client
            .migrate("command_log", &PgCommandLogDb::migrations())
            .await
            .unwrap();
    })
    .await;
    Some(PgCommandLogDb::new(&pool).await.unwrap())
}

fn cmd(version: i64, generation: i64) -> StoredCommand {
    StoredCommand {
        version,
        command_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        generation,
        data: format!(r#"{{"op":"set","idx":0,"value":{version}}}"#),
    }
}

/// Creates a stream and moves it to generation 1, as hydration does.
async fn new_stream(db: &PgCommandLogDb) -> Uuid {
    let stream_id = Uuid::new_v4();
    db.ensure_stream(stream_id, "demo").await.unwrap();
    assert_eq!(db.begin_generation(stream_id).await.unwrap().generation, 1);
    stream_id
}

#[test]
async fn ensure_stream_creates_once_and_rejects_other_kind() {
    let Some(db) = setup().await else { return };
    let stream_id = Uuid::new_v4();
    let head = db.ensure_stream(stream_id, "demo").await.unwrap();
    assert_eq!((head.kind.as_str(), head.version, head.generation), ("demo", 0, 0));
    assert_eq!(db.ensure_stream(stream_id, "demo").await.unwrap(), head);
    assert!(matches!(
        db.ensure_stream(stream_id, "other").await,
        Err(CommandLogError::KindMismatch { .. })
    ));
}

#[test]
async fn append_is_idempotent_and_accepts_overlap() {
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    let batch: Vec<_> = (1..=5).map(|v| cmd(v, 1)).collect();

    assert_eq!(db.append(stream_id, &batch[..3]).await.unwrap(), AppendOutcome::Appended { version: 3 });
    assert_eq!(db.append(stream_id, &batch[..3]).await.unwrap(), AppendOutcome::UpToDate { version: 3 });
    assert_eq!(db.append(stream_id, &batch[1..]).await.unwrap(), AppendOutcome::Appended { version: 5 });

    assert_eq!(db.commands_after(stream_id, 0, 100).await.unwrap(), batch);
    assert_eq!(db.last_commands(stream_id, 2).await.unwrap(), batch[3..].to_vec());
    assert_eq!(db.head(stream_id).await.unwrap().unwrap().version, 5);
}

#[test]
async fn append_matches_the_overlap_by_version() {
    // A worker that filled a hole from Postgres appends a list with a gap in it.
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    let batch: Vec<_> = (1..=10).map(|v| cmd(v, 1)).collect();
    db.append(stream_id, &batch).await.unwrap();

    let with_hole = vec![batch[1].clone(), batch[2].clone(), batch[8].clone(), batch[9].clone(), cmd(11, 1)];
    assert_eq!(db.append(stream_id, &with_hole).await.unwrap(), AppendOutcome::Appended { version: 11 });

    let mut other_user = batch[8].clone();
    other_user.user_id = Uuid::new_v4();
    assert_eq!(
        db.append(stream_id, &[batch[1].clone(), other_user]).await.unwrap(),
        AppendOutcome::Diverged { version: 9 }
    );
}

#[test]
async fn append_rejects_gap_divergence_and_stale_generation() {
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    db.append(stream_id, &[cmd(1, 1)]).await.unwrap();

    assert_eq!(db.append(stream_id, &[cmd(3, 1)]).await.unwrap(), AppendOutcome::Gap { head: 1 });
    assert_eq!(
        db.append(stream_id, &[cmd(1, 1)]).await.unwrap(),
        AppendOutcome::Diverged { version: 1 },
        "same version, different command id"
    );

    db.begin_generation(stream_id).await.unwrap(); // generation 2
    assert!(matches!(
        db.append(stream_id, &[cmd(2, 1)]).await.unwrap(),
        AppendOutcome::StaleGeneration { .. }
    ));
    assert_eq!(db.append(stream_id, &[cmd(2, 2)]).await.unwrap(), AppendOutcome::Appended { version: 2 });
}

#[test]
async fn command_text_is_stored_verbatim() {
    // jsonb would reject the \u0000 escape and reformat the text.
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    let mut command = cmd(1, 1);
    command.data = r#"{"op":"set","idx":0,"value":1,"note":"a\u0000b"}"#.to_string();
    db.append(stream_id, &[command.clone()]).await.unwrap();
    assert_eq!(db.commands_after(stream_id, 0, 10).await.unwrap(), vec![command]);
}

#[test]
async fn concurrent_identical_appends_converge() {
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    let batch: Vec<_> = (1..=20).map(|v| cmd(v, 1)).collect();

    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let (db, batch) = (db.clone(), batch.clone());
            tokio::spawn(async move { db.append(stream_id, &batch).await.unwrap() })
        })
        .collect();
    let mut appended = 0;
    for task in tasks {
        match task.await.unwrap() {
            AppendOutcome::Appended { version: 20 } => appended += 1,
            AppendOutcome::UpToDate { version: 20 } => {}
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    assert_eq!(appended, 1);
    assert_eq!(db.commands_after(stream_id, 0, 100).await.unwrap(), batch);
}

#[test]
async fn snapshot_race_reports_the_stored_hash() {
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    db.append(stream_id, &(1..=10).map(|v| cmd(v, 1)).collect::<Vec<_>>())
        .await
        .unwrap();

    let hash = u64::MAX - 1; // above i64::MAX, checks the bit cast
    assert_eq!(db.try_store_snapshot(stream_id, 5, hash, b"first").await.unwrap(), SnapshotOutcome::Stored);
    assert_eq!(
        db.try_store_snapshot(stream_id, 5, 7, b"second").await.unwrap(),
        SnapshotOutcome::Exists { hash }
    );
    let latest = db.latest_snapshot(stream_id, 9).await.unwrap().unwrap();
    assert_eq!((latest.version, latest.hash, latest.data.as_slice()), (5, hash, &b"first"[..]));
    assert!(db.latest_snapshot(stream_id, 4).await.unwrap().is_none());
}

#[test]
async fn snapshot_below_a_newer_one_is_superseded() {
    // Below the prune horizon a lagging, drifted instance must not certify its own state.
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    db.append(stream_id, &(1..=10).map(|v| cmd(v, 1)).collect::<Vec<_>>())
        .await
        .unwrap();
    assert_eq!(db.try_store_snapshot(stream_id, 10, 1, b"s").await.unwrap(), SnapshotOutcome::Stored);
    assert_eq!(db.try_store_snapshot(stream_id, 5, 2, b"s").await.unwrap(), SnapshotOutcome::Superseded);
    assert!(db.latest_snapshot(stream_id, 9).await.unwrap().is_none());
    assert_eq!(db.try_store_snapshot(stream_id, 10, 3, b"s").await.unwrap(), SnapshotOutcome::Exists { hash: 1 });
}

#[test]
async fn prune_keeps_the_latest_snapshots() {
    let Some(db) = setup().await else { return };
    let stream_id = new_stream(&db).await;
    db.append(stream_id, &(1..=20).map(|v| cmd(v, 1)).collect::<Vec<_>>())
        .await
        .unwrap();
    for version in [5, 10, 15, 20] {
        db.try_store_snapshot(stream_id, version, version as u64, b"s").await.unwrap();
    }
    assert_eq!(db.prune_snapshots(stream_id, 2).await.unwrap(), 2);
    assert_eq!(db.latest_snapshot(stream_id, 19).await.unwrap().unwrap().version, 15);
    assert!(db.latest_snapshot(stream_id, 14).await.unwrap().is_none());
    assert_eq!(db.prune_snapshots(stream_id, 5).await.unwrap(), 0, "fewer than keep: nothing to delete");
}
```

- [ ] **Step 5: Run to verify it fails**

Run (PowerShell): `$env:SHINE_TEST_PG_CNS="…/database-builder?sslmode=disable"; cargo test -p shine-infra --test pg_command_log`
Expected: compile errors (types not found).

- [ ] **Step 6: Implement the types**

`crates/shine-infra/src/db/command_log/mod.rs`:

```rust
//! State-agnostic, ordered command log per stream, with snapshots.

mod command_log_types;
mod pg_command_log;

pub use self::{
    command_log_types::{AppendOutcome, CommandLogError, SnapshotOutcome, StoredCommand, StoredSnapshot, StreamHead},
    pg_command_log::PgCommandLogDb,
};
```

`command_log_types.rs`:

```rust
use crate::db::postgres::PgError;
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamHead {
    pub kind: String,
    /// Highest persisted version.
    pub version: i64,
    /// Bumped each time the stream is repopulated into Redis.
    pub generation: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredCommand {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    /// Redis generation the command was ingested in.
    pub generation: i64,
    /// The canonical command JSON.
    pub data: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredSnapshot {
    pub version: i64,
    pub hash: u64,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppendOutcome {
    /// New rows were stored; `version` is the new head.
    Appended { version: i64 },
    /// Every given row was already stored; `version` is the head.
    UpToDate { version: i64 },
    /// New rows come from another generation than the head's.
    StaleGeneration { head: StreamHead },
    /// The first new row is not `head + 1`, or the new rows are not contiguous.
    Gap { head: i64 },
    /// A given row at or below the head differs from the stored row at `version`.
    Diverged { version: i64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotOutcome {
    Stored,
    /// A snapshot already exists at the version, with this hash.
    Exists { hash: u64 },
    /// A newer snapshot exists: the version is below the prune horizon and is not stored.
    Superseded,
}

#[derive(Debug, ThisError)]
pub enum CommandLogError {
    #[error("Stream not found")]
    StreamNotFound,
    #[error("Stream kind mismatch: stored {stored}, requested {requested}")]
    KindMismatch { stored: String, requested: String },
    #[error(transparent)]
    Db(#[from] PgError),
}
```

- [ ] **Step 7: Implement `PgCommandLogDb`**

`pg_command_log.rs`:

```rust
use crate::{
    db::{
        command_log::{AppendOutcome, CommandLogError, SnapshotOutcome, StoredCommand, StoredSnapshot, StreamHead},
        postgres::{PgConnection, PgConnectionPool, PgError, PgPooledConnection, PgRawConnection},
    },
    pg_query,
};
use postgres_from_row::FromRow;
use std::{collections::HashMap, sync::Arc};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

const MIGRATION_V1: &str = include_str!("sql/V1__command_log.sql");

/// Bounds of an append transaction: a dead or stuck holder must not keep the stream's row lock.
const APPEND_TIMEOUTS: &str = "SET LOCAL lock_timeout = '5s'; SET LOCAL idle_in_transaction_session_timeout = '30s'";

#[derive(FromRow)]
struct HeadRow {
    kind: String,
    version: i64,
    generation: i64,
}

impl From<HeadRow> for StreamHead {
    fn from(row: HeadRow) -> Self {
        Self {
            kind: row.kind,
            version: row.version,
            generation: row.generation,
        }
    }
}

#[derive(FromRow)]
struct CommandRow {
    version: i64,
    command_id: Uuid,
    user_id: Uuid,
    generation: i64,
    data: String,
}

impl From<CommandRow> for StoredCommand {
    fn from(row: CommandRow) -> Self {
        Self {
            version: row.version,
            command_id: row.command_id,
            user_id: row.user_id,
            generation: row.generation,
            data: row.data,
        }
    }
}

#[derive(FromRow)]
struct IdRow {
    version: i64,
    command_id: Uuid,
    user_id: Uuid,
}

#[derive(FromRow)]
struct SnapshotRow {
    version: i64,
    hash: i64,
    data: Vec<u8>,
}

pg_query!( InsertStream =>
    in = stream_id: Uuid, kind: &str;
    sql = "INSERT INTO cl_streams (stream_id, kind) VALUES ($1, $2) ON CONFLICT (stream_id) DO NOTHING"
);

pg_query!( GetHead =>
    in = stream_id: Uuid;
    out = HeadRow;
    sql = "SELECT kind, version, generation FROM cl_streams WHERE stream_id = $1"
);

pg_query!( LockHead =>
    in = stream_id: Uuid;
    out = HeadRow;
    sql = "SELECT kind, version, generation FROM cl_streams WHERE stream_id = $1 FOR UPDATE"
);

pg_query!( BumpGeneration =>
    in = stream_id: Uuid;
    out = HeadRow;
    sql = r#"
        UPDATE cl_streams SET generation = generation + 1, updated_at = now()
        WHERE stream_id = $1
        RETURNING kind, version, generation
    "#
);

pg_query!( SetHeadVersion =>
    in = stream_id: Uuid, version: i64;
    sql = "UPDATE cl_streams SET version = $2, updated_at = now() WHERE stream_id = $1"
);

pg_query!( InsertCommand =>
    in = stream_id: Uuid, version: i64, command_id: Uuid, user_id: Uuid, generation: i64, data: &str;
    sql = r#"
        INSERT INTO cl_commands (stream_id, version, command_id, user_id, generation, data)
        VALUES ($1, $2, $3, $4, $5, $6)
    "#
);

pg_query!( CommandIdsInRange =>
    in = stream_id: Uuid, from_version: i64, to_version: i64;
    out = IdRow;
    sql = r#"
        SELECT version, command_id, user_id FROM cl_commands
        WHERE stream_id = $1 AND version >= $2 AND version <= $3
    "#
);

pg_query!( CommandsAfter =>
    in = stream_id: Uuid, after: i64, limit: i64;
    out = CommandRow;
    sql = r#"
        SELECT version, command_id, user_id, generation, data FROM cl_commands
        WHERE stream_id = $1 AND version > $2
        ORDER BY version
        LIMIT $3
    "#
);

pg_query!( LastCommands =>
    in = stream_id: Uuid, count: i64;
    out = CommandRow;
    sql = r#"
        SELECT * FROM (
            SELECT version, command_id, user_id, generation, data FROM cl_commands
            WHERE stream_id = $1
            ORDER BY version DESC
            LIMIT $2
        ) tail ORDER BY version
    "#
);

pg_query!( InsertSnapshot =>
    in = stream_id: Uuid, version: i64, hash: i64, data: Vec<u8>;
    sql = r#"
        INSERT INTO cl_snapshots (stream_id, version, hash, data)
        SELECT $1, $2, $3, $4
        WHERE NOT EXISTS (SELECT 1 FROM cl_snapshots WHERE stream_id = $1 AND version > $2)
        ON CONFLICT (stream_id, version) DO NOTHING
    "#
);

pg_query!( GetSnapshotHash =>
    in = stream_id: Uuid, version: i64;
    out = hash: i64;
    sql = "SELECT hash FROM cl_snapshots WHERE stream_id = $1 AND version = $2"
);

pg_query!( LatestSnapshot =>
    in = stream_id: Uuid, at_or_before: i64;
    out = SnapshotRow;
    sql = r#"
        SELECT version, hash, data FROM cl_snapshots
        WHERE stream_id = $1 AND version <= $2
        ORDER BY version DESC
        LIMIT 1
    "#
);

pg_query!( PruneSnapshots =>
    in = stream_id: Uuid, keep: i64;
    sql = r#"
        DELETE FROM cl_snapshots
        WHERE stream_id = $1 AND version < (
            SELECT version FROM cl_snapshots WHERE stream_id = $1
            ORDER BY version DESC OFFSET ($2 - 1) LIMIT 1
        )
    "#
);

struct Statements {
    insert_stream: InsertStream,
    get_head: GetHead,
    lock_head: LockHead,
    bump_generation: BumpGeneration,
    set_head_version: SetHeadVersion,
    insert_command: InsertCommand,
    command_ids_in_range: CommandIdsInRange,
    commands_after: CommandsAfter,
    last_commands: LastCommands,
    insert_snapshot: InsertSnapshot,
    get_snapshot_hash: GetSnapshotHash,
    latest_snapshot: LatestSnapshot,
    prune_snapshots: PruneSnapshots,
}

struct Inner {
    pool: PgConnectionPool,
    stmts: Statements,
}

/// Postgres command log. Cheap to clone.
#[derive(Clone)]
pub struct PgCommandLogDb(Arc<Inner>);

fn db(err: impl Into<PgError>) -> CommandLogError {
    CommandLogError::Db(err.into())
}

impl PgCommandLogDb {
    /// Schema migrations; run with `PgConnection::migrate("command_log", …)`.
    pub fn migrations() -> Vec<String> {
        vec![MIGRATION_V1.to_string()]
    }

    pub async fn new(pool: &PgConnectionPool) -> Result<Self, CommandLogError> {
        let client = pool.get().await.map_err(PgError::PoolError)?;
        let stmts = Statements {
            insert_stream: InsertStream::new(&client).await.map_err(db)?,
            get_head: GetHead::new(&client).await.map_err(db)?,
            lock_head: LockHead::new(&client).await.map_err(db)?,
            bump_generation: BumpGeneration::new(&client).await.map_err(db)?,
            set_head_version: SetHeadVersion::new(&client).await.map_err(db)?,
            insert_command: InsertCommand::new(&client).await.map_err(db)?,
            command_ids_in_range: CommandIdsInRange::new(&client).await.map_err(db)?,
            commands_after: CommandsAfter::new(&client).await.map_err(db)?,
            last_commands: LastCommands::new(&client).await.map_err(db)?,
            insert_snapshot: InsertSnapshot::new(&client).await.map_err(db)?,
            get_snapshot_hash: GetSnapshotHash::new(&client).await.map_err(db)?,
            latest_snapshot: LatestSnapshot::new(&client).await.map_err(db)?,
            prune_snapshots: PruneSnapshots::new(&client).await.map_err(db)?,
        };
        Ok(Self(Arc::new(Inner {
            pool: pool.clone(),
            stmts,
        })))
    }

    async fn client(&self) -> Result<PgPooledConnection<'_>, CommandLogError> {
        self.0.pool.get().await.map_err(|err| db(PgError::PoolError(err)))
    }

    pub async fn ensure_stream(&self, stream_id: Uuid, kind: &str) -> Result<StreamHead, CommandLogError> {
        let client = self.client().await?;
        let stmts = &self.0.stmts;
        stmts.insert_stream.execute(&client, &stream_id, &kind).await.map_err(db)?;
        let head: StreamHead = stmts.get_head.query_one(&client, &stream_id).await.map_err(db)?.into();
        if head.kind != kind {
            return Err(CommandLogError::KindMismatch {
                stored: head.kind,
                requested: kind.to_string(),
            });
        }
        Ok(head)
    }

    pub async fn head(&self, stream_id: Uuid) -> Result<Option<StreamHead>, CommandLogError> {
        let client = self.client().await?;
        let head = self.0.stmts.get_head.query_opt(&client, &stream_id).await.map_err(db)?;
        Ok(head.map(StreamHead::from))
    }

    /// Starts a new Redis generation. Serialized with `append` by the row lock.
    pub async fn begin_generation(&self, stream_id: Uuid) -> Result<StreamHead, CommandLogError> {
        let client = self.client().await?;
        let head = self.0.stmts.bump_generation.query_opt(&client, &stream_id).await.map_err(db)?;
        head.map(StreamHead::from).ok_or(CommandLogError::StreamNotFound)
    }

    /// Stores the rows above the head and checks the ones at or below it. `commands` must be ordered
    /// by version; it may have holes below the head.
    pub async fn append(&self, stream_id: Uuid, commands: &[StoredCommand]) -> Result<AppendOutcome, CommandLogError> {
        let mut client = self.client().await?;
        let tx = client.transaction(Some(IsolationLevel::ReadCommitted)).await.map_err(db)?;
        let outcome = self.append_in(&tx, stream_id, commands).await;
        let finished = match &outcome {
            Ok(AppendOutcome::Appended { .. }) => tx.commit().await,
            _ => tx.rollback().await,
        };
        // The append's own error matters more than a failed rollback.
        let outcome = outcome?;
        finished.map_err(db)?;
        Ok(outcome)
    }

    async fn append_in<T: PgRawConnection>(
        &self,
        tx: &PgConnection<T>,
        stream_id: Uuid,
        commands: &[StoredCommand],
    ) -> Result<AppendOutcome, CommandLogError> {
        let stmts = &self.0.stmts;
        tx.batch_execute(APPEND_TIMEOUTS).await.map_err(db)?;
        let head: StreamHead = stmts
            .lock_head
            .query_opt(tx, &stream_id)
            .await
            .map_err(db)?
            .ok_or(CommandLogError::StreamNotFound)?
            .into();

        // Rows at or below the head must be the stored ones, matched by version.
        let overlap: Vec<&StoredCommand> = commands.iter().filter(|c| c.version <= head.version).collect();
        if let (Some(first), Some(last)) = (overlap.first(), overlap.last()) {
            let stored: HashMap<i64, (Uuid, Uuid)> = stmts
                .command_ids_in_range
                .query(tx, &stream_id, &first.version, &last.version)
                .await
                .map_err(db)?
                .into_iter()
                .map(|row| (row.version, (row.command_id, row.user_id)))
                .collect();
            for command in &overlap {
                if stored.get(&command.version) != Some(&(command.command_id, command.user_id)) {
                    return Ok(AppendOutcome::Diverged { version: command.version });
                }
            }
        }

        let new: Vec<&StoredCommand> = commands.iter().filter(|c| c.version > head.version).collect();
        let Some(new_first) = new.first() else {
            return Ok(AppendOutcome::UpToDate { version: head.version });
        };
        if new_first.version != head.version + 1 || new.windows(2).any(|w| w[1].version != w[0].version + 1) {
            return Ok(AppendOutcome::Gap { head: head.version });
        }
        if new.iter().any(|c| c.generation != head.generation) {
            return Ok(AppendOutcome::StaleGeneration { head });
        }

        // Prepared once, then issued together so tokio-postgres pipelines them in one round trip.
        stmts.insert_command.statement(tx).await.map_err(db)?;
        let data: Vec<&str> = new.iter().map(|c| c.data.as_str()).collect();
        futures::future::try_join_all(new.iter().zip(&data).map(|(command, data)| {
            stmts.insert_command.execute(
                tx,
                &stream_id,
                &command.version,
                &command.command_id,
                &command.user_id,
                &command.generation,
                data,
            )
        }))
        .await
        .map_err(db)?;
        let version = new.last().map_or(head.version, |c| c.version);
        stmts.set_head_version.execute(tx, &stream_id, &version).await.map_err(db)?;
        Ok(AppendOutcome::Appended { version })
    }

    pub async fn commands_after(&self, stream_id: Uuid, after: i64, limit: i64) -> Result<Vec<StoredCommand>, CommandLogError> {
        let client = self.client().await?;
        let rows = self.0.stmts.commands_after.query(&client, &stream_id, &after, &limit).await.map_err(db)?;
        Ok(rows.into_iter().map(StoredCommand::from).collect())
    }

    /// The last `count` commands, ordered by version.
    pub async fn last_commands(&self, stream_id: Uuid, count: i64) -> Result<Vec<StoredCommand>, CommandLogError> {
        let client = self.client().await?;
        let rows = self.0.stmts.last_commands.query(&client, &stream_id, &count).await.map_err(db)?;
        Ok(rows.into_iter().map(StoredCommand::from).collect())
    }

    pub async fn try_store_snapshot(
        &self,
        stream_id: Uuid,
        version: i64,
        hash: u64,
        data: &[u8],
    ) -> Result<SnapshotOutcome, CommandLogError> {
        let client = self.client().await?;
        let stmts = &self.0.stmts;
        // Losers of the race usually find the row here and never upload their bytes.
        if let Some(stored) = stmts.get_snapshot_hash.query_opt(&client, &stream_id, &version).await.map_err(db)? {
            return Ok(SnapshotOutcome::Exists { hash: stored as u64 });
        }
        let inserted = stmts
            .insert_snapshot
            .execute(&client, &stream_id, &version, &(hash as i64), &data.to_vec())
            .await
            .map_err(db)?;
        if inserted == 1 {
            return Ok(SnapshotOutcome::Stored);
        }
        // Lost the insert race, or a newer snapshot exists.
        match stmts.get_snapshot_hash.query_opt(&client, &stream_id, &version).await.map_err(db)? {
            Some(stored) => Ok(SnapshotOutcome::Exists { hash: stored as u64 }),
            None => Ok(SnapshotOutcome::Superseded),
        }
    }

    pub async fn latest_snapshot(&self, stream_id: Uuid, at_or_before: i64) -> Result<Option<StoredSnapshot>, CommandLogError> {
        let client = self.client().await?;
        let row = self
            .0
            .stmts
            .latest_snapshot
            .query_opt(&client, &stream_id, &at_or_before)
            .await
            .map_err(db)?;
        Ok(row.map(|row| StoredSnapshot {
            version: row.version,
            hash: row.hash as u64,
            data: row.data,
        }))
    }

    /// Deletes all but the latest `keep` snapshots; returns the number deleted.
    pub async fn prune_snapshots(&self, stream_id: Uuid, keep: i64) -> Result<u64, CommandLogError> {
        let client = self.client().await?;
        self.0
            .stmts
            .prune_snapshots
            .execute(&client, &stream_id, &keep.max(1))
            .await
            .map_err(db)
    }
}
```

Notes for the implementer:
- `pg_query!` turns `in = x: T` into a method parameter `x: &T`. That is why the calls pass `&stream_id`, `&kind` (a `&&str`), `data` (already a `&&str` from iterating `&data`) and `&data.to_vec()`.
- The `data` vector exists so the pipelined futures borrow strings that outlive them. Writing `&command.data.as_str()` inside the closure would borrow a temporary (E0716).
- `tx.batch_execute` resolves through `PgConnection`'s `Deref` to the tokio-postgres `Transaction`.

- [ ] **Step 8: Run tests**

Run: `$env:SHINE_TEST_PG_CNS="…/database-builder?sslmode=disable"; cargo test -p shine-infra --test pg_command_log -- --nocapture`

Expected: 9 tests PASS, and the output does **not** contain "skipping". Then run `cargo build -p shine-infra -p shine-builder -p shine-identity`; expected: it builds.

- [ ] **Step 9: Checkpoint.** Report and wait.

---

## Task 3: Builder Postgres wiring, config, settings, errors

**Files:**
- Modify: `services/builder/Cargo.toml`, `services/builder/src/repositories/db.rs`, `services/builder/src/repositories/mod.rs`, `services/builder/src/main.rs`, `services/builder/src/app_state.rs`, `services/builder/src/app_config.rs`, `services/builder/src/settings/mod.rs`, `services/builder/src/models/mod.rs`, `services/builder/server_config.test.json`
- Create: `services/builder/src/models/command_stream_error.rs`
- Delete: `services/builder/sql_migrations/V1__commands.sql` and the now-empty `sql_migrations` folder

**Interfaces — Produces:**
- `repositories::create_postgres_pool(&DBConfig) -> Result<PgConnectionPool, PgError>`, which runs the command-log migrations.
- `AppState::new(config, &redis_pool, &pg_pool, core_services)`.
- `app_config::CommandStreamConfig`, read as camelCase JSON under `builder.commandStream`.
- `settings::CommandStreamSettings` with `impl From<&CommandStreamConfig>`. Fields:
  - counts (`i64`): `redis_keep_commands`, `redis_max_unpersisted`, `hydrate_last_n`, `redis_ttl_seconds`, `snapshot_every`, `keep_snapshots`
  - durations: `sync_interval`, `persist_delay`, `unload_grace`, `orphan_sweep_interval`, `orphan_idle`
  - `max_command_bytes: usize`
- `models::CommandStreamError::{StreamNotFound, NotReady, Internal{..}}` and `CommandStreamError::internal(err)`, with `From<CommandLogError>` and `From<RedisError>`.

- [ ] **Step 1: Cargo**

In `services/builder/Cargo.toml`:
- `[dependencies]`: add `base64 = { workspace = true }` after `serde_json`, and `shine-game = { workspace = true }` after `shine-core`.
- `[dev-dependencies]`: add `rustls = { workspace = true, features = ["ring"] }`. The Redis + Postgres tests install the ring crypto provider, because both `ring` and `aws-lc-rs` are compiled in.

- [ ] **Step 2: DB config and pool**

Replace `services/builder/src/repositories/db.rs` with:

```rust
use serde::{Deserialize, Serialize};
use shine_infra::db::{
    command_log::PgCommandLogDb,
    postgres::{PgConnectionPool, PgError},
    redis::{RedisConnectionPool, RedisError},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DBConfig {
    pub redis_cns: String,
    pub sql_cns: String,
}

pub async fn create_redis_pool(config: &DBConfig) -> Result<RedisConnectionPool, RedisError> {
    tracing::info!("Creating redis pool...");
    shine_infra::db::redis::create_redis_pool(config.redis_cns.as_str())
        .await
        .map_err(RedisError::PoolError)
}

/// Creates the pool and applies the command-log schema before returning it.
pub async fn create_postgres_pool(config: &DBConfig) -> Result<PgConnectionPool, PgError> {
    tracing::info!("Creating postgres pool...");
    let pool = shine_infra::db::postgres::create_postgres_pool(config.sql_cns.as_str())
        .await
        .map_err(PgError::CreatePoolError)?;
    let mut client = pool.get().await.map_err(PgError::PoolError)?;
    client.migrate("command_log", &PgCommandLogDb::migrations()).await?;
    drop(client);
    Ok(pool)
}
```

In `repositories/mod.rs`, change the re-export to `pub use self::db::{create_postgres_pool, create_redis_pool, DBConfig};`. Then delete `services/builder/sql_migrations/V1__commands.sql` and the empty folder.

- [ ] **Step 3: Config**

In `app_config.rs`, add before `AppConfig`:

```rust
/// Raw command stream configuration loaded from config files.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CommandStreamConfig {
    /// Commands kept in Redis per stream after they are persisted.
    pub redis_keep_commands: i64,
    /// Unpersisted commands per stream before writes are refused.
    pub redis_max_unpersisted: i64,
    /// Commands copied from Postgres when Redis is repopulated.
    pub hydrate_last_n: i64,
    /// Inactivity TTL of the Redis keys of a stream.
    pub redis_ttl_seconds: i64,
    /// Snapshot and checkpoint cadence, in versions.
    pub snapshot_every: i64,
    /// Snapshots kept per stream.
    pub keep_snapshots: i64,
    /// Interval of the periodic sync of the stream workers.
    pub sync_interval_ms: u64,
    /// Delay before new commands are persisted; each instance adds up to the same again as jitter.
    pub persist_delay_ms: u64,
    /// Time a stream stays loaded without local subscribers.
    pub unload_grace_seconds: u64,
    /// Interval of the orphan sweep.
    pub orphan_sweep_seconds: u64,
    /// Time without writes before a stream is swept.
    pub orphan_idle_seconds: u64,
    /// Max size of a command's JSON.
    pub max_command_bytes: usize,
}

impl Default for CommandStreamConfig {
    fn default() -> Self {
        Self {
            redis_keep_commands: 1000,
            redis_max_unpersisted: 5000,
            hydrate_last_n: 200,
            redis_ttl_seconds: 24 * 60 * 60,
            snapshot_every: 100,
            keep_snapshots: 3,
            sync_interval_ms: 1000,
            persist_delay_ms: 20,
            unload_grace_seconds: 60,
            orphan_sweep_seconds: 30,
            orphan_idle_seconds: 10,
            max_command_bytes: 16 * 1024,
        }
    }
}
```

and add to `AppConfig` after `chat`:

```rust
    #[serde(default)]
    pub command_stream: CommandStreamConfig,
```

In `server_config.test.json`, inside `"builder"` after `"chat": {…},`, add:

```json
    "commandStream": {
      "snapshotEvery": 5,
      "syncIntervalMs": 200
    },
```

The API tests in Task 10 rely on `snapshotEvery: 5`.

- [ ] **Step 4: Settings**

In `settings/mod.rs`, add:

```rust
use crate::app_config::CommandStreamConfig;
use std::time::Duration;

/// Normalized command stream configuration.
#[derive(Clone, Debug)]
pub struct CommandStreamSettings {
    pub redis_keep_commands: i64,
    pub redis_max_unpersisted: i64,
    pub hydrate_last_n: i64,
    pub redis_ttl_seconds: i64,
    pub snapshot_every: i64,
    pub keep_snapshots: i64,
    pub sync_interval: Duration,
    pub persist_delay: Duration,
    pub unload_grace: Duration,
    pub orphan_sweep_interval: Duration,
    pub orphan_idle: Duration,
    pub max_command_bytes: usize,
}

impl From<&CommandStreamConfig> for CommandStreamSettings {
    fn from(config: &CommandStreamConfig) -> Self {
        Self {
            redis_keep_commands: config.redis_keep_commands.max(1),
            redis_max_unpersisted: config.redis_max_unpersisted.max(1),
            hydrate_last_n: config.hydrate_last_n.max(0),
            redis_ttl_seconds: config.redis_ttl_seconds.max(60),
            snapshot_every: config.snapshot_every.max(1),
            keep_snapshots: config.keep_snapshots.max(1),
            sync_interval: Duration::from_millis(config.sync_interval_ms.max(10)),
            persist_delay: Duration::from_millis(config.persist_delay_ms.max(1)),
            unload_grace: Duration::from_secs(config.unload_grace_seconds),
            orphan_sweep_interval: Duration::from_secs(config.orphan_sweep_seconds.max(1)),
            orphan_idle: Duration::from_secs(config.orphan_idle_seconds),
            max_command_bytes: config.max_command_bytes.max(1),
        }
    }
}
```

- [ ] **Step 5: Error type**

Create `models/command_stream_error.rs`:

```rust
use shine_infra::db::{command_log::CommandLogError, redis::RedisError};
use std::{backtrace::Backtrace as StdBacktrace, error::Error as StdError, panic::Location};
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum CommandStreamError {
    #[error("Stream not found")]
    StreamNotFound,
    #[error("Stream is not ready in Redis")]
    NotReady,
    #[error("Internal command stream error")]
    Internal {
        #[source]
        source: Box<dyn StdError + Send + Sync>,
        location: &'static Location<'static>,
        backtrace: StdBacktrace,
    },
}

impl CommandStreamError {
    #[track_caller]
    pub fn internal(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Internal {
            source: Box::new(source),
            location: Location::caller(),
            backtrace: StdBacktrace::capture(),
        }
    }
}

impl From<CommandLogError> for CommandStreamError {
    #[track_caller]
    fn from(err: CommandLogError) -> Self {
        match err {
            CommandLogError::StreamNotFound => Self::StreamNotFound,
            err => Self::internal(err),
        }
    }
}

impl From<RedisError> for CommandStreamError {
    #[track_caller]
    fn from(err: RedisError) -> Self {
        Self::internal(err)
    }
}
```

In `models/mod.rs`, add `mod command_stream_error;` and `pub use command_stream_error::CommandStreamError;`.

- [ ] **Step 6: Pool wiring**

In `main.rs`:
- Replace `use repositories::create_redis_pool;` with `use repositories::{create_postgres_pool, create_redis_pool};`.
- Add `use shine_infra::db::postgres::PostgresPoolStatus;`.
- In `create`, replace the two lines that create the pool and the state with:

```rust
        let redis_pool = create_redis_pool(&config.feature.db).await?;
        let pg_pool = create_postgres_pool(&config.feature.db).await?;
        context.add_health_provider(PostgresPoolStatus::new(pg_pool.clone()));
        let state = AppState::new(config, &redis_pool, &pg_pool, context.core_services()).await?;
```

If `add_health_provider` needs a different argument shape, mirror how `HubStatus` is passed.

In `app_state.rs`, add the parameter `pg_pool: &PgConnectionPool` after `redis_pool` (import `shine_infra::db::postgres::PgConnectionPool`), and mark it `let _ = pg_pool;` for now. Task 6 uses it.

- [ ] **Step 7: Verify**

- Run `cargo build -p shine-builder`. Expected: it builds.
- Run `cargo test -p shine-builder` with `SHINE_TEST_REDIS_CNS` set. Expected: the existing tests PASS.

- [ ] **Step 8: Checkpoint.** Report and wait. Tell the user that production needs `builder.db.sqlCns` in the key vault: `server_config.prod.json` has no `sqlCns`.

---

## Task 4: Redis command-stream repository + scripts

**Files:**
- Modify: `services/builder/src/repositories/mod.rs`
- Create: `services/builder/src/repositories/command_stream/mod.rs`, `…/command_stream/redis/{mod.rs, scripts.rs, redis_command_stream_db.rs, redis_command_stream_test.rs}`

**Interfaces — Produces (used by Tasks 6–8):**
```rust
repositories::command_stream::redis::{
    RedisCommandStreamDb,                 // Clone, Arc<Inner>
    RedisStreamConfig { prefix: String, keep_commands: i64, max_unpersisted: i64, ttl_seconds: i64 },
    RedisStreamMeta { generation: i64, version: i64, persisted: i64 },
    RedisStreamEntry { version: i64, generation: i64, command_id: Uuid, user_id: Uuid, data: String },
    RedisIngest::{Accepted(i64), Duplicate(i64), NotReady, Busy},
    MarkOutcome::{Marked, Unchanged, Refused},
}
impl From<RedisStreamEntry> for StoredCommand
impl RedisCommandStreamDb {
    pub const PREFIX: &'static str = "cs";
    pub fn new(pool: &RedisConnectionPool, config: RedisStreamConfig) -> Self;
    pub async fn ingest(&self, stream_id: Uuid, command_id: Uuid, user_id: Uuid, data: &str) -> Result<RedisIngest, RedisError>;
    pub async fn hydrate(&self, stream_id: Uuid, generation: i64, head: i64, tail: &[StoredCommand]) -> Result<bool, RedisError>;
    pub async fn mark_persisted(&self, stream_id: Uuid, generation: i64, version: i64, command_id: Uuid) -> Result<MarkOutcome, RedisError>;
    pub async fn reset(&self, stream_id: Uuid, generation: i64) -> Result<bool, RedisError>;
    pub async fn meta(&self, stream_id: Uuid) -> Result<Option<RedisStreamMeta>, RedisError>;
    pub async fn read_after(&self, stream_id: Uuid, after: i64, limit: i64) -> Result<(Option<RedisStreamMeta>, Vec<RedisStreamEntry>), RedisError>;
    pub async fn try_lock_hydration(&self, stream_id: Uuid) -> Result<Option<String>, RedisError>;
    pub async fn unlock_hydration(&self, stream_id: Uuid, token: &str) -> Result<(), RedisError>;
    pub async fn idle_streams(&self, idle: Duration, limit: usize) -> Result<Vec<Uuid>, RedisError>;
    pub async fn retire_active(&self, stream_id: Uuid) -> Result<bool, RedisError>;
    pub async fn requeue_active(&self, stream_id: Uuid) -> Result<(), RedisError>;
    pub async fn check_eviction_policy(&self);
    pub async fn listen_to_wakes<F: Fn(Option<Uuid>) + Send + Sync + 'static>(&self, handler: F) -> Result<(), RedisError>;
}
```

Keys, all under `{prefix}`: `cs` in production, `cs-test-<uuid>` in tests.

| Key | Type | Content |
|---|---|---|
| `{prefix}:{id}:meta` | hash | `generation`, `version`, `persisted`. Its existence means "ready". |
| `{prefix}:{id}:log` | stream | entry id `"{version}-0"`, fields `g` (generation), `c` (command id), `u` (user), `d` (canonical JSON) |
| `{prefix}:{id}:ids` | zset | member = command id, score = version (the dedupe window) |
| `{prefix}:{id}:hydrating` | string | hydration lock (`SET NX PX`) |
| `{prefix}:active` | zset | member = stream id, score = last write time (Redis `TIME`, ms, refreshed at most once a second) |
| `{prefix}:wake` | pub/sub channel | payload = stream id |

- [ ] **Step 1: Write the failing tests**

Create `redis_command_stream_test.rs`:

```rust
use super::{MarkOutcome, RedisCommandStreamDb, RedisIngest, RedisStreamConfig};
use shine_infra::db::{self, command_log::StoredCommand};
use shine_test::test;
use std::{env, time::Duration};
use tokio::{sync::mpsc, time::timeout};
use uuid::Uuid;

/// A repository on its own key prefix, so tests never share keys, the activity set or the wake channel.
async fn setup(keep_commands: i64, max_unpersisted: i64) -> Option<RedisCommandStreamDb> {
    let Ok(cns) = env::var("SHINE_TEST_REDIS_CNS") else {
        tracing::warn!("SHINE_TEST_REDIS_CNS not set, skipping test");
        return None;
    };
    let pool = db::redis::create_redis_pool(&cns).await.unwrap();
    let config = RedisStreamConfig {
        prefix: format!("cs-test-{}", Uuid::new_v4()),
        keep_commands,
        max_unpersisted,
        ttl_seconds: 600,
    };
    Some(RedisCommandStreamDb::new(&pool, config))
}

/// Ingests `count` commands and returns their ids, in version order.
async fn ingest_many(redis: &RedisCommandStreamDb, stream_id: Uuid, count: usize) -> Vec<Uuid> {
    let mut ids = Vec::new();
    for _ in 0..count {
        let command_id = Uuid::new_v4();
        assert!(matches!(
            redis.ingest(stream_id, command_id, Uuid::nil(), "{}").await.unwrap(),
            RedisIngest::Accepted(_)
        ));
        ids.push(command_id);
    }
    ids
}

fn stored(version: i64) -> StoredCommand {
    StoredCommand {
        version,
        command_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        generation: 1,
        data: r#"{"op":"set","idx":0,"value":1}"#.into(),
    }
}

#[test]
async fn ingest_requires_hydration_and_assigns_dense_versions() {
    let Some(redis) = setup(2, 3).await else { return };
    let stream_id = Uuid::new_v4();
    let user = Uuid::new_v4();
    assert_eq!(redis.ingest(stream_id, Uuid::new_v4(), user, "{}").await.unwrap(), RedisIngest::NotReady);

    assert!(redis.hydrate(stream_id, 1, 0, &[]).await.unwrap());
    assert_eq!(redis.ingest(stream_id, Uuid::new_v4(), user, "{}").await.unwrap(), RedisIngest::Accepted(1));
    assert_eq!(redis.ingest(stream_id, Uuid::new_v4(), user, "{}").await.unwrap(), RedisIngest::Accepted(2));

    let (meta, entries) = redis.read_after(stream_id, 0, 10).await.unwrap();
    let meta = meta.unwrap();
    assert_eq!((meta.generation, meta.version, meta.persisted), (1, 2, 0));
    assert_eq!(entries.iter().map(|e| e.version).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(redis.read_after(stream_id, 1, 10).await.unwrap().1.len(), 1);
}

#[test]
async fn ingest_dedupes_command_id() {
    let Some(redis) = setup(2, 3).await else { return };
    let stream_id = Uuid::new_v4();
    redis.hydrate(stream_id, 1, 0, &[]).await.unwrap();
    let command_id = Uuid::new_v4();
    assert_eq!(redis.ingest(stream_id, command_id, Uuid::nil(), "{}").await.unwrap(), RedisIngest::Accepted(1));
    assert_eq!(redis.ingest(stream_id, command_id, Uuid::nil(), "{}").await.unwrap(), RedisIngest::Duplicate(1));
    assert_eq!(redis.read_after(stream_id, 0, 10).await.unwrap().1.len(), 1);
}

#[test]
async fn ingest_is_busy_when_persistence_lags() {
    let Some(redis) = setup(2, 3).await else { return };
    let stream_id = Uuid::new_v4();
    redis.hydrate(stream_id, 1, 0, &[]).await.unwrap();
    let ids = ingest_many(&redis, stream_id, 3).await;
    assert_eq!(redis.ingest(stream_id, Uuid::new_v4(), Uuid::nil(), "{}").await.unwrap(), RedisIngest::Busy);
    assert_eq!(redis.mark_persisted(stream_id, 1, 3, ids[2]).await.unwrap(), MarkOutcome::Marked);
    assert_eq!(redis.ingest(stream_id, Uuid::new_v4(), Uuid::nil(), "{}").await.unwrap(), RedisIngest::Accepted(4));
}

#[test]
async fn mark_persisted_trims_only_persisted_entries() {
    let Some(redis) = setup(2, 100).await else { return };
    let stream_id = Uuid::new_v4();
    redis.hydrate(stream_id, 1, 0, &[]).await.unwrap();
    let ids = ingest_many(&redis, stream_id, 6).await;
    assert_eq!(
        redis.mark_persisted(stream_id, 2, 5, ids[4]).await.unwrap(),
        MarkOutcome::Unchanged,
        "other generation"
    );
    assert_eq!(redis.mark_persisted(stream_id, 1, 3, ids[2]).await.unwrap(), MarkOutcome::Marked);
    // keep=2 would allow trimming below 5, but only versions <= persisted (3) may go.
    let versions: Vec<_> = redis.read_after(stream_id, 0, 10).await.unwrap().1.iter().map(|e| e.version).collect();
    assert_eq!(versions, vec![4, 5, 6]);
    assert_eq!(redis.mark_persisted(stream_id, 1, 6, ids[5]).await.unwrap(), MarkOutcome::Marked);
    let versions: Vec<_> = redis.read_after(stream_id, 0, 10).await.unwrap().1.iter().map(|e| e.version).collect();
    assert_eq!(versions, vec![5, 6], "keeps the last keep_commands entries");
    assert_eq!(redis.mark_persisted(stream_id, 1, 6, ids[5]).await.unwrap(), MarkOutcome::Unchanged);
}

#[test]
async fn mark_persisted_refuses_a_copy_that_disagrees() {
    // A copy that went back in time holds other commands at the persisted versions, or none.
    let Some(redis) = setup(100, 100).await else { return };
    let stream_id = Uuid::new_v4();
    redis.hydrate(stream_id, 1, 0, &[]).await.unwrap();
    let ids = ingest_many(&redis, stream_id, 3).await;
    assert_eq!(
        redis.mark_persisted(stream_id, 1, 2, Uuid::new_v4()).await.unwrap(),
        MarkOutcome::Refused,
        "another command at the version"
    );
    assert_eq!(
        redis.mark_persisted(stream_id, 1, 4, ids[2]).await.unwrap(),
        MarkOutcome::Refused,
        "behind the version"
    );
    assert_eq!(redis.mark_persisted(stream_id, 1, 2, ids[1]).await.unwrap(), MarkOutcome::Marked);
}

#[test]
async fn hydrate_copies_tail_and_newer_generation_wins() {
    let Some(redis) = setup(2, 3).await else { return };
    let stream_id = Uuid::new_v4();
    let tail = vec![stored(9), stored(10)];
    assert!(redis.hydrate(stream_id, 2, 10, &tail).await.unwrap());
    assert!(!redis.hydrate(stream_id, 1, 0, &[]).await.unwrap(), "older generation is skipped");
    assert!(!redis.hydrate(stream_id, 2, 0, &[]).await.unwrap(), "same generation is skipped");

    let (meta, entries) = redis.read_after(stream_id, 0, 10).await.unwrap();
    let meta = meta.unwrap();
    assert_eq!((meta.generation, meta.version, meta.persisted), (2, 10, 10));
    assert_eq!(entries.iter().map(|e| e.command_id).collect::<Vec<_>>(), vec![tail[0].command_id, tail[1].command_id]);
    assert_eq!(
        redis.ingest(stream_id, tail[1].command_id, Uuid::nil(), "{}").await.unwrap(),
        RedisIngest::Duplicate(10),
        "hydrated commands keep their dedupe entries"
    );
    assert_eq!(redis.ingest(stream_id, Uuid::new_v4(), Uuid::nil(), "{}").await.unwrap(), RedisIngest::Accepted(11));

    assert!(redis.hydrate(stream_id, 3, 10, &[]).await.unwrap(), "newer generation overwrites");
    assert_eq!(redis.meta(stream_id).await.unwrap().unwrap().version, 10);
}

#[test]
async fn reset_requires_matching_generation() {
    let Some(redis) = setup(2, 3).await else { return };
    let stream_id = Uuid::new_v4();
    redis.hydrate(stream_id, 4, 0, &[]).await.unwrap();
    assert!(!redis.reset(stream_id, 3).await.unwrap());
    assert!(redis.meta(stream_id).await.unwrap().is_some());
    assert!(redis.reset(stream_id, 4).await.unwrap());
    assert!(redis.meta(stream_id).await.unwrap().is_none());
}

#[test]
async fn hydration_lock_is_exclusive() {
    let Some(redis) = setup(2, 3).await else { return };
    let stream_id = Uuid::new_v4();
    let token = redis.try_lock_hydration(stream_id).await.unwrap().unwrap();
    assert!(redis.try_lock_hydration(stream_id).await.unwrap().is_none());
    redis.unlock_hydration(stream_id, "not-the-token").await.unwrap();
    assert!(redis.try_lock_hydration(stream_id).await.unwrap().is_none());
    redis.unlock_hydration(stream_id, &token).await.unwrap();
    assert!(redis.try_lock_hydration(stream_id).await.unwrap().is_some());
}

#[test]
async fn the_activity_set_keeps_streams_until_fully_persisted() {
    let Some(redis) = setup(2, 100).await else { return };
    let stream_id = Uuid::new_v4();
    redis.hydrate(stream_id, 1, 0, &[]).await.unwrap();
    let ids = ingest_many(&redis, stream_id, 1).await;
    assert_eq!(redis.idle_streams(Duration::ZERO, 10).await.unwrap(), vec![stream_id]);

    assert!(!redis.retire_active(stream_id).await.unwrap(), "version 1 is not persisted yet");
    redis.requeue_active(stream_id).await.unwrap();
    assert_eq!(redis.idle_streams(Duration::ZERO, 10).await.unwrap(), vec![stream_id]);

    assert_eq!(redis.mark_persisted(stream_id, 1, 1, ids[0]).await.unwrap(), MarkOutcome::Marked);
    assert!(redis.retire_active(stream_id).await.unwrap());
    assert!(redis.idle_streams(Duration::ZERO, 10).await.unwrap().is_empty());

    redis.ingest(stream_id, Uuid::new_v4(), Uuid::nil(), "{}").await.unwrap();
    assert_eq!(
        redis.idle_streams(Duration::ZERO, 10).await.unwrap(),
        vec![stream_id],
        "a new write lists it again"
    );
}

#[test]
async fn ingest_wakes_listeners_with_the_stream_id() {
    let Some(redis) = setup(2, 100).await else { return };
    let stream_id = Uuid::new_v4();
    redis.hydrate(stream_id, 1, 0, &[]).await.unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    redis
        .listen_to_wakes(move |woken| {
            let _ = tx.send(woken);
        })
        .await
        .unwrap();

    // The subscription is asynchronous: keep writing until the first wake arrives.
    let woken = timeout(Duration::from_secs(5), async {
        loop {
            redis.ingest(stream_id, Uuid::new_v4(), Uuid::nil(), "{}").await.unwrap();
            if let Ok(Some(woken)) = timeout(Duration::from_millis(100), rx.recv()).await {
                return woken;
            }
        }
    })
    .await
    .expect("no wake within 5s");
    assert_eq!(woken, Some(stream_id));
}
```

Create `redis/mod.rs`:

```rust
mod redis_command_stream_db;
mod scripts;

#[cfg(test)]
mod redis_command_stream_test;

pub use self::redis_command_stream_db::{MarkOutcome, RedisCommandStreamDb, RedisIngest, RedisStreamConfig, RedisStreamEntry, RedisStreamMeta};
```

Create `command_stream/mod.rs` with `pub mod redis;`, and add `pub mod command_stream;` to `repositories/mod.rs`.

- [ ] **Step 2: Run to verify it fails**

Run: `$env:SHINE_TEST_REDIS_CNS="redis://redis.mockbox.foo:6379"; cargo test -p shine-builder redis_command_stream`
Expected: compile errors.

- [ ] **Step 3: Scripts**

Create `scripts.rs`. Lua does not roll back on error, so every script checks first and writes last:

```rust
use redis::Script;
use std::sync::LazyLock;

/// KEYS: meta, log, ids, active. ARGV: stream_id, command_id, user_id, data, max_unpersisted, ttl, wake channel.
pub static INGEST: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local meta = redis.call('HMGET', KEYS[1], 'generation', 'version', 'persisted')
if not meta[1] then return {'not_ready', 0} end
local dup = redis.call('ZSCORE', KEYS[3], ARGV[2])
if dup then return {'duplicate', tonumber(dup)} end
local version = tonumber(meta[2])
if version - tonumber(meta[3]) >= tonumber(ARGV[5]) then return {'busy', version} end
local v = version + 1
redis.call('XADD', KEYS[2], string.format('%d-0', v), 'g', meta[1], 'c', ARGV[2], 'u', ARGV[3], 'd', ARGV[4])
redis.call('HSET', KEYS[1], 'version', v)
redis.call('ZADD', KEYS[3], v, ARGV[2])
-- The TTLs and the activity score are refreshed at most once a second.
local now = redis.call('TIME')
local now_ms = tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000)
local seen = redis.call('ZSCORE', KEYS[4], ARGV[1])
if not seen or now_ms - tonumber(seen) >= 1000 then
  redis.call('ZADD', KEYS[4], now_ms, ARGV[1])
  redis.call('EXPIRE', KEYS[1], ARGV[6])
  redis.call('EXPIRE', KEYS[2], ARGV[6])
  redis.call('EXPIRE', KEYS[3], ARGV[6])
end
redis.call('PUBLISH', ARGV[7], ARGV[1])
return {'ok', v}
"#,
    )
});

/// KEYS: meta, log, ids, active. ARGV: generation, head, ttl, stream_id, then 5 per entry:
/// version, command_id, user_id, generation, data.
pub static HYDRATE: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local current = redis.call('HGET', KEYS[1], 'generation')
if current and tonumber(current) >= tonumber(ARGV[1]) then return 0 end
redis.call('DEL', KEYS[2], KEYS[3])
local i = 5
while i <= #ARGV do
  redis.call('XADD', KEYS[2], ARGV[i] .. '-0', 'g', ARGV[i + 3], 'c', ARGV[i + 1], 'u', ARGV[i + 2], 'd', ARGV[i + 4])
  redis.call('ZADD', KEYS[3], ARGV[i], ARGV[i + 1])
  i = i + 5
end
redis.call('DEL', KEYS[1])
redis.call('HSET', KEYS[1], 'generation', ARGV[1], 'version', ARGV[2], 'persisted', ARGV[2])
redis.call('EXPIRE', KEYS[1], ARGV[3])
redis.call('EXPIRE', KEYS[2], ARGV[3])
redis.call('EXPIRE', KEYS[3], ARGV[3])
-- Nothing is unpersisted now; the next write lists the stream again and refreshes the TTLs.
redis.call('ZREM', KEYS[4], ARGV[4])
return 1
"#,
    )
});

/// KEYS: meta, log, ids. ARGV: generation, version, command_id, keep.
/// Returns 1 when marked, 0 when nothing changed, -1 when this copy disagrees with what was persisted:
/// it is behind `version`, or holds another command there (it went back in time and reassigned it).
pub static MARK_PERSISTED: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local meta = redis.call('HMGET', KEYS[1], 'generation', 'version', 'persisted')
if not meta[1] or meta[1] ~= ARGV[1] then return 0 end
local version = tonumber(meta[2])
local v = tonumber(ARGV[2])
if v <= tonumber(meta[3]) then return 0 end
if v > version then return -1 end
local id = string.format('%d-0', v)
local entry = redis.call('XRANGE', KEYS[2], id, id)
if #entry == 0 then return -1 end
local fields = entry[1][2]
local command_id = nil
for i = 1, #fields, 2 do
  if fields[i] == 'c' then command_id = fields[i + 1] end
end
if command_id ~= ARGV[3] then return -1 end
redis.call('HSET', KEYS[1], 'persisted', v)
local cut = math.min(v + 1, version - tonumber(ARGV[4]) + 1)
if cut > 1 then
  redis.call('XTRIM', KEYS[2], 'MINID', string.format('%d-0', cut))
  redis.call('ZREMRANGEBYSCORE', KEYS[3], '-inf', string.format('(%d', cut))
end
return 1
"#,
    )
});

/// KEYS: meta, log, ids. ARGV: generation.
pub static RESET: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
if redis.call('HGET', KEYS[1], 'generation') ~= ARGV[1] then return 0 end
redis.call('DEL', KEYS[1], KEYS[2], KEYS[3])
return 1
"#,
    )
});

/// KEYS: lock. ARGV: token.
pub static UNLOCK: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then return redis.call('DEL', KEYS[1]) end
return 0
"#,
    )
});

/// KEYS: active. ARGV: idle_ms, limit. The streams without writes for at least idle_ms, oldest first.
pub static IDLE_STREAMS: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local now = redis.call('TIME')
local now_ms = tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000)
return redis.call('ZRANGEBYSCORE', KEYS[1], '-inf', now_ms - tonumber(ARGV[1]), 'LIMIT', 0, tonumber(ARGV[2]))
"#,
    )
});

/// KEYS: active, meta. ARGV: stream_id. Unlists the stream when nothing is unpersisted (atomic with
/// INGEST, which lists it again on its next write).
pub static RETIRE_ACTIVE: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local meta = redis.call('HMGET', KEYS[2], 'version', 'persisted')
if meta[1] and tonumber(meta[1]) > tonumber(meta[2]) then return 0 end
redis.call('ZREM', KEYS[1], ARGV[1])
return 1
"#,
    )
});

/// KEYS: active. ARGV: stream_id. Moves a listed stream behind the other idle streams.
pub static REQUEUE_ACTIVE: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local now = redis.call('TIME')
return redis.call('ZADD', KEYS[1], 'XX', tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000), ARGV[1])
"#,
    )
});
```

- [ ] **Step 4: Repository**

Create `redis_command_stream_db.rs`:

```rust
use super::scripts::{HYDRATE, IDLE_STREAMS, INGEST, MARK_PERSISTED, REQUEUE_ACTIVE, RESET, RETIRE_ACTIVE, UNLOCK};
use redis::streams::{StreamId, StreamRangeReply};
use shine_infra::db::{
    command_log::StoredCommand,
    redis::{RedisConnectionPool, RedisError, RedisPooledConnection},
};
use std::{sync::Arc, time::Duration};
use tracing::instrument;
use uuid::Uuid;

const HYDRATION_LOCK_MS: u64 = 5000;

#[derive(Clone, Debug)]
pub struct RedisStreamConfig {
    /// Key prefix: `RedisCommandStreamDb::PREFIX` in production, unique per test.
    pub prefix: String,
    pub keep_commands: i64,
    pub max_unpersisted: i64,
    pub ttl_seconds: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RedisStreamMeta {
    pub generation: i64,
    pub version: i64,
    pub persisted: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedisStreamEntry {
    pub version: i64,
    pub generation: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub data: String,
}

impl From<RedisStreamEntry> for StoredCommand {
    fn from(entry: RedisStreamEntry) -> Self {
        Self {
            version: entry.version,
            command_id: entry.command_id,
            user_id: entry.user_id,
            generation: entry.generation,
            data: entry.data,
        }
    }
}

/// Outcome of recording a persisted version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkOutcome {
    Marked,
    /// Another generation, or already recorded.
    Unchanged,
    /// The copy is behind the version or holds another command there: it went back in time.
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedisIngest {
    Accepted(i64),
    /// The command id was already ingested at this version.
    Duplicate(i64),
    NotReady,
    Busy,
}

struct Keys {
    meta: String,
    log: String,
    ids: String,
    lock: String,
}

type MetaReply = (Option<i64>, Option<i64>, Option<i64>);

fn to_meta((generation, version, persisted): MetaReply) -> Option<RedisStreamMeta> {
    Some(RedisStreamMeta {
        generation: generation?,
        version: version.unwrap_or(0),
        persisted: persisted.unwrap_or(0),
    })
}

fn parse_entry(entry: &StreamId) -> Option<RedisStreamEntry> {
    let (version, _) = entry.id.split_once('-')?;
    Some(RedisStreamEntry {
        version: version.parse().ok()?,
        generation: entry.get::<i64>("g")?,
        command_id: Uuid::parse_str(&entry.get::<String>("c")?).ok()?,
        user_id: Uuid::parse_str(&entry.get::<String>("u")?).ok()?,
        data: entry.get::<String>("d")?,
    })
}

struct Inner {
    pool: RedisConnectionPool,
    config: RedisStreamConfig,
    active_key: String,
    wake_channel: String,
}

/// Redis side of the command streams: ordering, dedupe, the recent window and wakes. Cheap to clone.
#[derive(Clone)]
pub struct RedisCommandStreamDb(Arc<Inner>);

impl RedisCommandStreamDb {
    pub const PREFIX: &'static str = "cs";

    pub fn new(pool: &RedisConnectionPool, config: RedisStreamConfig) -> Self {
        let active_key = format!("{}:active", config.prefix);
        let wake_channel = format!("{}:wake", config.prefix);
        Self(Arc::new(Inner {
            pool: pool.clone(),
            config,
            active_key,
            wake_channel,
        }))
    }

    fn keys(&self, stream_id: Uuid) -> Keys {
        let prefix = &self.0.config.prefix;
        Keys {
            meta: format!("{prefix}:{stream_id}:meta"),
            log: format!("{prefix}:{stream_id}:log"),
            ids: format!("{prefix}:{stream_id}:ids"),
            lock: format!("{prefix}:{stream_id}:hydrating"),
        }
    }

    async fn client(&self) -> Result<RedisPooledConnection<'_>, RedisError> {
        self.0.pool.get().await.map_err(RedisError::PoolError)
    }

    /// Appends a command; the hot path, a single script.
    #[instrument(name = "redis cs ingest", level = "debug", skip_all, fields(stream.id = %stream_id))]
    pub async fn ingest(&self, stream_id: Uuid, command_id: Uuid, user_id: Uuid, data: &str) -> Result<RedisIngest, RedisError> {
        let keys = self.keys(stream_id);
        let config = &self.0.config;
        let mut client = self.client().await?;
        let (status, version): (String, i64) = INGEST
            .key(&keys.meta)
            .key(&keys.log)
            .key(&keys.ids)
            .key(&self.0.active_key)
            .arg(stream_id.to_string())
            .arg(command_id.to_string())
            .arg(user_id.to_string())
            .arg(data)
            .arg(config.max_unpersisted)
            .arg(config.ttl_seconds)
            .arg(&self.0.wake_channel)
            .invoke_async(&mut *client)
            .await?;
        Ok(match status.as_str() {
            "ok" => RedisIngest::Accepted(version),
            "duplicate" => RedisIngest::Duplicate(version),
            "busy" => RedisIngest::Busy,
            _ => RedisIngest::NotReady,
        })
    }

    /// Repopulates the stream unless a same or newer generation is already there. Returns whether it wrote.
    #[instrument(name = "redis cs hydrate", level = "debug", skip_all, fields(stream.id = %stream_id))]
    pub async fn hydrate(&self, stream_id: Uuid, generation: i64, head: i64, tail: &[StoredCommand]) -> Result<bool, RedisError> {
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let mut invocation = HYDRATE.prepare_invoke();
        invocation
            .key(&keys.meta)
            .key(&keys.log)
            .key(&keys.ids)
            .key(&self.0.active_key)
            .arg(generation)
            .arg(head)
            .arg(self.0.config.ttl_seconds)
            .arg(stream_id.to_string());
        for command in tail {
            invocation
                .arg(command.version)
                .arg(command.command_id.to_string())
                .arg(command.user_id.to_string())
                .arg(command.generation)
                .arg(&command.data);
        }
        let written: i64 = invocation.invoke_async(&mut *client).await?;
        Ok(written == 1)
    }

    /// Records `version` as persisted when this copy holds `command_id` there, and trims what is both
    /// persisted and outside the keep window.
    #[instrument(name = "redis cs mark persisted", level = "debug", skip_all, fields(stream.id = %stream_id))]
    pub async fn mark_persisted(
        &self,
        stream_id: Uuid,
        generation: i64,
        version: i64,
        command_id: Uuid,
    ) -> Result<MarkOutcome, RedisError> {
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let outcome: i64 = MARK_PERSISTED
            .key(&keys.meta)
            .key(&keys.log)
            .key(&keys.ids)
            .arg(generation)
            .arg(version)
            .arg(command_id.to_string())
            .arg(self.0.config.keep_commands)
            .invoke_async(&mut *client)
            .await?;
        Ok(match outcome {
            1 => MarkOutcome::Marked,
            0 => MarkOutcome::Unchanged,
            _ => MarkOutcome::Refused,
        })
    }

    /// Drops the stream from Redis if it is still at `generation`.
    #[instrument(name = "redis cs reset", level = "debug", skip_all, fields(stream.id = %stream_id))]
    pub async fn reset(&self, stream_id: Uuid, generation: i64) -> Result<bool, RedisError> {
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let deleted: i64 = RESET
            .key(&keys.meta)
            .key(&keys.log)
            .key(&keys.ids)
            .arg(generation)
            .invoke_async(&mut *client)
            .await?;
        Ok(deleted == 1)
    }

    pub async fn meta(&self, stream_id: Uuid) -> Result<Option<RedisStreamMeta>, RedisError> {
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let reply: MetaReply = redis::cmd("HMGET")
            .arg(&keys.meta)
            .arg("generation")
            .arg("version")
            .arg("persisted")
            .query_async(&mut *client)
            .await?;
        Ok(to_meta(reply))
    }

    /// Meta and the entries above `after`, read atomically.
    pub async fn read_after(
        &self,
        stream_id: Uuid,
        after: i64,
        limit: i64,
    ) -> Result<(Option<RedisStreamMeta>, Vec<RedisStreamEntry>), RedisError> {
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let (meta, reply): (MetaReply, StreamRangeReply) = redis::pipe()
            .atomic()
            .cmd("HMGET")
            .arg(&keys.meta)
            .arg("generation")
            .arg("version")
            .arg("persisted")
            .cmd("XRANGE")
            .arg(&keys.log)
            .arg(format!("({after}-0"))
            .arg("+")
            .arg("COUNT")
            .arg(limit)
            .query_async(&mut *client)
            .await?;
        let mut entries = Vec::with_capacity(reply.ids.len());
        for raw in &reply.ids {
            match parse_entry(raw) {
                Some(entry) => entries.push(entry),
                None => tracing::error!("[{stream_id}] Unreadable command stream entry {}", raw.id),
            }
        }
        Ok((to_meta(meta), entries))
    }

    /// Returns a token when the lock was taken.
    pub async fn try_lock_hydration(&self, stream_id: Uuid) -> Result<Option<String>, RedisError> {
        let keys = self.keys(stream_id);
        let token = Uuid::new_v4().to_string();
        let mut client = self.client().await?;
        let set: Option<String> = redis::cmd("SET")
            .arg(&keys.lock)
            .arg(&token)
            .arg("NX")
            .arg("PX")
            .arg(HYDRATION_LOCK_MS)
            .query_async(&mut *client)
            .await?;
        Ok(set.map(|_| token))
    }

    pub async fn unlock_hydration(&self, stream_id: Uuid, token: &str) -> Result<(), RedisError> {
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let _: i64 = UNLOCK.key(&keys.lock).arg(token).invoke_async(&mut *client).await?;
        Ok(())
    }

    /// Streams without writes for at least `idle`, oldest first.
    pub async fn idle_streams(&self, idle: Duration, limit: usize) -> Result<Vec<Uuid>, RedisError> {
        let mut client = self.client().await?;
        let members: Vec<String> = IDLE_STREAMS
            .key(&self.0.active_key)
            .arg(idle.as_millis() as i64)
            .arg(limit)
            .invoke_async(&mut *client)
            .await?;
        Ok(members.iter().filter_map(|m| Uuid::parse_str(m).ok()).collect())
    }

    /// Unlists a stream with nothing unpersisted; `false` when it still has unpersisted commands.
    pub async fn retire_active(&self, stream_id: Uuid) -> Result<bool, RedisError> {
        let keys = self.keys(stream_id);
        let mut client = self.client().await?;
        let removed: i64 = RETIRE_ACTIVE
            .key(&self.0.active_key)
            .key(&keys.meta)
            .arg(stream_id.to_string())
            .invoke_async(&mut *client)
            .await?;
        Ok(removed == 1)
    }

    /// Moves a stream that could not be retired behind the other idle streams, so it cannot starve the sweep.
    pub async fn requeue_active(&self, stream_id: Uuid) -> Result<(), RedisError> {
        let mut client = self.client().await?;
        let _: i64 = REQUEUE_ACTIVE
            .key(&self.0.active_key)
            .arg(stream_id.to_string())
            .invoke_async(&mut *client)
            .await?;
        Ok(())
    }

    /// Warns unless Redis is configured not to evict keys: an evicted `ids` or `log` key breaks
    /// dedupe or loses unpersisted commands. Managed Redis may forbid `CONFIG`; that is only logged.
    pub async fn check_eviction_policy(&self) {
        let reply: Result<Vec<String>, RedisError> = async {
            let mut client = self.client().await?;
            Ok(redis::cmd("CONFIG")
                .arg("GET")
                .arg("maxmemory-policy")
                .query_async(&mut *client)
                .await?)
        }
        .await;
        match reply.as_deref() {
            Ok([_, policy]) if policy == "noeviction" => {}
            Ok([_, policy]) => tracing::warn!("Redis maxmemory-policy is {policy}; command streams need noeviction"),
            Ok(_) => tracing::warn!("Redis maxmemory-policy is unknown; command streams need noeviction"),
            Err(err) => tracing::debug!("Could not read the Redis maxmemory-policy: {err:#?}"),
        }
    }

    /// Calls `handler` on every ingest with the stream id; `None` after a listener reconnect (wake all).
    /// The handler runs under the listener's lock: it must not block or await.
    pub async fn listen_to_wakes<F>(&self, handler: F) -> Result<(), RedisError>
    where
        F: Fn(Option<Uuid>) + Send + Sync + 'static,
    {
        let client = self.client().await?;
        client
            .listen(&self.0.wake_channel, move |payload| {
                handler(payload.and_then(|p| Uuid::parse_str(p).ok()))
            })
            .await
    }
}
```

`Script::key()` and `arg()` return a `ScriptInvocation` that chains on a temporary inside one expression. `invoke_async` sends `EVALSHA` and falls back on `NOSCRIPT`.

- [ ] **Step 5: Run tests**

Run: `$env:SHINE_TEST_REDIS_CNS="redis://redis.mockbox.foo:6379"; cargo test -p shine-builder redis_command_stream -- --nocapture`

Expected: 10 tests PASS, with no "skipping" line.

- [ ] **Step 6: Checkpoint.** Report and wait.

---

## Task 5: Hub — stream subscriptions and stream messages

Re-read these first; they carry uncommitted user changes:
- `models/messages/hub.rs`
- `services/hub/{hub_command.rs, hub_connections.rs, hub_service.rs, mod.rs, test_utils.rs}`
- `services/builder/HUB.md`

**Files:**
- Create: `services/builder/src/models/messages/stream.rs`, `services/builder/src/services/hub/hub_stream_test.rs`
- Modify: `models/messages/{mod.rs, hub.rs}`, `services/hub/{hub_command.rs, hub_connections.rs, hub_service.rs, mod.rs, test_utils.rs}`

**Interfaces — Produces (used by Tasks 6–9):**
```rust
models::messages::{
    StreamCommand { version: i64, command_id: Uuid, user_id: Uuid, data: Arc<str> },
    StreamSnapshot { version: i64, hash: u64, data: Arc<[u8]> },
    StreamMessage::{Sync{stream_id, generation, snapshot: Option<StreamSnapshot>, commands: Arc<[StreamCommand]>},
                    Commands{stream_id, generation, commands: Arc<[StreamCommand]>}, Checkpoint{stream_id, version, hash: u64},
                    Reset{stream_id}, Nack{stream_id, command_id, reason: &'static str},
                    SubscribeFailed{stream_id, reason: &'static str}},
    StreamEvent::{Subscribed{connection_id, stream_id, kind: &'static str}, Unsubscribed{connection_id, stream_id}},
    HubMessage::Stream(StreamMessage), HubMessage::StreamEvent(StreamEvent), TopicKey::Stream,
}
HubService::request_subscribe_stream(&self, user_id: Uuid, connection_id: Uuid, stream_id: Uuid, kind: &'static str) -> Result<(), HubError>
HubService::request_unsubscribe_stream(&self, user_id: Uuid, connection_id: Uuid, stream_id: Uuid) -> Result<(), HubError>
HubService::request_reply(&self, connection_id: Uuid, message: HubMessage) -> Result<(), HubError>
Connections::contains(&self, connection_id: Uuid) -> bool   // async
```

A socket's own replies (nacks, subscribe failures) also go through the command loop (`request_reply`). They are queued behind the socket's `ConnectUser`, so a reply to a frame the client sent right after connecting is not lost while the connection is still being registered. A direct `send_to_connection` at that moment finds no connection and drops the message.

Stream subscriptions are hub requests. The command loop only checks that the connection is still registered, and publishes a `StreamEvent` on `TopicKey::Stream`. No socket subscribes to that topic, so only internal subscribers (the dispatcher) receive it. The hub keeps no stream set per connection: the dispatcher tracks subscriptions itself, and drops them on `UserDisconnected`.

- [ ] **Step 1: Write the failing test**

Create `services/hub/hub_stream_test.rs`:

```rust
use super::test_utils::{barrier, connect, create_test_hub, TestHubOptions};
use crate::models::messages::{HubMessage, StreamEvent, StreamMessage, TopicKey};
use shine_test::test;
use tokio::sync::mpsc::{error::TryRecvError, UnboundedReceiver};
use uuid::Uuid;

fn stream_events(rx: &mut UnboundedReceiver<HubMessage>) -> Vec<StreamEvent> {
    let mut events = vec![];
    loop {
        match rx.try_recv() {
            Ok(HubMessage::StreamEvent(event)) => events.push(event),
            Ok(_) => {}
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return events,
        }
    }
}

#[test(serial = "hub-service")]
async fn stream_subscriptions_are_published_for_registered_connections_only() {
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let mut streams = test.hub.subscribe(vec![TopicKey::Stream]).await;
    let user_id = Uuid::new_v4();
    let (connection_id, _egress) = connect(&test.hub, user_id, 16);
    let stream_id = Uuid::new_v4();

    // Unknown connection, then the live one, then after it disconnected: only the live one counts.
    test.hub
        .request_subscribe_stream(Uuid::new_v4(), Uuid::new_v4(), stream_id, "demo")
        .unwrap();
    test.hub
        .request_subscribe_stream(user_id, connection_id, stream_id, "demo")
        .unwrap();
    test.hub
        .request_unsubscribe_stream(user_id, connection_id, stream_id)
        .unwrap();
    test.hub.request_disconnection(user_id, connection_id).unwrap();
    test.hub
        .request_subscribe_stream(user_id, connection_id, stream_id, "demo")
        .unwrap();
    barrier(&test.hub, &mut test.events).await;

    let events = stream_events(&mut streams);
    assert!(
        matches!(
            events.as_slice(),
            [
                StreamEvent::Subscribed { connection_id: a, stream_id: s, kind: "demo" },
                StreamEvent::Unsubscribed { connection_id: b, .. },
            ] if *a == connection_id && *b == connection_id && *s == stream_id
        ),
        "unexpected stream events: {events:?}"
    );
}

#[test(serial = "hub-service")]
async fn a_reply_requested_right_after_connecting_is_delivered() {
    let Some(mut test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let (connection_id, mut egress) = connect(&test.hub, Uuid::new_v4(), 16);
    // Requested before the command loop registered the connection.
    test.hub
        .request_reply(connection_id, HubMessage::Stream(StreamMessage::Reset { stream_id: Uuid::nil() }))
        .unwrap();
    barrier(&test.hub, &mut test.events).await;

    let mut replies = vec![];
    while let Ok(message) = egress.try_recv() {
        if let HubMessage::Stream(reply) = message {
            replies.push(reply);
        }
    }
    assert_eq!(replies, vec![StreamMessage::Reset { stream_id: Uuid::nil() }]);
}
```

In `services/hub/mod.rs`, add `#[cfg(test)] mod hub_stream_test;` next to the other test modules.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p shine-builder services::hub::hub_stream_test`. Expected: compile errors.

- [ ] **Step 3: Message types**

Create `models/messages/stream.rs`:

```rust
use crate::models::messages::{ToTopic, TopicKey};
use std::sync::Arc;
use uuid::Uuid;

/// A command at its version; `data` is the canonical command JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamCommand {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub data: Arc<str>,
}

/// An authoritative snapshot (postcard bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamSnapshot {
    pub version: i64,
    pub hash: u64,
    pub data: Arc<[u8]>,
}

/// Egress to one connection about one stream. Sent directly, never broadcast; the command lists are
/// shared by every subscriber of a step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamMessage {
    /// Authoritative snapshot (if any) and the first commands after it; more follow as `Commands`.
    Sync {
        stream_id: Uuid,
        generation: i64,
        snapshot: Option<StreamSnapshot>,
        commands: Arc<[StreamCommand]>,
    },
    Commands {
        stream_id: Uuid,
        generation: i64,
        commands: Arc<[StreamCommand]>,
    },
    Checkpoint {
        stream_id: Uuid,
        version: i64,
        hash: u64,
    },
    /// The client's view was rolled back; a fresh `Sync` follows.
    Reset { stream_id: Uuid },
    Nack {
        stream_id: Uuid,
        command_id: Uuid,
        reason: &'static str,
    },
    SubscribeFailed { stream_id: Uuid, reason: &'static str },
}

/// Subscription changes, published by the hub's command loop to internal subscribers only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamEvent {
    Subscribed {
        connection_id: Uuid,
        stream_id: Uuid,
        kind: &'static str,
    },
    Unsubscribed {
        connection_id: Uuid,
        stream_id: Uuid,
    },
}

impl ToTopic for StreamEvent {
    fn topic(&self) -> TopicKey {
        TopicKey::Stream
    }
}
```

In `models/messages/mod.rs`, add `mod stream;` and `pub use self::stream::{StreamCommand, StreamEvent, StreamMessage, StreamSnapshot};`.

In `models/messages/hub.rs`:
- Add `Stream` to `TopicKey`, with the doc `/// Command stream subscription events; internal subscribers only.`
- Add these variants to `HubMessage`:

  ```rust
      /// Command stream egress to one connection.
      Stream(StreamMessage),
      /// Command stream subscription change, for internal subscribers.
      StreamEvent(StreamEvent),
  ```

- Add these arms to its `ToTopic` impl: `HubMessage::Stream(_) => TopicKey::Stream,` and `HubMessage::StreamEvent(event) => event.topic(),`.
- Import `StreamEvent, StreamMessage` from `crate::models::messages`.

Exhaustive matches on `HubMessage` that need the new arms:
- `hub_connections.rs` `try_send` (metric kind): `HubMessage::Stream(_) => "stream",` and `HubMessage::StreamEvent(_) => "stream_event",`.
- `test_utils.rs` `EgressState::record_for`: `HubMessage::Stream(_) | HubMessage::StreamEvent(_) => {}`.
- `routes/ws/connect.rs` `event_to_wire_message`: `HubMessage::Stream(_) | HubMessage::StreamEvent(_) => None,` (Task 9 replaces the `Stream` part).

Fix any other match the compiler reports the same way.

- [ ] **Step 4: Registration check**

In `hub_connections.rs`, add to `impl Connections`:

```rust
    /// Whether the connection is registered.
    pub async fn contains(&self, connection_id: Uuid) -> bool {
        self.registry.read().await.connections.contains_key(&connection_id)
    }
```

- [ ] **Step 5: Control commands**

In `hub_command.rs`, add these variants:

```rust
    /// A message to one connection, queued behind the commands requested before it.
    Reply {
        connection_id: Uuid,
        message: HubMessage,
    },
    SubscribeStream {
        user_id: Uuid,
        connection_id: Uuid,
        stream_id: Uuid,
        kind: &'static str,
    },
    UnsubscribeStream {
        user_id: Uuid,
        connection_id: Uuid,
        stream_id: Uuid,
    },
```

and extend the three helper matches:
- names: `"reply"`, `"subscribe_stream"` and `"unsubscribe_stream"`;
- `Reply` carries a `connection_id` but no `user_id`;
- the two stream variants carry both.

`HubMessage` already derives `Debug`, so the `#[derive(Debug)]` of `ControlCommand` still compiles.

In `hub_service.rs`, import `StreamEvent` from `crate::models::messages`, then add the request methods next to `request_disconnection`:

```rust
    /// Sends `message` to a connection through the command loop: behind the connection's registration,
    /// so a reply to a frame sent right after connecting is not lost.
    pub fn request_reply(&self, connection_id: Uuid, message: HubMessage) -> Result<(), HubError> {
        self.send_control(ControlCommand::Reply { connection_id, message })
    }

    /// Requests a stream subscription for a connection; ignored if the connection is gone by then.
    pub fn request_subscribe_stream(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
        stream_id: Uuid,
        kind: &'static str,
    ) -> Result<(), HubError> {
        self.send_control(ControlCommand::SubscribeStream {
            user_id,
            connection_id,
            stream_id,
            kind,
        })
    }

    pub fn request_unsubscribe_stream(&self, user_id: Uuid, connection_id: Uuid, stream_id: Uuid) -> Result<(), HubError> {
        self.send_control(ControlCommand::UnsubscribeStream {
            user_id,
            connection_id,
            stream_id,
        })
    }
```

Then add these arms in `process`, before `Shutdown`. A failed `Reply` is handled like a failed `send_to_connection`: the dead connection is pruned through its own queued `DropConnection`, using the same helper `send_to_connection` uses (`drop_dead` at the time of writing).

```rust
            ControlCommand::Reply { connection_id, message } => {
                if let Err(dead) = self.inner.connections.send_to_connection(connection_id, message).await {
                    if let Err(err) = self.drop_dead([dead]) {
                        tracing::debug!("[{connection_id}] Failed to queue the drop of a dead connection: {err:#?}");
                    }
                }
                true
            }
            ControlCommand::SubscribeStream {
                user_id,
                connection_id,
                stream_id,
                kind,
            } => {
                // A subscribe racing a replaced or dropped connection must not pin a stream worker.
                if self.inner.connections.contains(connection_id).await {
                    self.publish_stream_event(StreamEvent::Subscribed {
                        connection_id,
                        stream_id,
                        kind,
                    })
                    .await;
                } else {
                    tracing::debug!("[{user_id}] [{connection_id}] Stream subscribe for an unknown connection ignored");
                }
                true
            }
            ControlCommand::UnsubscribeStream {
                connection_id,
                stream_id,
                ..
            } => {
                if self.inner.connections.contains(connection_id).await {
                    self.publish_stream_event(StreamEvent::Unsubscribed {
                        connection_id,
                        stream_id,
                    })
                    .await;
                }
                true
            }
```

and the helper:

```rust
    /// Delivers a stream subscription change to the internal subscribers; no socket subscribes to its topic.
    async fn publish_stream_event(&self, event: StreamEvent) {
        self.inner.subscribers.publish(HubMessage::StreamEvent(event)).await;
    }
```

A disconnect needs no stream event: the dispatcher drops the connection's streams on `UserDisconnected`.

- [ ] **Step 6: Run tests**

Run: `$env:SHINE_TEST_REDIS_CNS="redis://redis.mockbox.foo:6379"; cargo test -p shine-builder services::hub -- --nocapture`

Expected: the 2 new tests and all existing hub tests PASS.

- [ ] **Step 7: Checkpoint.** Report and wait.

---

## Task 6: `CommandStreamService`, kinds, ports, and the pure `StreamCore`

**Files:**
- Create: `services/builder/src/services/command_stream/{mod.rs, command_stream_metrics.rs, stream_access.rs, stream_kind.rs, stream_egress.rs, command_stream_service.rs, stream_core.rs, stream_core_test.rs}`
- Modify: `services/builder/src/services/mod.rs`, `services/builder/src/app_state.rs`

**Interfaces:**
- Consumes:
  - Task 1: `ReplayState`, `Replica`, `CheckpointPolicy`, `CommandEnvelope`, `encode_snapshot`, `IntArray`, `DemoCommand`
  - Task 2: `PgCommandLogDb`, `AppendOutcome`, `SnapshotOutcome`, `StoredCommand`
  - Task 3: `CommandStreamSettings`, `CommandStreamError`
  - Task 4: `RedisCommandStreamDb`, `RedisStreamConfig`, `RedisStreamMeta`, `RedisIngest`
  - Task 5: `StreamCommand`, `StreamSnapshot`, `StreamMessage`
- Produces (used by Tasks 7–9):
```rust
services::command_stream::{
    CommandStreamService, StreamStores { redis, log, settings },
    OpenedStream { kind: Arc<dyn StreamKind>, can_write: bool },
    OpenError::{UnknownKind, KindMismatch, Forbidden, Failed(CommandStreamError)}, OpenError::reason(&self) -> &'static str,
    IngestOutcome::{Accepted { version }, Duplicate { version }, Invalid, NotReady, Busy}, IngestOutcome::reason(&self) -> &'static str,
    StreamAccess, AllowAll, StreamKind, ReplayKind<S>, StreamKindRegistry, StreamEgress,
    StreamCore<S>, Fault::{Reload(&'static str), Reset { generation, reason }}, Observed::{Current, NeedsProof(i64)}, Captured,
    // StreamCore::observe(&mut self, meta: &RedisStreamMeta, tip: Option<&RedisStreamEntry>) -> Result<Observed, Fault>
    // StreamCore::mark_due(&self) -> Option<(i64, Uuid)>;  StreamCore::needs_tip / set_tip(version, command_id)
    command_stream_metrics(), FRAME_COMMANDS,
}
impl CommandStreamService {
    pub fn new(stores: StreamStores, kinds: StreamKindRegistry, access: Arc<dyn StreamAccess>) -> Self;
    pub fn stores(&self) -> StreamStores;  pub fn kinds(&self) -> &StreamKindRegistry;  pub fn settings(&self) -> &CommandStreamSettings;
    pub async fn open(&self, user_id: Uuid, stream_id: Uuid, kind: &str) -> Result<OpenedStream, OpenError>;
    pub async fn ensure_hydrated(&self, stream_id: Uuid) -> Result<(), CommandStreamError>;
    pub async fn ingest(&self, kind: &dyn StreamKind, stream_id: Uuid, command_id: Uuid, user_id: Uuid, command: &str) -> Result<IngestOutcome, CommandStreamError>;
    pub async fn sweep_orphans(&self);
    pub async fn listen_to_wakes<F: Fn(Option<Uuid>) + Send + Sync + 'static>(&self, handler: F) -> Result<(), CommandStreamError>;
}
```

The command-stream service does not depend on the hub. The worker (Task 7) sends through the `StreamEgress` port, and the hub implements that port. Command-stream metrics have their own meter.

- [ ] **Step 1: Module skeleton, metrics, access, egress port**

`command_stream/mod.rs`:

```rust
mod command_stream_metrics;
mod command_stream_service;
mod stream_access;
mod stream_core;
mod stream_egress;
mod stream_kind;

#[cfg(test)]
mod stream_core_test;

pub use self::{
    command_stream_metrics::command_stream_metrics,
    command_stream_service::{CommandStreamService, IngestOutcome, OpenError, OpenedStream, StreamStores},
    stream_access::{AllowAll, StreamAccess},
    stream_core::{Captured, Fault, Observed, StreamCore, FRAME_COMMANDS},
    stream_egress::StreamEgress,
    stream_kind::{ReplayKind, StreamKind, StreamKindRegistry},
};
```

In `services/mod.rs`, add `pub mod command_stream;`, and re-export `command_stream::CommandStreamService` next to `ChatService`.

`command_stream_metrics.rs`:

```rust
use opentelemetry::{global, metrics::Counter};
use std::sync::LazyLock;

/// Metrics of the command streams. Labels have a small fixed set of values; stream ids are only in the traces.
pub struct CommandStreamMetrics {
    /// Commands received from the clients, by outcome.
    pub ingested: Counter<u64>,
    /// Snapshots that disagreed with the authoritative one: a determinism bug.
    pub drift: Counter<u64>,
}

static METRICS: LazyLock<CommandStreamMetrics> = LazyLock::new(CommandStreamMetrics::new);

/// The metrics of the command streams. Without a configured meter provider the instruments are no-ops.
pub fn command_stream_metrics() -> &'static CommandStreamMetrics {
    &METRICS
}

impl CommandStreamMetrics {
    fn new() -> Self {
        let meter = global::meter("shine-builder-command-stream");
        Self {
            ingested: meter
                .u64_counter("command_stream_ingested")
                .with_description("Commands received from the clients")
                .build(),
            drift: meter
                .u64_counter("command_stream_drift")
                .with_description("Snapshots that disagreed with the authoritative one")
                .build(),
        }
    }
}
```

`stream_access.rs`:

```rust
use uuid::Uuid;

/// Per-stream permission check, called on subscribe.
pub trait StreamAccess: Send + Sync + 'static {
    fn can_read(&self, user_id: Uuid, stream_id: Uuid) -> bool;
    fn can_write(&self, user_id: Uuid, stream_id: Uuid) -> bool;
}

/// Placeholder until stream ACLs exist: every logged-in user may read and write.
pub struct AllowAll;

impl StreamAccess for AllowAll {
    fn can_read(&self, _user_id: Uuid, _stream_id: Uuid) -> bool {
        true
    }

    fn can_write(&self, _user_id: Uuid, _stream_id: Uuid) -> bool {
        true
    }
}
```

`stream_egress.rs`:

```rust
use crate::models::messages::StreamMessage;
use futures::future::BoxFuture;
use uuid::Uuid;

/// Delivery of stream messages to one connection; the hub implements it.
pub trait StreamEgress: Send + Sync + 'static {
    /// Queues `message`; `false` when the connection is gone or too slow. A connection that returned
    /// `false` must get nothing more, or it could see a version gap.
    fn send(&self, connection_id: Uuid, message: StreamMessage) -> BoxFuture<'_, bool>;
}
```

- [ ] **Step 2: Kinds**

`stream_kind.rs`. Task 7 adds `create_worker` to the trait:

```rust
use shine_game::replay::ReplayState;
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

/// Type-erased stream kind, so the hub and the routes stay non-generic.
pub trait StreamKind: Send + Sync + 'static {
    fn kind(&self) -> &'static str;
    /// The canonical JSON of `command` when it is a well-formed command of this kind. Storing the
    /// re-serialized typed command drops unknown fields and keeps replay byte-for-byte deterministic.
    fn canonicalize(&self, command: &str) -> Option<String>;
}

pub struct ReplayKind<S>(PhantomData<fn() -> S>);

impl<S: ReplayState> StreamKind for ReplayKind<S> {
    fn kind(&self) -> &'static str {
        S::KIND
    }

    fn canonicalize(&self, command: &str) -> Option<String> {
        let command: S::Command = serde_json::from_str(command).ok()?;
        if !S::validate(&command) {
            return None;
        }
        serde_json::to_string(&command).ok()
    }
}

#[derive(Clone, Default)]
pub struct StreamKindRegistry(HashMap<&'static str, Arc<dyn StreamKind>>);

impl StreamKindRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with<S: ReplayState>(mut self) -> Self {
        self.0.insert(S::KIND, Arc::new(ReplayKind::<S>(PhantomData)));
        self
    }

    pub fn get(&self, kind: &str) -> Option<Arc<dyn StreamKind>> {
        self.0.get(kind).cloned()
    }
}
```

- [ ] **Step 3: Service**

`command_stream_service.rs`:

```rust
use crate::{
    models::CommandStreamError,
    repositories::command_stream::redis::{MarkOutcome, RedisCommandStreamDb, RedisIngest, RedisStreamMeta},
    services::command_stream::{command_stream_metrics, StreamAccess, StreamKind, StreamKindRegistry},
    settings::CommandStreamSettings,
};
use shine_infra::{
    db::command_log::{AppendOutcome, CommandLogError, PgCommandLogDb, StoredCommand, StreamHead},
    telemetry::label,
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

const HYDRATE_ATTEMPTS: usize = 100;
const HYDRATE_RETRY: Duration = Duration::from_millis(50);
const SWEEP_LIMIT: usize = 100;

/// The stores of the command streams. Cheap to clone.
#[derive(Clone)]
pub struct StreamStores {
    pub redis: RedisCommandStreamDb,
    pub log: PgCommandLogDb,
    pub settings: CommandStreamSettings,
}

pub struct OpenedStream {
    pub kind: Arc<dyn StreamKind>,
    pub can_write: bool,
}

#[derive(Debug)]
pub enum OpenError {
    UnknownKind,
    KindMismatch,
    Forbidden,
    Failed(CommandStreamError),
}

impl OpenError {
    pub fn reason(&self) -> &'static str {
        match self {
            OpenError::UnknownKind => "unknownKind",
            OpenError::KindMismatch => "kindMismatch",
            OpenError::Forbidden => "forbidden",
            OpenError::Failed(_) => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IngestOutcome {
    Accepted { version: i64 },
    /// Already in the log: a retry of a command that landed.
    Duplicate { version: i64 },
    Invalid,
    NotReady,
    Busy,
}

impl IngestOutcome {
    pub fn reason(&self) -> &'static str {
        match self {
            IngestOutcome::Accepted { .. } => "accepted",
            IngestOutcome::Duplicate { .. } => "duplicate",
            IngestOutcome::Invalid => "invalid",
            IngestOutcome::NotReady => "notReady",
            IngestOutcome::Busy => "busy",
        }
    }
}

/// A Redis copy is usable when it is not behind Postgres. A lower generation can never be persisted.
/// A lower version in the same generation means Redis went back in time (restart from a dump,
/// failover) and would reassign persisted versions. A higher generation is a hydration racing this
/// check (Postgres was bumped first); if Postgres was instead restored from a backup, the first
/// append returns `StaleGeneration` and the worker resets the copy.
fn is_fresh(meta: &RedisStreamMeta, head: &StreamHead) -> bool {
    meta.generation > head.generation || (meta.generation == head.generation && meta.version >= head.version)
}

struct Inner {
    stores: StreamStores,
    kinds: StreamKindRegistry,
    access: Arc<dyn StreamAccess>,
    /// Streams with a background hydration in flight on this instance.
    hydrating: Mutex<HashSet<Uuid>>,
}

/// Command stream entry points: open (subscribe path), ingest (hot path), hydration and the orphan sweep.
#[derive(Clone)]
pub struct CommandStreamService(Arc<Inner>);

/// Ends a background hydration when dropped.
struct HydrationGuard {
    service: CommandStreamService,
    stream_id: Uuid,
}

impl Drop for HydrationGuard {
    fn drop(&mut self) {
        self.service
            .0
            .hydrating
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.stream_id);
    }
}

impl CommandStreamService {
    pub fn new(stores: StreamStores, kinds: StreamKindRegistry, access: Arc<dyn StreamAccess>) -> Self {
        Self(Arc::new(Inner {
            stores,
            kinds,
            access,
            hydrating: Mutex::new(HashSet::new()),
        }))
    }

    pub fn stores(&self) -> StreamStores {
        self.0.stores.clone()
    }

    pub fn kinds(&self) -> &StreamKindRegistry {
        &self.0.kinds
    }

    pub fn settings(&self) -> &CommandStreamSettings {
        &self.0.stores.settings
    }

    /// Creates the stream if needed and makes sure Redis holds a usable copy.
    pub async fn open(&self, user_id: Uuid, stream_id: Uuid, kind: &str) -> Result<OpenedStream, OpenError> {
        let stream_kind = self.0.kinds.get(kind).ok_or(OpenError::UnknownKind)?;
        if !self.0.access.can_read(user_id, stream_id) {
            return Err(OpenError::Forbidden);
        }
        match self.0.stores.log.ensure_stream(stream_id, stream_kind.kind()).await {
            Ok(_) => {}
            Err(CommandLogError::KindMismatch { .. }) => return Err(OpenError::KindMismatch),
            Err(err) => return Err(OpenError::Failed(err.into())),
        }
        self.ensure_hydrated(stream_id).await.map_err(OpenError::Failed)?;
        Ok(OpenedStream {
            kind: stream_kind,
            can_write: self.0.access.can_write(user_id, stream_id),
        })
    }

    /// Makes sure Redis holds a copy of the stream that is not behind Postgres: drops a stale copy,
    /// and repopulates a missing one from Postgres.
    pub async fn ensure_hydrated(&self, stream_id: Uuid) -> Result<(), CommandStreamError> {
        let stores = &self.0.stores;
        for _ in 0..HYDRATE_ATTEMPTS {
            // Postgres first: its head only grows and never passes a healthy Redis copy, so a meta
            // read afterwards cannot look stale when it is not.
            let head = stores.log.head(stream_id).await?.ok_or(CommandStreamError::StreamNotFound)?;
            match stores.redis.meta(stream_id).await? {
                Some(meta) if is_fresh(&meta, &head) => return Ok(()),
                Some(meta) => {
                    tracing::warn!(
                        "[{stream_id}] Redis copy (generation {}, version {}) is behind Postgres (generation {}, version {}), dropping it",
                        meta.generation,
                        meta.version,
                        head.generation,
                        head.version
                    );
                    stores.redis.reset(stream_id, meta.generation).await?;
                }
                None => match stores.redis.try_lock_hydration(stream_id).await? {
                    Some(token) => {
                        let result = self.hydrate_locked(stream_id).await;
                        if let Err(err) = stores.redis.unlock_hydration(stream_id, &token).await {
                            tracing::warn!("[{stream_id}] Failed to release the hydration lock: {err:#?}");
                        }
                        result?;
                    }
                    None => tokio::time::sleep(HYDRATE_RETRY).await,
                },
            }
        }
        Err(CommandStreamError::NotReady)
    }

    async fn hydrate_locked(&self, stream_id: Uuid) -> Result<(), CommandStreamError> {
        let stores = &self.0.stores;
        if stores.redis.meta(stream_id).await?.is_some() {
            return Ok(());
        }
        // The new generation fences off appends from workers still reading the old Redis copy.
        let head = stores.log.begin_generation(stream_id).await?;
        let tail = stores.log.last_commands(stream_id, stores.settings.hydrate_last_n).await?;
        stores.redis.hydrate(stream_id, head.generation, head.version, &tail).await?;
        tracing::info!(
            "[{stream_id}] Stream hydrated into Redis at version {} (generation {})",
            head.version,
            head.generation
        );
        Ok(())
    }

    /// Canonicalizes and stores a command in Redis; no replay or persistence here.
    pub async fn ingest(
        &self,
        kind: &dyn StreamKind,
        stream_id: Uuid,
        command_id: Uuid,
        user_id: Uuid,
        command: &str,
    ) -> Result<IngestOutcome, CommandStreamError> {
        let canonical = if command.len() <= self.0.stores.settings.max_command_bytes {
            kind.canonicalize(command)
        } else {
            None
        };
        let outcome = match canonical {
            None => IngestOutcome::Invalid,
            Some(data) => match self.0.stores.redis.ingest(stream_id, command_id, user_id, &data).await? {
                RedisIngest::Accepted(version) => IngestOutcome::Accepted { version },
                RedisIngest::Duplicate(version) => IngestOutcome::Duplicate { version },
                RedisIngest::Busy => IngestOutcome::Busy,
                RedisIngest::NotReady => {
                    self.hydrate_in_background(stream_id);
                    IngestOutcome::NotReady
                }
            },
        };
        command_stream_metrics()
            .ingested
            .add(1, &[label("outcome", outcome.reason())]);
        Ok(outcome)
    }

    /// Starts a hydration unless one is already in flight on this instance: a client retrying while
    /// Redis is down must not start a poller per write.
    fn hydrate_in_background(&self, stream_id: Uuid) {
        if !self
            .0
            .hydrating
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(stream_id)
        {
            return;
        }
        let guard = HydrationGuard {
            service: self.clone(),
            stream_id,
        };
        tokio::spawn(async move {
            if let Err(err) = guard.service.ensure_hydrated(stream_id).await {
                tracing::warn!("[{stream_id}] Failed to hydrate the stream after a refused write: {err:#?}");
            }
            drop(guard);
        });
    }

    /// Persists the Redis tail of idle streams. Resident streams are not skipped: they would stay at
    /// the front of the idle list and hide real orphans. The work is idempotent with their workers.
    pub async fn sweep_orphans(&self) {
        let redis = &self.0.stores.redis;
        let idle = match redis.idle_streams(self.0.stores.settings.orphan_idle, SWEEP_LIMIT).await {
            Ok(idle) => idle,
            Err(err) => {
                tracing::warn!("Failed to list idle command streams: {err:#?}");
                return;
            }
        };
        for stream_id in idle {
            if let Err(err) = self.persist_tail(stream_id).await {
                tracing::warn!("[{stream_id}] Orphan sweep failed: {err:#?}");
            }
            let retired = match redis.retire_active(stream_id).await {
                Ok(retired) => retired,
                Err(err) => {
                    tracing::warn!("[{stream_id}] Failed to retire an idle stream: {err:#?}");
                    continue;
                }
            };
            // Not retired: move it back so it does not hide the other idle streams.
            if !retired {
                if let Err(err) = redis.requeue_active(stream_id).await {
                    tracing::warn!("[{stream_id}] Failed to requeue an idle stream: {err:#?}");
                }
            }
        }
    }

    /// Appends the unpersisted Redis commands to Postgres.
    async fn persist_tail(&self, stream_id: Uuid) -> Result<(), CommandStreamError> {
        let stores = &self.0.stores;
        let Some(meta) = stores.redis.meta(stream_id).await? else {
            return Ok(());
        };
        if meta.version <= meta.persisted {
            return Ok(());
        }
        let (_, entries) = stores
            .redis
            .read_after(stream_id, meta.persisted, stores.settings.redis_max_unpersisted)
            .await?;
        let commands: Vec<StoredCommand> = entries.into_iter().map(StoredCommand::from).collect();
        match stores.log.append(stream_id, &commands).await? {
            AppendOutcome::Appended { version } | AppendOutcome::UpToDate { version } => {
                // Recorded by content: the copy must hold the persisted command at the marked version.
                if let Some(head) = commands.iter().rev().find(|c| c.version <= version) {
                    let marked = stores
                        .redis
                        .mark_persisted(stream_id, meta.generation, head.version, head.command_id)
                        .await?;
                    if marked == MarkOutcome::Refused {
                        tracing::warn!("[{stream_id}] Orphan sweep dropping a Redis copy that disagrees with Postgres");
                        stores.redis.reset(stream_id, meta.generation).await?;
                        return Ok(());
                    }
                }
                tracing::info!("[{stream_id}] Orphan sweep persisted up to version {version}");
            }
            // This Redis copy cannot be persisted or disagrees with Postgres: Postgres wins.
            outcome @ (AppendOutcome::StaleGeneration { .. } | AppendOutcome::Diverged { .. } | AppendOutcome::Gap { .. }) => {
                tracing::warn!("[{stream_id}] Orphan sweep dropping the Redis copy: {outcome:?}");
                stores.redis.reset(stream_id, meta.generation).await?;
            }
        }
        Ok(())
    }

    pub async fn listen_to_wakes<F>(&self, handler: F) -> Result<(), CommandStreamError>
    where
        F: Fn(Option<Uuid>) + Send + Sync + 'static,
    {
        Ok(self.0.stores.redis.listen_to_wakes(handler).await?)
    }
}
```

- [ ] **Step 4: Write the failing core tests**

`stream_core_test.rs`. These are pure: no Redis, Postgres or hub.

```rust
use super::{Fault, Observed, StreamCore, FRAME_COMMANDS};
use crate::{
    models::messages::{StreamMessage, StreamSnapshot},
    repositories::command_stream::redis::{RedisStreamEntry, RedisStreamMeta},
};
use shine_game::replay::{encode_snapshot, stable_hash_of, CheckpointPolicy, DemoCommand, IntArray};
use shine_infra::db::command_log::{AppendOutcome, SnapshotOutcome, StoredCommand, StreamHead};
use shine_test::test;
use uuid::Uuid;

fn cmd(version: i64, command: DemoCommand) -> StoredCommand {
    StoredCommand {
        version,
        command_id: Uuid::from_u128(version as u128),
        user_id: Uuid::nil(),
        generation: 1,
        data: serde_json::to_string(&command).unwrap(),
    }
}

fn add(delta: i64) -> DemoCommand {
    DemoCommand::Add { idx: 0, delta }
}

fn meta(generation: i64, version: i64, persisted: i64) -> RedisStreamMeta {
    RedisStreamMeta {
        generation,
        version,
        persisted,
    }
}

fn entry(version: i64, command_id: Uuid) -> RedisStreamEntry {
    RedisStreamEntry {
        version,
        generation: 1,
        command_id,
        user_id: Uuid::nil(),
        data: "{}".into(),
    }
}

/// A loaded core at generation 1 with an empty log.
fn core(every: i64) -> StreamCore<IntArray> {
    let mut core = StreamCore::new(Uuid::nil(), CheckpointPolicy::new(every));
    core.begin_load(None);
    core.finish_load(&meta(1, 0, 0));
    core
}

fn apply_redis(core: &mut StreamCore<IntArray>, versions: std::ops::RangeInclusive<i64>) {
    for version in versions {
        core.apply(cmd(version, add(1)), false).unwrap();
    }
}

/// Versions delivered to `to`, in frame order.
fn delivered(frames: &[(Uuid, StreamMessage)], to: Uuid) -> Vec<i64> {
    frames
        .iter()
        .filter(|(id, _)| *id == to)
        .flat_map(|(_, frame)| match frame {
            StreamMessage::Sync { commands, .. } | StreamMessage::Commands { commands, .. } => {
                commands.iter().map(|c| c.version).collect()
            }
            _ => vec![],
        })
        .collect()
}

#[test]
fn every_version_is_delivered_once_per_sync_epoch() {
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let mut core = core(100);
    core.add_subscriber(a);
    apply_redis(&mut core, 1..=3);
    let frames = core.take_frames();
    assert!(matches!(frames[0].1, StreamMessage::Sync { .. }));
    assert_eq!(delivered(&frames, a), vec![1, 2, 3]);

    apply_redis(&mut core, 4..=5);
    core.add_subscriber(b);
    let frames = core.take_frames();
    assert_eq!(delivered(&frames, a), vec![4, 5]);
    assert_eq!(delivered(&frames, b), vec![1, 2, 3, 4, 5]);
    assert!(core.take_frames().is_empty(), "nothing new, nothing sent");
}

#[test]
fn large_backlogs_are_split_into_frames() {
    let a = Uuid::new_v4();
    let mut core = core(10_000);
    let total = 2 * FRAME_COMMANDS as i64 + 88;
    apply_redis(&mut core, 1..=total);
    core.add_subscriber(a);
    let frames = core.take_frames();
    let sizes: Vec<_> = frames
        .iter()
        .map(|(_, frame)| match frame {
            StreamMessage::Sync { commands, .. } | StreamMessage::Commands { commands, .. } => commands.len(),
            _ => 0,
        })
        .collect();
    assert_eq!(sizes, vec![FRAME_COMMANDS, FRAME_COMMANDS, 88]);
    assert!(matches!(frames[0].1, StreamMessage::Sync { .. }));
    assert_eq!(delivered(&frames, a), (1..=total).collect::<Vec<_>>());
}

#[test]
fn resolving_a_checkpoint_never_drops_undelivered_commands() {
    let a = Uuid::new_v4();
    let mut core = core(5);
    core.add_subscriber(a);
    core.take_frames();

    // Persist and resolve before fanning out: the commands must still reach the subscriber.
    apply_redis(&mut core, 1..=5);
    core.on_append(AppendOutcome::Appended { version: 5 }).unwrap();
    let checkpoint = core.ready_checkpoint().unwrap();
    assert_eq!(checkpoint.version, 5);
    assert_eq!(core.on_snapshot(&checkpoint, SnapshotOutcome::Stored), None);

    assert!(core.take_checkpoint_frames().is_empty(), "a checkpoint waits for its commands");
    let frames = core.take_frames();
    assert_eq!(delivered(&frames, a), vec![1, 2, 3, 4, 5]);
    let checkpoints = core.take_checkpoint_frames();
    assert!(matches!(
        checkpoints.as_slice(),
        [(id, StreamMessage::Checkpoint { version: 5, .. })] if *id == a
    ));
}

#[test]
fn reset_is_followed_by_one_sync() {
    let a = Uuid::new_v4();
    let mut core = core(100);
    core.add_subscriber(a);
    apply_redis(&mut core, 1..=2);
    core.take_frames();

    let resets = core.take_reset_frames();
    assert!(matches!(resets.as_slice(), [(id, StreamMessage::Reset { .. })] if *id == a));
    assert!(core.take_reset_frames().is_empty(), "one Reset per Sync");

    core.begin_load(None);
    core.apply(cmd(1, add(1)), true).unwrap();
    core.finish_load(&meta(1, 1, 1));
    let frames = core.take_frames();
    assert!(matches!(frames.as_slice(), [(id, StreamMessage::Sync { .. })] if *id == a));
    assert_eq!(delivered(&frames, a), vec![1]);
}

#[test]
fn a_gap_requires_a_reload() {
    let mut core = core(100);
    assert_eq!(core.apply(cmd(2, add(1)), false), Err(Fault::Reload("gap in the command log")));
}

#[test]
fn observe_adopts_only_proven_newer_generations() {
    let mut core = core(100);
    apply_redis(&mut core, 1..=3);

    assert_eq!(
        core.observe(&meta(0, 3, 0), None),
        Err(Fault::Reset {
            generation: 0,
            reason: "Redis went back to an older generation"
        })
    );
    assert_eq!(
        core.observe(&meta(1, 2, 0), None),
        Err(Fault::Reset {
            generation: 1,
            reason: "Redis is behind the applied commands"
        })
    );
    assert_eq!(core.observe(&meta(2, 3, 3), None), Ok(Observed::NeedsProof(2)));

    let head = StreamHead {
        kind: "demo".into(),
        version: 3,
        generation: 2,
    };
    assert!(core
        .prove_and_adopt(2, AppendOutcome::StaleGeneration { head })
        .is_err());
    core.prove_and_adopt(2, AppendOutcome::UpToDate { version: 3 }).unwrap();
    assert_eq!(core.generation(), 2);
    assert!(core.unpersisted().is_empty());
    assert_eq!(core.observe(&meta(2, 3, 3), None), Ok(Observed::Current), "version 3 is persisted");
}

#[test]
fn observe_detects_a_reassigned_or_lost_version() {
    // After a restart from a dump or a failover, Redis can catch up past the replica with other commands.
    let mut core = core(100);
    apply_redis(&mut core, 1..=3);
    let mine = entry(3, Uuid::from_u128(3));
    let other = entry(3, Uuid::new_v4());
    assert_eq!(core.observe(&meta(1, 5, 0), Some(&mine)), Ok(Observed::Current));
    assert_eq!(
        core.observe(&meta(1, 5, 0), Some(&other)),
        Err(Fault::Reset {
            generation: 1,
            reason: "Redis reassigned an applied version"
        })
    );
    assert_eq!(
        core.observe(&meta(1, 5, 0), None),
        Err(Fault::Reset {
            generation: 1,
            reason: "Redis lost an applied version"
        })
    );
    assert_eq!(core.observe(&meta(1, 5, 3), None), Ok(Observed::Current), "trimmed after it was persisted");
}

#[test]
fn the_persisted_mark_carries_the_command_at_the_version() {
    let mut core = core(100);
    apply_redis(&mut core, 1..=5);
    assert_eq!(core.mark_due(), None, "nothing persisted yet");
    core.on_append(AppendOutcome::Appended { version: 5 }).unwrap();
    assert_eq!(core.mark_due(), Some((5, Uuid::from_u128(5))));
    core.on_marked(5);
    assert_eq!(core.mark_due(), None);
}

#[test]
fn append_outcomes_map_to_faults() {
    let head = StreamHead {
        kind: "demo".into(),
        version: 1,
        generation: 2,
    };
    let cases = [
        (AppendOutcome::Appended { version: 1 }, Ok(())),
        (AppendOutcome::UpToDate { version: 1 }, Ok(())),
        (
            AppendOutcome::StaleGeneration { head },
            Err(Fault::Reset {
                generation: 1,
                reason: "Redis generation differs from Postgres",
            }),
        ),
        (AppendOutcome::Gap { head: 0 }, Err(Fault::Reload("Postgres is behind the applied commands"))),
        (
            AppendOutcome::Diverged { version: 1 },
            Err(Fault::Reset {
                generation: 1,
                reason: "Redis diverged from Postgres",
            }),
        ),
    ];
    for (outcome, expected) in cases {
        let mut core = core(100);
        apply_redis(&mut core, 1..=1);
        assert_eq!(core.on_append(outcome.clone()), expected, "{outcome:?}");
    }
}

#[test]
fn only_the_newest_ready_checkpoint_is_resolved() {
    let mut core = core(1);
    apply_redis(&mut core, 1..=5);
    core.on_append(AppendOutcome::Appended { version: 5 }).unwrap();
    assert_eq!(core.ready_checkpoint().map(|c| c.version), Some(5));
}

#[test]
fn a_superseded_checkpoint_is_dropped() {
    let mut core = core(2);
    apply_redis(&mut core, 1..=2);
    core.on_append(AppendOutcome::Appended { version: 2 }).unwrap();
    let checkpoint = core.ready_checkpoint().unwrap();
    assert_eq!(core.on_snapshot(&checkpoint, SnapshotOutcome::Superseded), None);
    assert!(core.ready_checkpoint().is_none());
    assert!(core.take_checkpoint_frames().is_empty());
}

#[test]
fn rebase_replays_the_newer_commands_on_the_authoritative_state() {
    let mut core = core(2);
    apply_redis(&mut core, 1..=4); // value 4, checkpoints captured at 2 and 4
    core.on_append(AppendOutcome::Appended { version: 2 }).unwrap();
    let checkpoint = core.ready_checkpoint().unwrap();
    assert_eq!(checkpoint.version, 2);

    let mut authoritative = IntArray::default();
    authoritative.values[0] = 100;
    let hash = stable_hash_of(&authoritative);
    assert_eq!(core.on_snapshot(&checkpoint, SnapshotOutcome::Exists { hash }), Some(2));

    let snapshot = StreamSnapshot {
        version: 2,
        hash,
        data: encode_snapshot(&authoritative).unwrap().into(),
    };
    core.rebase(snapshot, authoritative).unwrap();
    assert_eq!(core.version(), 4);
    assert_eq!(core.state().values[0], 102, "versions 3 and 4 replayed on the authoritative state");
}
```

In `stream_core.rs`, the accessor `state()` is test-only (`#[cfg(test)]`): builder is a binary crate, so an unused `pub fn` would fail `clippy -D warnings`.

- [ ] **Step 5: Run to verify they fail**

Run: `cargo test -p shine-builder stream_core`. Expected: compile errors.

- [ ] **Step 6: Implement the core**

`stream_core.rs`:

```rust
//! The replay state of one stream worker, without IO: unit-tested without Redis or Postgres.

use crate::{
    models::messages::{StreamCommand, StreamMessage, StreamSnapshot},
    repositories::command_stream::redis::{RedisStreamEntry, RedisStreamMeta},
};
use shine_game::replay::{encode_snapshot, ApplyOutcome, CheckpointPolicy, CommandEnvelope, ReplayState, Replica};
use shine_infra::db::command_log::{AppendOutcome, SnapshotOutcome, StoredCommand};
use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};
use uuid::Uuid;

/// Max commands per egress frame.
pub const FRAME_COMMANDS: usize = 256;

/// Captured checkpoints kept while waiting for persistence; only the newest ready one is resolved.
const MAX_CAPTURED: usize = 2;

/// Why the worker must rebuild its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Rebuild from Postgres.
    Reload(&'static str),
    /// Drop the Redis copy if it is still at `generation`, then rebuild from Postgres.
    Reset { generation: i64, reason: &'static str },
}

/// The verdict on the Redis meta of a read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observed {
    Current,
    /// Redis moved to a newer generation while commands applied from the old one are not proven
    /// persisted: append them and pass the outcome to `prove_and_adopt`.
    NeedsProof(i64),
}

/// A state captured at a checkpoint version, waiting for the version to be persisted.
#[derive(Clone, Debug)]
pub struct Captured {
    pub version: i64,
    pub hash: u64,
    pub data: Arc<[u8]>,
}

/// Replica, persistence progress and subscriber cursors of one stream on one instance.
pub struct StreamCore<S: ReplayState> {
    stream_id: Uuid,
    policy: CheckpointPolicy,
    replica: Replica<S>,
    generation: i64,
    /// Highest applied version known to be in Postgres.
    persisted: i64,
    /// Highest version this worker recorded as persisted in Redis.
    marked: i64,
    /// Version and command id of the last applied command, checked against Redis on every read.
    tip: Option<(i64, Uuid)>,
    /// Applied (version, command id) above `marked`: the persisted mark is recorded by content.
    recent: VecDeque<(i64, Uuid)>,
    authoritative: Option<StreamSnapshot>,
    /// Applied commands above min(authoritative version, fanned_out): for `Sync`, fan-out and rebase.
    since_snapshot: Vec<StreamCommand>,
    /// Commands applied from Redis that are not proven to be in Postgres, ascending.
    unpersisted: Vec<StoredCommand>,
    captured: VecDeque<Captured>,
    checkpoint_to_push: Option<(i64, u64)>,
    /// Subscribers that received every version up to `fanned_out`.
    synced: HashSet<Uuid>,
    /// Subscribers waiting for a `Sync`.
    pending_sync: HashSet<Uuid>,
    fanned_out: i64,
}

impl<S: ReplayState> StreamCore<S> {
    pub fn new(stream_id: Uuid, policy: CheckpointPolicy) -> Self {
        Self {
            stream_id,
            policy,
            replica: Replica::new(),
            generation: 0,
            persisted: 0,
            marked: 0,
            tip: None,
            recent: VecDeque::new(),
            authoritative: None,
            since_snapshot: Vec::new(),
            unpersisted: Vec::new(),
            captured: VecDeque::new(),
            checkpoint_to_push: None,
            synced: HashSet::new(),
            pending_sync: HashSet::new(),
            fanned_out: 0,
        }
    }

    pub fn version(&self) -> i64 {
        self.replica.version()
    }

    pub fn generation(&self) -> i64 {
        self.generation
    }

    #[cfg(test)]
    pub fn state(&self) -> &S {
        self.replica.state()
    }

    pub fn unpersisted(&self) -> &[StoredCommand] {
        &self.unpersisted
    }

    pub fn add_subscriber(&mut self, connection_id: Uuid) {
        self.synced.remove(&connection_id);
        self.pending_sync.insert(connection_id);
    }

    pub fn remove_subscriber(&mut self, connection_id: Uuid) {
        self.synced.remove(&connection_id);
        self.pending_sync.remove(&connection_id);
    }

    /// Starts a rebuild from `snapshot`, the latest stored one with its decoded state. Every
    /// subscriber gets a `Sync` once loaded.
    pub fn begin_load(&mut self, snapshot: Option<(StreamSnapshot, S)>) {
        match snapshot {
            Some((snapshot, state)) => {
                self.replica = Replica::from_snapshot(snapshot.version, state);
                self.authoritative = Some(snapshot);
            }
            None => {
                self.replica = Replica::new();
                self.authoritative = None;
            }
        }
        self.since_snapshot.clear();
        self.unpersisted.clear();
        self.captured.clear();
        self.checkpoint_to_push = None;
        self.persisted = self.replica.version();
        self.marked = 0;
        self.tip = None;
        self.recent.clear();
        self.fanned_out = self.replica.version();
        let synced = std::mem::take(&mut self.synced);
        self.pending_sync.extend(synced);
    }

    /// Ends a rebuild: the Postgres log is applied; `meta` is the Redis copy to continue from.
    pub fn finish_load(&mut self, meta: &RedisStreamMeta) {
        self.persisted = self.replica.version();
        self.generation = meta.generation;
    }

    /// Whether the load ended on a snapshot without a later command, so the tip's command id is unknown.
    pub fn needs_tip(&self) -> bool {
        self.tip.is_none() && self.replica.version() > 0
    }

    /// Sets the command id of the replica's version, read from Postgres after a load.
    pub fn set_tip(&mut self, version: i64, command_id: Uuid) {
        if version == self.replica.version() {
            self.tip = Some((version, command_id));
        }
    }

    /// Applies the next command; `persisted` when it was read from Postgres.
    pub fn apply(&mut self, command: StoredCommand, persisted: bool) -> Result<(), Fault> {
        let envelope = CommandEnvelope {
            version: command.version,
            command_id: command.command_id,
            user_id: command.user_id,
            command: serde_json::from_str::<S::Command>(&command.data).ok(),
        };
        match self.replica.apply_next(&envelope) {
            Ok(ApplyOutcome::AlreadyApplied) => return Ok(()),
            Ok(ApplyOutcome::Applied | ApplyOutcome::Ignored) => {}
            Err(_) => return Err(Fault::Reload("gap in the command log")),
        }
        if let Some(captured) = capture(self.stream_id, &self.policy, &self.replica) {
            push_captured(&mut self.captured, captured);
        }
        self.since_snapshot.push(StreamCommand {
            version: command.version,
            command_id: command.command_id,
            user_id: command.user_id,
            data: Arc::from(command.data.as_str()),
        });
        self.tip = Some((command.version, command.command_id));
        self.recent.push_back((command.version, command.command_id));
        if persisted {
            self.persisted = self.persisted.max(command.version);
        } else {
            self.unpersisted.push(command);
        }
        Ok(())
    }

    /// Checks the Redis meta that came with a read, and `tip`, the entry Redis holds at the replica's
    /// version (the read starts there).
    pub fn observe(&mut self, meta: &RedisStreamMeta, tip: Option<&RedisStreamEntry>) -> Result<Observed, Fault> {
        if meta.generation < self.generation {
            return Err(Fault::Reset {
                generation: meta.generation,
                reason: "Redis went back to an older generation",
            });
        }
        if meta.generation > self.generation {
            if !self.unpersisted.is_empty() {
                return Ok(Observed::NeedsProof(meta.generation));
            }
            self.adopt(meta.generation);
        }
        if meta.version < self.replica.version() {
            return Err(Fault::Reset {
                generation: meta.generation,
                reason: "Redis is behind the applied commands",
            });
        }
        self.verify_tip(meta, tip)?;
        Ok(Observed::Current)
    }

    /// Redis must still hold the command this replica applied last, unless that version was persisted
    /// and trimmed (only persisted versions are trimmed). Another command there means Redis went back
    /// in time and reassigned the version (restart from a dump, failover); a missing unpersisted one
    /// means Redis lost it. Counting versions alone cannot see a reassignment that already caught up.
    fn verify_tip(&self, meta: &RedisStreamMeta, tip: Option<&RedisStreamEntry>) -> Result<(), Fault> {
        let Some((version, command_id)) = self.tip else {
            return Ok(());
        };
        match tip.filter(|entry| entry.version == version) {
            Some(entry) if entry.command_id == command_id => Ok(()),
            Some(_) => Err(Fault::Reset {
                generation: meta.generation,
                reason: "Redis reassigned an applied version",
            }),
            None if version <= meta.persisted => Ok(()),
            None => Err(Fault::Reset {
                generation: meta.generation,
                reason: "Redis lost an applied version",
            }),
        }
    }

    /// Finishes a generation change with the outcome of appending the unpersisted commands.
    pub fn prove_and_adopt(&mut self, generation: i64, outcome: AppendOutcome) -> Result<(), Fault> {
        match outcome {
            AppendOutcome::UpToDate { version } if version >= self.replica.version() => {
                self.unpersisted.clear();
                self.persisted = self.replica.version();
                self.adopt(generation);
                Ok(())
            }
            _ => Err(Fault::Reload("Redis changed generation with commands not in Postgres")),
        }
    }

    fn adopt(&mut self, generation: i64) {
        self.generation = generation;
        self.marked = 0;
    }

    /// Applies the outcome of appending `unpersisted` to Postgres.
    pub fn on_append(&mut self, outcome: AppendOutcome) -> Result<(), Fault> {
        match outcome {
            AppendOutcome::Appended { version } | AppendOutcome::UpToDate { version } => {
                self.unpersisted.retain(|c| c.version > version);
                self.persisted = self.persisted.max(version.min(self.replica.version()));
                Ok(())
            }
            // RESET is generation-guarded: when Redis already moved on, it is a no-op.
            AppendOutcome::StaleGeneration { .. } => Err(Fault::Reset {
                generation: self.generation,
                reason: "Redis generation differs from Postgres",
            }),
            AppendOutcome::Gap { .. } => Err(Fault::Reload("Postgres is behind the applied commands")),
            AppendOutcome::Diverged { .. } => Err(Fault::Reset {
                generation: self.generation,
                reason: "Redis diverged from Postgres",
            }),
        }
    }

    /// The version to record as persisted in Redis, with the command applied there, when it is ahead
    /// of what is recorded.
    pub fn mark_due(&self) -> Option<(i64, Uuid)> {
        if self.persisted <= self.marked {
            return None;
        }
        let at = self.recent.partition_point(|(version, _)| *version < self.persisted);
        self.recent.get(at).filter(|(version, _)| *version == self.persisted).copied()
    }

    pub fn on_marked(&mut self, version: i64) {
        self.marked = self.marked.max(version);
        while self.recent.front().is_some_and(|(v, _)| *v <= self.marked) {
            self.recent.pop_front();
        }
    }

    /// The newest captured checkpoint that is persisted; older ready ones are dropped.
    pub fn ready_checkpoint(&mut self) -> Option<Captured> {
        while self.captured.len() > 1 && self.captured[1].version <= self.persisted {
            self.captured.pop_front();
        }
        self.captured.front().filter(|c| c.version <= self.persisted).cloned()
    }

    /// Applies the outcome of storing `checkpoint`. Returns the version to rebase onto when another
    /// instance stored a different state there first.
    pub fn on_snapshot(&mut self, checkpoint: &Captured, outcome: SnapshotOutcome) -> Option<i64> {
        if self.captured.front().map(|c| c.version) == Some(checkpoint.version) {
            self.captured.pop_front();
        }
        let agreed = match outcome {
            SnapshotOutcome::Stored => true,
            SnapshotOutcome::Exists { hash } => hash == checkpoint.hash,
            SnapshotOutcome::Superseded => return None,
        };
        if !agreed {
            return Some(checkpoint.version);
        }
        if self.authoritative.as_ref().is_some_and(|a| a.version >= checkpoint.version) {
            return None;
        }
        self.set_authoritative(StreamSnapshot {
            version: checkpoint.version,
            hash: checkpoint.hash,
            data: checkpoint.data.clone(),
        });
        None
    }

    /// Replaces the state with `state`, the authoritative one at `snapshot.version`, and replays the
    /// newer applied commands on it. Atomic: on error nothing changed.
    pub fn rebase(&mut self, snapshot: StreamSnapshot, state: S) -> Result<(), Fault> {
        let mut replica = Replica::from_snapshot(snapshot.version, state);
        let mut captured = VecDeque::new();
        for command in self.since_snapshot.iter().filter(|c| c.version > snapshot.version) {
            let envelope = CommandEnvelope {
                version: command.version,
                command_id: command.command_id,
                user_id: command.user_id,
                command: serde_json::from_str::<S::Command>(&command.data).ok(),
            };
            if replica.apply_next(&envelope).is_err() {
                return Err(Fault::Reload("gap while rebasing"));
            }
            if let Some(checkpoint) = capture(self.stream_id, &self.policy, &replica) {
                push_captured(&mut captured, checkpoint);
            }
        }
        if replica.version() != self.replica.version() {
            return Err(Fault::Reload("rebase did not reach the applied version"));
        }
        self.replica = replica;
        self.captured = captured;
        self.set_authoritative(snapshot);
        Ok(())
    }

    fn set_authoritative(&mut self, snapshot: StreamSnapshot) {
        self.checkpoint_to_push = Some((snapshot.version, snapshot.hash));
        self.authoritative = Some(snapshot);
        self.trim();
    }

    /// Drops the commands no `Sync`, fan-out or rebase needs: those covered by the authoritative
    /// snapshot that every synced subscriber already received.
    fn trim(&mut self) {
        let keep_after = self.authoritative.as_ref().map_or(0, |a| a.version).min(self.fanned_out);
        let cut = self.since_snapshot.partition_point(|c| c.version <= keep_after);
        self.since_snapshot.drain(..cut);
    }

    /// The frames to send now: new commands to the synced subscribers, a `Sync` (and the rest of a
    /// large backlog as `Commands`) to the new ones. Frames of one step are shared.
    pub fn take_frames(&mut self) -> Vec<(Uuid, StreamMessage)> {
        let (stream_id, generation) = (self.stream_id, self.generation);
        let mut frames = Vec::new();

        if self.replica.version() > self.fanned_out && !self.synced.is_empty() {
            let start = self.since_snapshot.partition_point(|c| c.version <= self.fanned_out);
            for chunk in self.since_snapshot[start..].chunks(FRAME_COMMANDS) {
                let commands: Arc<[StreamCommand]> = chunk.into();
                for connection_id in &self.synced {
                    frames.push((
                        *connection_id,
                        StreamMessage::Commands {
                            stream_id,
                            generation,
                            commands: commands.clone(),
                        },
                    ));
                }
            }
        }

        if !self.pending_sync.is_empty() {
            let after = self.authoritative.as_ref().map_or(0, |a| a.version);
            let start = self.since_snapshot.partition_point(|c| c.version <= after);
            let mut chunks = self.since_snapshot[start..].chunks(FRAME_COMMANDS).map(Arc::<[StreamCommand]>::from);
            let first = chunks.next().unwrap_or_else(|| Arc::from(Vec::new()));
            let rest: Vec<Arc<[StreamCommand]>> = chunks.collect();
            for connection_id in self.pending_sync.drain() {
                frames.push((
                    connection_id,
                    StreamMessage::Sync {
                        stream_id,
                        generation,
                        snapshot: self.authoritative.clone(),
                        commands: first.clone(),
                    },
                ));
                for commands in &rest {
                    frames.push((
                        connection_id,
                        StreamMessage::Commands {
                            stream_id,
                            generation,
                            commands: commands.clone(),
                        },
                    ));
                }
                self.synced.insert(connection_id);
            }
        }

        self.fanned_out = self.replica.version();
        self.trim();
        frames
    }

    /// The latest resolved checkpoint for every synced subscriber, once its commands were delivered.
    pub fn take_checkpoint_frames(&mut self) -> Vec<(Uuid, StreamMessage)> {
        let Some((version, hash)) = self.checkpoint_to_push.filter(|(version, _)| *version <= self.fanned_out) else {
            return Vec::new();
        };
        self.checkpoint_to_push = None;
        self.synced
            .iter()
            .map(|connection_id| {
                (
                    *connection_id,
                    StreamMessage::Checkpoint {
                        stream_id: self.stream_id,
                        version,
                        hash,
                    },
                )
            })
            .collect()
    }

    /// Starts a reload: the synced subscribers get `Reset`, then everyone gets a `Sync` once loaded.
    pub fn take_reset_frames(&mut self) -> Vec<(Uuid, StreamMessage)> {
        let synced: Vec<Uuid> = self.synced.drain().collect();
        self.pending_sync.extend(synced.iter().copied());
        synced
            .into_iter()
            .map(|connection_id| (connection_id, StreamMessage::Reset { stream_id: self.stream_id }))
            .collect()
    }
}

/// Captures the replica at a checkpoint version. An encoding failure (a bug of the state type) only
/// skips the checkpoint: snapshots are an optimization and a drift check.
fn capture<S: ReplayState>(stream_id: Uuid, policy: &CheckpointPolicy, replica: &Replica<S>) -> Option<Captured> {
    let version = replica.version();
    if !policy.is_checkpoint(version) {
        return None;
    }
    match encode_snapshot(replica.state()) {
        Ok(data) => Some(Captured {
            version,
            hash: replica.hash(),
            data: data.into(),
        }),
        Err(err) => {
            tracing::error!("[{stream_id}] Failed to encode the state at version {version}: {err}");
            None
        }
    }
}

fn push_captured(captured: &mut VecDeque<Captured>, checkpoint: Captured) {
    if captured.len() == MAX_CAPTURED {
        captured.pop_front();
    }
    captured.push_back(checkpoint);
}
```

Borrow notes:
- In `take_frames`, the loops read `self.since_snapshot` and `self.authoritative` while `self.pending_sync.drain()` and `self.synced.insert` mutate other fields. These are disjoint field borrows, which compile because the code uses the fields directly rather than methods.
- `chunks` is collected before the loop.

- [ ] **Step 7: AppState**

In `app_state.rs`, replace the `let _ = pg_pool;` placeholder with the following, placed before the hub is created:

```rust
        let stream_settings = CommandStreamSettings::from(&config.feature.command_stream);
        let stream_redis = RedisCommandStreamDb::new(
            redis_pool,
            RedisStreamConfig {
                prefix: RedisCommandStreamDb::PREFIX.to_string(),
                keep_commands: stream_settings.redis_keep_commands,
                max_unpersisted: stream_settings.redis_max_unpersisted,
                ttl_seconds: stream_settings.redis_ttl_seconds,
            },
        );
        stream_redis.check_eviction_policy().await;
        let command_stream_service = CommandStreamService::new(
            StreamStores {
                redis: stream_redis,
                log: PgCommandLogDb::new(pg_pool).await?,
                settings: stream_settings,
            },
            StreamKindRegistry::new().with::<IntArray>(),
            Arc::new(AllowAll),
        );
```

Then:
- Add `command_stream_service: CommandStreamService` to `Inner`, fill it in, and add the accessor:

  ```rust
      pub fn command_stream_service(&self) -> &CommandStreamService {
          &self.0.command_stream_service
      }
  ```

- Imports:
  - `shine_game::replay::IntArray`
  - `shine_infra::db::command_log::PgCommandLogDb`
  - `crate::repositories::command_stream::redis::{RedisCommandStreamDb, RedisStreamConfig}`
  - `crate::services::command_stream::{AllowAll, CommandStreamService, StreamKindRegistry, StreamStores}`
  - `crate::settings::CommandStreamSettings`
- The hub receives the service in Task 7.

- [ ] **Step 8: Run tests**

Run: `cargo test -p shine-builder stream_core`. Expected: 12 tests PASS. These need no env vars. Then run `cargo build -p shine-builder`; expected: it builds.

- [ ] **Step 9: Checkpoint.** Report and wait.

---

## Task 7: `StreamWorker<S>`, the `StreamDispatcher` hub task, and the hub's `StreamEgress`

Re-read `services/hub/{hub_service.rs, mod.rs, tasks/mod.rs, test_utils.rs}` first.

**Files:**
- Create: `services/builder/src/services/command_stream/stream_worker.rs`, `services/builder/src/services/hub/stream_egress.rs`, `services/builder/src/services/hub/tasks/stream_dispatcher.rs`
- Modify: `services/command_stream/{mod.rs, stream_kind.rs}`, `services/hub/{mod.rs, hub_service.rs, test_utils.rs, tasks/mod.rs}`, `app_state.rs`

**Interfaces:**
- Consumes: Tasks 1–6.
- Produces:
  - `StreamWorker::<S>::create(egress: Arc<dyn StreamEgress>, streams: CommandStreamService, stream_id: Uuid) -> (WorkerHandle, WorkerFuture)`
  - `WorkerHandle { tx, wake }`
  - `WorkerCommand::{AddSubscriber(Uuid), RemoveSubscriber(Uuid), Stop}`
  - `StreamKind::create_worker(&self, egress: Arc<dyn StreamEgress>, streams: CommandStreamService, stream_id: Uuid) -> (WorkerHandle, WorkerFuture)`
  - `StreamDispatcher::start(hub: HubService, streams: CommandStreamService) -> JoinHandle<()>` (async)
  - `HubService::new(hub_registry, session_service, chat_service, intervals, command_streams: Option<CommandStreamService>)`

**How the worker runs:** (every read starts at the replica's own version, so `StreamCore` checks that Redis still holds the command applied there)
- **Wake** (Redis `PUBLISH`, paced at ≥ 2 ms between steps): `deliver` applies the new entries and sends them.
- **Persist timer** (`persistDelayMs` plus the same again as jitter, armed when commands become unpersisted): `persist_cycle` checks the copy (content at the replica's version), appends, records the persisted mark by content, resolves the checkpoint and pushes it.
- **Periodic tick** (`syncIntervalMs`): runs both, which covers lost wakes and mark retries.

All replay decisions are made in `StreamCore` (Task 6); this shell only does IO.

- [ ] **Step 1: Write the failing send-order test**

At the end of `stream_worker.rs` (created in Step 2), add:

```rust
#[cfg(test)]
mod tests {
    use super::send_frames;
    use crate::{
        models::messages::StreamMessage,
        repositories::command_stream::redis::RedisStreamMeta,
        services::command_stream::{StreamCore, StreamEgress},
    };
    use futures::future::BoxFuture;
    use shine_game::replay::{CheckpointPolicy, DemoCommand, IntArray};
    use shine_infra::db::command_log::StoredCommand;
    use shine_test::test;
    use std::sync::Mutex;
    use uuid::Uuid;

    /// Refuses every message to `slow`, as a full egress channel does.
    struct Egress {
        slow: Uuid,
        attempts: Mutex<Vec<Uuid>>,
    }

    impl StreamEgress for Egress {
        fn send(&self, connection_id: Uuid, _message: StreamMessage) -> BoxFuture<'_, bool> {
            Box::pin(async move {
                self.attempts.lock().unwrap().push(connection_id);
                connection_id != self.slow
            })
        }
    }

    fn cmd(version: i64) -> StoredCommand {
        StoredCommand {
            version,
            command_id: Uuid::new_v4(),
            user_id: Uuid::nil(),
            generation: 1,
            data: serde_json::to_string(&DemoCommand::Add { idx: 0, delta: 1 }).unwrap(),
        }
    }

    #[test]
    async fn a_failed_send_stops_all_later_frames() {
        let (fast, slow) = (Uuid::new_v4(), Uuid::new_v4());
        let mut core = StreamCore::<IntArray>::new(Uuid::nil(), CheckpointPolicy::new(10_000));
        core.begin_load(None);
        core.finish_load(&RedisStreamMeta {
            generation: 1,
            version: 0,
            persisted: 0,
        });
        core.add_subscriber(fast);
        core.add_subscriber(slow);
        for version in 1..=600 {
            core.apply(cmd(version), false).unwrap(); // three frames per subscriber
        }

        let egress = Egress {
            slow,
            attempts: Mutex::new(vec![]),
        };
        let frames = core.take_frames();
        send_frames(&egress, &mut core, frames).await;
        let attempts = egress.attempts.lock().unwrap().clone();
        assert_eq!(attempts.iter().filter(|id| **id == fast).count(), 3);
        assert_eq!(attempts.iter().filter(|id| **id == slow).count(), 1, "nothing after the first failure");

        core.apply(cmd(601), false).unwrap();
        assert!(
            core.take_frames().iter().all(|(id, _)| *id != slow),
            "a subscriber whose send failed gets nothing more"
        );
    }
}
```

- [ ] **Step 2: Worker**

`services/command_stream/stream_worker.rs`:

```rust
use crate::{
    models::{
        messages::{StreamMessage, StreamSnapshot},
        CommandStreamError,
    },
    repositories::command_stream::redis::{MarkOutcome, RedisStreamEntry, RedisStreamMeta},
    services::command_stream::{
        command_stream_metrics, CommandStreamService, Fault, Observed, StreamCore, StreamEgress, StreamStores,
    },
};
use shine_game::replay::{decode_snapshot, CheckpointPolicy, ReplayState};
use shine_infra::db::{
    command_log::{CommandLogError, SnapshotOutcome},
    redis::RedisError,
};
use std::{collections::HashSet, future::Future, pin::Pin, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, Notify},
    time::{self, Instant},
};
use tracing::Instrument;
use uuid::Uuid;

const REDIS_READ_BATCH: i64 = 256;
/// Max entries applied per step; the rest is picked up by an immediate follow-up step.
const STEP_READ_LIMIT: usize = 1024;
const PG_READ_BATCH: i64 = 512;
/// Min time between two wake-driven steps: a busy stream sends batches, not one frame per command.
const STEP_SPACING: Duration = Duration::from_millis(2);
const RELOAD_BACKOFF_MIN: Duration = Duration::from_millis(200);
const RELOAD_BACKOFF_MAX: Duration = Duration::from_secs(10);

pub enum WorkerCommand {
    AddSubscriber(Uuid),
    RemoveSubscriber(Uuid),
    /// Persist what is pending and exit.
    Stop,
}

/// Control side of a running stream worker.
#[derive(Clone)]
pub struct WorkerHandle {
    pub tx: mpsc::UnboundedSender<WorkerCommand>,
    pub wake: Arc<Notify>,
}

pub type WorkerFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

#[derive(Debug)]
enum StepError {
    /// The state must be rebuilt.
    Fault(Fault),
    /// An infrastructure error; retried on the next step.
    Failed(CommandStreamError),
}

impl From<Fault> for StepError {
    fn from(fault: Fault) -> Self {
        Self::Fault(fault)
    }
}

impl From<CommandStreamError> for StepError {
    fn from(err: CommandStreamError) -> Self {
        Self::Failed(err)
    }
}

impl From<CommandLogError> for StepError {
    #[track_caller]
    fn from(err: CommandLogError) -> Self {
        Self::Failed(err.into())
    }
}

impl From<RedisError> for StepError {
    #[track_caller]
    fn from(err: RedisError) -> Self {
        Self::Failed(err.into())
    }
}

/// Sends frames in order. A connection whose send fails gets none of its later frames (it would see
/// a gap) and leaves the core; the hub drops that connection.
pub(super) async fn send_frames<S: ReplayState>(
    egress: &dyn StreamEgress,
    core: &mut StreamCore<S>,
    frames: Vec<(Uuid, StreamMessage)>,
) {
    let mut failed = HashSet::new();
    for (connection_id, message) in frames {
        if failed.contains(&connection_id) {
            continue;
        }
        if !egress.send(connection_id, message).await {
            failed.insert(connection_id);
            core.remove_subscriber(connection_id);
        }
    }
}

/// Replays one stream on this instance: the IO around a `StreamCore`.
pub struct StreamWorker<S: ReplayState> {
    stream_id: Uuid,
    core: StreamCore<S>,
    egress: Arc<dyn StreamEgress>,
    streams: CommandStreamService,
    stores: StreamStores,
    wake: Arc<Notify>,
    needs_load: bool,
    /// Earliest time of the next load attempt, and the delay after the next failure.
    next_load: Instant,
    load_backoff: Duration,
    /// When the next persist cycle is due, while commands are unpersisted.
    persist_due: Option<Instant>,
    last_step: Instant,
}

impl<S: ReplayState> StreamWorker<S> {
    pub fn create(egress: Arc<dyn StreamEgress>, streams: CommandStreamService, stream_id: Uuid) -> (WorkerHandle, WorkerFuture) {
        let stores = streams.stores();
        let (tx, rx) = mpsc::unbounded_channel();
        let wake = Arc::new(Notify::new());
        let worker = Self {
            stream_id,
            core: StreamCore::new(stream_id, CheckpointPolicy::new(stores.settings.snapshot_every)),
            egress,
            streams,
            stores,
            wake: wake.clone(),
            needs_load: true,
            next_load: Instant::now(),
            load_backoff: RELOAD_BACKOFF_MIN,
            persist_due: None,
            last_step: Instant::now(),
        };
        (WorkerHandle { tx, wake }, Box::pin(worker.run(rx)))
    }

    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<WorkerCommand>) {
        tracing::debug!("[{}] Stream worker started", self.stream_id);
        let wake = self.wake.clone();
        // The first tick fires at once and performs the initial load.
        let mut ticker = time::interval(self.stores.settings.sync_interval);
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
        loop {
            let persist_at = self.persist_due.unwrap_or_else(Instant::now);
            tokio::select! {
                biased;
                command = rx.recv() => match command {
                    Some(WorkerCommand::AddSubscriber(connection_id)) => {
                        self.core.add_subscriber(connection_id);
                        if !self.needs_load {
                            let frames = self.core.take_frames();
                            send_frames(self.egress.as_ref(), &mut self.core, frames).await;
                        }
                    }
                    Some(WorkerCommand::RemoveSubscriber(connection_id)) => self.core.remove_subscriber(connection_id),
                    Some(WorkerCommand::Stop) | None => {
                        if !self.needs_load {
                            if let Err(err) = self.persist_cycle().await {
                                tracing::warn!("[{}] Final persist failed, the orphan sweep retries it: {err:?}", self.stream_id);
                            }
                        }
                        break;
                    }
                },
                _ = time::sleep_until(persist_at), if self.persist_due.is_some() => self.step(true).await,
                _ = wake.notified() => {
                    let wait = STEP_SPACING.saturating_sub(self.last_step.elapsed());
                    if !wait.is_zero() {
                        time::sleep(wait).await;
                    }
                    self.step(false).await;
                }
                _ = ticker.tick() => self.step(true).await,
            }
        }
        tracing::debug!("[{}] Stream worker stopped", self.stream_id);
    }

    async fn step(&mut self, persist: bool) {
        let loading = self.needs_load;
        let span = tracing::debug_span!("stream step", stream.id = %self.stream_id);
        match self.try_step(persist).instrument(span).await {
            // Reset only after a full step, so a stream that reloads right after each load keeps backing off.
            Ok(()) if !self.needs_load => self.load_backoff = RELOAD_BACKOFF_MIN,
            Ok(()) => {}
            Err(StepError::Failed(err)) => {
                tracing::warn!("[{}] Stream step failed: {err:#?}", self.stream_id);
                // Only a failed load is retried with backoff; a later failure keeps the loaded state.
                if loading && self.needs_load {
                    self.schedule_load();
                }
            }
            Err(StepError::Fault(fault)) => self.begin_reload(fault).await,
        }
        self.last_step = Instant::now();
    }

    async fn try_step(&mut self, persist: bool) -> Result<(), StepError> {
        self.deliver().await?;
        if persist && !self.needs_load {
            self.persist_cycle().await?;
        }
        Ok(())
    }

    /// Loads if needed, applies the new Redis entries and sends them out.
    async fn deliver(&mut self) -> Result<(), StepError> {
        if self.needs_load {
            if Instant::now() < self.next_load {
                return Ok(());
            }
            self.load().await?;
            self.needs_load = false;
        }
        let more = self.read_redis().await?;
        let frames = self.core.take_frames();
        send_frames(self.egress.as_ref(), &mut self.core, frames).await;
        if !self.core.unpersisted().is_empty() && self.persist_due.is_none() {
            self.persist_due = Some(Instant::now() + self.persist_delay());
        }
        if more {
            self.wake.notify_one();
        }
        Ok(())
    }

    /// The configured delay plus up to the same again: the instances persisting the same commands spread
    /// their appends out instead of queueing on the stream's row lock together.
    fn persist_delay(&self) -> Duration {
        let delay = self.stores.settings.persist_delay;
        delay + Duration::from_millis(rand::random_range(0..=delay.as_millis() as u64))
    }

    /// Persists the applied commands, records the persisted version in Redis, resolves the newest
    /// checkpoint and pushes it.
    async fn persist_cycle(&mut self) -> Result<(), StepError> {
        self.persist_due = None;
        if !self.core.unpersisted().is_empty() {
            // Checked right before appending: a copy that went back in time must not be persisted.
            let tip = self.core.version();
            let (meta, entries) = self.stores.redis.read_after(self.stream_id, (tip - 1).max(0), 1).await?;
            if let Some(meta) = meta {
                self.observe(&meta, entries.first()).await?;
            }
        }
        if !self.core.unpersisted().is_empty() {
            let outcome = self.stores.log.append(self.stream_id, self.core.unpersisted()).await?;
            self.core.on_append(outcome)?;
        }
        if let Some((version, command_id)) = self.core.mark_due() {
            match self
                .stores
                .redis
                .mark_persisted(self.stream_id, self.core.generation(), version, command_id)
                .await
            {
                Ok(MarkOutcome::Marked | MarkOutcome::Unchanged) => self.core.on_marked(version),
                Ok(MarkOutcome::Refused) => {
                    return Err(Fault::Reset {
                        generation: self.core.generation(),
                        reason: "Redis disagrees with the persisted commands",
                    }
                    .into())
                }
                Err(err) => tracing::warn!("[{}] Failed to record the persisted version in Redis: {err:#?}", self.stream_id),
            }
        }
        self.resolve_checkpoint().await?;
        let frames = self.core.take_checkpoint_frames();
        send_frames(self.egress.as_ref(), &mut self.core, frames).await;
        if !self.core.unpersisted().is_empty() {
            self.persist_due = Some(Instant::now() + self.persist_delay());
        }
        Ok(())
    }

    /// Races to store the newest persisted checkpoint; rebases onto the stored state when it differs.
    async fn resolve_checkpoint(&mut self) -> Result<(), StepError> {
        while let Some(checkpoint) = self.core.ready_checkpoint() {
            let outcome = self
                .stores
                .log
                .try_store_snapshot(self.stream_id, checkpoint.version, checkpoint.hash, &checkpoint.data)
                .await?;
            let stored = outcome == SnapshotOutcome::Stored;
            if let Some(version) = self.core.on_snapshot(&checkpoint, outcome) {
                tracing::error!(
                    "[{}] State drift at version {version}: the local state differs from the authoritative snapshot",
                    self.stream_id
                );
                command_stream_metrics().drift.add(1, &[]);
                self.rebase(version).await?;
            }
            // Only the instance that stored a snapshot prunes.
            if stored {
                if let Err(err) = self
                    .stores
                    .log
                    .prune_snapshots(self.stream_id, self.stores.settings.keep_snapshots)
                    .await
                {
                    tracing::warn!("[{}] Failed to prune snapshots: {err:#?}", self.stream_id);
                }
            }
        }
        Ok(())
    }

    async fn rebase(&mut self, version: i64) -> Result<(), StepError> {
        let snapshot = self
            .stores
            .log
            .latest_snapshot(self.stream_id, version)
            .await?
            .filter(|snapshot| snapshot.version == version)
            .ok_or(Fault::Reload("the authoritative snapshot is gone"))?;
        let state = decode_snapshot::<S>(&snapshot.data).map_err(|_| Fault::Reload("the authoritative snapshot is unreadable"))?;
        self.core.rebase(
            StreamSnapshot {
                version,
                hash: snapshot.hash,
                data: snapshot.data.into(),
            },
            state,
        )?;
        Ok(())
    }

    /// Rebuilds the state from the latest snapshot and the Postgres log.
    async fn load(&mut self) -> Result<(), StepError> {
        self.streams.ensure_hydrated(self.stream_id).await?;
        let head = self
            .stores
            .log
            .head(self.stream_id)
            .await?
            .ok_or(CommandStreamError::StreamNotFound)?;
        let snapshot = match self.stores.log.latest_snapshot(self.stream_id, head.version).await? {
            Some(stored) => match decode_snapshot::<S>(&stored.data) {
                Ok(state) => Some((
                    StreamSnapshot {
                        version: stored.version,
                        hash: stored.hash,
                        data: stored.data.into(),
                    },
                    state,
                )),
                Err(err) => {
                    tracing::warn!(
                        "[{}] Snapshot at version {} is unreadable, replaying from the start: {err}",
                        self.stream_id,
                        stored.version
                    );
                    None
                }
            },
            None => None,
        };
        self.core.begin_load(snapshot);
        loop {
            let batch = self
                .stores
                .log
                .commands_after(self.stream_id, self.core.version(), PG_READ_BATCH)
                .await?;
            if batch.is_empty() {
                break;
            }
            for command in batch {
                self.core.apply(command, true)?;
            }
        }
        if self.core.needs_tip() {
            // Loaded onto a snapshot with no later command: the tip's command id comes from Postgres.
            let version = self.core.version();
            if let Some(command) = self.stores.log.commands_after(self.stream_id, version - 1, 1).await?.into_iter().next() {
                self.core.set_tip(command.version, command.command_id);
            }
        }
        let meta = self
            .stores
            .redis
            .meta(self.stream_id)
            .await?
            .ok_or(CommandStreamError::NotReady)?;
        self.core.finish_load(&meta);
        tracing::debug!("[{}] Stream loaded at version {}", self.stream_id, self.core.version());
        Ok(())
    }

    /// Applies the Redis entries after the replica, filling holes from Postgres. Returns whether more are waiting.
    async fn read_redis(&mut self) -> Result<bool, StepError> {
        let mut applied = 0;
        while applied < STEP_READ_LIMIT {
            // Starts at the replica's own version: that entry only serves the content check.
            let tip = self.core.version();
            let (meta, entries) = self
                .stores
                .redis
                .read_after(self.stream_id, (tip - 1).max(0), REDIS_READ_BATCH)
                .await?;
            let Some(meta) = meta else {
                // Redis lost the stream: persist what this worker holds before a new generation fences it off.
                if !self.core.unpersisted().is_empty() {
                    match self.stores.log.append(self.stream_id, self.core.unpersisted()).await {
                        Ok(outcome) => self.core.on_append(outcome)?,
                        Err(err) => tracing::warn!("[{}] Failed to persist before rehydrating: {err:#?}", self.stream_id),
                    }
                }
                self.streams.ensure_hydrated(self.stream_id).await?;
                return Ok(true);
            };
            self.observe(&meta, entries.first()).await?;

            let full = entries.len() as i64 == REDIS_READ_BATCH;
            let entries: Vec<_> = entries.into_iter().filter(|entry| entry.version > tip).collect();
            let Some(first) = entries.first() else {
                return Ok(false);
            };
            let expected = tip + 1;
            if first.version > expected {
                // Trimmed from Redis, so persisted: Postgres has them.
                let filled = self
                    .stores
                    .log
                    .commands_after(self.stream_id, tip, first.version - expected)
                    .await?;
                if filled.first().map(|c| c.version) != Some(expected) {
                    return Err(Fault::Reset {
                        generation: meta.generation,
                        reason: "commands missing from Redis and Postgres",
                    }
                    .into());
                }
                applied += filled.len();
                for command in filled {
                    self.core.apply(command, true)?;
                }
                continue;
            }

            applied += entries.len();
            for entry in entries {
                self.core.apply(entry.into(), false)?;
            }
            if !full {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Checks the Redis meta and tip entry of a read, proving persistence before adopting a newer generation.
    async fn observe(&mut self, meta: &RedisStreamMeta, tip: Option<&RedisStreamEntry>) -> Result<(), StepError> {
        if let Observed::NeedsProof(generation) = self.core.observe(meta, tip)? {
            // A database error is retried by the next read: nothing was adopted or applied yet.
            let outcome = self.stores.log.append(self.stream_id, self.core.unpersisted()).await?;
            self.core.prove_and_adopt(generation, outcome)?;
            self.core.observe(meta, tip)?;
        }
        Ok(())
    }

    async fn begin_reload(&mut self, fault: Fault) {
        match fault {
            Fault::Reload(reason) => tracing::info!("[{}] Reloading the stream: {reason}", self.stream_id),
            Fault::Reset { generation, reason } => {
                tracing::warn!(
                    "[{}] Dropping the Redis copy (generation {generation}) and reloading: {reason}",
                    self.stream_id
                );
                if let Err(err) = self.stores.redis.reset(self.stream_id, generation).await {
                    tracing::warn!("[{}] Failed to reset the Redis copy: {err:#?}", self.stream_id);
                }
            }
        }
        let frames = self.core.take_reset_frames();
        send_frames(self.egress.as_ref(), &mut self.core, frames).await;
        self.persist_due = None;
        self.schedule_load();
    }

    fn schedule_load(&mut self) {
        self.needs_load = true;
        self.next_load = Instant::now() + self.load_backoff;
        self.load_backoff = (self.load_backoff * 2).min(RELOAD_BACKOFF_MAX);
    }
}
```

In `command_stream/mod.rs`, add `mod stream_worker;` and `stream_worker::{StreamWorker, WorkerCommand, WorkerFuture, WorkerHandle}` to the `pub use`.

- [ ] **Step 3: Worker factory on the kinds**

In `stream_kind.rs`:
- Import `crate::services::command_stream::{stream_worker::StreamWorker, CommandStreamService, StreamEgress, WorkerFuture, WorkerHandle}` and `uuid::Uuid` (`Arc` is already imported).
- Add to the trait:

  ```rust
      fn create_worker(&self, egress: Arc<dyn StreamEgress>, streams: CommandStreamService, stream_id: Uuid) -> (WorkerHandle, WorkerFuture);
  ```

- Add to `impl<S: ReplayState> StreamKind for ReplayKind<S>`:

  ```rust
      fn create_worker(&self, egress: Arc<dyn StreamEgress>, streams: CommandStreamService, stream_id: Uuid) -> (WorkerHandle, WorkerFuture) {
          StreamWorker::<S>::create(egress, streams, stream_id)
      }
  ```

- [ ] **Step 4: The hub's egress**

Create `services/hub/stream_egress.rs`:

```rust
use crate::{
    models::messages::{HubMessage, StreamMessage},
    services::{command_stream::StreamEgress, HubService},
};
use futures::future::BoxFuture;
use uuid::Uuid;

impl StreamEgress for HubService {
    fn send(&self, connection_id: Uuid, message: StreamMessage) -> BoxFuture<'_, bool> {
        Box::pin(async move {
            match self.send_to_connection(connection_id, HubMessage::Stream(message)).await {
                Ok(queued) => queued,
                Err(err) => {
                    tracing::warn!("[{connection_id}] Failed to send a stream message: {err:#?}");
                    false
                }
            }
        })
    }
}
```

`send_to_connection` returns `Ok(false)` when the channel is full, and the connection is then dropped as `slow_consumer`. That is exactly when the worker must stop sending to it. In `services/hub/mod.rs`, add `mod stream_egress;`.

- [ ] **Step 5: Dispatcher**

`services/hub/tasks/stream_dispatcher.rs`:

```rust
use crate::{
    models::messages::{HubEvent, HubMessage, StreamEvent, TopicKey},
    services::{
        command_stream::{CommandStreamService, StreamEgress, WorkerCommand, WorkerHandle},
        HubService,
    },
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, Notify},
    task::{self, JoinError, JoinHandle, JoinSet},
    time,
};
use uuid::Uuid;

/// Min time between two starts of a stream's worker, so a worker that panics on start cannot spin.
const RESPAWN_DELAY: Duration = Duration::from_secs(1);

/// stream → worker wake, read by the Redis listener callback, which must never block or await.
#[derive(Clone, Default)]
struct WakeMap(Arc<RwLock<HashMap<Uuid, Arc<Notify>>>>);

impl WakeMap {
    fn insert(&self, stream_id: Uuid, wake: Arc<Notify>) {
        self.0.write().unwrap_or_else(|p| p.into_inner()).insert(stream_id, wake);
    }

    fn remove(&self, stream_id: Uuid) {
        self.0.write().unwrap_or_else(|p| p.into_inner()).remove(&stream_id);
    }

    /// Wakes one stream, or every stream on `None` (listener reconnect).
    fn wake(&self, stream_id: Option<Uuid>) {
        let map = self.0.read().unwrap_or_else(|p| p.into_inner());
        match stream_id {
            Some(stream_id) => {
                if let Some(wake) = map.get(&stream_id) {
                    wake.notify_one();
                }
            }
            None => map.values().for_each(|wake| wake.notify_one()),
        }
    }
}

struct Resident {
    kind: &'static str,
    handle: WorkerHandle,
    task: task::Id,
    subscribers: HashSet<Uuid>,
    idle_since: Option<Instant>,
    spawned_at: Instant,
    /// The worker ended while the stream was still resident; restarted by `on_tick`.
    dead: bool,
}

/// Hub task owning stream residency on this instance: one worker per stream with local subscribers.
pub struct StreamDispatcher {
    egress: Arc<dyn StreamEgress>,
    streams: CommandStreamService,
    wakes: WakeMap,
    residents: HashMap<Uuid, Resident>,
    by_connection: HashMap<Uuid, HashSet<Uuid>>,
    workers: JoinSet<()>,
    sweep: Option<JoinHandle<()>>,
    last_sweep: Instant,
}

impl Drop for StreamDispatcher {
    /// Detaches the workers instead of aborting them: also when the hub aborts this task first, each
    /// worker sees its command channel close and runs its final persist.
    fn drop(&mut self) {
        self.workers.detach_all();
    }
}

impl StreamDispatcher {
    /// Subscribes to the hub events before returning, so no subscription is missed.
    pub async fn start(hub: HubService, streams: CommandStreamService) -> JoinHandle<()> {
        let subscription = hub.subscribe(vec![TopicKey::Hub, TopicKey::Stream]).await;
        let wakes = WakeMap::default();
        let listener = wakes.clone();
        if let Err(err) = streams.listen_to_wakes(move |stream_id| listener.wake(stream_id)).await {
            tracing::error!("Failed to listen for stream wakes; falling back to periodic sync only: {err:#?}");
        }
        let dispatcher = Self {
            egress: Arc::new(hub),
            streams,
            wakes,
            residents: HashMap::new(),
            by_connection: HashMap::new(),
            workers: JoinSet::new(),
            sweep: None,
            last_sweep: Instant::now(),
        };
        tokio::spawn(dispatcher.run(subscription))
    }

    async fn run(mut self, mut subscription: mpsc::UnboundedReceiver<HubMessage>) {
        let mut ticker = time::interval(self.streams.settings().sync_interval);
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                biased;
                _ = ticker.tick() => self.on_tick(),
                message = subscription.recv() => match message {
                    Some(HubMessage::StreamEvent(StreamEvent::Subscribed { connection_id, stream_id, kind })) => {
                        self.subscribe(connection_id, stream_id, kind)
                    }
                    Some(HubMessage::StreamEvent(StreamEvent::Unsubscribed { connection_id, stream_id })) => {
                        self.unsubscribe(connection_id, stream_id)
                    }
                    Some(HubMessage::Hub(HubEvent::UserDisconnected { connection_id, .. })) => self.drop_connection(connection_id),
                    Some(HubMessage::Hub(HubEvent::Shutdown)) | None => break,
                    Some(_) => {}
                },
                Some(exited) = self.workers.join_next_with_id() => self.on_worker_exit(exited),
            }
        }
        if let Some(sweep) = self.sweep.take() {
            sweep.abort();
        }
        tracing::info!("Stream dispatcher stopped");
        // Dropping `self` detaches the workers and closes their command channels: they persist and exit.
    }

    fn spawn_worker(&mut self, stream_id: Uuid, kind: &'static str) -> Option<Resident> {
        let Some(stream_kind) = self.streams.kinds().get(kind) else {
            tracing::error!("[{stream_id}] Unknown stream kind {kind}");
            return None;
        };
        let (handle, future) = stream_kind.create_worker(self.egress.clone(), self.streams.clone(), stream_id);
        let task = self.workers.spawn(future).id();
        self.wakes.insert(stream_id, handle.wake.clone());
        Some(Resident {
            kind,
            handle,
            task,
            subscribers: HashSet::new(),
            idle_since: None,
            spawned_at: Instant::now(),
            dead: false,
        })
    }

    fn subscribe(&mut self, connection_id: Uuid, stream_id: Uuid, kind: &'static str) {
        if !self.residents.contains_key(&stream_id) {
            let Some(resident) = self.spawn_worker(stream_id, kind) else {
                return;
            };
            self.residents.insert(stream_id, resident);
        }
        let Some(resident) = self.residents.get_mut(&stream_id) else {
            return;
        };
        resident.subscribers.insert(connection_id);
        resident.idle_since = None;
        let _ = resident.handle.tx.send(WorkerCommand::AddSubscriber(connection_id));
        self.by_connection.entry(connection_id).or_default().insert(stream_id);
    }

    fn unsubscribe(&mut self, connection_id: Uuid, stream_id: Uuid) {
        if let Some(streams) = self.by_connection.get_mut(&connection_id) {
            streams.remove(&stream_id);
            if streams.is_empty() {
                self.by_connection.remove(&connection_id);
            }
        }
        if let Some(resident) = self.residents.get_mut(&stream_id) {
            if resident.subscribers.remove(&connection_id) {
                let _ = resident.handle.tx.send(WorkerCommand::RemoveSubscriber(connection_id));
            }
            if resident.subscribers.is_empty() && resident.idle_since.is_none() {
                resident.idle_since = Some(Instant::now());
            }
        }
    }

    fn drop_connection(&mut self, connection_id: Uuid) {
        for stream_id in self.by_connection.remove(&connection_id).unwrap_or_default() {
            self.unsubscribe(connection_id, stream_id);
        }
    }

    fn on_tick(&mut self) {
        self.respawn_dead();
        let grace = self.streams.settings().unload_grace;
        let idle: Vec<Uuid> = self
            .residents
            .iter()
            .filter(|(_, r)| r.idle_since.is_some_and(|since| since.elapsed() >= grace))
            .map(|(stream_id, _)| *stream_id)
            .collect();
        for stream_id in idle {
            if let Some(resident) = self.residents.remove(&stream_id) {
                self.wakes.remove(stream_id);
                let _ = resident.handle.tx.send(WorkerCommand::Stop);
                tracing::debug!("[{stream_id}] Stream unloaded");
            }
        }

        let sweep_due = self.last_sweep.elapsed() >= self.streams.settings().orphan_sweep_interval;
        let sweep_idle = self.sweep.as_ref().is_none_or(|sweep| sweep.is_finished());
        if sweep_due && sweep_idle {
            self.last_sweep = Instant::now();
            let streams = self.streams.clone();
            self.sweep = Some(tokio::spawn(async move { streams.sweep_orphans().await }));
        }
    }

    /// Marks the worker of a still-resident stream dead; `on_tick` restarts it.
    fn on_worker_exit(&mut self, exited: Result<(task::Id, ()), JoinError>) {
        let (id, panicked) = match exited {
            Ok((id, ())) => (id, false),
            Err(err) => (err.id(), err.is_panic()),
        };
        // An unloaded or replaced worker is not a resident anymore.
        let Some((stream_id, resident)) = self.residents.iter_mut().find(|(_, r)| r.task == id) else {
            return;
        };
        if panicked {
            tracing::error!("[{stream_id}] Stream worker panicked");
        } else {
            tracing::warn!("[{stream_id}] Stream worker ended unexpectedly");
        }
        resident.dead = true;
    }

    /// Restarts dead workers, at most once per `RESPAWN_DELAY` per stream.
    fn respawn_dead(&mut self) {
        let due: Vec<Uuid> = self
            .residents
            .iter()
            .filter(|(_, r)| r.dead && r.spawned_at.elapsed() >= RESPAWN_DELAY)
            .map(|(stream_id, _)| *stream_id)
            .collect();
        for stream_id in due {
            let Some(old) = self.residents.remove(&stream_id) else {
                continue;
            };
            self.wakes.remove(stream_id);
            if old.subscribers.is_empty() {
                continue;
            }
            if let Some(mut resident) = self.spawn_worker(stream_id, old.kind) {
                for connection_id in &old.subscribers {
                    let _ = resident.handle.tx.send(WorkerCommand::AddSubscriber(*connection_id));
                }
                resident.subscribers = old.subscribers;
                self.residents.insert(stream_id, resident);
            }
        }
    }
}
```

Notes:
- `tokio::select!` drops the branch futures before running a handler (tokio's macro polls in an inner scope). So a handler may borrow `self` mutably even though the `join_next_with_id` branch borrows `self.workers`.
- An empty `JoinSet` resolves to `None`, which disables that branch for the round.

In `tasks/mod.rs`, add `mod stream_dispatcher;` and `stream_dispatcher::StreamDispatcher` to the `pub use`.

- [ ] **Step 6: Start it from the hub**

In `hub_service.rs`:
- Add the parameter `command_streams: Option<CommandStreamService>` as the last parameter of `HubService::new`.
- Import `crate::services::CommandStreamService` and `tasks::StreamDispatcher`.
- Make the background task list mutable, and append the dispatcher **before** `Self::start_dispatcher(...)`:

```rust
        let mut background_tasks = vec![
            // … the existing entries, unchanged …
        ];
        if let Some(command_streams) = command_streams {
            background_tasks.push(StreamDispatcher::start(service.clone(), command_streams).await);
        }
        Self::start_dispatcher(service.clone(), control_rx, background_tasks);
```

Pass the new argument at both call sites:
- `test_utils.rs` (`create_test_hub`): pass `None`.
- `app_state.rs`: pass `Some(command_stream_service.clone())`.

- [ ] **Step 7: Verify**

- Run `cargo test -p shine-builder a_failed_send_stops_all_later_frames`. Expected: PASS (no env vars needed).
- Run `cargo build -p shine-builder`. Expected: it builds, with no warnings from the new files.
- Run `cargo test -p shine-builder` with `SHINE_TEST_REDIS_CNS` set. Expected: all existing tests PASS.

- [ ] **Step 8: Checkpoint.** Report and wait.

---

## Task 8: Integration tests for workers (Redis + Postgres)

**Files:**
- Create: `services/builder/src/services/command_stream/command_stream_test.rs`
- Modify: `services/builder/src/services/command_stream/mod.rs` (register the test module), `services/builder/src/services/hub/test_utils.rs` (a `command_streams` option)

Each test uses its own Redis key prefix, so its keys, activity set and wake channel never mix with another test's. Each simulated instance has its own Redis pool, and with it its own pub/sub listener. Two hubs sharing one listener would collide on the same channels.

- [ ] **Step 1: Hub harness option**

In `services/hub/test_utils.rs`:
- Add the field `pub command_streams: Option<CommandStreamService>` to `TestHubOptions`, and `command_streams: None` to its `Default` impl.
- In `create_test_hub`, pass `options.command_streams` as the new last argument of `HubService::new`. It replaces the `None` from Task 7.
- Import `crate::services::CommandStreamService`.

- [ ] **Step 2: Write the tests**

`command_stream_test.rs`, registered in `command_stream/mod.rs` with `#[cfg(test)] mod command_stream_test;`:

```rust
use crate::{
    app_config::CommandStreamConfig,
    models::messages::{HubMessage, StreamMessage},
    repositories::command_stream::redis::{RedisCommandStreamDb, RedisStreamConfig},
    services::{
        command_stream::{AllowAll, CommandStreamService, IngestOutcome, StreamKind, StreamKindRegistry, StreamStores},
        hub_test_utils::{connect, create_test_hub, Egress, TestHub, TestHubOptions},
    },
    settings::CommandStreamSettings,
};
use shine_game::replay::{encode_snapshot, stable_hash_of, CommandEnvelope, DemoCommand, IntArray, Replica};
use shine_infra::db::{
    self,
    command_log::{PgCommandLogDb, StoredCommand},
    redis::RedisConnectionPool,
};
use shine_test::test;
use std::{
    env,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::OnceCell,
    time::{sleep, timeout},
};
use uuid::Uuid;

const SNAPSHOT_EVERY: i64 = 5;
static MIGRATED: OnceCell<()> = OnceCell::const_new();

struct Env {
    redis_cns: String,
    /// The Redis key prefix of this test.
    prefix: String,
    /// For raw Redis edits that simulate failures.
    admin: RedisConnectionPool,
    log: PgCommandLogDb,
}

impl Env {
    fn key(&self, stream_id: Uuid, suffix: &str) -> String {
        format!("{}:{stream_id}:{suffix}", self.prefix)
    }
}

struct Instance {
    test: TestHub,
    streams: CommandStreamService,
}

async fn env_or_skip() -> Option<Env> {
    let (Ok(redis_cns), Ok(pg_cns)) = (env::var("SHINE_TEST_REDIS_CNS"), env::var("SHINE_TEST_PG_CNS")) else {
        tracing::warn!("SHINE_TEST_REDIS_CNS/SHINE_TEST_PG_CNS not set, skipping test");
        return None;
    };
    let _ = rustls::crypto::ring::default_provider().install_default();
    let pg_pool = db::postgres::create_postgres_pool(&pg_cns).await.unwrap();
    MIGRATED
        .get_or_init(|| async {
            let mut client = pg_pool.get().await.unwrap();
            client
                .migrate("command_log", &PgCommandLogDb::migrations())
                .await
                .unwrap();
        })
        .await;
    Some(Env {
        admin: db::redis::create_redis_pool(&redis_cns).await.unwrap(),
        redis_cns,
        prefix: format!("cs-test-{}", Uuid::new_v4()),
        log: PgCommandLogDb::new(&pg_pool).await.unwrap(),
    })
}

/// A simulated service instance: a hub with a stream dispatcher, on its own Redis pool.
async fn instance(env: &Env) -> Instance {
    let settings = CommandStreamSettings::from(&CommandStreamConfig {
        snapshot_every: SNAPSHOT_EVERY,
        sync_interval_ms: 100,
        persist_delay_ms: 5,
        redis_ttl_seconds: 600,
        ..CommandStreamConfig::default()
    });
    let redis_pool = db::redis::create_redis_pool(&env.redis_cns).await.unwrap();
    let redis = RedisCommandStreamDb::new(
        &redis_pool,
        RedisStreamConfig {
            prefix: env.prefix.clone(),
            keep_commands: settings.redis_keep_commands,
            max_unpersisted: settings.redis_max_unpersisted,
            ttl_seconds: settings.redis_ttl_seconds,
        },
    );
    let streams = CommandStreamService::new(
        StreamStores {
            redis,
            log: env.log.clone(),
            settings,
        },
        StreamKindRegistry::new().with::<IntArray>(),
        Arc::new(AllowAll),
    );
    let test = create_test_hub(TestHubOptions {
        redis_cns: Some(env.redis_cns.clone()),
        command_streams: Some(streams.clone()),
        ..TestHubOptions::default()
    })
    .await
    .unwrap();
    Instance { test, streams }
}

/// Connects a fake socket, opens and subscribes the stream, and waits for its initial `Sync`; returns
/// the kind, the user and the egress.
async fn subscribe(instance: &Instance, stream_id: Uuid) -> (Arc<dyn StreamKind>, Uuid, Egress) {
    let user_id = Uuid::new_v4();
    let (connection_id, mut egress) = connect(&instance.test.hub, user_id, 4096);
    let opened = instance.streams.open(user_id, stream_id, "demo").await.unwrap();
    instance
        .test
        .hub
        .request_subscribe_stream(user_id, connection_id, stream_id, opened.kind.kind())
        .unwrap();
    // Every test stream is new, so its Sync is empty: later frames carry only the test's commands.
    let synced = collect_until(&mut egress, Duration::from_secs(10), |m| {
        m.iter().any(|m| matches!(m, StreamMessage::Sync { .. }))
    })
    .await;
    assert!(
        synced.iter().any(|m| matches!(m, StreamMessage::Sync { .. })),
        "no initial Sync"
    );
    (opened.kind, user_id, egress)
}

async fn ingest(instance: &Instance, kind: &dyn StreamKind, stream_id: Uuid, user_id: Uuid, command_id: Uuid, command: &DemoCommand) -> IngestOutcome {
    let data = serde_json::to_string(command).unwrap();
    instance.streams.ingest(kind, stream_id, command_id, user_id, &data).await.unwrap()
}

/// Ingests until accepted, while the stream is being rebuilt; returns the version.
async fn ingest_until_accepted(instance: &Instance, kind: &dyn StreamKind, stream_id: Uuid, user_id: Uuid, command: &DemoCommand) -> i64 {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match ingest(instance, kind, stream_id, user_id, Uuid::new_v4(), command).await {
            IngestOutcome::Accepted { version } => return version,
            IngestOutcome::NotReady | IngestOutcome::Busy => {
                assert!(Instant::now() < deadline, "the stream did not recover");
                sleep(Duration::from_millis(50)).await;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}

/// Collects stream messages until `done` returns true or the window ends.
async fn collect_until(egress: &mut Egress, window: Duration, mut done: impl FnMut(&[StreamMessage]) -> bool) -> Vec<StreamMessage> {
    let deadline = Instant::now() + window;
    let mut messages = Vec::new();
    while !done(&messages) {
        let left = deadline.saturating_duration_since(Instant::now());
        match timeout(left, egress.recv()).await {
            Ok(Some(HubMessage::Stream(message))) => messages.push(message),
            Ok(Some(_)) => {}
            _ => break,
        }
    }
    messages
}

fn checkpoint_hash(messages: &[StreamMessage], version: i64) -> Option<u64> {
    messages.iter().rev().find_map(|m| match m {
        StreamMessage::Checkpoint { version: v, hash, .. } if *v == version => Some(*hash),
        _ => None,
    })
}

fn delivered_versions(messages: &[StreamMessage]) -> Vec<i64> {
    messages
        .iter()
        .flat_map(|m| match m {
            StreamMessage::Sync { commands, .. } | StreamMessage::Commands { commands, .. } => {
                commands.iter().map(|c| c.version).collect()
            }
            _ => vec![],
        })
        .collect()
}

async fn wait_persisted_within(log: &PgCommandLogDb, stream_id: Uuid, version: i64, window: Duration) {
    let deadline = Instant::now() + window;
    while log.head(stream_id).await.unwrap().unwrap().version < version {
        assert!(Instant::now() < deadline, "version {version} was not persisted in time");
        sleep(Duration::from_millis(20)).await;
    }
}

async fn wait_persisted(log: &PgCommandLogDb, stream_id: Uuid, version: i64) {
    wait_persisted_within(log, stream_id, version, Duration::from_secs(5)).await
}

fn expected_hash(commands: &[DemoCommand], base: IntArray, from_version: i64) -> u64 {
    let mut replica = Replica::from_snapshot(from_version, base);
    for (i, command) in commands.iter().enumerate() {
        replica
            .apply_next(&CommandEnvelope {
                version: from_version + 1 + i as i64,
                command_id: Uuid::nil(),
                user_id: Uuid::nil(),
                command: Some(command.clone()),
            })
            .unwrap();
    }
    replica.hash()
}

#[test]
async fn two_instances_converge_on_the_same_checkpoints() {
    let Some(env) = env_or_skip().await else { return };
    let (a, b) = (instance(&env).await, instance(&env).await);
    let stream_id = Uuid::new_v4();
    let (kind, user_a, mut rx_a) = subscribe(&a, stream_id).await;
    let (_, _, mut rx_b) = subscribe(&b, stream_id).await;

    let commands: Vec<DemoCommand> = (0..10)
        .map(|i| match i % 3 {
            0 => DemoCommand::Add { idx: i % 10, delta: 2 },
            1 => DemoCommand::Set { idx: 99, value: 1 }, // rejected by replay, still versioned
            _ => DemoCommand::Swap { a: 0, b: i % 10 },
        })
        .collect();
    for command in &commands {
        assert!(matches!(
            ingest(&a, kind.as_ref(), stream_id, user_a, Uuid::new_v4(), command).await,
            IngestOutcome::Accepted { .. }
        ));
    }

    let done = |m: &[StreamMessage]| checkpoint_hash(m, 10).is_some();
    let messages_a = collect_until(&mut rx_a, Duration::from_secs(10), done).await;
    let messages_b = collect_until(&mut rx_b, Duration::from_secs(10), done).await;
    let expected = expected_hash(&commands, IntArray::default(), 0);
    assert_eq!(checkpoint_hash(&messages_a, 10), Some(expected));
    assert_eq!(checkpoint_hash(&messages_b, 10), Some(expected));
    assert_eq!(delivered_versions(&messages_a), (1..=10).collect::<Vec<_>>());
    assert_eq!(delivered_versions(&messages_b), (1..=10).collect::<Vec<_>>());
}

#[test]
async fn drifted_worker_rebases_onto_authoritative_snapshot() {
    let Some(env) = env_or_skip().await else { return };
    let a = instance(&env).await;
    let stream_id = Uuid::new_v4();
    let (kind, user_id, mut rx) = subscribe(&a, stream_id).await;

    for i in 0..4 {
        ingest(&a, kind.as_ref(), stream_id, user_id, Uuid::new_v4(), &DemoCommand::Set { idx: i, value: i as i64 + 1 }).await;
    }
    wait_persisted(&env.log, stream_id, 4).await;

    // Another "instance" stored version 5 and a different snapshot at 5 first.
    let generation = a.streams.stores().redis.meta(stream_id).await.unwrap().unwrap().generation;
    let command_5 = DemoCommand::Set { idx: 4, value: 5 };
    let command_id_5 = Uuid::new_v4();
    env.log
        .append(
            stream_id,
            &[StoredCommand {
                version: 5,
                command_id: command_id_5,
                user_id,
                generation,
                data: serde_json::to_string(&command_5).unwrap(),
            }],
        )
        .await
        .unwrap();
    let bogus = IntArray { values: [999; 10] };
    env.log
        .try_store_snapshot(stream_id, 5, stable_hash_of(&bogus), &encode_snapshot(&bogus).unwrap())
        .await
        .unwrap();
    // Redis assigns version 5 to the same command id, so the worker's append is UpToDate.
    assert_eq!(
        ingest(&a, kind.as_ref(), stream_id, user_id, command_id_5, &command_5).await,
        IngestOutcome::Accepted { version: 5 }
    );
    let messages = collect_until(&mut rx, Duration::from_secs(10), |m| checkpoint_hash(m, 5).is_some()).await;
    assert_eq!(checkpoint_hash(&messages, 5), Some(stable_hash_of(&bogus)));

    let rest: Vec<DemoCommand> = (0..5).map(|_| DemoCommand::Add { idx: 0, delta: 1 }).collect();
    for command in &rest {
        ingest(&a, kind.as_ref(), stream_id, user_id, Uuid::new_v4(), command).await;
    }
    let messages = collect_until(&mut rx, Duration::from_secs(10), |m| checkpoint_hash(m, 10).is_some()).await;
    assert_eq!(checkpoint_hash(&messages, 10), Some(expected_hash(&rest, bogus, 5)));
}

#[test]
async fn redis_loss_continues_from_postgres_head() {
    let Some(env) = env_or_skip().await else { return };
    let a = instance(&env).await;
    let stream_id = Uuid::new_v4();
    let (kind, user_id, mut rx) = subscribe(&a, stream_id).await;
    for i in 0..3 {
        ingest(&a, kind.as_ref(), stream_id, user_id, Uuid::new_v4(), &DemoCommand::Set { idx: i, value: 1 }).await;
    }
    wait_persisted(&env.log, stream_id, 3).await;
    // The worker learns of the persistence through the Redis meta it marked.
    let deadline = Instant::now() + Duration::from_secs(5);
    while a.streams.stores().redis.meta(stream_id).await.unwrap().unwrap().persisted < 3 {
        assert!(Instant::now() < deadline, "persisted mark missing");
        sleep(Duration::from_millis(20)).await;
    }

    // Redis forgets the stream.
    let mut conn = env.admin.get().await.unwrap();
    let _: () = redis::cmd("DEL")
        .arg(env.key(stream_id, "meta"))
        .arg(env.key(stream_id, "log"))
        .arg(env.key(stream_id, "ids"))
        .query_async(&mut *conn)
        .await
        .unwrap();
    drop(conn);

    let version = ingest_until_accepted(&a, kind.as_ref(), stream_id, user_id, &DemoCommand::Set { idx: 5, value: 5 }).await;
    assert_eq!(version, 4, "versions continue after the Postgres head");

    let messages = collect_until(&mut rx, Duration::from_secs(10), |m| delivered_versions(m).contains(&4)).await;
    let versions = delivered_versions(&messages);
    let mut deduped = versions.clone();
    deduped.dedup();
    assert_eq!(versions, deduped, "no version is delivered twice");
    assert!(!messages.iter().any(|m| matches!(m, StreamMessage::Reset { .. })), "nothing was lost, no reset");
}

#[test]
async fn redis_rolled_back_within_a_generation_is_repaired() {
    // A restart from an older dump: same generation, lower version than Postgres.
    let Some(env) = env_or_skip().await else { return };
    let a = instance(&env).await;
    let stream_id = Uuid::new_v4();
    let (kind, user_id, mut rx) = subscribe(&a, stream_id).await;
    for i in 0..5 {
        ingest(&a, kind.as_ref(), stream_id, user_id, Uuid::new_v4(), &DemoCommand::Set { idx: i, value: 1 }).await;
    }
    wait_persisted(&env.log, stream_id, 5).await;
    let originals = env.log.commands_after(stream_id, 0, 100).await.unwrap();

    let mut conn = env.admin.get().await.unwrap();
    let _: () = redis::pipe()
        .cmd("HSET")
        .arg(env.key(stream_id, "meta"))
        .arg("version")
        .arg(3)
        .arg("persisted")
        .arg(3)
        .ignore()
        // A restored log has its old last id: drop it, entries below 4 are not needed here.
        .cmd("DEL")
        .arg(env.key(stream_id, "log"))
        .ignore()
        .query_async(&mut *conn)
        .await
        .unwrap();
    drop(conn);

    // This write gets the reused version 4 in the rolled-back copy; the worker's next read sees
    // Redis behind its replica and drops the copy.
    ingest(&a, kind.as_ref(), stream_id, user_id, Uuid::new_v4(), &DemoCommand::Set { idx: 9, value: 9 }).await;
    let messages = collect_until(&mut rx, Duration::from_secs(10), |m| m.iter().any(|m| matches!(m, StreamMessage::Reset { .. }))).await;
    assert!(messages.iter().any(|m| matches!(m, StreamMessage::Reset { .. })), "clients are told to resync");

    let version = ingest_until_accepted(&a, kind.as_ref(), stream_id, user_id, &DemoCommand::Set { idx: 8, value: 8 }).await;
    assert_eq!(version, 6, "versions continue after the Postgres head");
    wait_persisted(&env.log, stream_id, 6).await;
    let stored = env.log.commands_after(stream_id, 0, 100).await.unwrap();
    assert_eq!(&stored[..5], &originals[..], "persisted history is untouched");
    assert_eq!(stored.len(), 6);
}

#[test]
async fn a_reassigned_copy_that_caught_up_is_detected() {
    // A failover to a lagging replica: the copy reassigns versions 4.. and catches up past the worker
    // (here at 5) before its next read. Only the content check at the worker's version can see it.
    let Some(env) = env_or_skip().await else { return };
    let a = instance(&env).await;
    let stream_id = Uuid::new_v4();
    let (kind, user_id, mut rx) = subscribe(&a, stream_id).await;
    for i in 0..5 {
        ingest(&a, kind.as_ref(), stream_id, user_id, Uuid::new_v4(), &DemoCommand::Set { idx: i, value: 1 }).await;
    }
    wait_persisted(&env.log, stream_id, 5).await;
    let generation = a.streams.stores().redis.meta(stream_id).await.unwrap().unwrap().generation;

    // Rebuild the copy at version 3, then add 4..6 with other commands, without waking anyone.
    let mut conn = env.admin.get().await.unwrap();
    let mut pipe = redis::pipe();
    pipe.cmd("DEL").arg(env.key(stream_id, "log")).ignore();
    for version in 4..=6 {
        pipe.cmd("XADD")
            .arg(env.key(stream_id, "log"))
            .arg(format!("{version}-0"))
            .arg("g")
            .arg(generation)
            .arg("c")
            .arg(Uuid::new_v4().to_string())
            .arg("u")
            .arg(Uuid::nil().to_string())
            .arg("d")
            .arg(serde_json::to_string(&DemoCommand::Set { idx: 9, value: version }).unwrap())
            .ignore();
    }
    pipe.cmd("HSET")
        .arg(env.key(stream_id, "meta"))
        .arg("version")
        .arg(6)
        .arg("persisted")
        .arg(3)
        .ignore();
    let _: () = pipe.query_async(&mut *conn).await.unwrap();
    drop(conn);

    // The next write wakes the worker: Redis is ahead of it, but holds another command at version 5.
    ingest(&a, kind.as_ref(), stream_id, user_id, Uuid::new_v4(), &DemoCommand::Set { idx: 8, value: 8 }).await;
    let messages = collect_until(&mut rx, Duration::from_secs(10), |m| m.iter().any(|m| matches!(m, StreamMessage::Reset { .. }))).await;
    assert!(messages.iter().any(|m| matches!(m, StreamMessage::Reset { .. })), "the reassigned copy is dropped");

    let version = ingest_until_accepted(&a, kind.as_ref(), stream_id, user_id, &DemoCommand::Set { idx: 7, value: 7 }).await;
    assert_eq!(version, 6, "versions continue after the Postgres head");
    wait_persisted(&env.log, stream_id, 6).await;
    let stored = env.log.commands_after(stream_id, 0, 100).await.unwrap();
    assert_eq!(stored.len(), 6);
}

#[test]
async fn open_drops_a_redis_copy_behind_postgres() {
    let Some(env) = env_or_skip().await else { return };
    let a = instance(&env).await;
    let stream_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    a.streams.open(user_id, stream_id, "demo").await.unwrap();
    let redis = a.streams.stores().redis;
    let generation = redis.meta(stream_id).await.unwrap().unwrap().generation;

    // Postgres moves ahead of Redis (as after a Redis restart from an older dump).
    let rows: Vec<StoredCommand> = (1..=5)
        .map(|version| StoredCommand {
            version,
            command_id: Uuid::new_v4(),
            user_id,
            generation,
            data: serde_json::to_string(&DemoCommand::Add { idx: 0, delta: 1 }).unwrap(),
        })
        .collect();
    env.log.append(stream_id, &rows).await.unwrap();

    a.streams.open(user_id, stream_id, "demo").await.unwrap();
    let meta = redis.meta(stream_id).await.unwrap().unwrap();
    assert_eq!((meta.generation, meta.version), (generation + 1, 5));
}

#[test]
async fn redis_generation_behind_postgres_is_repaired() {
    let Some(env) = env_or_skip().await else { return };
    let a = instance(&env).await;
    let stream_id = Uuid::new_v4();
    let (kind, user_id, mut rx) = subscribe(&a, stream_id).await;
    ingest(&a, kind.as_ref(), stream_id, user_id, Uuid::new_v4(), &DemoCommand::Set { idx: 0, value: 1 }).await;
    wait_persisted(&env.log, stream_id, 1).await;

    // A hydrator bumped the generation and died before writing Redis.
    env.log.begin_generation(stream_id).await.unwrap();
    ingest(&a, kind.as_ref(), stream_id, user_id, Uuid::new_v4(), &DemoCommand::Set { idx: 1, value: 1 }).await;
    let messages = collect_until(&mut rx, Duration::from_secs(10), |m| m.iter().any(|m| matches!(m, StreamMessage::Reset { .. }))).await;
    assert!(messages.iter().any(|m| matches!(m, StreamMessage::Reset { .. })), "the unpersistable command is rolled back");

    let version = ingest_until_accepted(&a, kind.as_ref(), stream_id, user_id, &DemoCommand::Set { idx: 2, value: 2 }).await;
    wait_persisted(&env.log, stream_id, version).await;
}

/// Consumes stream messages until `version` is delivered; returns when it arrived.
async fn wait_delivered(mut egress: Egress, version: i64, window: Duration) -> Option<Instant> {
    let deadline = Instant::now() + window;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match timeout(left, egress.recv()).await {
            Ok(Some(HubMessage::Stream(message))) => {
                if delivered_versions(std::slice::from_ref(&message)).contains(&version) {
                    return Some(Instant::now());
                }
            }
            Ok(Some(_)) => {}
            _ => return None,
        }
    }
}

#[test(skip = "throughput probe, run explicitly")]
async fn single_stream_throughput() {
    let Some(env) = env_or_skip().await else { return };
    let (a, b) = (instance(&env).await, instance(&env).await);
    let stream_id = Uuid::new_v4();
    let (kind, _, mut rx_a) = subscribe(&a, stream_id).await;
    let (_, _, rx_b) = subscribe(&b, stream_id).await;
    const WRITERS: usize = 8;
    const PER_WRITER: usize = 1000;
    let total = (WRITERS * PER_WRITER) as i64;

    let started = Instant::now();
    ingest(&a, kind.as_ref(), stream_id, Uuid::new_v4(), Uuid::new_v4(), &DemoCommand::Add { idx: 0, delta: 1 }).await;
    collect_until(&mut rx_a, Duration::from_secs(5), |m| delivered_versions(m).contains(&1)).await;
    println!("idle round trip: {:?}", started.elapsed());

    // Drained concurrently: an undrained subscriber fills its channel and is dropped as slow.
    let last = total + 1;
    let delivered_a = tokio::spawn(wait_delivered(rx_a, last, Duration::from_secs(120)));
    let delivered_b = tokio::spawn(wait_delivered(rx_b, last, Duration::from_secs(120)));

    let started = Instant::now();
    let writers: Vec<_> = (0..WRITERS)
        .map(|_| {
            let (streams, kind) = (a.streams.clone(), kind.clone());
            tokio::spawn(async move {
                let user_id = Uuid::new_v4();
                let data = serde_json::to_string(&DemoCommand::Add { idx: 1, delta: 1 }).unwrap();
                for _ in 0..PER_WRITER {
                    loop {
                        match streams.ingest(kind.as_ref(), stream_id, Uuid::new_v4(), user_id, &data).await {
                            Ok(IngestOutcome::Accepted { .. }) => break,
                            Ok(IngestOutcome::Busy) => sleep(Duration::from_millis(5)).await,
                            other => return Err(format!("unexpected {other:?}")),
                        }
                    }
                }
                Ok(())
            })
        })
        .collect();
    for writer in writers {
        assert_eq!(writer.await.unwrap(), Ok(()));
    }
    let ingested = started.elapsed();
    let delivered = delivered_a.await.unwrap().expect("instance a did not deliver everything").duration_since(started);
    delivered_b.await.unwrap().expect("instance b did not deliver everything");
    wait_persisted_within(&env.log, stream_id, last, Duration::from_secs(120)).await;
    let persisted = started.elapsed();
    println!(
        "{total} commands, {WRITERS} writers: ingest {:.0}/s, delivered after {delivered:?}, persisted after {persisted:?} ({:.0}/s)",
        total as f64 / ingested.as_secs_f64(),
        total as f64 / persisted.as_secs_f64()
    );
}
```

`Egress` is `test_utils::Egress` (`mpsc::Receiver<HubMessage>`). `hub_test_utils` is the crate-visible re-export in `services/mod.rs`.

- [ ] **Step 3: Run**

Run with both env vars set (PowerShell): `cargo test -p shine-builder services::command_stream::command_stream_test -- --nocapture`. Use the full path: a bare `command_stream_test` also matches `redis_command_stream_test`.

Expected: 7 tests PASS, and the throughput probe is reported as ignored. If a test times out, debug with `RUST_LOG=shine_builder=debug` before changing any timeout, and never loosen an assertion.

Then run the probe once:

```powershell
cargo test -p shine-builder single_stream_throughput -- --ignored --nocapture
```

Paste its output into the checkpoint report, next to the plan's throughput model.

- [ ] **Step 4: Checkpoint.** Report and wait.

---

## Task 9: WebSocket protocol

Re-read `routes/ws/connect.rs`, `routes/ws/connect_test.rs` and `routes/ws/message.rs` first. They carry uncommitted user changes: `serve_socket`, `forward_egress`, and the close frames.

**Files:**
- Modify: `services/builder/src/routes/ws/message.rs`, `services/builder/src/routes/ws/connect.rs`, `services/builder/src/routes/ws/connect_test.rs`

The wire format follows the "Protocol rules" section at the top of this plan. Commands are JSON **strings** in both directions.

- [ ] **Step 1: Write the failing tests**

Append to `message.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;

    #[test]
    fn stream_command_request_carries_the_command_as_a_string() {
        let request: WSMessageRequest = serde_json::from_str(
            r#"{"type":"streamCommand","streamId":"6f9619ff-8b86-d011-b42d-00cf4fc964ff","commandId":"6f9619ff-8b86-d011-b42d-00cf4fc964fe","command":"{\"op\":\"set\",\"idx\":1,\"value\":9007199254740993}"}"#,
        )
        .unwrap();
        let WSMessageRequest::StreamCommand { command, .. } = request else {
            panic!("wrong variant");
        };
        assert_eq!(command, r#"{"op":"set","idx":1,"value":9007199254740993}"#, "no number is reparsed");
    }

    #[test]
    fn stream_commands_frame_carries_commands_as_strings() {
        let frame = WSMessageResponse::StreamCommands {
            stream_id: Uuid::nil(),
            generation: 1,
            commands: vec![WireCommand {
                version: 3,
                command_id: Uuid::nil(),
                user_id: Uuid::nil(),
                command: r#"{"op":"add","idx":0,"delta":1}"#.into(),
            }],
        };
        let json: serde_json::Value = serde_json::from_str(&serde_json::to_string(&frame).unwrap()).unwrap();
        assert_eq!(json["type"], "streamCommands");
        assert_eq!(json["streamId"], Uuid::nil().to_string());
        assert_eq!(json["commands"][0]["commandId"], Uuid::nil().to_string());
        assert_eq!(json["commands"][0]["command"], r#"{"op":"add","idx":0,"delta":1}"#);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run `cargo test -p shine-builder routes::ws::message`. Expected: compile errors.

- [ ] **Step 3: Wire types**

In `message.rs`, add `rename_all_fields = "camelCase"` to both enums. `rename_all` only renames variants, not the fields of struct variants. Add these variants and types:

```rust
// WSMessageRequest
    StreamSubscribe { stream_id: Uuid, kind: String },
    StreamUnsubscribe { stream_id: Uuid },
    /// `command` is the command JSON as a string, so a browser never parses its numbers.
    StreamCommand { stream_id: Uuid, command_id: Uuid, command: String },
```

```rust
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireSnapshot {
    pub version: i64,
    /// 16 hex digits.
    pub hash: String,
    /// Base64 postcard bytes.
    pub data: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireCommand {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    /// The canonical command JSON, as a string.
    pub command: String,
}

// WSMessageResponse
    StreamSync { stream_id: Uuid, generation: i64, snapshot: Option<WireSnapshot>, commands: Vec<WireCommand> },
    StreamCommands { stream_id: Uuid, generation: i64, commands: Vec<WireCommand> },
    StreamCheckpoint { stream_id: Uuid, version: i64, hash: String },
    StreamReset { stream_id: Uuid },
    StreamNack { stream_id: Uuid, command_id: Uuid, reason: &'static str },
    StreamSubscribeFailed { stream_id: Uuid, reason: &'static str },
```

- [ ] **Step 4: Egress mapping**

In `connect.rs` `event_to_wire_message`, replace the `HubMessage::Stream(_) | HubMessage::StreamEvent(_) => None` arm from Task 5 with:

```rust
        HubMessage::Stream(message) => Some(stream_to_wire(message)),
        HubMessage::StreamEvent(_) => None,
```

The imports this needs:
- `base64::{engine::general_purpose::STANDARD, Engine}`
- `crate::models::messages::{StreamCommand, StreamMessage}`
- `crate::routes::ws::message::{WireCommand, WireSnapshot}`

Add these helpers:

```rust
fn hash_hex(hash: u64) -> String {
    format!("{hash:016x}")
}

fn commands_to_wire(commands: &[StreamCommand]) -> Vec<WireCommand> {
    commands
        .iter()
        .map(|command| WireCommand {
            version: command.version,
            command_id: command.command_id,
            user_id: command.user_id,
            command: command.data.to_string(),
        })
        .collect()
}

fn stream_to_wire(message: StreamMessage) -> WSMessageResponse {
    match message {
        StreamMessage::Sync { stream_id, generation, snapshot, commands } => WSMessageResponse::StreamSync {
            stream_id,
            generation,
            snapshot: snapshot.map(|s| WireSnapshot {
                version: s.version,
                hash: hash_hex(s.hash),
                data: STANDARD.encode(&s.data),
            }),
            commands: commands_to_wire(&commands),
        },
        StreamMessage::Commands { stream_id, generation, commands } => WSMessageResponse::StreamCommands {
            stream_id,
            generation,
            commands: commands_to_wire(&commands),
        },
        StreamMessage::Checkpoint { stream_id, version, hash } => WSMessageResponse::StreamCheckpoint {
            stream_id,
            version,
            hash: hash_hex(hash),
        },
        StreamMessage::Reset { stream_id } => WSMessageResponse::StreamReset { stream_id },
        StreamMessage::Nack { stream_id, command_id, reason } => WSMessageResponse::StreamNack { stream_id, command_id, reason },
        StreamMessage::SubscribeFailed { stream_id, reason } => WSMessageResponse::StreamSubscribeFailed { stream_id, reason },
    }
}
```

- [ ] **Step 5: Ingress handling**

`serve_socket` gets a new parameter `stream_service: Option<CommandStreamService>` after `chat_service`. `None` turns streams off, which the Redis-only socket tests use. Wire it through:
- `connect` passes `Some(state.command_stream_service().clone())` to `handle_socket`.
- `handle_socket` passes it on.
- In `connect_test.rs`, add `None` after the `chat.clone()` argument at both `serve_socket(` calls.

Replace the body of the receive task with a receive context:

```rust
/// Per-socket receive state.
struct SocketReceiver {
    hub_service: HubService,
    chat_service: ChatService,
    stream_service: Option<CommandStreamService>,
    user_id: Uuid,
    connection_id: Uuid,
    /// Streams this socket subscribed to.
    streams: HashMap<Uuid, OpenedStream>,
}

impl SocketReceiver {
    async fn on_text(&mut self, text: &str) {
        let request = match serde_json::from_str::<WSMessageRequest>(text) {
            Ok(request) => request,
            Err(_) => {
                hub_metrics()
                    .ws_messages_received
                    .add(1, &[label("kind", "unknown"), label("outcome", "invalid")]);
                tracing::trace!("[{}] Received invalid message: {text}", self.user_id);
                return;
            }
        };
        match request {
            WSMessageRequest::Chat { text } => self.on_chat(text).await,
            WSMessageRequest::StreamSubscribe { stream_id, kind } => self.on_subscribe(stream_id, &kind).await,
            WSMessageRequest::StreamUnsubscribe { stream_id } => {
                if self.streams.remove(&stream_id).is_some() {
                    if let Err(err) = self
                        .hub_service
                        .request_unsubscribe_stream(self.user_id, self.connection_id, stream_id)
                    {
                        tracing::warn!("[{}] Failed to request a stream unsubscribe: {err:#?}", self.user_id);
                    }
                }
            }
            WSMessageRequest::StreamCommand { stream_id, command_id, command } => {
                self.on_command(stream_id, command_id, command).await
            }
        }
    }

    async fn on_chat(&self, text: String) {
        let span = info_span!(
            parent: None,
            "ws message",
            enduser.id = %self.user_id,
            ws.connection_id = %self.connection_id,
            ws.message.kind = "chat"
        );
        let appended = self
            .chat_service
            .append_comment(self.user_id, &text)
            .instrument(span)
            .await;
        let outcome = if appended.is_ok() { "ok" } else { "append_failed" };
        hub_metrics()
            .ws_messages_received
            .add(1, &[label("kind", "chat"), label("outcome", outcome)]);
        if let Err(err) = appended {
            tracing::error!("[{}] Failed to append chat message: {err:#?}", self.user_id);
        }
    }

    async fn on_subscribe(&mut self, stream_id: Uuid, kind: &str) {
        let Some(stream_service) = self.stream_service.clone() else {
            self.reply(StreamMessage::SubscribeFailed { stream_id, reason: "unavailable" });
            return;
        };
        let span = info_span!(
            parent: None,
            "ws message",
            enduser.id = %self.user_id,
            ws.connection_id = %self.connection_id,
            ws.message.kind = "stream_subscribe",
            stream.id = %stream_id
        );
        let opened = stream_service.open(self.user_id, stream_id, kind).instrument(span).await;
        let outcome = match opened {
            Ok(opened) => {
                let kind = opened.kind.kind();
                self.streams.insert(stream_id, opened);
                if let Err(err) = self
                    .hub_service
                    .request_subscribe_stream(self.user_id, self.connection_id, stream_id, kind)
                {
                    tracing::warn!("[{}] Failed to request a stream subscribe: {err:#?}", self.user_id);
                }
                "ok"
            }
            Err(err) => {
                if let OpenError::Failed(err) = &err {
                    tracing::warn!("[{}] Failed to open stream {stream_id}: {err:#?}", self.user_id);
                }
                let reason = err.reason();
                self.reply(StreamMessage::SubscribeFailed { stream_id, reason });
                reason
            }
        };
        hub_metrics()
            .ws_messages_received
            .add(1, &[label("kind", "stream_subscribe"), label("outcome", outcome)]);
    }

    async fn on_command(&self, stream_id: Uuid, command_id: Uuid, command: String) {
        let reason = match (&self.stream_service, self.streams.get(&stream_id)) {
            (None, _) => Some("unavailable"),
            (Some(_), None) => Some("notSubscribed"),
            (Some(_), Some(opened)) if !opened.can_write => Some("forbidden"),
            (Some(stream_service), Some(opened)) => match stream_service
                .ingest(opened.kind.as_ref(), stream_id, command_id, self.user_id, &command)
                .await
            {
                Ok(IngestOutcome::Accepted { .. }) => None,
                // duplicate, invalid, notReady, busy
                Ok(outcome) => Some(outcome.reason()),
                Err(err) => {
                    tracing::warn!("[{}] Failed to ingest a stream command: {err:#?}", self.user_id);
                    Some("failed")
                }
            },
        };
        if let Some(reason) = reason {
            self.reply(StreamMessage::Nack {
                stream_id,
                command_id,
                reason,
            });
        }
        hub_metrics()
            .ws_messages_received
            .add(1, &[label("kind", "stream_command"), label("outcome", reason.unwrap_or("ok"))]);
    }

    /// Replies through the hub's command loop, behind this connection's registration.
    fn reply(&self, message: StreamMessage) {
        if let Err(err) = self
            .hub_service
            .request_reply(self.connection_id, HubMessage::Stream(message))
        {
            tracing::warn!("[{}] Failed to reply on the websocket: {err:#?}", self.user_id);
        }
    }
}
```

The receive task becomes:

```rust
    let mut recv_task = {
        let mut receiver = SocketReceiver {
            hub_service: hub_service.clone(),
            chat_service,
            stream_service,
            user_id: current_user_id,
            connection_id,
            streams: HashMap::new(),
        };
        tokio::spawn(async move {
            while let Some(Ok(message)) = ws_receiver.next().await {
                tracing::debug!("[{current_user_id}] WsMessage received");
                if let WsMessage::Text(text) = message {
                    receiver.on_text(&text).await;
                }
            }
        })
    };
```

Two details for the implementer:
- Ingest stays awaited inline, so a connection's commands keep their order.
- Add imports `std::collections::HashMap` and `crate::services::command_stream::{CommandStreamService, IngestOutcome, OpenError, OpenedStream}`. Remove the now-unused chat-specific `match` imports, if any.

- [ ] **Step 6: Verify**

- Run `cargo test -p shine-builder routes::ws`: the 2 new tests and the existing `connect_test` tests PASS, the latter with `SHINE_TEST_REDIS_CNS`.
- Run `cargo fmt --all`, `cargo build -p shine-builder` and `cargo clippy -p shine-builder -- -D warnings`. Expected: all clean.

- [ ] **Step 7: Checkpoint.** Report and wait.

---

## Task 10: API tests

Invoke the `api-test-writing` skill first, then the `local-development` skill to run the builder locally.

**Files:**
- Create: `tests/src/api/builder_stream_ws.ts`, `tests/api-tests/builder/command_stream.ts`

- [ ] **Step 1: Client helper**

`tests/src/api/builder_stream_ws.ts`:

```ts
import { DEFAULT_USER_AGENT, joinURL } from '$lib/utils';
import { IncomingMessage } from 'node:http';
import { WebSocket } from 'ws';

// Mirrors the stream variants of WSMessageRequest/WSMessageResponse in services/builder/src/routes/ws/message.rs.
// `command` travels as a JSON string; the tests only use small integers, so JSON.parse is exact here.
export type DemoCommand =
    | { op: 'set'; idx: number; value: number }
    | { op: 'add'; idx: number; delta: number }
    | { op: 'swap'; a: number; b: number }
    | { op: 'compareAndSet'; idx: number; expected: number; value: number }
    | { op: 'batch'; commands: DemoCommand[] };

export type WireCommand = { version: number; commandId: string; userId: string; command: string };
export type WireSnapshot = { version: number; hash: string; data: string };
export type StreamFrame =
    | { type: 'streamSync'; streamId: string; generation: number; snapshot: WireSnapshot | null; commands: WireCommand[] }
    | { type: 'streamCommands'; streamId: string; generation: number; commands: WireCommand[] }
    | { type: 'streamCheckpoint'; streamId: string; version: number; hash: string }
    | { type: 'streamReset'; streamId: string }
    | { type: 'streamNack'; streamId: string; commandId: string; reason: string }
    | { type: 'streamSubscribeFailed'; streamId: string; reason: string };

export const DEMO_LEN = 10;
const I64_MIN = -(1n << 63n);
const I64_MAX = (1n << 63n) - 1n;

export function parseCommand(command: WireCommand): DemoCommand {
    return JSON.parse(command.command) as DemoCommand;
}

/** Applies a demo command; null when it is rejected (mirrors IntArray::apply). */
export function applyDemo(values: bigint[], command: DemoCommand): bigint[] | null {
    const next = [...values];
    const inRange = (i: number) => Number.isInteger(i) && i >= 0 && i < DEMO_LEN;
    const step = (c: DemoCommand): boolean => {
        switch (c.op) {
            case 'set':
                if (!inRange(c.idx)) return false;
                next[c.idx] = BigInt(c.value);
                return true;
            case 'add': {
                if (!inRange(c.idx)) return false;
                const sum = next[c.idx] + BigInt(c.delta);
                if (sum < I64_MIN || sum > I64_MAX) return false;
                next[c.idx] = sum;
                return true;
            }
            case 'swap':
                if (!inRange(c.a) || !inRange(c.b)) return false;
                [next[c.a], next[c.b]] = [next[c.b], next[c.a]];
                return true;
            case 'compareAndSet':
                if (!inRange(c.idx) || next[c.idx] !== BigInt(c.expected)) return false;
                next[c.idx] = BigInt(c.value);
                return true;
            case 'batch':
                return c.commands.every(step);
        }
    };
    return step(command) ? next : null;
}

/** Decodes a postcard IntArray snapshot: ten zigzag varint i64 values. */
export function decodeIntArray(base64: string): bigint[] {
    const bytes = Buffer.from(base64, 'base64');
    let pos = 0;
    const values: bigint[] = [];
    for (let i = 0; i < DEMO_LEN; i++) {
        let raw = 0n;
        let shift = 0n;
        for (;;) {
            const byte = bytes[pos++];
            raw |= BigInt(byte & 0x7f) << shift;
            if ((byte & 0x80) === 0) break;
            shift += 7n;
        }
        values.push((raw >> 1n) ^ -(raw & 1n));
    }
    return values;
}

/** FNV-1a 64 of the length then each value (little endian), as StableHash for IntArray. */
export function demoHash(values: bigint[]): string {
    const mask = (1n << 64n) - 1n;
    let hash = 0xcbf29ce484222325n;
    const write = (value: bigint) => {
        let x = BigInt.asUintN(64, value);
        for (let i = 0; i < 8; i++) {
            hash ^= x & 0xffn;
            hash = (hash * 0x100000001b3n) & mask;
            x >>= 8n;
        }
    };
    write(BigInt(values.length));
    values.forEach(write);
    return hash.toString(16).padStart(16, '0');
}

/** Replays a sync frame plus later commands into the demo state. */
export function replayDemo(sync: Extract<StreamFrame, { type: 'streamSync' }>, more: WireCommand[] = []): bigint[] {
    let values = sync.snapshot ? decodeIntArray(sync.snapshot.data) : Array<bigint>(DEMO_LEN).fill(0n);
    for (const c of [...sync.commands, ...more]) {
        values = applyDemo(values, parseCommand(c)) ?? values;
    }
    return values;
}

export type StreamSession = {
    subscribe: (streamId: string, kind?: string) => void;
    unsubscribe: (streamId: string) => void;
    /** Sends `command` as JSON text. */
    command: (streamId: string, commandId: string, command: unknown) => void;
    /** Sends `text` verbatim as the command (for invalid input). */
    commandText: (streamId: string, commandId: string, text: string) => void;
    /** Resolves with the first not yet consumed frame matching the predicate. */
    next: <T extends StreamFrame>(predicate: (frame: StreamFrame) => frame is T, timeoutMs?: number) => Promise<T>;
    /** Commands of a stream since its latest streamSync, in arrival order. */
    commands: (streamId: string) => WireCommand[];
    close: () => void;
};

export type StreamSessionOptions = { sid: string; origin: string; forwardedHost: string };

export function wsConnectUrl(builderUrl: string): string {
    const url = new URL(joinURL(builderUrl, '/api/connect'));
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    return url.toString();
}

export async function openStreamSession(url: string, options: StreamSessionOptions): Promise<StreamSession> {
    const ws = new WebSocket(url, {
        origin: options.origin,
        headers: {
            'User-Agent': DEFAULT_USER_AGENT,
            Cookie: `sid=${options.sid}`,
            'x-forwarded-host': options.forwardedHost
        },
        // Same as ws_connect.ts: the local stack uses a self-signed certificate.
        rejectUnauthorized: false
    });

    const frames: StreamFrame[] = [];
    const consumed = new Set<number>();
    const listeners: Array<() => void> = [];

    ws.on('message', (data: Buffer) => {
        const frame = JSON.parse(data.toString()) as StreamFrame;
        if (typeof frame.type === 'string' && frame.type.startsWith('stream')) {
            frames.push(frame);
            listeners.splice(0).forEach((l) => l());
        }
    });

    await new Promise<void>((resolve, reject) => {
        ws.once('open', () => resolve());
        ws.once('unexpected-response', (_req: unknown, res: IncomingMessage) =>
            reject(new Error(`unexpected upgrade response: ${res.statusCode}`))
        );
        ws.once('error', (err: Error) => reject(err));
    });

    const next = <T extends StreamFrame>(predicate: (frame: StreamFrame) => frame is T, timeoutMs = 10_000) =>
        new Promise<T>((resolve, reject) => {
            const timer = setTimeout(() => reject(new Error(`no matching stream frame after ${timeoutMs}ms`)), timeoutMs);
            const check = () => {
                const idx = frames.findIndex((f, i) => !consumed.has(i) && predicate(f));
                if (idx >= 0) {
                    consumed.add(idx);
                    clearTimeout(timer);
                    resolve(frames[idx] as T);
                } else {
                    listeners.push(check);
                }
            };
            check();
        });

    const send = (streamId: string, commandId: string, text: string) =>
        ws.send(JSON.stringify({ type: 'streamCommand', streamId, commandId, command: text }));

    return {
        subscribe: (streamId, kind = 'demo') => ws.send(JSON.stringify({ type: 'streamSubscribe', streamId, kind })),
        unsubscribe: (streamId) => ws.send(JSON.stringify({ type: 'streamUnsubscribe', streamId })),
        command: (streamId, commandId, command) => send(streamId, commandId, JSON.stringify(command)),
        commandText: send,
        next,
        // Every streamSync replaces the client state, so only frames from the last one count.
        commands: (streamId) => {
            const own = frames.filter((f) => f.streamId === streamId);
            const lastSync = own.map((f) => f.type).lastIndexOf('streamSync');
            return own
                .slice(Math.max(lastSync, 0))
                .flatMap((f) => (f.type === 'streamSync' || f.type === 'streamCommands' ? f.commands : []));
        },
        close: () => ws.close()
    };
}
```

- [ ] **Step 2: Tests**

`tests/api-tests/builder/command_stream.ts`:

```ts
import { expect, test } from '$fixtures/setup';
import {
    DemoCommand,
    StreamFrame,
    StreamSession,
    applyDemo,
    demoHash,
    openStreamSession,
    parseCommand,
    replayDemo,
    wsConnectUrl
} from '$lib/api/builder_stream_ws';
import { MintedSession, SessionMint } from '$lib/mocks/session_mint';
import { randomUUID } from 'node:crypto';

// Must match builder.commandStream.snapshotEvery in services/builder/server_config.test.json.
const SNAPSHOT_EVERY = 5;

type Sync = Extract<StreamFrame, { type: 'streamSync' }>;
const isSync = (streamId: string) => (f: StreamFrame): f is Sync => f.type === 'streamSync' && f.streamId === streamId;
const isCheckpoint = (streamId: string, version: number) =>
    (f: StreamFrame): f is Extract<StreamFrame, { type: 'streamCheckpoint' }> =>
        f.type === 'streamCheckpoint' && f.streamId === streamId && f.version === version;
const isNack = (commandId: string) => (f: StreamFrame): f is Extract<StreamFrame, { type: 'streamNack' }> =>
    f.type === 'streamNack' && f.commandId === commandId;

/** Waits until the stream's commands (since the last Sync) include `commandId`. Commands of one
 * connection are ingested in order, so everything sent before it has been processed then. */
async function waitFor(session: StreamSession, streamId: string, commandId: string, timeoutMs = 10_000) {
    const deadline = Date.now() + timeoutMs;
    while (!session.commands(streamId).some((c) => c.commandId === commandId)) {
        if (Date.now() > deadline) throw new Error(`command ${commandId} not delivered`);
        await new Promise((r) => setTimeout(r, 25));
    }
    return session.commands(streamId);
}

function replayAll(commands: DemoCommand[]): bigint[] {
    return commands.reduce((values, c) => applyDemo(values, c) ?? values, Array<bigint>(10).fill(0n));
}

test.describe('Builder command streams', { tag: ['@regression'] }, () => {
    let mint: SessionMint;
    let users: MintedSession[];
    let sessions: StreamSession[];

    test.beforeEach(async () => {
        mint = await SessionMint.fromServerConfig();
        users = [
            await mint.createUserSession({ userId: randomUUID() }),
            await mint.createUserSession({ userId: randomUUID() })
        ];
        sessions = [];
    });

    test.afterEach(async () => {
        sessions.forEach((s) => s.close());
        await mint.teardownCreatedSessions();
    });

    async function open(index: number, homeUrl: string, builderWSUrl: string) {
        const session = await openStreamSession(wsConnectUrl(builderWSUrl), {
            sid: users[index].sessionCookie,
            origin: new URL(homeUrl).origin,
            forwardedHost: new URL(builderWSUrl).host
        });
        sessions.push(session);
        return session;
    }

    async function subscribed(index: number, homeUrl: string, builderWSUrl: string, streamId: string) {
        const session = await open(index, homeUrl, builderWSUrl);
        session.subscribe(streamId);
        await session.next(isSync(streamId));
        return session;
    }

    test('subscribe to a new stream shall return an empty sync', async ({ homeUrl, builderWSUrl }) => {
        const session = await open(0, homeUrl, builderWSUrl);
        const streamId = randomUUID();
        session.subscribe(streamId);
        const sync = await session.next(isSync(streamId));
        expect(sync.snapshot).toBeNull();
        expect(sync.commands).toEqual([]);
    });

    test('commands shall be delivered in order to every subscriber, rejected ones included', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const streamId = randomUUID();
        const a = await subscribed(0, homeUrl, builderWSUrl, streamId);
        const b = await subscribed(1, homeUrl, builderWSUrl, streamId);

        const commands: DemoCommand[] = [
            { op: 'set', idx: 0, value: 7 },
            { op: 'compareAndSet', idx: 0, expected: 1, value: 2 }, // rejected by replay
            { op: 'add', idx: 0, delta: 3 }
        ];
        const ids = commands.map(() => randomUUID());
        commands.forEach((c, i) => a.command(streamId, ids[i], c));

        for (const session of [a, b]) {
            const received = await waitFor(session, streamId, ids[2]);
            expect(received.map((c) => c.version)).toEqual([1, 2, 3]);
            expect(received.map((c) => c.commandId)).toEqual(ids);
            expect(replayAll(received.map(parseCommand))[0]).toBe(10n);
        }
    });

    test('a late subscriber shall get a snapshot plus tail that replays to the same state', async ({
        homeUrl,
        builderWSUrl
    }) => {
        const streamId = randomUUID();
        const a = await subscribed(0, homeUrl, builderWSUrl, streamId);
        const commands: DemoCommand[] = Array.from({ length: 2 * SNAPSHOT_EVERY + 2 }, (_, i) => ({
            op: 'add',
            idx: i % 10,
            delta: i + 1
        }));
        const ids = commands.map(() => randomUUID());
        commands.forEach((c, i) => a.command(streamId, ids[i], c));
        const checkpoint = await a.next(isCheckpoint(streamId, 2 * SNAPSHOT_EVERY));
        expect(checkpoint.hash).toBe(demoHash(replayAll(commands.slice(0, 2 * SNAPSHOT_EVERY))));
        await waitFor(a, streamId, ids[ids.length - 1]);

        const b = await open(1, homeUrl, builderWSUrl);
        b.subscribe(streamId);
        const sync = await b.next(isSync(streamId));
        expect(sync.snapshot).not.toBeNull();
        expect(sync.snapshot!.version % SNAPSHOT_EVERY).toBe(0);
        expect(sync.snapshot!.hash).toBe(demoHash(replayAll(commands.slice(0, sync.snapshot!.version))));
        expect(replayDemo(sync)).toEqual(replayAll(commands));
    });

    test('resending the same commandId shall not create a second version', async ({ homeUrl, builderWSUrl }) => {
        const streamId = randomUUID();
        const a = await subscribed(0, homeUrl, builderWSUrl, streamId);
        const id = randomUUID();
        const other = randomUUID();
        a.command(streamId, id, { op: 'add', idx: 0, delta: 1 });
        a.command(streamId, id, { op: 'add', idx: 0, delta: 1 });
        a.command(streamId, other, { op: 'add', idx: 0, delta: 1 });

        expect((await a.next(isNack(id))).reason).toBe('duplicate');
        const received = await waitFor(a, streamId, other);
        expect(received.map((c) => [c.version, c.commandId])).toEqual([
            [1, id],
            [2, other]
        ]);
    });

    test('a command without subscription shall be nacked', async ({ homeUrl, builderWSUrl }) => {
        const a = await open(0, homeUrl, builderWSUrl);
        const commandId = randomUUID();
        a.command(randomUUID(), commandId, { op: 'set', idx: 0, value: 1 });
        expect((await a.next(isNack(commandId))).reason).toBe('notSubscribed');
    });

    for (const invalid of [
        { title: 'wrong shape', text: JSON.stringify({ op: 'set', idx: 'x', value: 1 }) },
        { title: 'nested batch', text: JSON.stringify({ op: 'batch', commands: [{ op: 'batch', commands: [] }] }) },
        { title: 'oversized', text: JSON.stringify({ op: 'set', idx: 0, value: 1, pad: 'x'.repeat(17_000) }) },
        { title: 'not JSON', text: 'not json' }
    ]) {
        test(`an invalid command (${invalid.title}) shall be nacked and not stored`, async ({ homeUrl, builderWSUrl }) => {
            const streamId = randomUUID();
            const a = await subscribed(0, homeUrl, builderWSUrl, streamId);
            const bad = randomUUID();
            a.commandText(streamId, bad, invalid.text);
            expect((await a.next(isNack(bad))).reason).toBe('invalid');

            const valid = randomUUID();
            a.command(streamId, valid, { op: 'set', idx: 0, value: 1 });
            const [first] = await waitFor(a, streamId, valid);
            expect([first.version, first.commandId]).toEqual([1, valid]);
        });
    }

    test('unknown fields shall be dropped from stored commands', async ({ homeUrl, builderWSUrl }) => {
        const streamId = randomUUID();
        const a = await subscribed(0, homeUrl, builderWSUrl, streamId);
        const id = randomUUID();
        a.command(streamId, id, { op: 'set', idx: 0, value: 1, note: 'ignored' });
        const [first] = await waitFor(a, streamId, id);
        expect(parseCommand(first)).toEqual({ op: 'set', idx: 0, value: 1 });
    });

    test('subscribing with an unknown kind shall fail', async ({ homeUrl, builderWSUrl }) => {
        const a = await open(0, homeUrl, builderWSUrl);
        const streamId = randomUUID();
        a.subscribe(streamId, 'no-such-kind');
        const failed = await a.next(
            (f): f is Extract<StreamFrame, { type: 'streamSubscribeFailed' }> =>
                f.type === 'streamSubscribeFailed' && f.streamId === streamId
        );
        expect(failed.reason).toBe('unknownKind');
    });

    test('subscribing again shall resync the client with a fresh sync', async ({ homeUrl, builderWSUrl }) => {
        // A client that detects drift (checkpoint hash mismatch) resyncs by subscribing again.
        const streamId = randomUUID();
        const a = await subscribed(0, homeUrl, builderWSUrl, streamId);
        const commands: DemoCommand[] = [
            { op: 'set', idx: 0, value: 4 },
            { op: 'add', idx: 0, delta: 1 }
        ];
        const ids = commands.map(() => randomUUID());
        commands.forEach((c, i) => a.command(streamId, ids[i], c));
        await waitFor(a, streamId, ids[1]);

        a.subscribe(streamId);
        const resync = await a.next(isSync(streamId));
        expect(replayDemo(resync)).toEqual(replayAll(commands));
    });
});
```

- [ ] **Step 3: Run**

Start the builder locally with the `local-development` skill. It must load `server_config.test.json` (with `snapshotEvery: 5`) and reach the Postgres database `database-builder`.

Run: `cd tests && pnpm test:local -- api-tests/builder/command_stream.ts`. Expected: 12 tests PASS.

Then run the full `pnpm test:local`. Expected: no regressions; the existing `ws_connect.ts` chat tests still pass.

Finally run `cd tests && pnpm run format && pnpm run lint`. Expected: clean. Prettier wraps at 120 columns, so let it reformat the long type lines.

- [ ] **Step 4: Checkpoint.** Report and wait.

---

## Task 11: Docs

The `sdp` skill named in CLAUDE.md does not exist in `.claude/skills`. Write the docs by hand, following the existing docs.

- [ ] **Step 1:** Write `docs/services/builder/command-streams.html`.
  - Use `docs/services/builder/hub-connection-lifecycle.html` as the style template, with the shared `docs/doc.css`.
  - Content: "Protocol rules", "Soundness" and "Throughput model" from `temp/command-streams/plan.md`, plus the rationale from its Appendix A.
  - Where the appendix differs from the code, describe the code.
  - Put the client engine (Appendix §9) in a clearly marked "not yet implemented" section.
- [ ] **Step 2:** Update `services/builder/HUB.md`:
  - add `SubscribeStream` and `UnsubscribeStream` to the `ControlCommand` table;
  - add `Stream` and `StreamEvent` to the `HubMessage` table;
  - add `TopicKey::Stream` (internal subscribers only) to the topics bullets;
  - add `StreamDispatcher` to the components diagram and table;
  - add a short "Command streams" section that points to the HTML doc.
- [ ] **Step 3:** Update the file map in `docs/shared/db-migrations.html`: builder's `sql_migrations` is gone, and the command-log schema lives in `crates/shine-infra/src/db/command_log/sql/`, applied with `PgConnection::migrate("command_log", …)`.
- [ ] **Step 4: Checkpoint.** Report and wait. Ask the user whether CLAUDE.md's skill table should drop or replace the missing `sdp` skill; don't edit CLAUDE.md yourself.

---

## Task 12 (optional, after the Task 8 probe): pool checkout without a round trip

bb8 0.9 defaults to `test_on_check_out = true`, and neither pool turns it off. So every `pool.get()` first sends a validation round trip: a `PING` for Redis, an empty query for Postgres. That doubles the cost of every Redis call, including the ingest hot path, and caps an instance at about 10 / (2 × r) ≈ 16k Redis calls/s.

This changes shared infra, so it also changes identity's behaviour. Only do it if the user agrees after seeing the Task 8 probe numbers.

**Files:**
- Modify: `crates/shine-infra/src/db/redis/redis_connection.rs`, `crates/shine-infra/src/db/postgres/pg_connection.rs`

- [ ] **Step 1: Postgres**

In `create_postgres_pool`, add `.test_on_check_out(false)` to the `bb8::Pool::builder()` chain. bb8-postgres's `has_broken` already drops a closed client when it returns to the pool, and a query on a client that died meanwhile fails like any other database error.

- [ ] **Step 2: Redis**

redis-rs's `MultiplexedConnection` does not reconnect, so the pool must learn when a connection broke. Make these changes:

1. Add a field `broken: Arc<AtomicBool>` to `RedisConnection` (in `std::sync::atomic`), initialized to `false` in `RedisConnectionManager::connect`.
2. In both `req_packed_command` and `req_packed_commands`, wrap the inner future so an error marks the connection:

   ```rust
   let broken = self.broken.clone();
   Box::pin(async move {
       let result = time_db_operation("redis", name, self.client.req_packed_command(cmd).instrument(span)).await;
       if let Err(err) = &result {
           // A dropped or unrecoverable connection is replaced instead of being handed out again.
           if err.is_unrecoverable_error() || err.is_connection_dropped() {
               broken.store(true, Ordering::Relaxed);
           }
       }
       result
   })
   ```

   Do the same in `req_packed_commands`, with `"PIPELINE"` as the name.
3. Change `RedisConnectionManager::has_broken` to return `conn.broken.load(Ordering::Relaxed)`.
4. Add `.test_on_check_out(false)` to the pool builder in `create_redis_pool`.
5. For builder, set `max_size=32` in its Redis connection string, rather than changing the code default that identity shares.

- [ ] **Step 3: Verify**

- Run the full Rust suites with both env vars: `cargo test -p shine-infra`, `cargo test -p shine-builder`, `cargo test -p shine-identity`.
- Run the toxiproxy resiliency tests, if `crates/shine-test`'s `toxiproxy` feature has them wired: a Redis/Postgres restart must still heal.
- Rerun the Task 8 probe and report before and after.

- [ ] **Step 4: Checkpoint.** Report and wait.

---

## Final verification

- [ ] `cargo test -p shine-game replay`
- [ ] `cargo test -p shine-infra --test pg_command_log` (with `SHINE_TEST_PG_CNS`)
- [ ] `cargo test -p shine-builder` (with both env vars): no "skipping" lines for the new tests
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace -- -D warnings`
- [ ] `cd tests && pnpm run lint`
- [ ] `cd tests && pnpm test:local` against the local builder
- [ ] Manual: with two subscribed clients, run `docker restart` on the Redis container mid-run. Check that:
  - the clients may receive `streamReset` followed by `streamSync`;
  - persisted commands survive;
  - new versions continue from the Postgres head.

  `redis:latest` restarts from its RDB dump, so this exercises the "Redis went back in time" path.

---

# Appendix A — Design spec (Command Streams)

> **Background rationale.** The "Protocol rules", "Soundness" and "Throughput model" sections and the tasks above are normative. Known stale points in this appendix:
> - wire commands are JSON strings stored as `TEXT`, not embedded JSON or `jsonb`;
> - the worker is `StreamCore` plus an I/O shell with a persist cadence, and every read checks content at the worker's version;
> - the dispatcher uses a `JoinSet`;
> - the hub keeps no per-connection stream set;
> - Redis keys are prefixed;
> - the orphan sweep covers every idle stream and retires by meta;
> - `Gap` reloads;
> - a Redis copy behind Postgres, or one that reassigned versions, is dropped;
> - the persisted mark is checked by content;
> - `duplicate` is a nack reason;
> - the §10 property table is replaced by "Soundness".

---

## 1. Why

We want collaborative, server-authoritative state (game/world editing later) without a single
"owner" server. Chat already gives us a fast fan-out over Redis, but it is lossy, unordered by
anything except Redis ids, and has no durable history. A **command stream** adds:

- a **dense version** per command (gap detection, "state at version N"),
- **durable history** in Postgres,
- **deterministic replay** of commands into a state (same code on servers and later the wasm client),
- **snapshots** so new clients and restarting servers don't replay from zero,
- **hash checkpoints** so servers and clients detect and heal drift.

The hub stays the **central control**: it owns subscriptions, distribution and the replay workers,
the same way it owns connections and the chat dispatcher today.

## 2. Requirements (agreed)

| # | Requirement | Where it is met |
|---|---|---|
| R1 | Client → server write path is high-perf: only store the command in Redis | §6.1 `INGEST` (one round trip) |
| R2 | Redis orders commands; each stream has a unique id | §4 `version`, §5.3 |
| R3 | Servers poll Redis, store commands in Postgres | §6.4 worker step 4, §6.5 orphan sweep |
| R4 | Servers apply commands to a local state, snapshot from time to time | §6.4 steps 3 & 5 |
| R5 | Concurrent servers check state hash vs the authoritative snapshot; no drift | §6.6 snapshot race |
| R6 | When a stream is requested and Redis isn't ready, populate from psql (last N only) | §6.2 hydration |
| R7 | New clients get snapshot + commands since | §6.3 `streamSync` |
| R8 | Clients replay; periodically check drift against the server | §6.7 checkpoints, §9 client engine |
| R9 | Clients get only commands they have not seen | per-connection cursor, §6.4 step 6 |
| R10 | Client has authoritative + local (pending) state | §9 (designed, not built) |
| R11 | Commands/states are generic, shared server/client | §5.1 `shine-game::replay` |
| R12 | A command that cannot be applied is ignored during replay | §5.1 `ApplyOutcome::Ignored` |
| R13 | Multiple server instances, single Redis; losing pending commands on Redis restart is OK | §6.8 generations |
| R14 | WebSocket, async send and receive | existing hub socket, §7 protocol |
| R15 | Per-stream ACL prepared, placeholder only | `StreamAccess` + `AllowAll` |

Out of scope for this plan: the TS client, the wasm bindings, real ACLs, command schema
versioning.

## 3. The big picture

```mermaid
flowchart LR
    subgraph Client["Client (browser)"]
        C1["ClientStream&lt;S&gt;<br/>(designed, not built)"]
    end

    subgraph Instance["Builder instance (N of these)"]
        WS["WS route<br/>recv task / send task"]
        HUB["HubService<br/>control loop"]
        ING["CommandStreamService<br/>ingest (hot path)"]
        DSP["StreamDispatcher<br/>(hub task)"]
        WRK["StreamWorker&lt;S&gt;<br/>one per resident stream"]
    end

    subgraph Redis["Redis (single)"]
        RMETA["cs:{id}:meta"]
        RLOG["cs:{id}:log (stream)"]
        RIDS["cs:{id}:ids (dedupe)"]
        RWAKE(("cs:wake pub/sub"))
        RACT["cs:active (zset)"]
    end

    subgraph PG["Postgres"]
        PSTR["cl_streams"]
        PCMD["cl_commands"]
        PSNP["cl_snapshots"]
    end

    C1 -- "streamCommand" --> WS
    WS -- "ingest" --> ING
    ING -- "EVALSHA INGEST" --> RLOG
    RLOG -. publish .-> RWAKE
    RWAKE -. wake .-> DSP
    WS -- "Subscribe/Unsubscribe" --> HUB
    HUB -- "StreamEvent" --> DSP
    DSP -- "spawn / route" --> WRK
    WRK -- "read_after" --> RLOG
    WRK -- "append / snapshot" --> PCMD
    WRK -- "try_store_snapshot" --> PSNP
    WRK -- "Sync / Commands / Checkpoint" --> HUB
    HUB -- "try_send" --> WS
    WS -- "streamSync / streamCommands / streamCheckpoint" --> C1
```

**Layers** (per `CLAUDE.md`): routes (`routes/ws`) → hub/handlers (`HubService`, dispatcher) →
services (`CommandStreamService`, `StreamWorker`) → repositories (Redis repo in builder,
`PgCommandLogDb` in infra). The replay logic itself has no IO and lives in `shine-game`.

## 4. Core concepts

| Concept | Type | Owner | Meaning |
|---|---|---|---|
| `stream_id` | `Uuid` | client/app | Unique id of a stream. |
| `kind` | `String` | psql `cl_streams` | Which `ReplayState` type the stream uses (`"demo"`). Set at creation, never from later client input. |
| `version` | `i64` | Redis `INGEST` | Dense, 1-based position of a command. Version *v* is the *v*-th command. |
| `command_id` | `Uuid` | client | Identifies a submission; used for dedupe on retries and for the client's pending list. |
| `generation` | `i64` | psql `cl_streams.generation` | Redis "epoch". Bumped every time Redis is (re)populated from psql. Monotonic. |
| `persisted` | `i64` | Redis meta (mirror of psql head) | Highest version known to be in psql; Redis never trims above it. |
| checkpoint | `version % snapshot_every == 0` | config | Deterministic: every instance snapshots and pushes hashes at the same versions. |
| authoritative snapshot | psql row | first writer | The snapshot that won the insert race at a checkpoint version. |

**Authorities:** Redis decides *order*. Postgres decides *truth* — on any conflict, psql wins and
Redis is reset from it.

## 5. Building blocks

### 5.1 `crates/shine-game/src/replay/` — pure replay core (no IO)

Shared by servers (native) and later the client (wasm). Fully unit-testable.

```rust
pub trait ReplayState: Clone + Default + Serialize + DeserializeOwned + StableHash + Send + 'static {
    type Command: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;
    type Reject: fmt::Debug;

    /// Applies atomically: on Err the state is unchanged.
    fn apply(&mut self, cmd: &Self::Command) -> Result<(), Self::Reject>;

    /// Ingest-time shape limits (e.g. batch length); default accepts.
    fn validate(_cmd: &Self::Command) -> bool { true }
}
```

| File | Content |
|---|---|
| `stable_hash.rs` | `StableHash` trait + `StableHasher` (64-bit, identical on native and wasm; xxh3 via `xxhash-rust` — check docs.rs — or FNV-1a from `math/prng/hash.rs`). Impls for ints, bool, arrays, slices, `Vec`, `Option`, `BTreeMap`. **No `HashMap`, no floats** — iteration order and float formatting are not portable. |
| `envelope.rs` | `CommandEnvelope<C> { version, command_id, user_id, command: Option<C> }` (`None` = failed to decode ⇒ ignored), `Checkpoint { version, hash }`. |
| `replica.rs` | `Replica<S> { state, version }` with `apply_next(&env) -> Result<ApplyOutcome, Gap>`, `hash()`, `from_snapshot()`. `ApplyOutcome = Applied \| Ignored`. |
| `policy.rs` | `CheckpointPolicy { every }::is_checkpoint(version)`. |
| `codec.rs` | `encode_snapshot` / `decode_snapshot` (postcard). |
| `demo.rs` | The demo state (below). |

**Demo state** `IntArray([i64; 10])` — tiny, but exercises every rule:

| Command | Fails when |
|---|---|
| `Set { idx, value }` | `idx >= 10` |
| `Add { idx, delta }` | `idx >= 10`, overflow |
| `Swap { a, b }` | any index `>= 10` |
| `CompareAndSet { idx, expected, value }` | current `!= expected` |
| `Batch(Vec<DemoCommand>)` | any inner command fails (all-or-nothing); length capped by `validate` |

### 5.2 `crates/shine-infra/src/db/command_log/` — Postgres store (replaces `db/event_source`)

`event_source` is unused outside its own tests; it is reshaped into a state-agnostic command-log
store. The aggregate/snapshot-chain/`pg_notify` machinery and the buggy `StoreNextEvent`
(hard-coded `es_heads_test`) are dropped.

```sql
cl_streams  (stream_id UUID PRIMARY KEY, kind TEXT NOT NULL,
             version BIGINT NOT NULL DEFAULT 0, generation BIGINT NOT NULL DEFAULT 0,
             created_at, updated_at)
cl_commands (stream_id UUID, version BIGINT, command_id UUID, user_id UUID, generation BIGINT,
             data JSONB, created_at,
             PRIMARY KEY (stream_id, version), FOREIGN KEY (stream_id) -> cl_streams)
             -- rows are immutable (trigger kept from event_source)
cl_snapshots(stream_id UUID, version BIGINT, hash BIGINT, data BYTEA, created_at,
             PRIMARY KEY (stream_id, version),
             FOREIGN KEY (stream_id, version) -> cl_commands)
```

| Operation | Behaviour |
|---|---|
| `ensure_stream(id, kind)` | Upsert; returns head `{kind, version, generation}`; `KindMismatch` if the stream exists with another kind. |
| `begin_generation(id)` | `generation += 1` under the row lock; returns the head **atomically** with it. |
| `append(id, rows)` | One tx, `SELECT … FOR UPDATE` on the head. Rows `<= head.version` must match stored `command_id` (else `Diverged`). New rows must have `generation == head.generation` (else `StaleGeneration`) and start at `head + 1` (else `Gap{head}`). Inserts, bumps head. Returns `Appended \| UpToDate \| StaleGeneration \| Gap \| Diverged`. |
| `commands_after(id, after, limit)` / `last_commands(id, n)` | Reads. |
| `try_store_snapshot(id, v, hash, bytes)` | `INSERT … ON CONFLICT DO NOTHING`, then read back ⇒ `Stored \| Exists{hash}`. |
| `latest_snapshot(id, at_or_before)` / `prune_snapshots(id, keep)` | Reads / cleanup. |

### 5.3 `services/builder/src/repositories/command_stream/redis/` — Redis repository

| Key | Type | Content |
|---|---|---|
| `cs:{id}:meta` | hash | `generation`, `version`, `persisted` — its existence means "ready" |
| `cs:{id}:log` | stream | entry id `"{version}-0"`, fields `g` (generation), `c` (command id), `u` (user), `d` (json) |
| `cs:{id}:ids` | zset | member = `command_id`, score = `version` (dedupe window) |
| `cs:{id}:hydrating` | string | short hydration lock (`SET NX PX`) |
| `cs:active` | zset (global) | member = stream id, score = last write time (Redis `TIME`) |
| `cs:wake` | pub/sub channel (global) | payload = stream id |

All per-stream keys get the same `EXPIRE` refresh in every writing script, so they live and die
together.

**Lua scripts** (`redis::Script` statics; `invoke_async` sends EVALSHA with NOSCRIPT fallback;
each call gets its own named span because telemetry only sees "EVALSHA"). Lua does not roll back on
error, so **every script checks first and writes last**.

| Script | Logic |
|---|---|
| `INGEST(id, cmd_id, user, data)` | meta missing ⇒ `NOT_READY` · `ZSCORE ids cmd_id` hit ⇒ return that version (dedupe) · `version - persisted >= max_unpersisted` ⇒ `BUSY` · else `v = version+1`, `XADD log "{v}-0"`, `HSET meta version v`, `ZADD ids v cmd_id`, `ZADD cs:active`, `EXPIRE`s, `PUBLISH cs:wake id` ⇒ `v` |
| `HYDRATE(id, gen, head, entries…)` | meta exists with `generation >= gen` ⇒ skip · else `DEL log ids`, `XADD` the entries, `ZADD ids` for them, `HSET meta {generation=gen, version=head, persisted=head}`, `EXPIRE`s |
| `MARK_PERSISTED(id, gen, v)` | generation differs or `v <= persisted` ⇒ skip · else `persisted = v`, `cut = min(persisted+1, version-keep+1)`, `XTRIM log MINID cut`, `ZREMRANGEBYSCORE ids -inf (cut` |
| `RESET(id, gen)` | delete all per-stream keys only if `meta.generation == gen` |
| `RETIRE_ACTIVE(id, score)` | `ZREM cs:active id` only if its score is unchanged |

Reads: `read_after(id, version, limit)` = pipelined `HGETALL meta` + `XRANGE log (… COUNT`;
`idle_streams(older_than)`; `listen_to_wakes` on `RedisListener` (same as chat's
`listen_to_room_changes`; a `None` payload on reconnect wakes every resident stream).

## 6. Flows

### 6.1 Write — the hot path

Only validation and one Redis round trip. No replay, no control-loop hop, no Postgres.

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant R as WS recv task
    participant S as CommandStreamService
    participant X as Redis
    participant D as StreamDispatcher (each instance)
    participant W as StreamWorker

    C->>+R: streamCommand(streamId, commandId, command)
    R->>R: write set has streamId? size ok?
    R->>+S: ingest(streamId, kind, commandId, raw)
    S->>S: decode as kind Command + validate
    S->>+X: EVALSHA INGEST
    X->>X: check ready / dedupe / busy, XADD "{v}-0", HSET version
    X--)D: PUBLISH cs:wake streamId
    X-->>-S: v
    S-->>-R: Ok(v)
    R-->>-C: (nothing on success)
    D->>W: notify (std RwLock map lookup, no await)
    Note over W: background: read, apply, persist, fan out (6.4)
```

Failure replies are a `streamNack{commandId, reason}`:
`notSubscribed` (not in write set), `invalid` (decode/validate/size), `notReady` (Redis lost the
stream — a hydrate request is sent to the dispatcher), `busy` (persistence lags beyond
`max_unpersisted`), `failed` (Redis error/timeout — outcome unknown, a retry with the same
`commandId` is deduped).

There is **no success ack**: the writer sees its own `commandId` in the next `streamCommands`
batch, which carries the assigned version. This keeps a burst of writes from filling the
256-message egress channel with acks.

The recv task awaits each ingest before reading the next frame, so commands from one connection
keep their order.

### 6.2 Opening a stream and hydration

Done on the **subscribe path** (not the write path), so a client that waits for `streamSync`
before writing never sees `notReady` under normal operation.

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant R as WS recv task
    participant S as CommandStreamService
    participant P as Postgres
    participant X as Redis
    participant H as Hub control loop

    C->>+R: streamSubscribe(streamId, kind)
    R->>+S: open(streamId, kind, user)
    S->>+P: ensure_stream(streamId, kind)
    P-->>-S: head {kind, version, generation}
    S->>S: StreamAccess.can_read / can_write (AllowAll)
    S->>+X: HGETALL meta
    X-->>-S: missing
    alt Redis not ready
        S->>+X: SET hydrating NX PX 5000
        X-->>-S: OK (lock holder)
        S->>+P: begin_generation(streamId)
        P-->>-S: head {version H, generation G}
        S->>+P: last_commands(streamId, N)
        P-->>-S: commands H-N+1 .. H
        S->>+X: EVALSHA HYDRATE(G, H, entries)
        X-->>-S: ok (skipped if someone hydrated a newer G)
    end
    S-->>-R: resolved kind
    R->>R: write set += streamId → kind
    R->>-H: ControlCommand SubscribeStream
    Note over H: continues in 6.3
```

If the hydration lock is taken, the caller waits briefly and re-checks meta. If the holder dies,
the lock expires and the next caller bumps the generation again; `HYDRATE`'s "newer generation
wins" guard keeps the result consistent.

### 6.3 Subscribe through the hub and first sync

```mermaid
sequenceDiagram
    autonumber
    participant H as Hub control loop
    participant D as StreamDispatcher
    participant W as StreamWorker
    participant P as Postgres
    participant X as Redis
    participant T as Connection channel (256)
    participant C as Client

    H->>H: connection still registered? else ignore
    H->>H: connection.streams += streamId
    H->>+D: publish_internal StreamEvent Subscribed
    alt no worker for streamId on this instance
        D->>+W: spawn (JoinSet), register wake Notify
        W->>+P: latest_snapshot + commands_after(snapshot)
        P-->>-W: snapshot s, commands s+1..H
        W->>+X: read_after(H)
        X-->>-W: commands H+1..L
        W->>W: replay to version L
    end
    D->>-W: add subscriber(connectionId)
    W->>T: streamSync(generation, snapshot s, commands s+1..L)
    W->>W: cursor[connectionId] = L
    T->>-C: streamSync
    Note over W,C: afterwards only streamCommands with versions > cursor
```

Subscribers that arrive while the worker is still loading are queued and synced once it has
loaded. `UserDisconnected` (or a replaced connection) removes the connection from every stream in
the dispatcher's tracker. When a stream has no local subscribers for `unload_grace`, the worker
flushes (persist + `MARK_PERSISTED`) and stops.

### 6.4 The stream worker loop

One task per resident stream per instance, woken by `cs:wake` or every `sync_interval`.

```mermaid
stateDiagram-v2
    [*] --> Loading
    Loading --> Running: snapshot + psql + Redis replayed
    Running --> Running: read → apply → persist → snapshot → fan out
    Running --> Reloading: generation lost commands / Diverged / StaleGeneration / unfillable gap
    Running --> Rebasing: snapshot race lost with other hash
    Rebasing --> Running: load authoritative snapshot, replay buffered commands
    Reloading --> Loading: send streamReset to subscribers
    Running --> Flushing: no subscribers for unload_grace / shutdown
    Flushing --> [*]
```

Each tick:

1. **Read** `read_after(local_version)` (meta + entries in one pipeline).
   If `meta.generation` changed: with `meta.version`/psql head `>= local_version` nothing this
   worker applied was lost ⇒ adopt the new generation silently; otherwise ⇒ **Reloading**.
2. **Apply** each entry to the `Replica` (`Applied` or `Ignored`). When applying reaches a
   checkpoint version, capture `(version, hash, snapshot bytes)` **at that exact version**, even if
   the batch continues past it.
   - First entry above `local_version + 1` (Redis trimmed it) ⇒ fill from psql `commands_after`.
     Missing in psql too ⇒ the commands are gone ⇒ `RESET` + **Reloading**.
3. **Persist**: `append` the new entries (every resident instance does this; idempotent), then
   `MARK_PERSISTED`.
   - `StaleGeneration` ⇒ this worker read an older Redis epoch ⇒ **Reloading**.
   - `Gap{head}` ⇒ re-read Redis from `head + 1` and retry.
   - `Diverged` ⇒ Redis and psql disagree ⇒ `RESET` + hydrate + **Reloading** (psql wins).
4. **Snapshot** captured checkpoints once persisted (§6.6).
5. **Fan out** to each subscriber: entries above its cursor as one `streamCommands`, then
   `streamCheckpoint` for any checkpoint resolved this tick. `Ok(false)` from `send_to_connection`
   still advances the cursor (same as chat).

### 6.5 Orphan sweep

Covers a stream whose last resident worker died (crash, deploy) before persisting its tail.
It runs on the dispatcher's tick every `orphan_sweep_seconds`, in a spawned task.

```mermaid
sequenceDiagram
    autonumber
    participant D as StreamDispatcher (any instance)
    participant X as Redis
    participant P as Postgres

    D->>+X: idle_streams(now - orphan_idle)
    X-->>-D: [streamId, score]...
    loop each stream not resident here
        D->>+X: read_after(meta.persisted)
        X-->>-D: unpersisted entries (maybe none)
        opt entries
            D->>+P: append(entries)
            P-->>-D: Appended / UpToDate
            D->>X: MARK_PERSISTED
        end
        D->>X: RETIRE_ACTIVE(streamId, score)
    end
```

`RETIRE_ACTIVE` only removes the entry if no write arrived in the meantime (score compare). The
Redis TTL (default 24 h) is far longer than the sweep interval, so an orphaned tail survives until
it is swept.

### 6.6 Snapshot race between instances

All resident instances reach checkpoint *v* with their own state and try to insert.

```mermaid
sequenceDiagram
    autonumber
    participant A as Worker on instance A
    participant B as Worker on instance B
    participant P as Postgres

    A->>+P: try_store_snapshot(v, hashA, bytesA)
    P-->>-A: Stored
    Note over A: A's snapshot is authoritative
    B->>+P: try_store_snapshot(v, hashB, bytesB)
    P-->>-B: Exists(hashA)
    alt hashB == hashA
        Note over B: same state, keep going
    else hashB != hashA (B drifted)
        B->>+P: latest_snapshot(at v)
        P-->>-B: bytesA
        B->>B: state = decode(bytesA), replay buffered v+1..L
        Note over B: error log + metric, this is a determinism bug
    end
    A->>A: push streamCheckpoint(v, hashA)
    B->>B: push streamCheckpoint(v, hashA)
```

Both instances push the **authoritative** hash, so every client compares against the same value.

### 6.7 Client drift detection (server side)

`streamCheckpoint{version, hash}` is pushed after each resolved checkpoint. A checkpoint can
arrive after `streamCommands` that go past it, so the client keeps its hashes for recent checkpoint
versions and compares when both are known. A mismatch ⇒ the client resubscribes and rebuilds from
`streamSync`.

### 6.8 Redis restart — generations

```mermaid
sequenceDiagram
    autonumber
    participant B as Worker B (lagging, gen 5)
    participant A as Instance A (subscribe path)
    participant X as Redis
    participant P as Postgres

    Note over X: Redis restarts, everything gone. psql head = 50, gen 5
    Note over B: B has unpersisted entries 51..60 from gen 5 in memory
    A->>+P: begin_generation()
    P-->>-A: version 50, generation 6
    A->>X: HYDRATE(gen 6, head 50, last N)
    Note over X: new writes get 51, 52 ... in gen 6
    B->>+P: append(51..60, gen 5)
    P-->>-B: StaleGeneration
    B->>B: drop buffer, streamReset to subscribers, reload
    B->>+X: read_after(50)
    X-->>-B: gen 6 entries 51...
```

Why this is safe: `begin_generation` and `append` both take the `cl_streams` row lock, so an
append either commits before the bump (its rows are real, valid history, and hydration starts
after them) or is rejected after it. Gen-5 commands that were never persisted are lost — the
accepted trade-off (R13). Their writers saw them in `streamCommands`; the `streamReset` tells
them their view was rolled back.

## 7. Hub integration

How today's hub pieces are extended:

| Existing piece | Today | Extension |
|---|---|---|
| `ControlCommand` + `process` (sole writer) | Connect / Disconnect / Drop / RegistryChanged / Shutdown | `SubscribeStream`, `UnsubscribeStream`. Subscribe is ignored if the connection is no longer registered (replaced, dropped, or `ConnectUser` bailed), so a late subscribe can't pin a worker. |
| `Connections` registry | `connection → {session, tx, topics}` | `+ streams: HashSet<Uuid>`, dropped with the connection. |
| `HubService::publish` | Hub events to internal subscribers **and** Hub-topic sockets | New `publish_internal` for `StreamEvent` on `TopicKey::Stream`: subscribers only, so subscribe churn never touches the 256-slot socket channels. |
| `Subscribers` (lossless, ordered) | Heartbeat, SessionChecker, ChatDispatcher on `[Hub]` | `StreamDispatcher` on `[Hub, Stream]`: one channel, loop order, so it sees a connection's `Subscribed` before its `UserDisconnected`. |
| Consumer loop (`run_connection_loop`) | Events only fold into a tracker, work runs on tick/wake | StreamDispatcher has its **own** loop with the same `select! biased` shape, routing events at once (no tick latency on subscribe). |
| `ChatDispatcher` cursor fan-out | per-connection Redis-id cursor | per-(connection, stream) version cursor in each worker. |
| `send_to_connection` / `drop_dead` | `try_send`, slow/dead ⇒ DropConnection | reused as-is; new `HubMessage::Stream(…)` variant, metric kind `"stream"`. |
| Chat ingest in `connect.rs` | recv task → `ChatService` directly | recv task → `HubService::ingest_stream_command` → `CommandStreamService` (hub API, off the loop). |
| `background_tasks` | aborted on loop exit | the dispatcher owns workers in a `JoinSet`, aborted on `Shutdown`. |

`StreamKindRegistry` (built in `app_state.rs`; v1 registers `"demo"` → `IntArray`) supplies the
ingest validator and the `StreamWorker<S>` factory, so the hub itself stays non-generic.

The `cs:wake` listener callback runs under `RedisListener`'s lock with a slow-handler warning, so it
only does a `std::sync::RwLock<HashMap<stream_id, Arc<Notify>>>` lookup and `notify_one`. It never
awaits.

## 8. WebSocket protocol

JSON text frames, `#[serde(tag = "type", rename_all = "camelCase")]`, same as `chat`.

**Client → server**

| Message | Fields | Notes |
|---|---|---|
| `streamSubscribe` | `streamId, kind` | `kind` is only used when the stream is created |
| `streamUnsubscribe` | `streamId` | |
| `streamCommand` | `streamId, commandId, command` | `command` is the kind's JSON command |

**Server → client**

| Message | Fields |
|---|---|
| `streamSync` | `streamId, generation, snapshot: {version, hash, data (base64 postcard)} \| null, commands[]` |
| `streamCommands` | `streamId, generation, commands: [{version, commandId, userId, command}]` |
| `streamCheckpoint` | `streamId, version, hash` |
| `streamReset` | `streamId` — a fresh `streamSync` follows |
| `streamNack` | `streamId, commandId, reason` |
| `streamSubscribeFailed` | `streamId, reason: kindMismatch \| forbidden \| failed` |

Hashes are hex strings (u64 is not JS-safe); versions are numbers (< 2^53).

Protocol rules for clients:
- Every `streamSync` replaces the client's state for that stream, with or without a preceding `streamReset`. This covers re-subscribes and server-side worker restarts.
- A change of `generation` inside `streamCommands` is not a reset. Versions stay contiguous.
- A large backlog arrives as one `streamSync` followed by `streamCommands` frames of at most 256 commands.
- At most one `streamCheckpoint` is sent per server step, always the latest. Clients must not expect one per `snapshotEvery` versions.

## 9. Client engine (designed, not built)

```mermaid
sequenceDiagram
    autonumber
    participant U as User
    participant CS as ClientStream
    participant S as Server

    U->>+CS: submit(cmd)
    CS->>CS: local.apply(cmd), pending += (id, cmd)
    CS->>S: streamCommand(id, cmd)
    CS-->>-U: local view updated at once
    S->>+CS: streamCommands([... (v, id, cmd) ...])
    CS->>CS: authoritative.apply_next (Applied or Ignored)
    CS->>CS: pending -= ids seen
    CS->>CS: local = authoritative + replay pending (rejects ignored)
    deactivate CS
    S->>+CS: streamCheckpoint(v, hash)
    CS->>CS: compare with own hash at v
    alt mismatch
        CS->>S: streamSubscribe (resync)
    end
    deactivate CS
```

- `ClientStream<S> { authoritative: Replica<S>, pending: Vec<(command_id, S::Command)>, local: S }`
  in `shine-game::replay`, reusing `Replica` and the checkpoint policy.
- `streamNack` drops the pending entry, except `failed`, which stays pending until its id appears
  or the next resync (the write may have landed).
- `streamReset` ⇒ rebuild from the next `streamSync`.
- After any `streamSync`, pending entries whose id does not appear in it are **resubmitted with
  the same `commandId`**, so local edits survive a resync. Server deduplication makes this safe
  within the dedupe window: a command that already landed is not applied twice. The residual case
  is one that landed but has since moved behind the snapshot and out of the window, which is rare.

### Client sync status (for the UI)

Each signal the client can receive maps to a state, so the UI can show "resyncing…" whenever the
view may be stale.

```mermaid
stateDiagram-v2
    [*] --> Connecting
    Connecting --> Syncing: socket open, streamSubscribe sent
    Syncing --> Live: streamSync received
    Live --> Resyncing: streamReset
    Live --> Resyncing: checkpoint hash mismatch (client sends streamSubscribe)
    Live --> Resyncing: unexpected streamSync (worker restart)
    Live --> Offline: socket closed
    Resyncing --> Live: streamSync received
    Offline --> Connecting: reconnect with backoff
    Syncing --> Offline: no streamSync within timeout (close and reconnect)
    Resyncing --> Offline: no streamSync within timeout
    Live --> Failed: streamSubscribeFailed
```

| Trigger | How the client learns about it | Client action | UI |
|---|---|---|---|
| Server rolled back commands (Redis loss, divergence, stale generation) | `streamReset`, then `streamSync` | drop authoritative state, rebuild from Sync, resubmit pending | "Resyncing…" until Sync |
| Client state drifted (determinism bug) | its own check: hash at a `streamCheckpoint` differs | send `streamSubscribe` again; the server replies with a fresh `streamSync` | "Resyncing…" until Sync |
| Server worker restarted | a `streamSync` while already Live | treat as a resync | brief or none |
| Disconnect (network, replaced, slow consumer, shutdown) | WS close; the close frame code/reason says why (`4000` + reason, `1001` for shutdown) | reconnect with backoff, resubscribe, resubmit pending | "Offline / reconnecting…" |
| Server cannot load the stream (Postgres down) | Reset/Subscribe sent, but no `streamSync` arrives | client-side timeout (e.g. 10 s) → close and reconnect | "Resyncing…" then "Offline" |
| A single write failed | `streamNack{reason}` (`busy`, `notReady`, `invalid`, `failed`, …) | drop it, or keep it pending for `failed`, `busy` and `notReady` and resubmit | per-edit hint, e.g. "change not saved" |
| Subscribe refused | `streamSubscribeFailed{reason}` | stop; show the reason | error |

No extra server message is needed: Reset, Sync, the close frame and the checkpoints already cover every case. A client-side timeout covers the one silent case, a server that can't finish loading.

## 10. Soundness review

The properties below were checked against each failure scenario. Each has a short argument and
the fix already in the design.

| Property | Argument | Residual risk |
|---|---|---|
| **P1 Total order** | Versions are assigned only inside `INGEST`, which runs atomically in the single Redis. | — |
| **P2 No half-written Redis state** | Every script checks first and writes last; `XADD` comes before `HSET version`, so a failed `XADD` leaves the version untouched. | — |
| **P3 No retry duplicates** | `INGEST` dedupes `command_id` via `cs:{id}:ids`; the window covers every entry still in Redis, and `HYDRATE` re-adds the ids of the entries it restores. | After a Redis loss, a retry of a command older than the last N hydrated is accepted again. Acceptable. |
| **P4 Never trim unpersisted** | `MARK_PERSISTED` trims to `min(persisted+1, version-keep+1)`; `BUSY` caps the unpersisted backlog. | If all workers stall, writers get `busy` (back-pressure, by design). |
| **P5 psql history is a prefix of the true order** | `append` holds the head lock, requires `head+1`, checks `command_id` on overlap and the generation on new rows. | — |
| **P6 No old-epoch commands after a Redis reset** | The generation is bumped under the same row lock as `append`; stale rows ⇒ `StaleGeneration`. | Unpersisted commands of the lost epoch are lost (R13). |
| **P7 Concurrent hydrators converge** | Lock + "newer generation wins" in `HYDRATE` + psql generation check. A slower hydrator with an older generation is skipped. If it wins first, the newer one overwrites it, and its few commands are lost like a Redis loss. | Only right after a Redis loss with an expired lock. |
| **P8 Deterministic replay** | Pure `apply`, atomic on reject, integer-only demo, `StableHash` without HashMap/float. Undecodable ⇒ ignored. | Code skew during a rolling deploy can make instances disagree; the snapshot race heals it, but **command schemas must stay backward compatible** (versioning is future work). |
| **P9 Servers converge on the authoritative state** | Snapshots are captured at the exact checkpoint version and written once (PK); losers with another hash rebase. | The **first writer wins even if it is the buggy one** — a drifted node can make its state authoritative. This is the agreed semantics; mismatches are logged at `error` with a metric so the bug is visible. |
| **P10 Snapshot integrity** | FK to `cl_commands(stream, version)`: a snapshot can only exist for a persisted version. | — |
| **P11 Clients see every version exactly once and in order** | One worker per stream per instance; `Sync` sets the cursor to L and later batches start at L+1; per-connection egress is FIFO. | A dead or slow socket is dropped by the hub; the client reconnects and resyncs. |
| **P12 Gaps always resolve** | A Redis gap is filled from psql; a gap missing from both ⇒ `RESET`. No retry loop. | — |
| **P13 No leaked workers** | Subscribe for an unregistered connection is ignored; `UserDisconnected` clears streams; idle ⇒ flush + stop; `Shutdown` aborts the `JoinSet`. | — |
| **P14 Hot path stays hot** | One EVALSHA per command; the wake callback does no IO; no control-loop hop; no egress per write. | Global `cs:wake` reaches every instance (a map lookup each); revisit if stream count gets large. |
| **P15 Kind cannot be spoofed** | The kind comes from `cl_streams` at subscribe and is cached in the recv task. | — |
| **P16 Orphans are persisted** | Sweep plus a TTL far longer than the sweep interval; compare-and-retire on `cs:active`. | Lost only if Redis restarts before a sweep (R13). |

Known limitations (accepted for v1): a placeholder ACL (subscribe-time check only, no revocation);
every resident instance appends, so with many instances there is row-lock contention per stream;
apply runs inline in the worker (move to `spawn_blocking` for heavy states).

## 11. Implementation phases

| Phase | Deliverable | Key files |
|---|---|---|
| 0 | SDP design doc from this file (`sdp` skill) + update `hub-connection-lifecycle.html` | `docs/services/builder/command-streams.html` |
| 1 | Replay core + demo + unit tests | `crates/shine-game/src/replay/*`, `crates/shine-game/Cargo.toml` (`postcard`, hash dep, `uuid`) |
| 2 | Postgres command log (rework `event_source`) + DB tests | `crates/shine-infra/src/db/command_log/*`, `crates/shine-infra/tests/pg_command_log.rs` |
| 3 | Builder pg pool + migrations + config + Redis repo + scripts + tests | `services/builder/src/repositories/{db.rs, command_stream/redis/*}`, `app_config.rs`, `main.rs`, `app_state.rs`, `sql_migrations/` (replace the unused `V1__commands.sql`), `Cargo.toml` (`shine-game`) |
| 4 | Hub extensions: control commands, Stream topic, `publish_internal`, egress variant | `services/hub/{hub_command.rs, hub_service.rs, hub_connections.rs, hub_metrics.rs}`, `models/messages/{hub.rs, mod.rs}` |
| 5 | `CommandStreamService` (open/hydrate/ingest), `StreamDispatcher`, `StreamWorker`, registry, `StreamAccess` | `services/command_stream/*`, `services/hub/tasks/stream_dispatcher.rs` |
| 6 | WS protocol | `routes/ws/{connect.rs, message.rs}` |
| 7 | API tests (`api-test-writing` skill) | `tests/api-tests/builder/command-stream*.ts` |

**Config** (`CommandStreamConfig`, defaults): `redis_keep_commands 1000`, `redis_max_unpersisted 5000`,
`hydrate_last_n 200`, `redis_ttl_seconds 86400`, `snapshot_every 100`, `keep_snapshots 3`,
`sync_interval_ms 1000`, `unload_grace_seconds 60`, `orphan_sweep_seconds 30`,
`orphan_idle_seconds 10`, `max_command_bytes 16384`.

**Reused code:** `Subscribers::publish`, `HubService::send_to_connection` / `drop_dead`,
the chat cursor fan-out (`tasks/chat_dispatcher.rs`), the `select! biased` loop shape
(`tasks/connection_tracker.rs`), `RedisListener` / `RedisConnection::listen`
(`shine-infra/src/db/redis/`), the pg pool + refinery pattern (`identity/src/repositories/db.rs`).

## 12. Testing & verification

| Level | What | How |
|---|---|---|
| Unit (`shine-game`) | reject ⇒ unchanged state/hash; batch atomicity; gap; undecodable ⇒ ignored; snapshot round trip; two replicas ⇒ same hash; snapshot replay == full replay | `cargo test -p shine-game replay` |
| DB (`shine-infra`) | concurrent identical appends; StaleGeneration; Diverged; Gap; snapshot race same/other hash; `begin_generation` vs `append` ordering | `cargo test -p shine-infra --test pg_command_log` with `SHINE_TEST_PG_CNS` |
| Redis scripts (builder) | dedupe; BUSY; MINID never above persisted; HYDRATE generation guard; RESET guard; RETIRE_ACTIVE compare | `cargo test -p builder` with `SHINE_TEST_REDIS_CNS` |
| Hub (builder) | subscribe/unsubscribe events; subscribe after replacement ignored; disconnect clears streams; `publish_internal` doesn't reach sockets | `hub_service_test.rs` style |
| Worker (builder) | two workers ⇒ equal hashes; forced drift heals via snapshot race; `FLUSHALL` ⇒ new generation, lagging worker's stale tail rejected, reload | `cargo test -p builder` with both env vars |
| API (e2e) | subscribe ⇒ sync; ordered delivery to a second client incl. the writer's own `commandId`; a rejected command still gets a version; late subscriber gets snapshot + tail; equal checkpoint hashes across clients; write without subscribe ⇒ nack; same `commandId` twice ⇒ one version | run builder via `/local-development`, then `cd tests && pnpm test:local` |
| Manual | restart Redis mid-test ⇒ `streamReset` + `streamSync`, persisted commands survive, versions continue from the psql head | local |
