# Command Streams Implementation Plan (v2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (inline, in one session; no subagents). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replicated, replayable command streams in the builder hub, per `docs/services/builder/hub.md` (*Command streams*): clients subscribe to a stream, receive a snapshot plus commands, write commands over the socket, and can start from an HTTP snapshot. The platform is kind-agnostic; chat is only the example kind built at the end.

**Architecture:** A pure replay core (`shine-game`), a Postgres command log (`shine-infra`), a Redis stream repository (builder), a `CommandStreamService` (ingest) and one `StreamWorker` per resident stream under a `StreamDispatcher` hub task. The hub gets subscribe/unsubscribe commands and stream messages. Redis orders the commands, Postgres is the truth.

**Tech Stack:** Rust 2021, tokio, axum ws, redis-rs 1.x (`redis::Script`), tokio-postgres + bb8 + refinery, serde/serde_json, postcard (in `shine-game`), TypeScript API tests.

**Spec:** `docs/services/builder/hub.md` (design). Reference for algorithms and invariants: `docs/plans/command-streams-plan.md` ("Protocol rules", "Soundness" I1-I16, Tasks 1-9 for SQL, Lua and core algorithms). Where this plan differs from that one, **this plan wins**; the old plan's chat parts and docs task are not used.

## Progress

| Task | State | Evidence |
|---|---|---|
| S1 Replay core and chat state | done | `cargo test -p shine-game --test replay` (25 tests), wasm build |
| S2 Postgres command log | done | `cargo test -p shine-infra --test pg_command_log` (15 tests, `database-test`) |
| S3 Builder Postgres and settings | done | `cargo test -p shine-builder` (56 tests; 7 new: settings, schema on startup) |
| S4 Redis stream repository | done, scripts reviewed | `cargo test -p shine-builder` (76 tests; 20 in `redis_command_stream`) |
| S5 Hub subscriptions and stream messages | done | `cargo test -p shine-builder` (`hub::tests::stream`, 5 tests) |
| S6a pure StreamCore | done | `cargo test -p shine-builder` (100 tests; 19 in `stream_core`) |
| S6b kinds, service, ingest | done | `cargo test -p shine-builder` (114 tests; 13 in `command_stream_service`) |
| S6c worker, dispatcher, egress | done | `cargo test -p shine-builder` (124 tests; 5 in `stream_worker`, 5 in `hub::tests::stream_dispatcher`) |
| S7 wire protocol, snapshot route, resume | done | `cargo test -p shine-builder` (144 tests: 6 `read_snapshot`, 4 snapshot route, 11 stream socket); `cargo test -p shine-game --test replay` (35, 7 protocol); wasm check |
| S8 integration and fault tests | done | `cargo test -p shine-builder` (152 tests, 3 runs; 6 in `stream_integration`, 2 new in `hub::tests::stream_dispatcher`) |
| S9 chat kind, switch from the old chat | done | `cargo test -p shine-builder` (156 tests); `pnpm exec playwright test api-tests/builder api-tests/shared` against the local builder (35 builder tests passed; the identity cases of `shared` need the identity service); `pnpm run lint` |
| S10 | open (S10 next) | |
| S12 client engine in `shine-game` | open (after S9) | |
| S11 let the worker prune | decision pending | |

**Decisions taken while implementing** (they supersede the older wording where they differ):
- `ReplayState::apply` is infallible; a command the state does not accept changes nothing. `CommandEnvelope.command` is a plain `C`: an undecodable command is an error of the caller (ingest answers `streamNack invalid`; a stored one stops that stream's worker and the stream answers `unavailable`), never an ignored command.
- There is no ownership check at ingest and no `refers_to`: whether a command takes effect is the state's private decision in `apply`.
- `shine-game` holds no storage code. The snapshot cadence is a builder setting (`KindSettings::is_snapshot_version`). The postcard snapshot codec (`encode_snapshot`, `decode_snapshot`) lives in `shine-game`, because it is the wire encoding of a state that the web client decodes too (postcard builds for wasm); `shine-infra` stores the bytes opaquely.
- The chat state has `Add`, `Remove`, `Replace`; a message carries `edited`. There is no demo kind: the replay and platform tests use `ChatState`.
- `PgCommandLogDb::prune(stream, keep)` deletes old snapshots and the commands below the oldest kept snapshot in one statement; it has no caller yet (S11).
- `generation` is the history epoch (a fencing token owned by the log); the comments say so, the name stays. Renaming it to `epoch` is open.
- `db.sqlCns` is optional; the Postgres pool is created only when `commandStream.enabled`. Production needs `builder.db.sqlCns` in the key vault before enabling.
- The Redis repository has no keep-count setting: `mark_persisted(stream, generation, version, command_id, keep_from)` trims only below `keep_from` (the oldest stored snapshot version, 1 when none) and never above what is persisted. Stream ids are `StreamId` strings in the keys; the module carries a temporary `allow(dead_code, unused_imports)` until S6 uses it. The eviction-policy warning is part of the hardening plan (H4), not here.
- Script review of S4 (line by line, plus concurrency, TTL, trim-boundary, payload and idle tests): `INGEST`, `MARK_PERSISTED`, `RESET`, `RETIRE_ACTIVE`, `IDLE_STREAMS` and `UNLOCK` held up. Two defects were fixed: `HYDRATE` deleted the old log before it wrote the new one, so a tail that is not contiguous or does not end at the head left a wiped log (the Rust side now checks the tail and returns an error before any write), and a configured TTL of 0 made `EXPIRE` delete every key (the config is clamped to at least 1; `max_unpersisted` too). Known limits: the dedupe window covers only the commands still in Redis; multi-key scripts need a single-primary Redis; the scripts call `TIME`, which needs effect replication (always so on the required Redis 6.2).
- Connection strings for tests come from `shine_test::db::{get_redis_cns, get_pg_cns}`; with `CI` set a missing string fails the test. Test files of the builder are declared at the bottom of the owning file with `#[path]`.

**Running the database tests locally** (Docker dev environment, see the `local-development` skill; the compose file needs the external network once: `docker network create shine`):
```
docker compose -f services/docker-compose.yml -p shine up -d
export CI=true
export SHINE_TEST_REDIS_CNS="redis://localhost:6379"
export SHINE_TEST_PG_CNS="postgres://username:password@localhost:5432/database-test?sslmode=disable"   # builder tests may use database-builder
export SHINE_TEST_TOXIPROXY_URL="http://localhost:8474"
```
Run the shell commands one at a time (parallel Git Bash starts can crash).

**Open questions**: Task S11 (whether the worker prunes commands as well as snapshots); the decode-error policy above; the `generation` -> `epoch` rename; the hardening plan's close codes and limits (H-D3, H-D4).

## Global Constraints

**Process**
- The user reviews and commits. Never run git write commands (`git add`, `commit`, `push`, `stash`, branch or tag changes). Every task ends with a **Checkpoint**: stop, report what changed and the test output, wait. The user may ask for changes or renames at a checkpoint; apply them before the next task.
- Run inline in one session; no subagents.
- Before editing builder routes, services or repositories invoke `service-architecture`; before API tests `api-test-writing`; before running the service `local-development`. Re-read each hub/ws file right before editing it; never revert user changes.

**Style and logging**
- Comments say what, not how; concise. `tracing::*!` only. Prefix log lines with `[{stream_id}]` or `[{user_id}]`. `trace`: command payloads and JSON; `debug`: per-step and per-message; `info`: prod root-cause context; `warn`: infrastructure errors that heal by retrying; `error`: bugs and unexpected state (for example snapshot drift).
- `cargo fmt --all` before every checkpoint; clippy clean; after TS changes `cd tests && pnpm run format && pnpm run lint`.

**Data and protocol** (copied from the spec)
- Frames are JSON text, `type` tag, camelCase. `version` and `generation` are `i64` below 2^53; `hash` is `u64` (Postgres `i64` by bit cast) and 16 lower-case hex digits on the wire. A `command` travels as a JSON **string** and is stored as canonical text (the re-serialized typed command) in a `TEXT` column, never `jsonb`.
- Client frames: `streamSubscribe{streamId, kind, resume?{generation, version}}`, `streamUnsubscribe{streamId}`, `streamCommand{streamId, commandId, command}`. Server frames: `streamSync`, `streamCommands`, `streamCheckpoint`, `streamReset`, `streamNack{reason}`, `streamSubscribeFailed{reason}`. Nack reasons: `notReady, busy, notSubscribed, forbidden, invalid, failed, duplicate, unavailable`. Subscribe-failed reasons: `unknownKind, kindMismatch, forbidden, failed, unavailable`.
- A large backlog is one `streamSync` followed by `streamCommands` frames of at most 256 commands. At most one `streamCheckpoint` per persist cycle, never ahead of the delivered commands.
- Redis >= 6.2, `maxmemory-policy noeviction`; all stream keys under a prefix (`cs` in production, `cs-test-<uuid>` in every test). Redis infrastructure errors, Postgres errors and network glitches never stop the process: retry with backoff (reloads 200 ms to 10 s) and recover.

**Tests**
- `shine_test::test`; connection strings from `shine_test::db::{get_redis_cns, get_pg_cns}`; with `CI` set a missing string fails the test. A panic in a background thread aborts the whole test run. Run DB tests with both strings set and check the output has no "skipping" line. Windows: PowerShell for env vars.
- There is no demo kind. The replay core tests and the platform tests (S6-S8) use the real `ChatState` (built and tested in S1), with the small snapshot cadence and retention of the test config; S9 only wires it into the running service.

## Review Focus

1. **A client retries a command after a timeout** (same `commandId` twice): one version, the retry gets `streamNack duplicate`. Test: Task S6 `ingest_dedupes_command_id`.
2. **Redis lost or back in time** while clients are connected, including a copy that reassigned versions: versions continue from the Postgres head, clients get `streamReset` then `streamSync`. Tests: Task S8 `redis_loss_continues_from_the_postgres_head`, `redis_rolled_back_within_a_generation_is_repaired`.
3. **Slow client** with a full channel: dropped cleanly, never sees a version gap or a checkpoint ahead of its commands. Tests: Task S6 `a_failed_send_stops_all_later_frames`, Task S8 `a_slow_subscriber_never_sees_a_version_gap`.
4. **Subscribe/unsubscribe churn** and a subscribe for a connection that is not registered: no leaked subscription or worker. Tests: Task S5 `subscriptions_are_published_for_registered_connections_only`, Task S8 `subscribe_unsubscribe_churn_leaves_no_subscription`.
5. **Resume at the edges** (same version, older than the worker's snapshot, changed generation, above the head): contiguous tail or a full sync, never a gap. Tests: Task S7 `resume_inside_the_window_sends_only_the_tail`, `resume_with_a_changed_generation_sends_a_full_sync`, `resume_older_than_the_base_version_sends_a_full_sync`, `resume_above_the_head_sends_a_full_sync`.

---

## Decisions

| # | Decision |
|---|---|
| S-D1 | `ReplayState::apply(&mut self, ctx: &CommandContext, command: &Self::Command)` (infallible: it is called only for the next version, and once called the command counts as integrated, whatever the state does with it); `CommandContext { version: i64, command_id: Uuid, user_id: Uuid }`. A command the state does not accept changes nothing and the replay continues; `Replica` never calls `apply` with a gap or a repeat. |
| S-D2 | Whether a command takes effect is the state's private decision inside `apply`: a command it refuses (an unknown target, a command of another user) is stored and ignored on replay, and its version is still consumed. Ingest checks only the shape (`validate`), the size and the `commandId` (dedupe); it never compares a command with another command's author. |
| S-D3 | Retention is per kind: `snapshotEvery` (K) and `keepSnapshots` (M), global defaults 100 and 3, overridable under `commandStream.kinds.<kind>`. Redis keeps the commands from the oldest stored snapshot on (at most K × M persisted commands) plus every unpersisted command (never trimmed). There is no separate Redis size setting. |
| S-D4 | Postgres keeps the latest M snapshots and the commands from the oldest of them on. `PgCommandLogDb::prune(stream, keep)` does both in one statement (built in S2); until the worker calls it (S6, see S11), nothing is deleted. State older than the oldest stored snapshot is never served from history; the request gets a full sync from the newest snapshot. |
| S-D5 | `GET streams/{streamId}/snapshot` returns the `streamSync` payload built from storage (newest stored snapshot, stored commands after it, Redis commands above the stored head, contiguous by version); 401 without a session, 404 unknown stream, 503 storage unavailable. |
| S-D6 | Resume is accepted when `resume.generation` equals the stream's generation and `resume.version >= worker.base_version()` (the version of the worker's latest snapshot) and `resume.version <= fanned_out`; the worker sends `streamCommands` from `resume.version + 1` and joins the shared cursor in the same step. Otherwise it sends `streamSync`. |
| S-D7 | A stream id is a validated string `StreamId` (`^[A-Za-z0-9_.:-]{1,64}$`), Postgres `TEXT`, Redis key part, wire `streamId`. |
| S-D8 | The builder has its own Postgres database: `builder.db.sqlCns`, its own pool, its own migrations (the command-log schema from `shine-infra` applied at startup with `PgConnection::migrate("command_log", ...)`, plus the builder's embedded `sql_migrations`). CI and Docker provide the builder's database. |
| S-D9 | Streams are off (`Option<CommandStreamService>` is `None`, config switch `commandStream.enabled`, default false) until Task S9; nothing in S1-S8 changes the running service. |
| S-D10 | Access: any authenticated user may subscribe and write, through the `StreamAccess` port (`AllowAll`). |

## Milestones

| M | Slice | Task | Independent test | Depends on |
|---|---|---|---|---|
| S1 | Replay core | S1 | `cargo test -p shine-game` | - |
| S2 | Postgres command log | S2 | `shine-infra` DB tests | - |
| S3 | Builder Postgres, per-kind settings | S3 | config + migration tests | S2 |
| S4 | Redis repository | S4 | Redis repo tests | S3 |
| S5 | Hub subscriptions and stream messages | S5 | hub tests | - |
| S6 | Service, kinds, `StreamCore`, worker, dispatcher | S6 | pure core tests + worker tests | S1, S4, S5 |
| S7 | Wire protocol, snapshot route, resume | S7 | route tests with the test kind; API tests come with chat in S9 | S6 |
| S8 | Integration and fault tests | S8 | Redis + Postgres | S6 |
| S9 | Chat example kind, old chat removed | S9 | chat tests + API tests | S7, S8 |
| S10 | Test hardening, doc sync | S10 | whole suite | all |
| S12 | Client engine (`ClientStream<S>`) | S12 | `cargo test -p shine-game` only | S7, S9 |

S1, S2 and S5 are independent and can be done in any order.

---

## Task S1: Replay core and chat state (`shine-game`) - done

**Files:** Create `crates/shine-game/src/replay/{mod.rs, state.rs, envelope.rs, replica.rs, stable_hash.rs, chat.rs}` (as in plan Task 1, with the S-D1 signature). Tests: `crates/shine-game/tests/replay/{main.rs, common.rs, chat.rs, replica.rs, hash.rs, snapshot.rs}` (integration tests through the public API).

**Interfaces:**
- Produces: `trait ReplayState` (`const KIND`, `type Command`, `apply(&mut self, &CommandContext, &Command)` infallible, `validate`); `struct CommandContext`; `struct CommandEnvelope<C> { version, command_id, user_id, command: C }`; `trait StableHash` (with `str` and `Uuid`); `struct Replica<S>` (`apply_next` -> `Result<ApplyOutcome, Gap>`, `from_snapshot`, `version`, `hash`, `checkpoint`); `Checkpoint`. `shine-game` knows nothing about storage: no snapshot codec, no snapshot cadence, no database types (the codec lives in `shine-infra`, the cadence in the builder settings).
- Produces (chat, the first real kind): `CHAT_WINDOW = 200`, `CHAT_MAX_TEXT_BYTES = 2000`, `ChatMessage { id, user_id, text, edited }` (the id is the add command's id; `edited` is set by a replace), `ChatState { messages: VecDeque<ChatMessage> }`, `ChatCommand::{Add { text }, Remove { id }, Replace { id, text }}` (`{"type":"add","text":..}`, `{"type":"remove","id":..}`, `{"type":"replace","id":..,"text":..}`), kind `"chat"`. `apply(Add)` appends and drops the oldest beyond the window; `apply(Remove)` removes the message only when its author is the sender, otherwise nothing; `apply(Replace)` does the same check, then replaces the text in place and sets `edited`; `validate(Add)` and `validate(Replace)` = trimmed text non-empty and within the limit.

- [x] **Step 1: Tests** (all driven through `Replica::apply_next`): chat rules (`add_appends_in_order_with_the_sender_and_the_command_id`, `the_window_keeps_the_last_messages`, `a_user_removes_their_own_message`, `removing_an_unknown_or_an_already_removed_message_is_ignored`, `removing_another_users_message_is_ignored`, `validate_checks_the_text`, `command_json_shape`, the replace tests: in place and edited, twice, ignored for unknown/removed/foreign ids, validation, JSON shape); replica contract (`the_state_never_sees_a_gap_or_a_repeat`, `a_command_the_state_ignores_still_consumes_its_version`, `a_checkpoint_names_the_version_and_the_hash`); hashing (reference vectors, layout, the hash covers the author, same log same hash); state tests (serde round trip keeps the hash, replay from a state equals replay from zero, a resumed replica rejects older versions). 25 tests.
- [x] **Step 2:** Implemented; `cargo test -p shine-game --test replay` passes, the wasm build works. **Checkpoint.** (Done.)

## Task S2: Postgres command log (`shine-infra`) - done (15 tests, `database-test`)

**Files:** Create `crates/shine-infra/src/db/command_log/{mod.rs, types.rs, stream_id.rs, pg_command_log.rs, sql/V1__command_log.sql}`; test `crates/shine-infra/tests/pg_command_log.rs`. Tests use `database-test` (`SHINE_TEST_PG_CNS`). The old `db/event_source` is removed as in plan Task 2.

**Interfaces:**
- Produces: `struct StreamId(String)` with `StreamId::parse(&str) -> Result<StreamId, StreamIdError>` (S-D7); `PgCommandLogDb` as in plan Task 2 with `stream_id: &StreamId` instead of `Uuid` (`open_stream`, `append`, `read_after`, `latest_snapshot`, `try_store_snapshot`, `prune(stream, keep) -> PruneOutcome { snapshots, commands }` (one statement: the oldest kept snapshot defines the command cutoff), `oldest_snapshot_version(stream) -> Option<i64>`, generation begin); the log stores snapshot bytes opaquely (the codec is in `shine-game`).

- [ ] **Step 1: Failing tests.** All of plan Task 2's tests with string ids, plus `stream_id_accepts_chat_global_and_rejects_bad_ids` (empty, 65 chars, spaces, `/`), `prune_keeps_the_latest_snapshots` (M = 2 of 5), `oldest_snapshot_version_follows_pruning`.
- [ ] **Step 2:** Implement per the old plan Task 2 (row lock on the head, id-checked appends, immutable rows, generation fencing, first-writer-wins snapshots with a foreign key to the command row); `stream_id` columns are `TEXT`.
- [ ] **Step 3: Verify** `cargo test -p shine-infra command_log` with `SHINE_TEST_PG_CNS` set. **Step 4: Checkpoint.**

## Task S3: Builder Postgres and per-kind settings - done (7 tests)

**Files:** Modify `services/builder/src/repositories/db.rs`, `app_config.rs`, `app_state.rs`, `main.rs`, `server_config.{prod,test}.json`, `services/Dockerfile`/CI env for the builder database. Create `services/builder/src/settings/command_stream.rs`. Test: unit tests in `settings/command_stream.rs`, `repositories/db.rs`.

**Interfaces:**
- Produces: `DBConfig { redis_cns, sql_cns }` (S-D8); `create_pg_pool(&DBConfig) -> Result<PgPool, ...>` running the migrations at startup; `CommandStreamConfig { enabled: bool, snapshot_every: i64, keep_snapshots: i64, kinds: HashMap<String, KindRetention>, sync_interval_ms, persist_delay_ms, unload_grace_seconds, orphan_sweep_seconds, orphan_idle_seconds, max_command_bytes, redis_max_unpersisted, redis_ttl_seconds }`; `CommandStreamSettings::for_kind(&str) -> KindSettings { snapshot_every, keep_snapshots }` with `fn is_snapshot_version(&self, version: i64) -> bool` (`version > 0 && version % snapshot_every == 0`, the cadence the worker and the checkpoints use; not in `shine-game`).

- [ ] **Step 1: Failing tests.** `kind_overrides_win_over_defaults`; `invalid_retention_fails_the_start` (zero or negative K or M); `the_builder_creates_its_schema_on_startup` (fresh database: tables exist); `streams_are_off_by_default`.
- [ ] **Step 2:** Implement; remove builder's never-applied `sql_migrations/V1__commands.sql` if present (the command-log schema comes from `shine-infra`). Test config: `snapshotEvery` 5, `keepSnapshots` 2, `syncIntervalMs` 200 for API tests.
- [ ] **Step 3: Verify. Step 4: Checkpoint.**

## Task S4: Redis stream repository - done (20 tests)

**Files:** Create `services/builder/src/repositories/command_stream/{mod.rs, redis/{mod.rs, scripts.rs, redis_command_stream_db.rs, redis_command_stream_test.rs}}`.

**Interfaces:**
- Produces: `RedisCommandStreamDb` (a cheap `Arc<Inner>` handle) with `ingest(stream, command_id, user_id, canonical) -> IngestResult { Version(i64) | NotReady | Busy }`, `hydrate(stream, generation, head, entries)`, `mark_persisted(stream, generation, version, keep_from) -> ..`, `reset(stream, generation)`, `read_after(stream, version, limit)`, `idle_streams`, `retire_active`, `listen_to_wakes`.
- Redis keys, scripts (`INGEST`, `HYDRATE`, `MARK_PERSISTED`, `RESET`, `RETIRE_ACTIVE`) and their check-first-write-last rule are as in plan Task 4. Differences: ids are strings; `MARK_PERSISTED` trims at `min(persisted + 1, keep_from)` where `keep_from` is the oldest stored snapshot version (1 when there is none), which is the S-D3 rule; there is no `keep` count argument.

- [ ] **Step 1: Failing tests** (unique prefix per test): the plan Task 4 tests, adapted, plus `mark_persisted_keeps_everything_from_the_oldest_snapshot`; `mark_persisted_never_trims_unpersisted_commands`; `ingest_is_not_ready_before_hydrate`.
- [ ] **Step 2:** Implement. **Step 3: Verify** with Redis. **Step 4: Checkpoint.**

## Task S5: Hub subscriptions and stream messages - done (5 tests)

**Files:** Create `services/builder/src/models/messages/stream.rs`, `services/hub/tests/stream.rs`; modify `services/hub/hub_command.rs`, `hub_service.rs`, `hub_connections.rs`, `models/messages/{mod.rs,hub.rs}`, and the exhaustive matches on `HubMessage` (`routes/ws/connect.rs`, `tests/test_utils.rs`).

**Interfaces:**
- Produces: `ControlCommand::{SubscribeStream { user_id, connection_id, stream_id: StreamId, kind: &'static str, resume: Option<StreamResume> }, UnsubscribeStream { user_id, connection_id, stream_id }}`; `StreamResume { generation, version }`; `StreamEvent::{Subscribed { connection_id, stream_id, kind, resume }, Unsubscribed { connection_id, stream_id }}` on `TopicKey::Stream` (internal subscribers only; no socket subscribes); `HubMessage::{Stream(StreamMessage), StreamEvent(StreamEvent)}` with `StreamMessage::{Sync, Commands, Checkpoint, Reset, Nack, SubscribeFailed}`; `HubService::{request_subscribe_stream, request_unsubscribe_stream}`; `Connections::contains`.
- The hub keeps no subscription set per connection: the loop only checks that the connection is registered and publishes the event; the dispatcher (S6) tracks subscriptions and drops a connection's streams on `UserDisconnected`. A socket's own replies (nacks, subscribe failures) do not go through the loop: the socket keeps a `WeakSender` of the egress channel it created (`tx.downgrade()`) and sends into it directly, so they cost the command loop nothing and are not lost while the connection is still being registered; being weak, the sender does not keep the channel open when the hub drops the connection (S7, proven by `a_socket_replies_through_a_weak_sender_without_keeping_its_channel_open`).

- [x] **Step 1: Tests** (`hub/tests/stream.rs`, Redis): `subscriptions_are_published_for_registered_connections_only`; `a_subscribe_carries_the_resume_position`; `unsubscribing_keeps_the_socket_connected`; `stream_events_reach_only_internal_subscribers`; `a_socket_replies_through_a_weak_sender_without_keeping_its_channel_open`.
- [x] **Step 2:** Implemented in the command loop (sole writer). The new items carry a temporary `allow(dead_code)` until S6 and S7 construct them. **Checkpoint.** (Done.)

## Task S6: Stream service, core, worker and dispatcher (three steps)

The reference for the algorithms is the old plan's Tasks 6 and 7 (`docs/plans/command-streams-plan.md`); the decisions in this plan win. All three parts live in `services/builder/src/services/command_stream/` (`mod.rs` exports them).

### S6a: the pure `StreamCore` (no I/O) - done (19 tests)

**Files:** Create `stream_core.rs` and `stream_core_test.rs` (declared at the bottom of `stream_core.rs`).

**Interfaces:** `StreamCore<S: ReplayState>`: the worker's state machine over one stream. It owns the `Replica<S>`, the shared delivery cursor `fanned_out`, the commands `since_snapshot` (above the latest snapshot, never trimmed above `fanned_out`), the subscribers (each synced or `pending_sync`), the unpersisted commands, and the checkpoint cadence taken from `KindSettings`. Inputs are plain values (Redis meta and entries, Postgres snapshots and heads); outputs are plain values (frames per subscriber, what to append and persist, faults), so every rule is testable without Redis or Postgres. `Fault::{Reload(&'static str), Reset { generation, reason }}`, `Observed::{Current, NeedsProof(i64)}`, `base_version()`, `fanned_out()`.

**Built as planned, with these additions.** `Fault::Undecodable { version }` (a stored command that cannot be decoded; never ignored); `add_subscriber_resuming(connection, StreamResume)` with the eligibility re-checked at delivery time (a snapshot taken in between can trim the commands a resume needs; an ineligible or reset resume becomes a full `Sync`); `base_version()`; `Sync` announces `snapshot_every`. The snapshot cadence comes from `KindSettings`, the codec from `shine-game`. `shine-game` is now a dependency of the builder.

**Tests** (pure, no database): one subscriber gets `Sync` then `Commands` in order with contiguous versions; two subscribers share one cursor and a new one gets the snapshot plus `since_snapshot`; a checkpoint is only sent at `snapshot_every` multiples, never ahead of the delivered commands, and at most one per step; a failed send removes the subscriber and it sees no later frame (no version gap); `Reset` is followed by exactly one `Sync` and nothing in between; a newer generation is adopted only when everything applied is persisted; an older generation or a lower version is a `Reset` fault; another command id at the replica's version is a `Reset` fault; holes filled from Postgres cause no false divergence; a resume inside the window gets only the tail and one outside gets a `Sync`; `apply` is never called with a gap or a repeat.

### S6b: kinds, access, metrics and `CommandStreamService`

**Files:** Create `mod.rs`, `command_stream_metrics.rs`, `stream_access.rs`, `stream_kind.rs`, `command_stream_service.rs`, `command_stream_test.rs`; create `models/command_stream_error.rs`.

**Interfaces:** `trait StreamKind { fn kind(&self) -> &'static str; fn canonicalize(&self, command: &str) -> Option<String>; }` with `ReplayKind<S: ReplayState>` (decode, `validate`, re-serialize); `StreamKindRegistry::new().with::<S>()`; `trait StreamAccess` with `AllowAll`; `CommandStreamError::{StreamNotFound, NotReady, Internal}`; `CommandStreamService::{open, ensure_hydrated, ingest, persist_tail, sweep_orphans, listen_to_wakes, kinds, settings}` (`persist_tail` is shared by the sweep and the S6c worker; the worker takes the stores from the service when it needs them); `IngestOutcome::{Accepted(i64), Duplicate(i64), Invalid, NotReady, Busy}`; `OpenError::{UnknownKind, KindMismatch, Forbidden, Failed}`.

- Command decoding is shared with the client: `shine_game::replay::decode_command::<S>(&str) -> Result<S::Command, CommandDecodeError>` (serde_json), used by ingest and by `StreamCore`, which replaces its private `decode` with it.
- Ingest order: decode as the kind's command (a failure is `Invalid`), `validate`, canonicalize, size check (`max_command_bytes`), then the Redis `INGEST`. Nothing undecodable is ever stored.
- `ensure_hydrated` repopulates Redis from Postgres under the Redis hydration lock: a new generation from `begin_generation`, the commands from the oldest stored snapshot on, and a `Reset` of a Redis copy that is older than the Postgres head.

**Tests** (Redis + Postgres): `ingest_dedupes_command_id`; `ingest_rejects_invalid_oversized_and_nested_commands`; `ingest_canonicalizes_the_command_text`; `open_creates_the_stream_with_the_kind_and_rejects_another_kind`; `an_unknown_kind_is_refused`; `ensure_hydrated_copies_the_tail_from_the_oldest_snapshot`; `ensure_hydrated_resets_a_redis_copy_behind_postgres`; `concurrent_hydrations_converge`; `sweep_orphans_persists_and_retires_idle_streams`.

### S6c: `StreamWorker<S>`, `StreamDispatcher`, the hub's `StreamEgress`, wiring

**Files:** Create `stream_worker.rs`, `stream_egress.rs` (the port) in `command_stream/`, `services/hub/stream_egress.rs`, `services/hub/tasks/stream_dispatcher.rs`; modify `app_state.rs`, `hub_service.rs` (start the dispatcher and give it the egress), `services/hub/mod.rs`.

**Interfaces:** `trait StreamEgress { fn send(&self, connection_id, StreamMessage) -> BoxFuture<bool> }`; `StreamWorker<S>` (the I/O shell around `StreamCore`: delivery on every Redis wake, persistence on a jittered cadence, a periodic tick, reloads with backoff 200 ms to 10 s); `StreamDispatcher` (a hub consumer: routes `StreamEvent`s and `UserDisconnected` to the workers, spawns a worker when a stream is first subscribed and unloads it after `unload_grace` without subscribers, supervises workers in a `JoinSet`, restarts a crashed one at most once per `RESPAWN_DELAY`). A stored command that cannot be decoded stops that stream's worker, which logs it at `error`, counts `command_stream_decode_errors`, and the stream answers `unavailable`.

**Tests** (Redis + Postgres): `a_failed_send_stops_all_later_frames`; `worker_persists_commands_and_stores_a_snapshot_every_k`; `worker_trims_redis_to_the_oldest_stored_snapshot`; `worker_survives_a_redis_outage_and_recovers` (backoff, no `fatal`); `a_stored_command_that_cannot_be_decoded_stops_the_worker_and_the_stream_is_unavailable`; `the_dispatcher_drops_the_streams_of_a_disconnected_connection`; `a_stream_without_subscribers_is_unloaded_after_the_grace_time`; `a_crashed_worker_is_restarted_once_per_respawn_delay` (the test worker ends unexpectedly: the shared test panic hook aborts the run on a real panic).

- [ ] Each part ends with a checkpoint: S6a, then S6b, then S6c.

## Task S7: Wire protocol, snapshot route, resume

**Files:** Create `services/builder/src/routes/command_stream.rs`; modify `routes/mod.rs`, `routes/ws/message.rs`, `routes/ws/connect.rs`, `services/command_stream/{command_stream_service.rs, stream_worker.rs}`, router wiring next to `ws_routes()`. Tests: `routes/command_stream_test.rs`, `routes/ws/connect_test.rs`, worker tests.

**Interfaces:**
- The wire frames are defined once, in `shine-game::replay::protocol` (serde types with plain `String` stream ids, `snapshot.data` as base64, `hash` as 16 hex digits, `command` as a JSON string), so the server and the web client compile against the same definitions; the builder converts its internal `StreamMessage` into them. `Sync` carries `snapshotEvery` (built in S6a, `StreamMessage::Sync::snapshot_every`), so a client hashes its state at the checkpoint versions only.
- Produces: `CommandStreamService::read_snapshot(stream) -> Result<Option<StreamSyncPayload>, CommandStreamError>` (S-D5); `GET streams/{streamId}/snapshot`; the socket parses the client frames of the spec (`resume` included) and sends the server frames; the socket's own replies (nacks, subscribe failures) are written by the socket itself into its egress channel through a `WeakSender` (S5), never through the command loop; resume handling in the worker per S-D6.

- [ ] **Step 1: Failing tests.** `snapshot_route_returns_snapshot_plus_tail`; `snapshot_route_merges_the_redis_tail_above_the_postgres_head` (contiguous, no duplicates); `snapshot_route_without_a_session_is_401`; `snapshot_route_for_an_unknown_stream_is_404`; `snapshot_route_with_storage_down_is_503`; `ws_resume_frame_is_parsed`; `resume_inside_the_window_sends_only_the_tail`; `resume_with_a_changed_generation_sends_a_full_sync`; `resume_older_than_the_base_version_sends_a_full_sync`; `resume_above_the_head_sends_a_full_sync`; `resume_after_a_drop_is_contiguous_or_a_full_sync`; `a_command_for_an_unsubscribed_stream_gets_not_subscribed`; `an_unknown_kind_gets_subscribe_failed_unknown_kind`.
- [ ] **Step 2:** Implement. Metrics: counter `command_stream_snapshot_reads{outcome}`.
- [x] **Step 3: Verify.** Done, with these differences from Step 1: `snapshot_route_without_a_session_is_401` is the extractor's job and moves to the API tests of S9; `resume_after_a_drop_is_contiguous_or_a_full_sync` is covered by the four resume tests; the snapshot route sends `Cache-Control: no-store`. The temporary `allow(dead_code)` markers of S5-S7 are removed. **Step 4: Checkpoint.**

## Task S8: Integration and fault tests (Redis + Postgres)

**Files:** `services/builder/src/services/command_stream/command_stream_test.rs`, fault variants in `services/hub/tests/fault.rs`.

- [ ] **Step 1: Tests** (the `ChatState` registered as a test kind): the plan Task 8 set (ordering across two instances, dedupe window, snapshot race with a drifted loser, generation fencing, orphan sweep, hydration lock) plus `redis_loss_continues_from_the_postgres_head`; `redis_rolled_back_within_a_generation_is_repaired`; `a_slow_subscriber_never_sees_a_version_gap`; `subscribe_unsubscribe_churn_leaves_no_subscription` (500 cycles); `a_postgres_outage_does_not_stop_the_process_and_persistence_resumes` (toxiproxy).
- [x] **Step 2-4:** Done; the tests exposed no defect. Notes: the dedupe, hydration lock and orphan sweep tests already exist from S6b; the stale-generation fence is `a_copy_of_another_generation_is_dropped_by_the_sweep`; tests that cut a database off hold `test_utils::setup_for_outage`, which waits for the other tests with a service (a shared gate) so the outage cannot hit them. The user-facing parts of this list (order seen by two users, dedupe, resume, snapshot route status codes) are repeated black-box in the Playwright API tests of S9; the fault, drift, churn and two-instance tests stay in Rust because they need the stores or toxiproxy.

## Task S9: Example kind - chat, and the switch from the old chat

**Files:**
- Create: `services/builder/src/services/command_stream/chat.rs`, `tests/src/api/builder_stream_ws.ts`, `tests/api-tests/builder/chat_stream.ts`
- Modify: `app_state.rs`, `routes/ws/connect.rs`, `routes/ws/message.rs`, `services/hub/hub_service.rs`, `models/messages/{hub.rs,mod.rs}`, `services/mod.rs`, `repositories/mod.rs`, `server_config.*.json`, `tests/api-tests/builder/ws_connect.ts`
- Delete (second step, after the chat tests pass): `services/chat_service.rs`, `services/hub/tasks/chat_dispatcher.rs`, `chat_dispatcher_test.rs`, `services/hub/tests/chat.rs`, `repositories/chat_comments/`, `models/chat_error.rs`, `models/messages/chat.rs`, `ChatConfig` and the `chat` config blocks

**Interfaces:**
- Consumes: `ChatState`, `ChatCommand` from S1.
- Produces: `pub const CHAT_STREAM_ID: &str = "chat-global"` in `services/command_stream/chat.rs`; the registry `StreamKindRegistry::new().with::<ChatState>()`; chat settings `kinds.chat = { snapshotEvery: 50, keepSnapshots: 5 }` (Redis keeps at most 250 persisted commands). Sockets register no topics; `HubMessage` has `Hub` and `Stream` only.

- [x] **Step 1:** The chat state and its unit tests were built in S1.
- [x] **Step 2: Failing integration tests** (Redis + Postgres): `adding_a_chat_message_is_delivered_to_every_subscriber` (two sockets, same version and `commandId`); `a_joining_client_gets_the_snapshot_and_the_commands_after_it` (12 adds with K = 5: snapshot at 10 plus 2 commands); `snapshots_are_stored_every_k_commands`; `only_the_last_m_snapshots_are_kept`; `redis_keeps_only_the_commands_from_the_oldest_stored_snapshot`; `remove_of_another_users_message_is_stored_and_ignored` (the log grows by one, the message stays on every replica); `remove_of_unknown_id_is_stored_and_ignored`; `a_user_can_remove_their_own_message` (every replica drops it); `a_user_can_replace_their_own_message` (every replica shows the new text, edited); `replace_of_another_users_message_is_stored_and_ignored`; `unsubscribe_stops_the_chat_frames`; `redis_loss_resyncs_the_chat_from_postgres`. Register the kind, set `commandStream.enabled` true in the test config, route the socket through the streams. Checkpoint.
- [x] **Step 3: API tests** (invoke `api-test-writing`). `builder_stream_ws.ts` helper: connect, wait for the first server frame, subscribe, command, HTTP snapshot. `chat_stream.ts`, one test per scenario: gating (no session 401, bad origin 403); add and receive own message; a second user sees it; remove own; remove another user's -> the message stays; remove unknown id -> no state change; HTTP snapshot then subscribe with `resume` receives only the tail; unsubscribe stops frames and the socket stays open; takeover closes the old socket; session expiry closes the socket. Run the service (`local-development`), `cd tests && pnpm test:local`, then format and lint. Checkpoint.
- [x] **Step 4: Remove the old chat** (files listed above); `cargo test --workspace` green. Checkpoint.

## Task S10: Test hardening and doc sync

**Files:** `services/hub/tests/{contention,fault,lifecycle,test_utils}.rs`, `app_state.rs` tests, `docs/services/builder/hub.md`, `docs/plans/hub_issues.md`.

- [ ] **Step 1:** Replace fixed sleeps with barriers or a paused clock in the tests of item 34. Add `heartbeat_enabled_stress_with_300_connections` (heartbeat, session check and stream workers on 200 ms ticks; replay invariant checker; clean shutdown) and `app_state_wires_the_intervals` (the real `AppState::new`).
- [ ] **Step 2:** Run the `shine-skills:sdp` Sync for `hub.md` against the code; update `hub_issues.md` statuses for items 10, 16, 29, 31, 40, 45, 49 and the test items.
- [ ] **Step 3: Final verification.** `cargo fmt --all --check`; `cargo clippy --workspace --all-targets`; `CI=true cargo test --workspace` with both connection strings set; `cd tests && pnpm test:local` with the service running. Expected: all green, no "skipping" lines. Checkpoint.

## Task S11 (decision pending): let the worker prune

`PgCommandLogDb::prune` is built and tested in S2 (it deletes the old snapshots and the commands below the oldest kept snapshot in one transaction) and has no caller. The worker would call it after storing a snapshot, which also starts deleting commands, so the call is switched on only after the decision. Until then S6 leaves snapshots (and commands) unpruned. Tests when wired: `the_worker_prunes_after_storing_a_snapshot`, `hydration_after_pruning_starts_at_the_oldest_snapshot`, `a_resume_below_the_oldest_snapshot_gets_a_full_sync`, `the_snapshot_route_after_pruning_returns_the_newest_snapshot_and_its_tail`.

## Task S12: Client engine in `shine-game` (after S9)

The web client lives in another repository and replays the same states. Its logic that is not UI belongs in `shine-game::replay` (pure, wasm-compatible), next to `Replica`, which it reuses; it does not reuse the server's `StreamCore`, which is mostly persistence, Redis checks and fan-out and stays in the builder.

**Files:** Create `crates/shine-game/src/replay/{client_stream.rs, protocol.rs}` (the protocol types come with S7), tests `crates/shine-game/tests/replay/client.rs`.

**Interfaces:** `ClientStream<S: ReplayState> { authoritative: Replica<S>, pending: Vec<(Uuid, S::Command)>, local: S }` with `submit(command) -> (Uuid, outgoing frame)`, `on_frame(ServerFrame) -> ClientEvent`, and a sync status (`Connecting`, `Syncing`, `Live`, `Resyncing`, `Offline`, `Failed`).
- `Sync` replaces the state (snapshot bytes decoded with `decode_snapshot`, then the commands); `Commands` must be contiguous, a gap means resubscribe; `Reset` means rebuild from the next `Sync`.
- After a `Sync`, pending commands whose id is not in it are resubmitted with the same `commandId`; `Nack duplicate` drops the pending entry, `failed`, `busy` and `notReady` keep it.
- A `Checkpoint` is compared with the client's own hash at that version; a mismatch is a resync. The client hashes its state at the multiples of the `snapshotEvery` the `Sync` announced and keeps the last few hashes, so it can compare a checkpoint that arrives after later commands.

**Tests** (pure, on `ChatState`): a sync then commands reach the server's state; a gap asks for a resync; a reset then a sync rebuilds; pending commands survive a resync and are resubmitted; a duplicate nack drops a pending command; a checkpoint mismatch asks for a resync; the optimistic local view equals authoritative plus pending.

## Self-review notes

- Spec coverage: subscribe/unsubscribe, sync, commands, checkpoints, reset, nack (S5-S7); HTTP snapshot and resume (S7); ownership rule (S1 context, S9 chat `apply`); retention (S2 pruning of snapshots, S4 trimming, S6 worker, S3 settings); chat example (S9).
- Names used across tasks are defined once: `CommandContext`, `base_version`, `StreamId`, `KindSettings`, `keep_from`, `CHAT_STREAM_ID`.
- The old `command-streams-plan.md` stays the reference for SQL, Lua and core algorithms; it keeps a separate chat and the `Uuid` stream id, which this plan replaces.

**S9 notes.** Access is `chat_access()` (`AllowList` with `chat-global` only): users may read and write the global chat, and no other stream can be created. The API tests assert the protocol (ordering, delivery to every subscriber, nacks, resume, snapshot route statuses); what a remove or replace does to the state is asserted in Rust (`chat_test.rs`), because the TS side has no replay engine until S12. A resume is served as a tail only by a worker that already holds the history (another subscriber keeps it loaded); a cold worker answers every subscriber with a full sync. `commandStream.enabled` is true in the test and prod configs: production needs `builder.db.sqlCns` in the key vault before it is deployed. The generic hub tests now use a Stream-topic payload (`payload(id)`) where they used chat batches. `connection_tracker`'s wake support has no production user left (only its tests).
