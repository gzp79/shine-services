# Hub and Command Stream Hardening Plan

This plan covers the open defects in the builder hub, the WebSocket layer, the command streams (Redis, Postgres, worker), and the client stream engine (`crates/shine-game/src/replay`). It also covers the test suite that guards them. Every finding comes from reading the code at commit `17a980ea`, the shipped configuration, and the dependency sources at their `Cargo.lock` versions. Each one was traced step by step and checked by a second reader, but none was reproduced by running anything. Line numbers refer to that commit. Builder paths are relative to `services/builder/src/` unless they start with `crates/`, `services/` or `tests/`.

The design these findings are measured against is `docs/services/builder/hub.md`.

## Legend

| Field | Values |
|---|---|
| **Sev** | **High**: accepted commands lost or duplicated, a stream or the service made unavailable, or a cheap remote DoS. **Med**: delayed or degraded service, or loss under narrow conditions. **Low**: latent, hygiene, performance or documentation. |
| **Status** | **Open**: ready to fix. **Blocked (Dn)**: waits for decision Dn. **Confirm**: changes production behaviour; confirm with the user first. |
| **Cx** | Complexity. **S**: up to half a day, local change, a few tests. **M**: 1-2 days, several files, new tests, no design work. **L**: 3 days or more, or needs a design decision or a cross-component protocol change. |
| **Phase** | The position in the fix order (see *Fix order*). |

## Root causes

Most findings are symptoms of a few root causes. Each root cause is fixed once, by the findings listed under it.

| RC | Root cause | Findings |
|---|---|---|
| **A** | **Replacing a stream's Redis copy is not fenced.** A reset, a re-hydration or a sweep drop does not first make sure the copy's unpersisted commands are in Postgres. A generation change is also not atomic with the copy it replaces. Every path that replaces or drops a copy can lose commands that clients were already told were accepted. | STR-1, STR-2, STR-3, STR-4, STR-5, STR-6, STR-9, RDS-5, RDS-7, RDS-8 |
| **B** | **Command identity depends on retention.** A command id is unique only while Redis still holds it, and that window is the log trim. | STR-8 |
| **C** | **Single-task loops await I/O without bounds.** The stream worker and the hub command loop await Postgres and Redis inline, with no timeout and no backoff. One slow dependency stalls delivery, lifecycle handling or startup. | PG-1, PG-2, PG-3, PG-5, STR-12, STR-13, STR-14, HUB-2, HUB-4, HUB-13, RDS-3, RDS-16 |
| **D** | **Missing budgets and limits.** Frames, unpersisted commands, Redis memory, pending client writes, snapshot reads, connects and per-machine connections have no bound, or a bound far too high. | WS-1, WS-12, WS-13, STR-10, STR-11, STR-16, RDS-4, CLI-6, HUB-14, OPS-4 |
| **E** | **No single owner for the lifetime of a socket or a subscription.** The send task, the receive task, the hub and the dispatcher each hold part of the state. Ending one part does not end the others. | WS-2, WS-3, WS-4, WS-5, WS-6, WS-7, WS-9, WS-16 |
| **F** | **Hub fan-out and registry atomicity.** Lifecycle events are broadcast to every socket. Registry compare-and-set runs as client-side WATCH/MULTI. Lookups and backpressure are ad hoc. | HUB-1, HUB-3, HUB-6, HUB-7, HUB-8, WS-14 |
| **G** | **Background tasks and listeners are unsupervised.** A dead task, a failed first subscribe or a silently lost pub/sub connection goes unnoticed until restart. | HUB-5, HUB-9, HUB-10, HUB-11, RDS-1, RDS-2 |
| **H** | **Process lifecycle and environment guards.** Shutdown has no deadline and does not wait for the hub or the workers. Startup does not check the environment (Redis version, TLS, migrations, config ranges). | OPS-1, OPS-2, OPS-3, OPS-5, OPS-6, OPS-7, PG-4, RDS-14, RDS-15 |
| **I** | **Formats and protocol are unversioned.** Snapshots, state encoding, Redis keys and the socket protocol carry no version, and close codes do not classify drops, so a changed side cannot be detected or handled. | STR-7, CLI-5, HUB-12, WS-8, WS-10, WS-11 |
| **J** | **The client engine does not enforce write order or state invariants.** | CLI-1, CLI-2, CLI-3, CLI-4, CLI-7, CLI-8, STR-17 |
| **K** | **Missing defensive checks** in the scripts, in parsing and in read paths. | RDS-6, RDS-9, RDS-10, RDS-11, RDS-12, RDS-13, STR-15 |
| **L** | **Observability and layering.** | OBS-1, OBS-2, OBS-3, WS-15 |
| **T** | **The test suite** has a flaky test, tests that assert defects, redundancy, wrong layering and gaps. | TST-1 … TST-8 |

## Fix order

The phases run in order. Inside a phase the findings are listed in the order to fix them. Each phase ends with a checkpoint (see *Constraints*).

| Phase | Goal | Findings, in order | Needs | Cx |
|---|---|---|---|---|
| **0. Test baseline** | Make the suite trustworthy before any behaviour changes. | TST-1, TST-7, TST-3 (only the drops that have no API replacement), TST-4 + TST-5 (pin current behaviour in the API) | - | M |
| **1. Cheap guards** | Small, independent fixes that remove a large blast radius. | WS-1 (tungstenite ceiling only), OPS-3, PG-1, PG-2, PG-3, PG-5 (done early), RDS-3, RDS-6, RDS-16 (done early), HUB-4, OBS-1, OBS-3, RDS-14, OPS-6, OPS-5 (Confirm), OPS-4 (Confirm) | - | M in total |
| **2. Durability (RC A)** | No accepted command is ever lost or rewritten. | STR-12 → STR-1 → STR-4 → STR-5 + RDS-7 → STR-3 → STR-2 + RDS-5 + RDS-8 → STR-6 → STR-7 → STR-9, STR-10, STR-11 | D2, D3 | L |
| **3. Dedupe and client order (RC B, J)** | A command id is applied once, and a user's writes land in order. | STR-8 → CLI-2 → CLI-1 → CLI-3 → CLI-7 → CLI-8 → CLI-6 → CLI-4 | D1 | L |
| **4. Socket and session lifetime (RC E)** | A dropped connection stops at once, and subscriptions stay consistent. | WS-3 → WS-2 → WS-4 → WS-5 → WS-6 → WS-7 → WS-9 → STR-14 → STR-13 → STR-16 → STR-15 → WS-16 → WS-15 | - | L |
| **5. Hub registry, fan-out, supervision (RC F, G)** | The hub loop does bounded work, and a dead part is noticed. | HUB-3 + HUB-2 + HUB-12 → HUB-1 → HUB-6 → HUB-7 → HUB-8 → HUB-14 → HUB-9 → HUB-11 → HUB-10 → HUB-5 → RDS-2 → RDS-1 → HUB-13 → WS-14 | D6, D7, D8 | L |
| **6. Process lifecycle (RC H)** | Shutdown closes every socket and saves every stream within a budget. | OPS-1 → OPS-2 → PG-4 → RDS-15, OPS-7 (docs) | - | M |
| **7. Protocol and versioning (RC I)** | Clients can tell drop causes apart, and either side can change safely. | WS-10 → WS-11 → WS-8 → WS-1 (rate limit) → WS-12 → WS-13 → CLI-5 → STR-17 | D4, D5 | L |
| **8. Hygiene** | The rest. | RDS-4, RDS-9 … RDS-13, OBS-2; TST-6 can run at any time | - | M |

Why this order:
- **Phase 0 comes first** because a flaky test hides regressions, and the API tests should pin today's behaviour before it changes.
- **Phase 1 is first among the fixes** because it is cheap and independent, and it removes the worst amplifiers: the 64 MiB frames, hung Postgres calls, and a per-machine limit of 25 connections.
- **Phase 2 is the only data-loss class.** STR-12 opens it because the "flush before reset" fix (STR-4) needs persistence that does not block delivery.
- **Phase 3 changes the wire protocol** (the version in a nack, a `blocked` reason), so the client and server changes land together.
- **Phase 4 goes before phase 5** because both edit `connect.rs` and `stream_socket.rs`, and the session refactor (WS-15) is easier before the hub changes its connection API.
- **Phase 7 waits for the contract decisions** (D4, D5), and touches the client and the API tests in one go.

## Decisions

| # | Decision | Status |
|---|---|---|
| **D1** | **Command id dedupe window (STR-8).** Redis keeps the command ids for a fixed window, by count or by time. The window does not depend on `keep_from` or on the log trim (STR-11). HYDRATE rebuilds the ids for the whole window, not only for its tail. The window size decides how long a client may stay offline and still resend safely, so it is configurable and small in the test config. A unique index in Postgres is ruled out: the duplicate would surface only at append time, after fan-out, as a unique violation that wedges the stream. A synchronous Postgres lookup in INGEST is the costly alternative. | proposed |
| **D2** | **Hydration fencing (STR-2).** Three parts, all required: <br>(1) the generation bump is a compare-and-set (`UPDATE cl_streams ... WHERE generation = $seen`); <br>(2) HYDRATE **and** RESET check the hydration lock token inside the script; <br>(3) before a copy that holds unpersisted rows (`version > persisted`) is replaced, those rows are appended to Postgres. <br>A HYDRATE that only refuses, without (1) and (3), wedges the stream: Postgres is already at G+1 while Redis holds a G copy that can never be appended. | proposed |
| **D3** | **Pruned versus Diverged (STR-1).** `Pruned` means no row exists at that version, at or below the head. This is race-free, because rows are immutable and only the prune deletes them. It is benign, and the same append still stores the rows above the head. `Diverged` is only a row that exists with another id. A truly divergent row that was pruned would be misclassified; the snapshot hash check catches it. | proposed |
| **D4** | **Close codes (WS-10).** 4001: replaced or registry takeover (stay down until the tab is active again). 4002: session invalid (re-authenticate). 4003: registry row missing or registration failure (reconnect with jitter). 1013: slow consumer or dead channel. 1001: shutdown. | proposed |
| **D5** | **Frames and limits (WS-1, WS-8, WS-11, WS-12, WS-13).**<br>- Frames: the first server frame is `hello{version:1, connectionId}`. Input the socket cannot parse or accept gets `error{reason}` with `invalid`, `too_long` or `rate_limited` (stream commands keep `streamNack`). 10 consecutive refused frames close the socket with 1008.<br>- Limits: frame 8 KiB; token bucket of 5 burst + 1 per second per connection; write timeout 10 s; ping every 30 s; idle timeout 90 s; per-user connect throttle 500 ms (429); instance cap of 20 000 connections (503). All are configurable under `builder.ws`.<br>- Oversized frames: tungstenite 0.29 sends no 1009, and the client sees 1005. Choose either a tungstenite ceiling above the app limit (for example 4 × `maxFrameBytes`) with the app answering `too_long`, or an explicit app `Close(1009)` sent through the egress channel.<br>- The test config sets a large token bucket and `connectThrottleMs: 0`, so the existing API tests keep working. | proposed; the oversized-frame choice is open |
| **D6** | **Control queue (HUB-14).** Only the `ConnectUser` lane is bounded (10 000). A full lane refuses connects: a pre-upgrade depth check answers 503, and a connect that still finds the lane full after the upgrade closes with 1013. Registry notices, client disconnects, the connection guard and `request_shutdown` stay lossless, because they have no next tick to rely on. Periodic producers may `try_send`. | decided |
| **D7** | **Registry keys (HUB-12).** Keys and the channel are prefixed and versioned: `{prefix}:v1:conn:<user>` and `{prefix}:v1:registry-changed`, with default prefix `hub`. Not deployed yet, so no migration. | decided |
| **D8** | **Registry check result (HUB-3, WS-10).** The registry check reports `Active`, `Missing` or `Other`. The two drop causes get distinct reasons (`registry_missing`, `registry_inactive`). | decided |
| **D9** | **The registry write stays inline in the command loop,** behind one function, so it can move off the loop later. Connect throughput is not optimised now. | decided (postponed) |

## Status overview

| ID | Finding | Sev | RC | Status | Cx | Phase |
|---|---|---|---|---|---|---|
| STR-1 | Pruned rows are reported as divergence and reset the shared copy | Med | A | Blocked (D3) | M | 2 |
| STR-2 | A stale hydrator wipes a newer copy | High | A | Blocked (D2) | L | 2 |
| STR-3 | A lost Redis key locks out the commands a worker holds | Med | A | Open | L | 2 |
| STR-4 | A Redis reset discards the worker's unpersisted commands | Med | A | Open | L | 2 |
| STR-5 | Postgres going back makes the sweep delete the only copy | Med | A | Open | M | 2 |
| STR-6 | Commands reassigned by a Redis failover are dropped after their echo | Med | A | Open | L | 2 |
| STR-7 | An undecodable snapshot makes the stream reload forever | High | I | Open | M | 2 |
| STR-8 | A command resent after the dedupe window is applied twice | Med | B | Blocked (D1) | M | 3 |
| STR-9 | Only an accepted ingest refreshes the stream TTL | Low | A | Open | M | 2 |
| STR-10 | No global budget for unpersisted commands | Low | D | Open | M | 2 |
| STR-11 | The Redis log is never trimmed until enough snapshots exist | Low | D | Open | S | 2 |
| STR-12 | A Postgres outage slows live delivery to once per 30 s | Med | C | Open | M | 2 |
| STR-13 | Concurrent opens each poll the hydration | Low | C | Open | M | 4 |
| STR-14 | Every subscribe needs Postgres, even for a resident stream | Low | C | Open | S | 4 |
| STR-15 | The snapshot route does not check the Postgres part is contiguous | Low | K | Open | S | 4 |
| STR-16 | The snapshot route has no rate or concurrency limit | Med | D | Open | M | 4 |
| STR-17 | A re-hydration forces every subscriber into a full resync | Low | J | Open | M | 7 |
| PG-1 | No timeout on any Postgres call | Med | C | Done | S | 1 |
| PG-2 | The worker's hole fill reads an unbounded number of rows | Low | C | Open | S | 1 |
| PG-3 | Appends that only verify ids still take the row lock | Low | C | Open | S | 1 |
| PG-4 | Two instances race on the migrations of a fresh database | Low | H | Open | S | 6 |
| PG-5 | A Postgres outage stalls every command-log call for the full pool timeout (30 s), not a fast failure | Med | C | Done | S | 1 |
| RDS-1 | The pub/sub listener never checks its connection is alive | Med | G | Open | M | 5 |
| RDS-2 | A failed first `listen` is permanent | Med | G | Open | S | 5 |
| RDS-3 | The pool PINGs on every checkout | Low | C | Done | S | 1 |
| RDS-4 | The HYDRATE tail is bounded by count, not bytes | Low | D | Open | S | 8 |
| RDS-5 | HYDRATE deletes before it writes | Low | A | Open | S | 2 |
| RDS-6 | Scripts do not guard missing meta fields | Low | K | Done | S | 1 |
| RDS-7 | An unreadable log entry loops the worker or makes the sweep delete the copy | Low | A | Open | S | 2 |
| RDS-8 | The generation of unpersisted entries is not checked | Low | A | Open | S | 2 |
| RDS-9 | `to_meta` turns missing fields into zeros | Low | K | Open | S | 8 |
| RDS-10 | MARK_PERSISTED does not validate `keep_from` | Low | K | Open | S | 8 |
| RDS-11 | The unreadable-entry log does not name the field | Low | K | Open | S | 8 |
| RDS-12 | `idle_streams` drops unparsable ids silently | Low | K | Open | S | 8 |
| RDS-13 | An unparsable wake wakes every stream | Low | K | Open | S | 8 |
| RDS-14 | Nothing checks the Redis version or eviction policy | Low | H | Open | S | 1 |
| RDS-15 | Redis Cluster is unsupported but undocumented as a limit | Low | H | Open | S | 6 |
| RDS-16 | A Redis outage stalls `ingest` for the full pool timeout (30 s), not a fast nack | Med | C | Done | S | 1 |
| HUB-1 | Every socket gets every lifecycle event | High | F | Open | S | 5 |
| HUB-2 | Serial registry round trips on the command loop, without a timeout | High | C | Open | M | 5 |
| HUB-3 | An aborted registry transaction becomes an error | Low | F | Open | M | 5 |
| HUB-4 | A long tick starves lifecycle events | Low | C | Done | S | 1 |
| HUB-5 | The command loop and the consumers are unsupervised | High | G | Open | M | 5 |
| HUB-6 | `remove_connection_by_id` scans every connection | Low | F | Open | S | 5 |
| HUB-7 | Every foreign registry notice becomes a command | Low | F | Open | S | 5 |
| HUB-8 | Backpressure is handled inconsistently | Med | F | Open | M | 5 |
| HUB-9 | The tracker's wake path has no production caller | Low | G | Open | S | 5 |
| HUB-10 | A hub dropped without Shutdown leaks its tasks | Low | G | Open | S | 5 |
| HUB-11 | Dead code and API misuse in the hub | Low | G | Open | S | 5 |
| HUB-12 | Registry keys are not prefixed or versioned | Low | I | Open | S | 5 |
| HUB-13 | Startup waits for the stream dispatcher's listener | Low | C | Open | S | 5 |
| HUB-14 | The control queue is unbounded | Med | D | Open | M | 5 |
| WS-1 | 64 MiB frames, no rate limit | High | D | Blocked (D5) for the rate limit | S + M | 1, 7 |
| WS-2 | No write timeout, ping or idle timeout | Med | E | Open | M | 4 |
| WS-3 | A dropped connection keeps writing | Med | E | Open | M | 4 |
| WS-4 | No guard ends the connection when the socket task is cancelled | Low | E | Open | S | 4 |
| WS-5 | A refused subscription stays writable | Low | E | Open | M | 4 |
| WS-6 | A failed re-subscribe leaves the old subscription live | Low | E | Open | S | 4 |
| WS-7 | A reply is lost silently on a full channel | Low | E | Open | S | 4 |
| WS-8 | Input the socket cannot accept is ignored without an answer | Med | I | Blocked (D5) | S | 7 |
| WS-9 | A registration failure closes with no code | Low | E | Open | S | 4 |
| WS-10 | Every drop closes with 4000 | High | I | Blocked (D4) | M | 7 |
| WS-11 | No protocol version or hello frame | Low | I | Blocked (D5) | M | 7 |
| WS-12 | No connect throttle | Med | D | Blocked (D5) | M | 7 |
| WS-13 | No instance connection cap; reconnect storms are amplified | High | D | Blocked (D5) | M | 7 |
| WS-14 | Stream frames are serialized per socket | Low | F | Open | S | 5 |
| WS-15 | The stream session orchestrates services inside the routes layer | Low | L | Open | M | 4 |
| WS-16 | Stream access is checked once and cached | Low | E | Open | S | 4 |
| CLI-1 | A transient nack reorders a user's writes; the later one is lost | Med | J | Open | M | 3 |
| CLI-2 | A `duplicate` nack drops the write before its echo | Low | J | Open | S | 3 |
| CLI-3 | After a checkpoint mismatch the client can resume on the diverged state | Low | J | Open | S | 3 |
| CLI-4 | No way to seed the client from the HTTP snapshot | Low | J | Open | S | 3 |
| CLI-5 | No forward compatibility in the client protocol | Low | I | Open | M | 7 |
| CLI-6 | The pending-write queue is unbounded | Low | D | Open | S | 3 |
| CLI-7 | An undecodable command mid-batch leaves a stale view | Low | J | Open | S | 3 |
| CLI-8 | Chat text is not trimmed; invisible text passes | Low | J | Open | S | 3 |
| OPS-1 | Shutdown has no deadline on the production path and closes sockets one by one | High | H | Open | M | 6 |
| OPS-2 | Shutdown does not wait for the workers' final persist | Low | H | Open | S | 6 |
| OPS-3 | Unchecked numeric config (a huge heartbeat breaks every connect) | Low | H | Done | S | 1 |
| OPS-4 | Fly caps a machine at 25 connections | Med | D | Confirm | S | 1 |
| OPS-5 | Prod origins trust the dev site | Low | H | Confirm | S | 1 |
| OPS-6 | TLS to Postgres and Redis is not enforced | Low | H | Open | S | 1 |
| OPS-7 | The host allow-list trusts client headers | Low | H | Open | S | 6 |
| OBS-1 | Registry per-event spans are `info` | Low | L | Done | S | 1 |
| OBS-2 | Metric and status gaps | Low | L | Open | S | 8 |
| OBS-3 | Per-tick warn/error logs during outages | Low | L | Done | S | 1 |
| TST-1 … TST-8 | Test suite (see *Test plan*) | High … Low | T | Open | M | 0, then with each fix |

---

## Findings

Each finding states where the defect is, what goes wrong, the fix, and the tests (with their layer; see *Test plan*). Test names in backticks are new Rust tests unless marked API.

### RC A: replacing the Redis copy is not fenced

**STR-1. Pruned rows are reported as divergence and reset the shared copy.** `stream_core.rs:313`, `crates/shine-infra/src/db/command_log/pg_command_log.rs:379`, `command_stream_service.rs:462`.
- **Problem:** a `Diverged` append makes the worker reset the Redis copy that every instance shares. But `Diverged` is also reported for a row that another instance persisted and then pruned. Scenario: instance B keeps reading Redis while only its appends fail, typically because its own pool times out, for about 200-250 versions (prod chat: `snapshotEvery` 50 × `keepSnapshots` 5). Meanwhile instance A persists, stores newer snapshots and prunes. When B retries, its old rows fall below the horizon, the append reports `Diverged`, and B resets the copy. Commands that no worker had read yet are lost, and every client resyncs. The sweep drops the copy on the same outcome, but only after repeated MARK failures. A frozen B (PG-1) reloads through the hole fill instead and resets nothing.
- **Fix:** D3. `Pruned` is benign, and the same append stores the rows above the head.
- **Tests:** `a_pruned_row_is_not_divergence` (two instances, made deterministic with `PersistNow`); pg `a_pruned_row_is_reported_as_pruned`; a `Pruned` row in the core outcome test; a sweep variant. Flips the TST-2 rows for STR-1.

**STR-2. A stale hydrator wipes a newer copy.** `repositories/command_stream/redis/scripts.rs:42`, `redis_command_stream_db.rs:11`, `command_stream_service.rs:415`.
- **Problem:** the hydration lock lasts 5 s, is never renewed, and does not stop a slow holder from writing after it expires. HYDRATE overwrites any copy of a lower generation, even one holding unpersisted commands, and the RESET in `hydrate_locked` checks only the generation. Any stall over 5 s triggers it, most likely a 30 s pool checkout. Scenario: A stalls, B takes the lock and finishes G+1, and clients write to G+1. A then bumps to G+2 without re-reading meta, and its RESET and HYDRATE wipe G+1 with the accepted commands. Workers holding them get `StaleGeneration` and reload.
- **Fix:** D2: compare-and-set on the bump, the lock token checked in HYDRATE and RESET, and persist before replace. A `lock_timeout` on `begin_generation` also helps, but it cannot bound a pool checkout.
- **Tests:** script test `hydrate_refuses_a_copy_with_unpersisted_commands`; pg `begin_generation_times_out_behind_a_held_row_lock`; `a_late_hydrate_never_overwrites_unpersisted_commands` (two hydrators, stall hook, short lock TTL). Flips the TST-2 row for STR-2.

**STR-3. A lost Redis key locks out the commands a worker holds.** `stream_worker.rs:457-467`, `command_stream_service.rs:338`.
- **Problem:** when a key is lost (eviction, failover), the next ingest gets `not_ready`, and background hydration bumps the Postgres generation at once. No wake is sent. A worker notices within 20-40 ms through its persist timer and sometimes wins the race. When it loses, its append gets `StaleGeneration` and it reloads, discarding the commands it held. If the worker's own save fails while Postgres is still reachable, it calls `ensure_hydrated` and locks out its own commands.
- **Fix:** hydration publishes a wake and gives resident workers a grace period to flush before it bumps. A failed save retries in place and never calls `ensure_hydrated`. `begin_generation` accepts appends of the previous generation for rows not yet in Postgres, up to a seal point, and HYDRATE reads the head only after the seal. Without the seal, late rows of the old generation would take versions ahead of the new copy, which would then diverge and be reset.
- **Tests:** `a_lost_key_does_not_lock_out_held_commands` (must force the ingest-first order); `a_failed_save_does_not_bump_the_generation` (toxiproxy on Postgres).

**STR-4. A Redis reset discards the worker's unpersisted commands.** `stream_worker.rs:518-545`, `stream_core.rs:175,249`.
- **Problem:** the `Fault::Reset` paths ("Redis is behind the applied commands", "Redis lost an applied version") reset Redis and reload without first saving the worker's unpersisted commands, and those are exactly what Redis lost. `persist_cycle` checks Redis before appending (`:266-274`), so the rows never reach Postgres, and `begin_load` clears the unpersisted list. The loss is bounded by the replication lag that overlaps the persist window.
- **Fix:**
  1. Freeze the lagging copy, so new ingests cannot reuse the held versions.
  2. Append the unpersisted commands, then reset.
  3. Carry the pending flush as worker state across steps, retried with backoff (STR-12), never as a blocking retry inside `step()`.
  4. `begin_load` keeps the rows that are not in Postgres yet.

  Commands whose version was reassigned are handled by STR-6.
- **Tests:** `a_reset_after_failover_keeps_unpersisted_commands`, one case per reset reason; a core test that `begin_load` keeps rows not yet in Postgres. Flips the TST-2 row for STR-4.

**STR-5. Postgres going back makes the sweep delete the only copy.** `command_stream_service.rs:410-416,452-463`, `pg_command_log.rs:101-109`.
- **Problem:** this applies when Postgres fails over to an asynchronous standby that lacks the last commits; it does not happen with synchronous HA. `persist_tail` starts at `meta.persisted`, so with the Postgres head below it the append returns `Gap`, and `drop_copy` deletes the Redis copy. That copy was the only one still holding the lost commits and everything written during the outage. `hydrate_locked` likewise resets a copy whose generation is above Postgres. The generation number is then reused, so an old resume token can pass `can_resume` on a different history. A worker that appends first recovers correctly (Gap, reload, re-apply from Redis).
- **Fix:** `persist_tail` starts at `min(meta.persisted, head.version)`. A copy is never dropped on `Gap`. A Redis generation above the Postgres one moves Postgres forward, so a generation is never reused. Postgres going back logs at `error`.
- **Tests:** `postgres_going_back_never_drops_the_redis_copy` (delete the last rows and lower the head, then run the sweep and a hydration); `a_generation_is_never_reused_after_postgres_went_back`.

**STR-6. Commands reassigned by a Redis failover are dropped after their echo.** `pg_command_log.rs:378`, `crates/shine-game/src/replay/client_stream.rs:276`.
- **Problem:** Postgres holds 106..110 from worker A. Worker B, at 105, applies the new primary's 106'..112' and fans them out, so their writers drop them from pending. B's append returns `Diverged`, B resets, and the new Sync no longer contains them: acknowledged commands are lost silently. Appending first (STR-4) cannot help, because this append diverges by construction.
- **Fix:** on divergence, re-ingest the divergent unpersisted commands, by command id and with new versions, into the rebuilt copy. Dedupe keeps this idempotent. Alternatively, the client keeps echoed writes until a persisted watermark, which needs a protocol field.
- **Tests:** `divergent_unpersisted_commands_are_resequenced` (two workers; the Redis log above the Postgres head is rewritten with other ids). Every acknowledged id ends up in Postgres exactly once.

**STR-9. Only an accepted ingest refreshes the stream TTL.** `scripts.rs:15,20-29`, `command_stream_service.rs:442-444`, `settings/command_stream.rs:87,92`.
- **Problem:** `busy`, MARK_PERSISTED, reads and the sweep never refresh the TTL (24 h by default), and nothing checks `orphanIdle` against it.
  - With Postgres down for more than the TTL and no resident worker, writers get `busy`, the keys expire, and up to `redisMaxUnpersisted` accepted commands vanish. `persist_tail` then reports `Persisted` and retires the stream silently.
  - A stream that is only read for longer than the TTL expires and re-hydrates, and every resume token goes stale (STR-17).
- **Fix:** keys never expire while `version > persisted` (refresh in MARK and on `busy`); refresh while the stream is resident; validate `orphanIdle < ttl`; warn when meta is missing for a listed stream.
- **Tests:** `unpersisted_keys_do_not_expire` (short TTL, Postgres down); `a_read_only_resident_stream_keeps_its_keys`; a config test for `orphanIdle < ttl`.

**RDS-5. HYDRATE deletes before it writes.** `scripts.rs:43,50`.
- **Problem:** the log, ids and meta are deleted before the new copy is written. No Lua error is reachable after the delete today (`check_tail` validates the tail, and OOM is checked only before the first write), and a partial log has no meta, so nothing reads it. This is hygiene that becomes relevant once HYDRATE is chunked (RDS-4).
- **Fix:** validate all arguments first, write the new copy, and `HSET` meta last, with no meta `DEL`. Do it together with D2.
- **Tests:** `hydrate_validates_before_writing`, which calls the script directly and bypasses `check_tail`.

**RDS-7. An unreadable log entry loops the worker or makes the sweep delete the copy.** `redis_command_stream_db.rs:309`, `stream_worker.rs:477`.
- **Problem:** `parse_entry` drops an entry it cannot read. Postgres refills the gap only if the bad entry is the first one above the tip and already persisted. Otherwise the worker reloads in a loop with a misleading reason ("behind the retained history"). Through the sweep, `persist_tail` hits `Gap` and `drop_copy` deletes the accepted commands above the bad entry. This needs corruption: unlikely, but high impact.
- **Fix:** an unreadable entry is one bounded fault (an `error` log and a metric) on both the worker path and the sweep path. The sweep never drops a copy on `Gap` (STR-5). The worker uses the `Unavailable` exit of STR-7.
- **Tests:** `an_unreadable_log_entry_is_one_bounded_fault`.

**RDS-8. The generation of unpersisted entries is not checked.** `redis_command_stream_db.rs:107`, `scripts.rs:46`.
- **Problem:** the generation stored with each entry is parsed but never validated. HYDRATE writes each entry's original Postgres generation, so hydrated entries legitimately carry older generations. Only the unpersisted entries above the hydrated head must match the meta generation.
- **Fix:** check exactly that; a mismatch is a fault.
- **Tests:** `an_unpersisted_entry_of_another_generation_is_a_fault`.

### RC B: command identity depends on retention

**STR-8. A command resent after the dedupe window is applied twice.** `crates/shine-game/src/replay/client_stream.rs:249`, `scripts.rs:89`.
- **Problem:** after a full Sync the client resends every pending write whose id is not in the Sync's command list. It cannot see writes that were folded into the snapshot. Server-side dedupe covers only the ids still in Redis: MARK_PERSISTED trims them below `keep_from`, and HYDRATE rebuilds them only for its tail.
- **Scenario:** a client's write is accepted at version 100, and the client disconnects before seeing it. More than about 200-250 versions later it reconnects, the resume is refused, and the Sync's snapshot already contains version 100. The resent id is accepted again, and the chat message appears twice with the same id. A user can also force the trim on purpose (about 250 no-op removes) and then reuse a victim's public command id. On top of this, `client_stream.rs:250,276` clears pending writes by command id without checking the user.
- **Fix:** D1. The client also matches the user when it clears pending writes.
- **Tests:** API "Chat shall refuse a command id resent after its version left the Redis window" (send X, write 11 or more commands, wait for a checkpoint, resend X, expect `duplicate`; it fails today); a layer test for the D1 window. Flips the TST-2 rows for STR-8.

### RC C: single-task loops await I/O without bounds

**PG-1. No timeout on any Postgres call.** `crates/shine-infra/src/db/command_log/pg_command_log.rs:19`, `crates/shine-infra/src/db/postgres/pg_connection.rs:334-347`, `stream_worker.rs:262-443`. — **Done**
- **Problem:** only `append` sets `lock_timeout` and `idle_in_transaction_session_timeout`. There is no `statement_timeout`, no `tcp_user_timeout` (tokio-postgres keepalives default to 2 h), and no `tokio::time::timeout`. After a failover or a network blackhole, a query hangs on a half-open connection. The worker awaits it inside `step()`, so for 15 min to 2 h it delivers nothing, ignores `Stop`, and keeps the connection checked out. Workers are detached, so shutdown itself does not hang, but the final persist is lost, and an unload can start a second worker beside the stuck one.
- **Fix:** `apply_session_timeouts` (`pg_connection.rs`) sets `statement_timeout=10000` (via `options=-c ...`) and `tcp_user_timeout=10s` as session defaults, unless the connection string already sets its own. Both are well below the pool's default `pool_timeout` (30 s). The separate `tokio::time::timeout` wrapper the fix text asked for already exists as PG-5's `CLIENT_TIMEOUT` around the pool checkout — that covers checkout contention, this covers a query stuck on a connection already checked out; together they bound every path. The migration connection shares the pool, so `PgConnection::migrate` lifts the session default (`SET statement_timeout = 0`) for the run and restores it (`RESET statement_timeout`) after, so the connection is unaffected once back in the pool.
- **Tests:** `session_timeout_defaults_are_applied_when_unset`, `an_explicit_tcp_user_timeout_is_not_overridden`, `an_explicit_statement_timeout_is_not_overridden`, `other_options_keep_the_statement_timeout_appended` (`pg_connection.rs`, pure config, no server) — the config-building logic. `a_stalled_postgres_query_does_not_freeze_the_worker` (`stream_worker_test.rs`, toxiproxy `timeout` toxic on an already-established, single-connection pool) — the worker recovers within ~2.3 s and keeps delivering; empirically the bound comes from whichever of PG-1's session timeout or PG-5's checkout timeout fires first, which the test treats as an implementation detail, not something to pin down.

**PG-2. The worker's hole fill reads an unbounded number of rows.** `stream_worker.rs:479-483`.
- **Problem:** `commands_after(tip, first.version - expected)` is not capped by `PG_READ_BATCH`. This is latent until a `keepHistory` kind exists: a worker 100 k versions behind would load about 1.6 GB in one result set. For pruned kinds, Postgres returns only the retained rows.
- **Fix:** `.min(PG_READ_BATCH)`; the existing `continue` already loops.
- **Tests:** `the_hole_fill_reads_in_batches`.

**PG-3. Appends that only verify ids still take the row lock.** `pg_command_log.rs:359-388`.
- **Problem:** every instance re-appends the same rows, and each append takes `FOR UPDATE` on `cl_streams` even when every row is at or below the head. Each lock is held for milliseconds, but behind a slow holder the verify-only appends exceed the 5 s `lock_timeout`, and the unpersisted backlog grows (feeding STR-1).
- **Fix:** read the head without a lock first. If every row is at or below it, verify the ids without locking and return `UpToDate`.
- **Tests:** `an_append_of_persisted_rows_takes_no_row_lock`.

**STR-12. A Postgres outage slows live delivery to once per 30 s.** `stream_worker.rs:187,244,277`, `pg_connection.rs:311,341`.
- **Problem:** `deliver` re-arms `persist_due` 20-40 ms after each failure, and the biased select then runs `persist_cycle`, which blocks for the 30 s pool timeout. Deliveries, `AddSubscriber` and `Stop` all wait. `stream_integration_test.rs:283-284` hides this by setting `pool_timeout=2`.
- **Fix:** exponential backoff of persist after a failure; take persistence off the delivery path (a separate task, or a short checkout timeout for the worker's pool calls).
- **Tests:** `a_postgres_outage_does_not_slow_delivery`, with the default 30 s pool timeout. Remove the override (TST-2).

**STR-13. Concurrent opens each poll the hydration.** `command_stream_service.rs:370-397`.
- **Problem:** each `open()`/`load()` polls `ensure_hydrated` on its own (Postgres head, Redis meta and `SET NX` every 20 ms, for up to 10 s). The in-process set covers only background hydration. The 10-connection pool caps the load, so the effect is pool starvation of the hydrator, which can push it past the 5 s lock (STR-2). Each waiting socket's frame loop is blocked.
- **Fix:** one in-flight hydration per stream (a shared future or `Notify`), and jittered backoff for the cross-instance wait.
- **Tests:** extend `command_stream_test.rs:232` (concurrent opens) to assert one hydration and a bounded call count.

**STR-14. Every subscribe needs Postgres, even for a resident stream.** `routes/ws/stream_socket.rs:409`, `command_stream_service.rs:166,374`.
- **Problem:** `open()` always calls `ensure_stream` (a Postgres write) and `ensure_hydrated` (a Postgres head read). During a Postgres outage, every resume, gap resync, slow-consumer reconnect and deploy move fails after up to 30 s, and blocks that socket's frame loop meanwhile.
- **Fix:** skip both calls when the stream is resident with a matching kind.
- **Tests:** `a_resident_stream_resubscribes_without_postgres` (toxiproxy). Optional API `@infrastructure`: "Chat resume during a Postgres outage shall succeed for a loaded stream".

**HUB-2. Serial registry round trips on the command loop, without a timeout.** `repositories/hub_registry/redis/redis_hub_registry.rs`, `services/hub/hub_service.rs`.
- **Problem:** about 2 round trips per connect and 6 per disconnect (PING, WATCH, GET, MULTI/DEL/EXEC, UNWATCH, PUBLISH), with no hub-level timeout. The registry shares the 10-connection pool (30 s pool timeout) with the stream logs. The heartbeat is one unchunked `MGET` plus a pipeline of N `EXPIRE`s, so a Redis stall blocks the single command loop.
- **Fix:**
  - a dedicated registry pool;
  - Lua compare-and-set scripts with one round trip each (HUB-3);
  - the heartbeat chunked at 256 keys;
  - a 2 s `tokio::time::timeout` per registry call (safe only after the Lua rewrite, because a timeout would cancel a WATCH sequence mid-flight);
  - explicit `timeout`/`pool_timeout` in the production connection string.
- **Tests:** `batch_heartbeat_chunks_cover_the_edges` (255, 256 and 257 connections; replaces `lifecycle.rs:591`); `a_registry_operation_times_out_in_two_seconds` (toxiproxy).

**HUB-4. A long tick starves lifecycle events.** `services/hub/tasks/connection_tracker.rs`. — **Done**
- **Problem:** the tracker loop is `biased` towards its ticker with `MissedTickBehavior::Delay`, so a tick that lasts at least one interval is due again at once, and lifecycle events are never read. The session checker hits this during a Redis outage with more than 256 connections, or after any stall of 2 s or more with the test config. It heals after the outage.
- **Fix:** `drain_pending` applies every already-queued lifecycle event (capped at 1 024) before running a due tick, on both the ticker and the wake branch of the select.
- **Tests:** `ticks_longer_than_the_interval_still_apply_lifecycle_events` (a tick that always sleeps three times its own interval; written to fail first — it hung without the fix — then verified green after it).

**HUB-13. Startup waits for the stream dispatcher's listener.** `HubService::new`, `services/hub/tasks/stream_dispatcher.rs`.
- **Problem:** `StreamDispatcher::start` is awaited inside `HubService::new`, so a pool stall delays startup by up to 30 s.
- **Fix:** start the listener in the background with the RDS-2 retry.
- **Tests:** covered by RDS-2's tests.

**RDS-3. The pool PINGs on every checkout.** `crates/shine-infra/src/db/redis/redis_connection.rs:163,264-268`. — **Done**
- **Problem:** bb8's default `test_on_check_out = true` sends a PING on every `pool.get()`, which doubles the round trips of every ingest and read.
- **Fix:** `test_on_check_out(false)` plus a shared broken flag (`Arc<AtomicBool>`) read by `has_broken`, set from `ConnectionLike::req_packed_command`/`req_packed_commands` when a result's `is_connection_dropped()` or `is_unrecoverable_error()` is true — not on a plain response timeout. Without it, dead connections stay pooled for 10-30 min, because redis 1.7.1 has no public `is_closed`.
- **Found along the way:** this trades the double round trip for a one-shot cost: a connection that died invisibly while idle (e.g. every pooled connection through a disabled toxiproxy) now fails once, on its first reuse after the outage, instead of being caught at checkout. The failing call gets a normal nack/error — nothing wedges — and the connection is evicted right after (bb8 checks `has_broken` when a connection is returned). Production-shaped (one client, steady traffic) self-heals on the very next call; the test suite's fault-injection tests needed the same tolerance a real client already has to have: `find_registry_connection`/`delete_key` (`hub/tests/test_utils.rs`) retry a few times, `chat_stream_failures.ts`'s post-recovery write retries once, and `StreamSocket.open` (`builder_stream_ws.ts`) retries a 503 a few times before giving up.
- **Tests:** `a_pool_checkout_sends_no_ping` (Redis `INFO commandstats`; a single-connection pool forces the second checkout to reuse the first connection).

**RDS-16. A Redis outage stalls `ingest` for the full pool timeout (30 s), not a fast nack.** `repositories/command_stream/redis/redis_command_stream_db.rs:152-158` (`client()` → `self.0.pool.get()`), `crates/shine-infra/src/db/redis/redis_connection.rs:228,249,263,266` (`pool_timeout` default 30 000 ms, no override on the builder's command-stream pool). — **Done**
- **Problem:** found while writing the API test for "a Redis outage nacks writes" (TST-4). `ingest`'s `pool.get()` carried no `tokio::time::timeout` of its own, so when every pooled connection is broken and a fresh one can't be established, bb8 blocked the whole call for up to `pool_timeout` (30 s, unset in `server_config.test.json`) before returning `PoolError::TimedOut`, which only then became a `failed` nack. The socket itself stays open and usable (confirmed live: `streamSync`/`streamCommands` worked before and after), so there was no gap or reset — but a client that simply tried to write during an outage waited 30 s for its nack, not milliseconds. The worker's own `persist_delay` step hit the identical stall (`Internal { source: PoolError(TimedOut) }`), which is the Redis analogue of STR-12's Postgres finding, on the same root cause (RC C); `client()` is shared by every `RedisCommandStreamDb` method, so the same fix covers both.
- **Fix:** `client()` now wraps `self.0.pool.get()` in a 2 s `tokio::time::timeout`, well under the pool's 30 s, and maps a timeout to the same `RedisError::PoolError(RedisConnectionError::TimedOut)` the pool itself already returns — no new error variant needed, no behaviour change on the success path.
- **Tests:** verified live against toxiproxy (`enabled:false` on the `redis` proxy): the nack now lands in ~2 s instead of ~30 s. `cargo test -p shine-builder` (145 passed), `cargo fmt`/`clippy` clean. API: `chat_stream_failures.ts`'s "Chat stream shall nack writes during a Redis outage..." pins the now-fast nack.

### RC D: missing budgets and limits

**WS-1. 64 MiB frames, no rate limit.** `routes/ws/connect.rs:96`.
- **Problem:** tungstenite's defaults allow 64 MiB per message and 16 MiB per frame. A command is checked against `maxCommandBytes` only after the whole frame is buffered and JSON-parsed, and there is no rate limit. An authenticated client sending 64 MiB frames on several sockets exhausts memory and CPU.
- **Fix:**
  - phase 1: set a tungstenite ceiling (for example 64 KiB);
  - phase 7: add `maxFrameBytes` and the token bucket from D5.
- **Tests:** API (`ws_protocol.ts`) "WS frame larger than the limit shall be refused" (with the D5 choice of `too_long` or 1009); Rust `burst_then_refill` (token bucket unit) and `a_client_faster_than_the_rate_limit_gets_rate_limited` (a route test with small limits; the API config keeps the limits off).

**WS-12. No connect throttle.** `routes/ws/connect.rs`.
- **Problem:** every tab focus switch is a disconnect plus a connect. Each reconnect costs an upgrade, two tasks, a channel, a registry SET + PUBLISH, a replaced drop and events, and there is no per-user throttle.
- **Fix:** a per-user throttle before `ws.on_upgrade` (D5).
- **Tests:** `a_user_reconnecting_too_fast_is_throttled` (Rust route test; 429).

**WS-13. No instance connection cap; reconnect storms are amplified.**
- **Problem:** after a deploy or a load-balancer flip, all clients reconnect at once. Jittered backoff is only a documented client rule, there is no connection cap, and shutdown closes sockets one by one (OPS-1).
- **Fix:** a per-instance cap (503 before the upgrade, 1013 after), aligned with OPS-4; jittered backoff in the client contract; a staggered 1001 on shutdown.
- **Tests:** `connects_above_the_instance_cap_get_503`.

**STR-10. No global budget for unpersisted commands.**
- **Problem:** unpersisted commands are capped per stream only (`redisMaxUnpersisted` 5 000). With one stream and the 2000-byte chat limit that is about 10-20 MB, so the risk is latent today. With many streams during a Postgres outage, Redis (`noeviction`) runs out of memory, and every write fails, the hub registry included.
- **Fix:** a byte cap per stream, a global budget, and a Redis memory alert.
- **Tests:** `the_unpersisted_byte_budget_refuses_with_busy`.

**STR-11. The Redis log is never trimmed until enough snapshots exist.** `command_stream_service.rs:153-156`, `scripts.rs:86-90`.
- **Problem:** while a stream has fewer than `keepSnapshots` snapshots, `keep_from` stays 1, and MARK trims nothing. If snapshots keep failing, a hot stream holds its whole history in Redis.
- **Fix:** also bound the persisted window by count, for example `max(keep_from, persisted - snapshotEvery × (keep + 1))`.
- **Tests:** `the_redis_log_is_trimmed_without_snapshots`.

**STR-16. The snapshot route has no rate or concurrency limit.** `routes/command_stream.rs:64-79`, `command_stream_service.rs:201-268`.
- **Problem:** each GET does a session lookup, a head read, `latest_snapshot` (with its data), `commands_after` and a Redis page-through of up to 5 000 entries, with no cap and no cache. One user who fills chat (about 550 KB per response) and runs 20-30 parallel loops keeps all 10 Postgres connections busy. Subscribes then fail, hydration returns `NotReady`, persistence stalls, and ingest answers `busy` for everyone.
- **Fix:** a per-user limiter or semaphore on the route, a short per-stream cache of the built frame, and a pool share (a semaphore) for snapshot reads.
- **Tests:** API "Chat snapshot route shall throttle a user's parallel requests" (429); Rust `snapshot_reads_cannot_take_the_whole_pool`.

**RDS-4. The HYDRATE tail is bounded by count, not bytes.** `command_stream_service.rs:421`, `redis_command_stream_db.rs:212-219`.
- **Problem:** the tail goes in one EVALSHA, which is sent again on NOSCRIPT. The defaults give about 6.5 MB, which already exceeds what the 500 ms default response timeout comfortably allows. `snapshotEvery` 1000, `keepSnapshots` 3 and 16 KiB commands would give about 65 MB. A timed-out HYDRATE still completes in Redis, so the impact is a failed `open`.
- **Fix:** validate the product at config load, or write the tail in chunks. Chunks write meta last (RDS-5) and finish within the hydration lock.
- **Tests:** `a_large_hydrate_tail_is_refused_or_chunked`.

**CLI-6. The pending-write queue is unbounded.** `crates/shine-game/src/replay/client_stream.rs:160`.
- **Problem:** `rebuild_local` replays every pending write on every frame (even empty ones) and on every final nack, and `submit` accepts writes while Failed.
- **Fix:** cap the queue (refuse `submit` beyond it with an event), and refuse writes while Failed.
- **Tests:** unit `the_pending_queue_is_bounded`.

**HUB-14. The control queue is unbounded.**
- **Problem:** every connect is queued without limit, so a reconnect storm grows the queue and the latency of every command.
- **Fix:** D6.
- **Tests:** `a_full_control_queue_refuses_new_connects`.

**OPS-4. Fly caps a machine at 25 connections.** `services/fly.json:29-32`.
- **Problem:** `concurrency.hard_limit` is 25, and the limit type defaults to connections. Every builder socket and every identity HTTP connection on a machine share those 25 slots. 25 users, or one actor holding 25 idle connections to the unauthenticated `/ping`, fill a machine, and new logins and sockets fail until another machine starts.
- **Fix:** `type = "requests"` for HTTP, or a separate fly service for the WebSocket endpoint with a connection limit that matches the instance cap (WS-13). This changes production capacity, so confirm it with the user first.
- **Tests:** none (deploy config).

### RC E: no single owner for socket and subscription lifetime

**WS-3. A dropped connection keeps writing.** `routes/ws/connect.rs:191-221`, `routes/ws/stream_socket.rs:148-181`.
- **Problem:** when the hub drops a connection it only drops the egress sender. The receive task is aborted only after the send task exits. A client that stops reading blocks the send task in `ws_sender.send` indefinitely, and until then its commands and subscribes are still ingested. A revoked session keeps write access, against the promise that `authCheckInterval` is the longest a revoked session keeps its socket.
- **Fix:** refuse every command and subscribe once the hub has dropped the connection, and stop the receive task right away. The signal comes from the hub side (`WeakSender::upgrade()` is `None`, or a token dropped with the removed connection), because the stuck send task never reads its own close notice.
- **Tests:** Rust `a_dropped_connection_cannot_write` (the hub drops the connection while its sink is stalled); API "WS connection with a revoked session shall not accept commands after the drop" (delete the session, wait for the close, and check over the snapshot route that a command sent meanwhile did not land).

**WS-2. No write timeout, ping or idle timeout.** `routes/ws/connect.rs` (`forward_egress`).
- **Problem:** `forward_egress` logs send errors and keeps looping, and `ws_sender.send` has no timeout. There is no server ping and no read-idle timeout. A zero-window client or an idle half-open client pins its socket, two tasks and up to 1000 queued frames for hours. `SEND_DRAIN_TIMEOUT` covers only the client-close path.
- **Fix:** end the socket on a send error; a write timeout (10 s); a ping every 30 s; a read-idle timeout (90 s). Make the timeouts configurable, so tests can use short real values instead of a paused clock, which fires Redis timeouts spuriously.
- **Tests:** `a_silent_client_is_closed_after_the_idle_timeout`; `a_client_that_does_not_read_is_closed_after_the_write_timeout`; `the_server_pings_on_its_interval`.

**WS-4. No guard ends the connection when the socket task is cancelled.**
- **Problem:** the two socket tasks are detached and the receiver lives in the send task. If `serve_socket` is cancelled, which happens only at runtime drop, the entry stays registered until the session expires.
- **Fix:** a `ConnectionGuard` whose drop requests the disconnection.
- **Tests:** `a_socket_task_that_is_cancelled_still_disconnects`.

**WS-5. A refused subscription stays writable.** `routes/ws/stream_socket.rs:111`, `services/hub/tasks/stream_dispatcher.rs:183,258`.
- **Problem:** `StreamSession` marks a stream as followed (and writable) before the dispatcher accepts the subscription, and never undoes it. When the worker exited `Unavailable` (an undecodable stored command), subscribers get `SubscribeFailed`, but new and existing subscribers can still write. Ingest then stores commands for a stream nobody can replay. `SubscribeFailed` goes to the send task, so the session sees neither the refusal nor the acceptance. Marking the stream only after acceptance would break "a command right after subscribe".
- **Fix:** refuse on the server side. Ingest checks a set of unavailable streams that the dispatcher maintains, and the dispatcher removes the stream's subscribers. A back channel to the session is the alternative.
- **Tests:** Rust `a_refused_subscription_does_not_allow_writes` (a seeded undecodable command); the API guard "Chat shall accept a command sent right after the subscribe, before the sync" stays green.

**WS-6. A failed re-subscribe leaves the old subscription live.** `routes/ws/stream_socket.rs:95-103`.
- **Problem:** if `open()` fails for a stream the socket already follows (for example `NotReady` after 10 s during a Postgres outage), the client gets `SubscribeFailed` and goes Failed. Meanwhile the follow entry, the hub subscription and the worker subscription all stay: frames keep coming, and writes are accepted.
- **Fix:** on any subscribe failure, unsubscribe fully (hub, worker, follow entry). Alternatively, answer with a reason the client treats as "retry" and keep the old subscription consistent.
- **Tests:** `a_failed_resubscribe_leaves_no_live_subscription`.

**WS-7. A reply is lost silently on a full channel.** `routes/ws/stream_socket.rs:65-70`.
- **Problem:** `reply()` sends `Nack`/`SubscribeFailed` with `try_send` and ignores `Full`. The hub drops the connection only if a later hub send also finds the channel full. After a burst, the client never learns of the nack, and no metric counts it.
- **Fix:** on `Full`, request `DropConnection{SlowConsumer}` through the metrics helper.
- **Tests:** `a_lost_reply_drops_the_connection_as_slow_consumer`.

**WS-9. A registration failure closes with no code.** `routes/ws/connect.rs`.
- **Problem:** the client gets a plain close, and "Connected" is logged before the hub registers anything. A registry write that lands but whose read-back fails leaves the registry naming a discarded connection.
- **Fix:** close with 1013 (D4), and log "Connected" from the command loop after the registry write.
- **Tests:** `registration_failure_closes_with_1013`, with one case for a failure before registering and one for a registry failure inside the loop (that one needs a notice before the drop).

**WS-16. Stream access is checked once and cached.** `routes/ws/stream_socket.rs:111-117,164`, `command_stream_service.rs:162,174`.
- **Problem:** `can_write` is computed at subscribe and cached for the socket's lifetime. This is harmless with the static `chat_access()` allow-list, but a removed member would keep access once access becomes per-user.
- **Fix:** document "access is fixed at subscribe" on the `StreamAccess` port; add a revocation path together with dynamic access.
- **Tests:** none until dynamic access exists.

### RC F: hub fan-out and registry atomicity

**HUB-1. Every socket gets every lifecycle event.** `routes/ws/connect.rs:287`, `services/hub/hub_service.rs` (`publish`).
- **Problem:** every socket subscribes to the Hub topic. Each connect or disconnect is pushed to every socket's 1000-slot channel, which is O(N²) on the single loop during a reconnect storm. It cascades into slow-consumer drops, and each drop publishes another event. Sockets never need these events, because their own end arrives through the removed connection's notice. `UserConnected` also carries the session key to every Hub-topic channel. It never reaches the wire (`event_to_wire_message` drops Hub messages), but it is a latent leak.
- **Fix:** sockets register on no topic, and `publish` stops scanning `Connections::broadcast`. Keep the session key out of any broadcast payload.
- **Tests:** `sockets_do_not_receive_hub_events`. Reduce the Hub-topic tests (`lifecycle.rs:125` second half, `:314`, `contention.rs:352`) to what a production subscriber still needs.

**HUB-3. An aborted registry transaction becomes an error.** `redis_hub_registry.rs:94,198`.
- **Problem:** WATCH/MULTI destructures the EXEC reply as `(usize,)` with `?`, so an aborted EXEC (nil) becomes `Err`, and `transaction_async` never retries. The competing SET is always followed by its PUBLISH, which re-runs the check and drops our connection, so a takeover is delayed by one loop command, not skipped. The real effects are an `error!` log and, on a disconnect, a stale row until the TTL (900 s).
- **Fix:** Lua scripts with one round trip each: `HEARTBEAT` (0 missing / 1 refreshed / 2 other), `HEARTBEAT_MANY`, `REMOVE_IF` (compare, `DEL`, `PUBLISH`). Report `Active`/`Missing`/`Other` (D8). Remove every WATCH/MULTI and `transaction_async`. The codebase already has the pattern (the stream `UNLOCK` script).
- **Tests:** `remove_only_deletes_an_entry_owned_by_the_connection`; `a_concurrent_takeover_is_never_an_error` (50 interleaved writes; a regression guard, because before the fix it fails only by chance); `a_missing_row_is_reported_missing_not_other`, at repository level and on both the heartbeat and the registry-change paths of the hub.

**HUB-6. `remove_connection_by_id` scans every connection.** `services/hub/hub_connections.rs`.
- **Problem:** it is an O(N) scan under the write lock, tens of µs at 20 k connections, and k drops cost k × N.
- **Fix:** store the user id per connection, and keep a connection-id → user-id index.
- **Tests:** `remove_by_id_uses_the_index` (structural unit test, no wall-clock limit).

**HUB-7. Every foreign registry notice becomes a command.**
- **Problem:** every connect or disconnect on another instance makes this instance enqueue a command and open an `info` root span, even for users it does not hold.
- **Fix:** pre-filter in the listener with a local lookup. The callback is synchronous, so use `try_read` or a sync map. Notice-only spans are `debug`.
- **Tests:** unit `notices_for_unknown_users_create_no_command` (on the extracted filter).

**HUB-8. Backpressure is handled inconsistently.**
- **Problem:**
  - the removed connection's notice bypasses the metrics helper, so a lost close notice has no log or metric;
  - lifecycle noise shares the 1000 slots with stream frames;
  - a full connection gets a duplicate `DropConnection` from every producer;
  - the subscribers and the control queue have no depth gauge.

  Most of the noise goes away with HUB-1.
- **Fix:** route the notice through `try_send`, deduplicate in-flight drops per connection, and document the matrix.
- **Tests:** `duplicate_drop_requests_for_a_full_connection_are_deduplicated`; `the_close_notice_goes_through_the_metrics_helper` (metrics are process-global, so use a test-local recorder in the `hub-service` serial group, or skip this one).

**WS-14. Stream frames are serialized per socket.** `routes/ws/connect.rs:251`, `routes/ws/message.rs:11`.
- **Problem:** frames are shared through `Arc`, but each socket clones every command's data and runs `serde_json::to_string`. A 256-command frame to 2000 subscribers is encoded 2000 times.
- **Fix:** serialize once per step, and share the text (`Arc<str>` / `Utf8Bytes`).
- **Tests:** `a_shared_frame_is_serialized_once`.

### RC G: unsupervised tasks and listeners

**HUB-5. The command loop and the consumers are unsupervised.** `services/hub/hub_service.rs`.
- **Problem:** the handles are only aborted when the loop ends. A panic leaves the hub dead, or a consumer silent, while `HubStatus` looks healthy. A dead session checker keeps revoked sessions on their sockets. Prod builds with `--release`, where a panic unwinds (`panic = "abort"` is set only in the `release-lto` profile), so a panicking task does not stop the process either. After a loop panic, `send_control` fails and new sockets are closed.
- **Fix:** keep the consumer handles in a `JoinSet`. When one ends unexpectedly, set `running = false`, record its name in `stopped_tasks`, log `error!` and call a `fatal` callback that exits the process. Do the same for the command loop. Redis errors inside a consumer never reach `fatal`.
- **Tests:** `a_stopped_consumer_calls_fatal_and_marks_the_status` (a consumer that returns early; real panics abort the test run); `a_stopped_command_loop_calls_fatal`. Assert "a Redis error does not call fatal" inside `fault.rs:183`.

**RDS-2. A failed first `listen` is permanent.** `crates/shine-infra/src/db/redis/redis_listener.rs:278-299`, `services/hub/hub_service.rs:331`, `services/hub/tasks/stream_dispatcher.rs:93`.
- **Problem:** `listen` returns `Err` before it inserts the handler and never wakes keep-alive, so the reconnect never subscribes the channel. It needs a transient failure in the short startup window, or a SUBSCRIBE timeout; Redis being down at boot already fails startup first.
  - For the registry listener, cross-instance takeovers then wait for the heartbeat (300 s) until restart.
  - For the wake listener, delivery falls back to the tick.

  The registry listener also ignores a reconnect (`None`), so notices lost in the gap wait for the heartbeat.
- **Fix:** insert the handler and wake keep-alive even when the first listen fails, or await the first listen with retry (200 ms to 10 s). On reconnect, the registry enqueues `ResyncRegistry`; the wake listener already wakes every stream. Release the listeners in `shutdown`.
- **Tests:** `a_failed_first_registry_listen_is_retried`; `a_failed_first_wake_listen_is_retried`; `a_listener_reconnect_resyncs_local_connections`; `a_wake_listener_reconnect_wakes_every_resident_stream`.

**RDS-1. The pub/sub listener never checks its connection is alive.** `crates/shine-infra/src/db/redis/redis_listener.rs:181,212`.
- **Problem:** redis-rs 1.7.1 leaves TCP keepalive off, and the listener sends nothing after subscribing. After a silent loss (a partition, a host that is gone, or a middlebox dropping the flow in under 300 s without an RST), the stream never ends. There is no reconnect, no wake-all and no resync until restart, and no log or metric. Redis's server-side `tcp-keepalive 300` keeps longer NAT mappings alive.
- **Fix:** ping through the sink periodically with a timeout. On failure, abort the stream task and wake keep-alive explicitly, because in redis 1.7.1 a dropped sink does not end the pub/sub stream (`aio/pubsub.rs:256-261`). The keep-alive loop gets a periodic arm. Also set TCP keepalive and a user timeout on the pub/sub connection.
- **Tests:** `a_silently_lost_pubsub_connection_is_detected` (toxiproxy `timeout` toxic; Rust only, because the toxic also breaks the API session check).

**HUB-9. The tracker's wake path has no production caller.** `services/hub/tasks/connection_tracker.rs:78`.
- **Problem:** `spawn_connection_loop` always passes `wake: None`. The wake support and its tests serve no consumer, and wake-driven ticks would have no minimum spacing.
- **Fix:** remove the wake support and its tests.
- **Tests:** none.

**HUB-10. A hub dropped without Shutdown leaks its tasks.**
- **Problem:** the tasks hold `HubService` clones, and the loop holds `control_tx`. Each test has its own runtime, so this is test hygiene.
- **Fix:** the loop holds a weak handle.
- **Tests:** none.

**HUB-11. Dead code and API misuse in the hub.**
- **Problem:**
  - `list_connections` (no callers) and `find_redis_keys` (a whole-keyspace `SCAN`) are reachable only from tests;
  - `send_to_user` and `broadcast` have no production caller;
  - `HubError::UserAlreadyConnected` is never built;
  - `request_drop` accepts `DisconnectReason::Client`;
  - a second `HubService` on one pool loses takeover (`AlreadyListening` is only logged).
- **Fix:** delete them (or move them under `#[cfg(test)]`); `request_drop` asserts `reason.is_drop()`; `new` returns the listener error.
- **Tests:** `a_second_hub_on_one_pool_fails_to_start`.

### RC H: process lifecycle and environment guards

**OPS-1. Shutdown has no deadline on the production path and closes sockets one by one.** `services/hub/hub_service.rs:576-593`, `crates/shine-infra/src/web/web_app.rs:91-100,331-339`, `main.rs:47-51`.
- **Problem:**
  - The shutdown hook is synchronous and only enqueues `request_shutdown`; nothing waits for the hub.
  - Prod runs plain HTTP behind the fly proxy (`axum::serve(..).with_graceful_shutdown`). That path has no deadline, does not track upgraded sockets, and drops the runtime as soon as HTTP drains.
  - Inside the loop, each socket is notified just before its own registry removal (several Redis round trips), so later sockets wait behind earlier ones.

  Most sockets can miss their 1001 and leave their rows until the TTL. Only the TLS path has a 10 s budget, and it does not wait for the hub either.
- **Fix:** notify and drop every connection first; then remove the rows concurrently (chunks of 256, 32 in flight) under a 10 s budget. Add `HubService::wait_stopped()` and an async shutdown hook, awaited with a deadline before `start_web_app` returns, on both the TLS and the non-TLS path.
- **Tests:** `shutdown_closes_every_socket_before_touching_redis` (300 connections, plus the same during a Redis outage); `wait_stopped_resolves_after_the_loop_finished`; optionally a load-plus-latency budget test (5 000 rows).

**OPS-2. Shutdown does not wait for the workers' final persist.** `services/hub/tasks/stream_dispatcher.rs:79-85`.
- **Problem:** the workers are detached and nothing awaits their final `persist_cycle`, so process exit cancels in-flight appends, marks and snapshot stores. The commands then live only in Redis until another instance's sweep finds them, and are lost if this was the last instance and Redis loses them.
- **Fix:** join the workers with a deadline inside the OPS-1 budget, and include them in `wait_stopped`.
- **Tests:** `shutdown_persists_the_workers_tail_before_exit`.

**OPS-3. Unchecked numeric config (a huge heartbeat breaks every connect).** `app_state.rs`, `redis_hub_registry.rs:61,76,129`. — **Done**
- **Problem:** a huge `hubHeartbeatSeconds` gives a TTL that `SET EX` refuses, so every connect fails, and each failed connect still publishes a registry change. The refresh path casts the TTL `as i64` to -1, and `EXPIRE -1` deletes the row. Huge intervals can also overflow tokio `Instant` arithmetic. The shipped config is not affected.
- **Fix:** rather than silently clamping, `AppConfig` derives `validator::Validate` (the same crate request bodies validate with) with `#[validate(range(min = 1, max = 86_400))]` on `hub_heartbeat_seconds`. A new `FeatureConfig::validate()` hook (default no-op; `web_app.rs`'s `load_app_config` calls it once right after the config loads) wires it in, so an out-of-range value fails *startup*, before any pool or hub exists, instead of running with an adjusted value or failing deep inside `HubTiming`. `HubTiming::from_config` keeps its pre-existing `.max(1)` only as a last-ditch guard, since `validate()` already guarantees the range.
- **Tests:** none — `#[validate(range(...))]` is `validator` crate behavior, not ours to test. Replaces `app_state_test.rs:41,49,57` and the now-dropped `config_is_clamped`/clamping tests (TST-2): the old tests encoded "clamp to a value that still connects"; the new behavior is "reject before connecting."

**OPS-5. Prod origins trust the dev site.** `services/server_config.json:4-8`, `web_app.rs:164-180`, `app_state.rs:62-67`.
- **Problem:** prod `allowedOrigins` lists `https://dev.scytta.com` and `https://ws.scytta.com`. The same list drives credentialed CORS and the WebSocket Origin check, which is the only CSWSH defence. The `sid` cookie is scoped to `scytta.com` with SameSite=Lax, so a preview build or an XSS on dev can open an authenticated socket to prod as any visiting user and read snapshots.
- **Fix:** one origin list per stage, and drop `ws.scytta.com`, which serves no pages. Confirm with the user first.
- **Tests:** `prod_allowed_origins_contain_only_prod_hosts` (a config test, because the API suite runs the test config).

**OPS-6. TLS to Postgres and Redis is not enforced.** `crates/shine-infra/src/db/postgres/pg_connection.rs:327-334`, `create_redis_pool`.
- **Problem:** tokio-postgres keeps `sslmode=prefer`, which an active attacker can strip, and `redis://` is accepted. A connection string copied from the test shape would send the command log and credentials in plaintext.
- **Fix:** in prod, refuse `sslmode` below `require` and Redis strings that are not `rediss://`.
- **Tests:** `prod_connection_strings_require_tls`.

**OPS-7. The host allow-list trusts client headers.**
- **Problem:** the allow-list reads the client-sent `X-Forwarded-Host`/`Forwarded`. The Origin check is the real CSWSH defence.
- **Fix:** document that the host check is not a security control, or trust those headers only from the proxy.
- **Tests:** none.

**PG-4. Two instances race on the migrations of a fresh database.** `crates/shine-infra/src/db/postgres/pg_connection.rs:133-149`, `repositories/db.rs:36`, `services/identity/src/repositories/db.rs:42-46`.
- **Problem:** refinery 0.10 takes no lock, and each migration runs in its own transaction. Two replicas starting on a fresh database both run `CREATE TABLE`, and one fails its startup (it recovers on restart). Identity runs its own refinery runner.
- **Fix:** one shared helper that takes a session-level `pg_advisory_lock` around the run and releases it on every path, errors included. The transaction-scoped lock cannot be used, because refinery opens its own transactions. Builder and identity both use the helper.
- **Tests:** `concurrent_migrations_on_a_fresh_database_both_start` (in `shine-infra`).

**PG-5. A Postgres outage stalls every command-log call for the full pool timeout (30 s), not a fast failure.** `crates/shine-infra/src/db/command_log/pg_command_log.rs:284-286` (`client()` → `self.0.pool.get()`). — **Done**
- **Problem:** found alongside RDS-16, while writing the API test for a Postgres outage (TST-4). `client()`'s `pool.get()` carried no `tokio::time::timeout`, so when every pooled connection is broken, `pool.get()` blocked for the full `pool_timeout` (30 s) before erroring. This is the checkout-contention half of PG-1 (not the "a query hangs on an already-checked-out half-open connection" half, which `statement_timeout`/`tcp_user_timeout` still need to cover): the snapshot route's `read_snapshot` and the worker's persist cycle both go through this same `client()`, so both stalled 30 s.
- **Fix:** `client()` now wraps `self.0.pool.get()` in a 2 s `tokio::time::timeout`, mapping a timeout to the same `PgError::PoolError(PgPoolError::TimedOut)` the pool itself already returns.
- **Tests:** verified live against toxiproxy (`enabled:false` on the `postgres` proxy): the snapshot route now answers 503 in ~3 s instead of hanging to the test's own timeout, and `streamSubscribeFailed` arrives in ~2 s. `cargo test -p shine-infra -p shine-builder -p shine-identity` all green, `cargo fmt`/`clippy` clean. API: `chat_stream_failures.ts`'s three Postgres-outage tests pin the now-fast failures.

**RDS-14. Nothing checks the Redis version or eviction policy.**
- **Problem:** the stream scripts need Redis 6.2 or later (exclusive `XRANGE (id`, `XTRIM MINID`), and the registry and the streams need `maxmemory-policy noeviction`. Below 6.2 the failure is loud; eviction would silently drop registry rows and unpersisted commands.
- **Fix:** `check_redis` at startup. It parses `redis_version` from `INFO server` and fails below 6.2. `CONFIG GET maxmemory-policy` warns when the policy is not `noeviction`, and a denied command is ignored.
- **Tests:** `redis_older_than_6_2_fails_the_start` (unit test on the version parser).

**RDS-15. Redis Cluster is unsupported, and the limit should be stated where it matters.**
- **Problem:** the multi-key `MGET`, the `EXPIRE` pipeline, `SCAN`, the session check's multi-key `EXISTS`, and the stream scripts with the global `{p}:active` key cannot run on a cluster. Per-key calls cannot fix the scripts while the active set is global.
- **Fix:** keep "single-primary Redis required" in `hub.md` and in the deployment notes.
- **Tests:** none.

### RC I: unversioned formats and protocol

**STR-7. An undecodable snapshot makes the stream reload forever.** `stream_worker.rs:395-401,537`, `services/hub/tasks/stream_dispatcher.rs:258`.
- **Problem:** if the newest snapshot cannot be decoded, the worker replays from version 0. On a pruned stream that always hits a gap, so the worker reloads every 10 s indefinitely. Subscribers never get a Sync or a `SubscribeFailed` (only an undecodable command marks a stream unavailable), and clients loop through their sync timeout. A deploy that changes the postcard layout of the state breaks every stored snapshot the same way, so falling back to an older snapshot does not help.
- **Fix:**
  - add a snapshot/state format version, so a layout change is detected as "incompatible format", not as corruption;
  - on a format mismatch, rebuild from the full log where it is kept (`keepHistory`), or reset the stream deliberately (an operator decision for pruned kinds);
  - for a single corrupt snapshot, try the older kept snapshots, newest first;
  - when nothing works, exit `Unavailable` (pending subscribers get `SubscribeFailed`), log at `error`, and count it.

  CLI-5 uses the same version.
- **Tests:** `an_undecodable_snapshot_falls_back_to_an_older_one`; `no_decodable_snapshot_fails_subscribers` (through the dispatcher with a real chat worker, with bounded loads); `an_undecodable_snapshot_on_an_unpruned_stream_replays_from_the_start`; `a_snapshot_of_another_format_version_is_not_decoded`.

**CLI-5. No forward compatibility in the client protocol.** `crates/shine-game/src/replay/client_stream.rs:336`, `protocol.rs:61`.
- **Problem:**
  - An unknown nack reason counts as final and drops the write.
  - `ServerFrame` has no catch-all variant, so a new frame type (`hello`, `error`) fails to decode.
  - The state encoding has no version. A changed layout fails as "snapshot cannot be decoded", and changed apply semantics give a resync loop at every checkpoint, with no signal to reload.
- **Fix:** unknown nack reasons are transient (capped by retries); an ignored `Unknown` frame variant; a state/codec version in `streamSync` (STR-7), where a mismatch raises a "reload the client" event.
- **Tests:** unit `an_unknown_nack_reason_is_not_final`, `an_unknown_server_frame_is_ignored`, `a_state_version_mismatch_asks_for_a_reload`.

**HUB-12. Registry keys are not prefixed or versioned.**
- **Problem:** with a shared Redis, environments take each other's users over. A mixed-version deploy makes instances drop each other's users, because an unparsable value is "not mine". Hub tests hardcode `hub-connection:` in their cleanup (`hub/tests/test_utils.rs:351`), so they would leak rows after a rename.
- **Fix:** D7. An unknown format counts as foreign and is skipped. Tests use the same key builder.
- **Tests:** unit `registry_keys_are_prefixed_and_versioned`.

**WS-10. Every drop closes with 4000.** `routes/ws/connect.rs` (`close_code`).
- **Problem:** every hub drop uses 4000 with a free-text reason. A registration failure has no code, and a slow-consumer notice is never delivered (the channel is full). The client cannot tell a takeover (stay down) from a lost registry row (reconnect with backoff); today both are 4000.
- **Fix:** D4 and D8: `Other` → `registry_inactive` 4001, `Missing` → `registry_missing` 4003, on both the heartbeat and the registry-change path. Update `HUB_DROPPED_CLOSE_CODE` in the API tests and the client contract in `hub.md`.
- **Tests:** unit `every_drop_reason_maps_to_its_close_code`; API table "WS connection dropped for ${reason} shall close with ${code}" over replaced (a second socket), registry_inactive (`SET` + `PUBLISH`), registry_missing (`DEL` + `PUBLISH`) and session_invalid (delete the session). The table replaces the separate replaced and session tests.

**WS-11. No protocol version or hello frame.**
- **Problem:** the socket protocol has no version and no hello. Binary frames are ignored without a metric, and the OpenAPI path documents only `OK`.
- **Fix:** `hello{version, connectionId}` as the first frame (D5); document the frame schema.
- **Tests:** API "WS connect shall send hello as the first frame" (version 1, a UUID connection id); Rust: the hello connection id equals the registry value.

**WS-8. Input the socket cannot accept is ignored without an answer.** `routes/ws/stream_socket.rs:88,134,165`.
- **Problem:** none of the following gets an answer or a disconnect:
  - a frame that does not parse;
  - a binary frame;
  - a non-UUID command id (it gets no nack at all);
  - an invalid stream id in a subscribe, command or unsubscribe.
- **Fix:** `error{invalid}` frames (D5), and a 1008 close after 10 consecutive refused frames.
- **Tests:** API "WS connection shall be closed with 1008 after ten consecutive refused frames"; flip the "silently ignored input" rows of TST-5 to `error invalid`; update `chat_stream.ts:177`.

### RC J: client engine order and state

**CLI-1. A transient nack reorders a user's writes; the later one is lost.** `crates/shine-game/src/replay/client_stream.rs:174,320`, `routes/ws/connect.rs:163-175`.
- **Problem:** while Live, `submit` sends each write at once. A `failed`/`busy`/`notReady` nack keeps the write pending, but it goes out again only on `retry_pending` or the next sync, and later writes are not held back. The server handles one socket's frames in order, so B is ingested after A's transient nack. If B is a Replace or Remove of A, the state ignores it (A does not exist yet), B is confirmed by its own echo and never resent, and the edit is lost or a deleted message comes back. Only writes that refer to the still-pending one are lost; independent writes are just reordered.
- **Fix:**
  - **server:** after a transient nack, refuse later writes of that connection and stream with a `blocked` reason, until the nacked id is resent. This is required, because the client may already have pipelined B before A's nack arrived, and there is no positive ack;
  - **client:** resend in order from the nacked write. This depends on CLI-2.
- **Tests:** unit `a_transient_nack_keeps_later_writes_in_order`; Rust session-layer `a_write_after_a_transient_nack_is_refused_as_blocked` (a transient nack cannot be forced through the API).

**CLI-2. A `duplicate` nack drops the write before its echo.** `crates/shine-game/src/replay/client_stream.rs:336`, `routes/ws/stream_socket.rs:172`.
- **Problem:** the nack is sent straight from the socket, while the echo waits for a wake and a worker step. After `retry_pending`, or a resend after a sync, the write disappears and comes back, and the UI is told it was dropped. INGEST already returns the version of the duplicate, but the nack drops it.
- **Fix:** add the version to the `duplicate` nack frame. The client drops the pending write only once its authoritative version reaches that version.
- **Tests:** unit `a_duplicate_nack_keeps_the_write_until_its_version_arrives` (flips `crates/shine-game/tests/replay/client.rs:246`, TST-2); extend the API duplicate test to check the version.

**CLI-3. After a checkpoint mismatch the client can resume on the diverged state.** `crates/shine-game/src/replay/client_stream.rs:310,349,366`.
- **Problem:** the resync after a mismatch keeps the generation. If the socket drops before the fresh Sync arrives, the reconnect resumes, and the client stays Live on the diverged state until the next checkpoint catches it.
- **Fix:** clear the resume position on a mismatch, so the next subscribe asks for a full Sync.
- **Tests:** unit `a_checkpoint_mismatch_forces_a_full_sync_on_reconnect`.

**CLI-4. No way to seed the client from the HTTP snapshot.** `crates/shine-game/src/replay/client_stream.rs:192-261`.
- **Problem:** the only way in is `on_frame(StreamSync)`, which sets Live with no subscription and accepts an older state. A write sent before subscribing gets `notSubscribed`, and a late HTTP response can replace a newer state. This is latent: there is no production client yet.
- **Fix:** `seed_snapshot` leaves the status alone and takes the state only if it is newer. It verifies the hash, keeps the hashes, and sets the generation, so the next subscribe resumes.
- **Tests:** unit `seed_snapshot_takes_only_a_newer_state`.

**CLI-7. An undecodable command mid-batch leaves a stale view.** `crates/shine-game/src/replay/client_stream.rs:280`.
- **Problem:** `on_commands` returns before `rebuild_local`. The commands applied earlier in the batch, and the pending writes they confirmed, are missing from the local view. The stream fails, but its last view is stale.
- **Fix:** rebuild the local view before failing.
- **Tests:** unit `an_undecodable_command_mid_batch_still_rebuilds_the_view`.

**CLI-8. Chat text is not trimmed; invisible text passes.** `crates/shine-game/src/replay/chat.rs:64-70,90-96`.
- **Problem:** the design says the text is trimmed, but the code only checks `!text.trim().is_empty()` and stores the text as sent. U+200B, U+2060 and U+3164 are not whitespace to `trim`, so visually empty messages pass.
- **Fix:** canonicalization stores the trimmed text and refuses text without a visible character. `ReplayKind::canonicalize` is generic, so this needs a `ReplayState` normalize hook (`state.rs`). "Visible" is defined as at least one character that is not whitespace and not in the Unicode categories Cf, Zs or Cc, with U+3164 treated explicitly. The client hashes the canonical text it receives.
- **Tests:** unit `chat_text_is_stored_trimmed`, `invisible_only_text_is_refused`; API row "Chat shall refuse a message of only invisible characters".

**STR-17. A re-hydration forces every subscriber into a full resync.** `stream_core.rs:243-248,294`, `crates/shine-game/src/replay/client_stream.rs:268`.
- **Problem:** a re-hydration adopts the new generation without a reset, and the client drops the frame as "the history changed". When a watched but idle stream's keys expire (STR-9), every subscriber does a full resync at once, and every offline client's resume is refused.
- **Fix:** refresh the TTL while there are subscribers (STR-9). Carry subscribers across a proven adopt: the first frame of the new generation carries `previousGeneration`, and a client at that generation and position continues without a full Sync. Keeping the generation instead would break the generation fence (D2).
- **Tests:** `a_proven_adopt_keeps_subscribers_without_a_full_sync`; unit test of the client side.

### RC K: missing defensive checks

**RDS-6. Scripts do not guard missing meta fields.** `scripts.rs:14-15` (INGEST), MARK_PERSISTED, RETIRE_ACTIVE. — **Done**
- **Problem:** once `meta[1]` exists, `tonumber` on a missing `version` or `persisted` raises a Lua error instead of a clean outcome. This needs corrupted meta, because HYDRATE always writes all three fields.
- **Fix:** treat a missing field as `not_ready` (INGEST), `refused` (MARK, returning -1, which the Rust side already maps to `Refused`) or `false` (RETIRE, extending its `HMGET` to also read `generation` so a hydrated-but-corrupted meta is told apart from "no meta at all", which stays a safe retire).
- **Tests:** `missing_meta_fields_are_not_ready` (all three scripts; written to fail first — it hit the Lua arithmetic error directly — then verified green after the fix).

**RDS-9. `to_meta` turns missing fields into zeros.** `redis_command_stream_db.rs:98`.
- **Problem:** missing fields default to 0 without a warning, which hides corruption.
- **Fix:** log at `error` and return `None` (not ready).
- **Tests:** covered by `missing_meta_fields_are_not_ready`.

**RDS-10. MARK_PERSISTED does not validate `keep_from`.** `scripts.rs:86`.
- **Problem:** a non-numeric `keep_from` would raise a Lua error. This needs a bug on the Rust side.
- **Fix:** an assert in Rust; no script change.
- **Tests:** none.

**RDS-11. The unreadable-entry log does not name the field.** `redis_command_stream_db.rs:311`.
- **Fix:** name the failing field (no payload above `trace`).
- **Tests:** none.

**RDS-12. `idle_streams` drops unparsable ids silently.** `redis_command_stream_db.rs:349`.
- **Problem:** a bad member stays in the active set forever.
- **Fix:** log at `error` and remove it with a raw `ZREM` (`retire_active` builds its keys from a parsed id, so it cannot).
- **Tests:** `an_unparsable_active_id_is_retired`.

**RDS-13. An unparsable wake wakes every stream.** `redis_command_stream_db.rs:376-388`.
- **Problem:** a payload that does not parse is treated like a reconnect, so every resident stream is woken. Only INGEST publishes, always with a valid id, so this needs a foreign publisher, and each wake costs one idempotent pass.
- **Fix:** log and ignore an unparsable payload; only a reconnect wakes everything.
- **Tests:** `an_unparsable_wake_is_ignored` (publish garbage, then a valid id).

**STR-15. The snapshot route does not check the Postgres part is contiguous.** `command_stream_service.rs:211-232`.
- **Problem:** `latest_snapshot` and `commands_after` are two statements on two checkouts, and the first row is never checked against `version + 1` (the Redis part is checked). If `keepSnapshots` newer snapshots are stored between the two reads (10 commands with the test config, 250 in prod), the route answers 200 with a gap. The client then rejects it as not contiguous.
- **Fix:** read both in one REPEATABLE READ transaction, or check the first row and answer not ready.
- **Tests:** `the_snapshot_route_never_serves_a_non_contiguous_sync` (a prune between the two reads).

### RC L: observability and layering

**OBS-1. Registry per-event spans are `info`.** — **Done**
- **Fix:** `debug`, per the log-level rules (`repositories/hub_registry/redis/redis_hub_registry.rs`).
- **Tests:** none.

**OBS-2. Metric and status gaps.**
- **Problem:** there is no control-queue depth, and no counters for failed connects, registry and heartbeat errors, or subscriber prunes. `hub_commands` has no `outcome` label. `HubStatus` has the tick counters but no liveness and no queue depth.
- **Fix:**
  - add a `hub_control_queue_depth` gauge;
  - add counters for rejected connects, supervisor stops and registry failures;
  - add an `outcome` label to `hub_commands`;
  - add `running`, `pending_commands` and `stopped_tasks` to `HubStatus`.
- **Tests:** none beyond HUB-5.

**OBS-3. Per-tick warn/error logs during outages.** `stream_worker.rs:213`, `services/hub/tasks/heartbeat.rs:46`, `services/hub/hub_service.rs:366,427,445,544,617`. — **Done**
- **Problem:** the worker logs `warn!` on every failed step, once per tick per resident stream. The heartbeat and the hub log transient Redis errors at `error!`.
- **Fix:** `step_failing`/`failing` bools on `StreamWorker`/`Heartbeat` gate the log: `warn` on the first failure and on recovery, `debug` for the repeats. The five `hub_service.rs` sites (all per-connect/disconnect registry errors, not a tight loop) drop straight from `error!` to `warn!`: transient infrastructure errors are warn, never error.
- **Tests:** none; checked at the checkpoint.

**WS-15. The stream session orchestrates services inside the routes layer.** `routes/ws/stream_socket.rs:164`.
- **Problem:** `StreamSession` coordinates `CommandStreamService` (open, ingest) with `HubService` (subscribe) and makes the `can_write` decision. The layer rules in `CLAUDE.md` give orchestration to handlers.
- **Fix:** move the session workflow into a handler under `handlers/`, wired in `app_state.rs`. The route keeps frame parsing and response formatting. Invoke `service-architecture` first.
- **Tests:** handler-layer tests for `forbidden`, `notReady`/`busy` and `Unavailable`.

---

## Test plan

### Layer policy

- **API test** (`tests/api-tests/builder/`, Playwright as an HTTP/WS harness): covers everything a client can observe deterministically through one running builder. That includes upgrade checks, close codes and reasons, frames, takeover, subscribe/sync/resume, ack/nack, dedupe, the snapshot route, and Redis/Postgres outages through toxiproxy (tagged `@infrastructure`).
- **Rust integration test** (a `HubService`/`AppState` with real Redis/Postgres): used only when the API cannot reach the behaviour. Valid reasons:
  - internal hooks or state (barrier, `wait_ticks`, drop counts, an injected worker fault);
  - two instances on one Redis;
  - a fault at a chosen internal point;
  - Redis script or SQL semantics;
  - volume or timing;
  - configuration that is not deployed (other kinds, read-only access, disabled streams, unique stream ids);
  - shutdown.
- **Unit test** (no Redis/Postgres): pure logic. That means the stream core, the replay engine, `ChatState::apply`, mappings (`close_code`, `OpenError::reason`, nack strings), config clamps and payload parsing.

### What the API tests can reach

- **One instance:** CI and local runs start one builder with `services/builder/server_config.test.json`, so two-instance behaviour stays in Rust (`contention.rs:456,511`, `stream_integration_test.rs:42`).
- **Takeover without a second instance:** `SET` the user's registry row to another id and `PUBLISH` the change, and the socket closes with `registry_inactive`. `DEL` + `PUBLISH` gives `registry_missing` (after HUB-3).
- **One stream:** only `chat-global` exists. It is shared by every run, always read+write, and every other id gets 403 before the head lookup. So `kindMismatch`, read-only `forbidden`, snapshot 404 and "streams disabled" are Rust-only.
- **Toxiproxy:** fronts builder's Redis (sessions, registry, streams) and Postgres. A Redis outage also fails every new upgrade and snapshot request at the session check, and `SessionMint` goes through the same proxy, so mint sessions before disabling the proxy. The test `redisCns` needs `timeout`/`pool_timeout`, and outages use `enabled:false`, not latency toxics.
- **Timing:**
  - `authCheckInterval` 2 s, `syncIntervalMs` 200, `snapshotEvery` 5 and `keepSnapshots` 2 are fast enough.
  - `hubHeartbeatSeconds` is 300, so registry tests use the registry-change path.
  - `redisMaxUnpersisted` is 5000, so `busy` is unreachable.
  - The chat limit of 2000 characters masks `maxCommandBytes` unless a frame exceeds 16 KiB. After WS-1, `maxCommandBytes` stays a Rust unit check.
- **Out of reach:** `slow_consumer`/`dead_channel` (racy), shutdown, registration failure and a Redis flush (shared state).

### TST-1. A flaky API test (High, S, phase 0) — Done

`chat_stream.ts:242` ("snapshot shall be readable over HTTP and resume with only the tail") fails whenever bob's command lands on a snapshot boundary (`(tip+1) % 5 == 0`). The snapshot at tip+1 is stored 20-40 ms after bob's echo, which usually beats the new TLS socket's subscribe. That snapshot becomes the base version, and the resume at `tip` gets a full Sync. `chat-global` persists across runs, so about one run in five is at risk.

**Fix:** if bob's command lands on a multiple of 5, write one more command and resume at that version. Resuming from the snapshot version itself does not work, because it is ≤ tip < the new base.

### TST-2. Tests that assert a defect (flip with their fix)

| Test | Encodes | Flipped by |
|---|---|---|
| `app_state_test.rs:49` `a_huge_heartbeat_does_not_overflow_the_ttl` | a TTL of `u64::MAX` | OPS-3 |
| `crates/shine-infra/tests/pg_command_log.rs:319` `a_repeated_pruned_row_is_reported_as_diverged`; `command_stream/stream_core_test.rs:262` (`Diverged` → `Reset`, no `Pruned`) | pruned = diverged | STR-1 |
| `repositories/command_stream/redis/redis_command_stream_test.rs:201` lines 238-242 | HYDRATE overwrites a copy with unpersisted rows | STR-2 |
| `redis_command_stream_test.rs:158` lines 170-174, `:450` lines 470-474 | a trimmed command id is accepted again | STR-8 |
| `command_stream/stream_core_test.rs:145` | `begin_load` clears unpersisted rows | STR-4 |
| `crates/shine-game/tests/replay/client.rs:246` | a `duplicate` nack drops the write at once | CLI-2 |
| `command_stream/stream_integration_test.rs:283-284` | `pool_timeout=2` hides the 30 s stall | STR-12 |

Three tests come close to a defect without hitting it, and are the starting point for its test:
- `stream_integration_test.rs:163`: only the order where the worker notices first (STR-3).
- `:220`: persists everything before rolling Redis back (STR-4).
- `:470`: the exact STR-1 setup, escaping only through its 60 s persist delay.

### TST-3. Redundant tests (Med, M, phase 0)

Keep → drop. Drop a test that TST-4 replaces only when its API replacement lands.

**`routes/ws/connect_test.rs`:**
- The stream section `:396-577` repeats the API and core tests: `:396` (= `replay/protocol.rs:10,61`), `:408`, `:440`, `:458`, `:477`, `:523`, `:549`, `:563` and `:577`. `:477` becomes a unit table over `OpenError::reason()`. Keep `:497` (streams disabled).
  - **Done** for the part with no API dependency: dropped `:396` (`a_resume_frame_is_parsed`, an exact duplicate of `replay/protocol.rs:10,61`); added `every_open_error_maps_to_its_reason` as the unit table over `OpenError::reason()` (`command_stream_test.rs`), alongside the existing socket-level test (kept — it also proves the reason reaches the wire end to end).
  - `:408`, `:440`, `:458`, `:523`, `:549`, `:563`, `:577` are in TST-4's "Replaces" column. **Done**, now that TST-4's replacements landed.
- `:202` (replaced socket) → extend `ws_connect.ts:236`. `lifecycle.rs:24` keeps the drop counts and registry state. **Done.**
- The `request_drop` half of `:226` (= `:87`, `lifecycle.rs:100`, `ws_connect.ts:264`) → keep only the shutdown and hub-stopped parts. **Done** (`a_shutdown_closes_every_socket_and_stops_new_connections`).

**Hub:**
- `hub_connections_test.rs:56,91,110` → one table test. **Done.**
- `lifecycle.rs:165,194,271` (the same `drop_dead` → `DropConnection` path from three entry points) → one test. **Done.**
- `lifecycle.rs:226` and the broadcast half of `:314` test dead code → drop them with HUB-11.
- `lifecycle.rs:591` → absorbed by HUB-2's chunked heartbeat test.
- `app_state_test.rs:57` → replaced by `config_is_clamped` (OPS-3).

**Command stream:**
- **Dedupe:** keep `redis_command_stream_test.rs:401` and `chat_stream.ts:144`; drop `redis_command_stream_test.rs:84` (a subset of `:401`) and `command_stream_test.rs:83` (an enum mapping). **Done.**
- **Invalid or oversized:** keep `replay/chat.rs:75` and the API table; drop `command_stream_test.rs:111` (moves to the API). **Done.**
- **Canonical text:** keep `replay/chat.rs:183` and add a pure `ReplayKind::canonicalize` test; drop `command_stream_test.rs:141` (moves to the API). **Done** for the drop; the pure `ReplayKind::canonicalize` test is still open, tracked under TST-6.
- **Subscribe → Sync → delivered:** keep `chat_stream.ts:49` and `stream_worker_test.rs:64`; drop `hub/tests/stream_dispatcher.rs:268` and `chat_test.rs:187`. **Done.**
- **Chat logic:**
  - `chat_test.rs:104,116,130,143,167` run Redis+Postgres to test pure `ChatState::apply`, which `replay/chat.rs` covers → drop. **Done.**
  - `chat_test.rs:33` (= `chat_stream.ts:212` + `command_stream_test.rs:16,59`) → drop. **Done.**
- **Resume:** keep `stream_core_test.rs:363,386,399,421` and the API resume table; drop `stream_integration_test.rs:415`. `hub/tests/stream.rs:69` is optional. **Done.**
- **Snapshot route:**
  - `routes/command_stream_test.rs:52` is a weaker, Redis-only subset of `command_stream_test.rs:395` → dropped now (**Done**); `routes/command_stream_test.rs:19` and `stream_integration_test.rs:447` repeat `command_stream_test.rs:370` too, but each exercises a distinct code path the API test can't reach deterministically — kept both instead of dropping them (see TST-4's deviation note).
  - `routes/command_stream_test.rs:82` keeps only its "disabled → 503" line and the 404 mapping (Rust-only). **Done.**
- **Other:**
  - `replay/snapshot.rs:32` is a subset of `:83` → drop. **Done.**
  - Fold `redis_command_stream_test.rs:127` into `:450`. **Done.**
  - `stream_integration_test.rs:357` repeats the wait of its own helper → drop the repeat. **Done.**

**API:** All items below are **Done**, verified against a live builder instance (15/15 passing, the formerly-flaky resume test run 5x green).
- `chat_stream.ts:296` = `ws_connect.ts:264` → drop the chat one.
- `chat_stream.ts:49,75,111` → one happy-path test with steps: two writers doing add, remove and replace; both sockets see the same command id, version, user id, command and generation; each writer keeps its own order.
- `ws_connect.ts:183,192,202,226` → one rejection table: missing Origin 400, foreign Origin 403, non-ws host 403, no cookie 401, tampered cookie 401. (The "different User-Agent" and "deleted session before connect" rows mentioned in the plan are new coverage, not a consolidation of existing tests — left out here; add them separately if wanted, verified against actual fingerprint/session behavior first.)
- `ws_connect.ts:211` → fold into a fuller connect test (open, subscribe, sync; `hello` still pending WS-11).
- `chat_stream.ts:144` → split into a duplicate test and an invalid table that includes the non-JSON case of `:170`.
- `chat_stream.ts:288` → a table, plus a `Cache-Control: no-store` row (added `cacheControl` to `fetchSnapshot`'s return value).
- `chat_stream.ts:239` → also check that the resync contains bob's command id (or that it's already folded into the snapshot).

### TST-4. Rust tests to move to the API (Med, M, phase 0)

| API test (file) | Replaces | Status |
|---|---|---|
| "WS second connection for a user shall close the first with replaced and keep serving the second" (`ws_connect.ts`, extends `:236`; one row per minted session of the same user) | `connect_test.rs:202` | **Done** |
| "Chat shall refuse commands after the socket unsubscribed" (`chat_stream.ts`; today the API covers only the never-subscribed nack) | `connect_test.rs:440` | **Done** |
| Resume table with `chat_stream.ts:275`: "Chat resume at the head shall be answered with an empty streamCommands and then follow live"; "… above the head shall be answered with a full sync"; "… older than the newest snapshot shall be answered with a full sync" (wait for `streamCheckpoint v`, then resume at v-1) | `connect_test.rs:523,549,563,577`, `stream_integration_test.rs:415` | **Done**, plus dropped the now-redundant `resume_with_a_changed_generation_sends_a_full_sync` (covered by the existing "another generation" API test) |
| "Chat shall start a joining client from the newest snapshot and only the commands after it"; extend `chat_stream.ts:242` to check the commands start right after the snapshot and are contiguous | `chat_test.rs:217`, `routes/command_stream_test.rs:19`, `stream_integration_test.rs:447` | **Done**, with a deviation: `routes/command_stream_test.rs:19` (`postgres_stream`, no Redis copy at all) and `stream_integration_test.rs:447` (post-pruning, via `pruned_stream`) each exercise a distinct code path the API test can't reach deterministically (`chat-global` always has a resident worker when the API test runs, and pruning on the shared room isn't controllable) — kept both instead of dropping them. |
| "Chat shall store and deliver a command in its canonical form" (exact bytes, not `JSON.parse`) | `command_stream_test.rs:141`, `connect_test.rs:408` | **Done** |
| "Chat shall refuse a command longer than maxCommandBytes as invalid" (a frame over 16 KiB; kept until WS-1 lowers the frame limit, then a Rust unit check) | `command_stream_test.rs:111`, `connect_test.rs:458` | **Done** |
| `chat_stream_failures.ts` (`@infrastructure`): "Chat stream shall nack writes during a Redis outage and continue without a gap or a reset after it" (the live socket stays open, because the session check skips transient errors) | `stream_worker_test.rs:176` | **Done**. Along the way, found and fixed RDS-16 (the nack took ~30 s before the fix; serial-mode `test.describe.configure` added so these toxiproxy tests, which share proxy state, never interleave). `stream_worker_test.rs:176` not dropped — it's a Rust-only fault-injection hook, kept. |
| `chat_stream_failures.ts`: "Chat stream shall keep delivering during a Postgres outage and push the checkpoint once Postgres returns"; "Chat snapshot route shall answer 503 while Postgres is down and recover"; "Chat subscribe shall fail with streamSubscribeFailed while Postgres is down and the stream is cold" | `stream_integration_test.rs:276` | **Done**. Along the way, found and fixed PG-5. `stream_integration_test.rs:276` not dropped — see "stays in Rust" notes below (two-instance/internal-hook cases). |
| `ws_connect.ts` (`@infrastructure`): "WS connect shall be refused while Redis is down and accepted after recovery" | none | **Done** (503; the session check needs Redis) |

These stay in Rust, with the reason for each:
- **Config not deployed:** `command_stream_test.rs:16,59`, `connect_test.rs:497`.
- **Keys rewritten at a chosen moment, or a second instance:** `stream_integration_test.rs:163,220,470`.
- **Internal hooks:** `hub/tests/lifecycle.rs`, apart from the drops above.
- **Byte-level faults, and the path after the upgrade:** `hub/tests/fault.rs`.
- **Two instances:** `contention.rs:456,511`.
- **Volume:** the rest of `contention.rs` and `stress.rs`.
- **Ticks the test config lacks:** `lifecycle.rs:460,525`.

### TST-5. Missing API coverage of current behaviour (Med, S, phase 0) — Done

All items below are done, verified against live identity + builder instances.

- "WS connection taken over through the registry shall close with registry_inactive" (`SET` + `PUBLISH`). Needed a new test helper, `simulateRegistryTakeover` in `builder_stream_ws.ts`, which connects to the shared test Redis directly (reading `services/server_config_test.json`, same pattern `SessionMint` uses) and does the `SET`+`PUBLISH` against today's unprefixed `hub-connection:`/`hub-registry-changed` keys (HUB-12 will version these later).
- "Chat checkpoint hash shall equal the snapshot route hash" (at each `streamCheckpoint`).
- "Chat command after a failed subscribe shall be nacked notSubscribed"; "Chat command id reused by another user shall be refused as duplicate".
- "Chat shall accept a command sent right after the subscribe, before the sync" (the guard for WS-5).
- Silently ignored input: binary frames, a non-UUID command id, and invalid stream ids in subscribe, command and unsubscribe. Pinned today's silence with `expectNone`; WS-8 flips the rows. Needed a new `StreamSocket.sendBinary` helper.
- The `hub` status in `shared/health.ts`.

### TST-6. Missing unit and Rust coverage of current behaviour (Med, M, any time)

If one of these exposes a new defect, report it instead of fixing it in the same step.

**Unit:**
- the tracker's stale-disconnect guard, and its exit on shutdown or a closed subscription;
- `remove_connection_by_id`, a re-register with the same id, and the dead `remove_connection(None)` branch;
- `Subscribers::clear`;
- registry listener payload parsing (own-instance filter, malformed payload, reconnect `None`);
- nack mapping (`busy`, `notReady`, `forbidden`);
- the `OpenError::reason` table;
- `ReplayKind::canonicalize`;
- the stream settings clamps;
- `prove_and_adopt` with `UpToDate` below the replica, and with `Appended`;
- client engine:
  - `notSubscribed` → resubscribe, and the `forbidden` drop;
  - commands before the first Sync;
  - a non-contiguous Sync;
  - a checkpoint while not Live, and eviction of kept hashes;
  - submit while Syncing;
  - a Sync followed by `Commands` frames that confirm pending writes.

**Rust integration:**
- the heartbeat finding a row held by another connection;
- a registry change to a missing row, and one during an outage;
- a client disconnect after another instance took the row;
- PUBLISH failing after a successful DEL;
- a stalled socket sink filling the channel, and the 5 s drain timeout expiring;
- consumers after shutdown;
- a read-only `forbidden`, and no reply to an invalid id or to an unsubscribe for a stream not followed;
- `StaleGeneration` from the worker's own append, `MarkOutcome::Refused` → `Reset`, and the Postgres gap fill;
- `persist_tail` on `Gap`, `Refused` or a missing head, and a requeue after a persist error;
- the `ensure_hydrated` timeout (needs a configurable timeout, because it uses `std::time::Instant`);
- a same-generation copy behind Postgres, and a rebase on a missing or unreadable snapshot;
- a wake for every stream (`WakeMap::wake(None)`), and respawn or unload of a dead or unavailable stream.

### TST-7. Test helper hygiene (Low, S, phase 0) — Partially done

- `ws_connect.ts:32-99` duplicates `tests/src/api/builder_stream_ws.ts`. **Done**: `ws_connect.ts` now imports `wsConnectUrl` from `builder_stream_ws.ts` instead of redefining it.
- The 4000 close code is a constant in `ws_connect.ts:9` but a literal in `chat_stream.ts:309`; both change with WS-10. Deferred to WS-10.
- `StreamSocket.next` does not fail early on a closed socket, so a drop shows up as a 5 s timeout. **Done**: `take()` now throws immediately once the socket's close event has fired, instead of waiting out the full timeout.
- `hub/tests/test_utils.rs:351` hardcodes the registry key prefix (see HUB-12). Deferred to HUB-12.

### TST-8. Test seams (with the first test that needs them, `#[cfg(test)]` only)

- `WorkerCommand::PersistNow`, which makes the STR-1, STR-3 and STR-4 tests deterministic.
- A configurable hydration lock TTL and a stall hook in `hydrate_locked` (STR-2).
- A call counter, or Redis `INFO commandstats` deltas (STR-13, RDS-3).
- Short configurable socket timeouts (WS-2).

---

## Constraints

**Process**
- The user reviews and commits. Never run git write commands (`add`, `commit`, `push`, `stash`, branch or tag changes). Each phase, and each larger step inside one, ends with a **checkpoint**: stop, report what changed and the test output, and wait. Apply any requested change before going on.
- Run inline in one session.
- Before editing builder routes, handlers, services or repositories, invoke `service-architecture`. Before writing API tests, invoke `api-test-writing`. Before running the service, invoke `local-development`.
- Re-read each file right before editing it, because the user edits these files between steps. Never revert their changes.
- Write each test before its fix and watch it fail. Flip a TST-2 test in the same step as its fix.

**Style**
- Comments say what, not how. Keep them concise, at the surrounding density.
- Use `tracing::*!` only. `trace`: payloads; `debug`: per step and per message; `info`: root-cause context; `warn`: infrastructure errors that heal by retrying; `error`: bugs.
- Run `cargo fmt --all` before every checkpoint, and keep `cargo clippy --workspace --all-targets` clean. After TS changes, run `cd tests && pnpm run format && pnpm run lint`.

**Behaviour rules**
- A command acknowledged to a client is never discarded by the server. It must reach Postgres, or the server must prove it is already there. A reset or reload may only drop commands that are persisted.
- Fail fast only on server bugs (a hub task or the command loop ending unexpectedly): log at `error` and exit. Redis, Postgres and network errors never stop the process; they are retried with backoff.
- The registry row is the ground truth for the active connection. A missing row or another id means "not registered", and nothing restores a row.
- Requirements: Redis 6.2 or later, `maxmemory-policy noeviction`, single-primary Redis.
- Nothing is deployed yet, so key and channel renames need no migration.

**Tests**
- Use `shine_test::test`. Skip with `#[test(skip = "reason")]` and serialize with `#[test(serial = "key")]`. Connection strings come from `shine_test::db::{get_redis_cns, get_pg_cns}`; with `CI` set, a missing string fails the test.
- Hub tests use the `serial = "hub-service"` group, the FIFO `barrier` and `wait_ticks`, never sleeps. A panic in a background thread aborts the whole run.
- Run DB tests with the connection strings set, and check that no test reports "skipping". On Windows, set environment variables in PowerShell. Each registry test uses a unique key prefix.

## Checked and found sound

Do not re-review these unless the code around them changes.

- **Hub:**
  - no nested locks, and no lock held across an await on another lock;
  - connection ids are v4 UUIDs, so a stale drop cannot hit a reconnect;
  - the command FIFO keeps a connect before a later disconnect;
  - a stale registry notice is harmless (the handler re-reads the registry);
  - concurrent connects of one user on two instances converge;
  - topics are fixed at connect;
  - every removal path publishes `UserDisconnected`, so subscriber maps cannot leak;
  - the session check is read-only and batched;
  - `transaction_async` does not leave a pooled connection WATCHed;
  - every `select!` branch is cancel-safe, and the multiplexed Redis connection is cancel-safe on `abort()`;
  - no panic is reachable from client input.
- **Security:**
  - `can_read` runs before `ensure_stream` and the head read, so no stream is created and no existence leaks;
  - the stream kind is fixed at creation, and the command author always comes from the session;
  - stream ids are restricted to `[A-Za-z0-9_.:-]{1,64}`, and Lua and SQL are parameterised;
  - origin patterns are fully anchored, so `null`, case, trailing-dot, punycode and port variants fail closed;
  - the session cookie is signed and fingerprinted, and the builder never creates sessions;
  - close, nack and failure reasons are static strings, metric labels are fixed sets, and nothing sensitive is logged above `trace`;
  - missing config fields fail closed.
- **Streams:**
  - `pending_sync`/`pending_resume` are cleared on unsubscribe;
  - all frames of a stream to one connection come from one worker task, in order;
  - a slow subscriber is removed at its first failed send, so it never sees a gap;
  - resume bounds are safe for any client value;
  - a resubscribe on one socket gives exactly one Sync;
  - client and server checkpoint at the same versions.
- **Postgres:**
  - appends are idempotent (a lost commit ack retries as `UpToDate`), and duplicate versions are impossible under the head lock;
  - generation fencing of appends holds;
  - the prune boundary is correct, and no prune, snapshot or append deadlock exists;
  - `SET LOCAL` does not leak across pooled connections;
  - every hot query has an index, inserts are single-row, and no path holds two checkouts at once.
- **Redis:**
  - versions are dense, with explicit `<version>-0` entry ids;
  - XTRIM MINID and ZREMRANGEBYSCORE cut the same range, and nothing above persisted is trimmed;
  - the persisted pointer only moves up, and every key is declared in KEYS;
  - every key has a TTL set in the script that creates it;
  - "unpersisted ⇒ listed" holds (apart from STR-2), and `read_after` is MULTI-atomic;
  - NOSCRIPT is reloaded by redis-rs, a script refused on OOM writes nothing, and the PUBLISH inside INGEST is atomic with the write;
  - `read_after`'s `max(0)` clamp is intentional and harmless.
- **Client:**
  - the state hash is identical on native and wasm (no usize, float or HashMap), and postcard is deterministic;
  - the wire format matches the TS client, with no number above 2^53;
  - chat remove and replace are limited to the author;
  - 16 KiB covers the worst-case escaping of 2000 bytes of text.
- **Already fixed in `hub.md`:** the wake channel is described as one global channel routed by stream id, and the session checker as checking that the session exists. The code comment at `models/messages/hub.rs:128` still says "expired or compromised"; fix it with the next change there.

## Final verification

- [ ] `cargo fmt --all --check`; `cargo clippy --workspace --all-targets`; `CI=true cargo test --workspace` with the connection strings set; `cd tests && pnpm test:local` with the service running, including the `@infrastructure` tests. Everything passes, and no test reports "skipping".
- [ ] Every TST-2 test is flipped. No Rust integration test remains whose behaviour an API test covers. The API suite passes five runs in a row.
- [ ] `docs/services/builder/hub.md` is synced with the code (`shine-skills:sdp` Sync).
