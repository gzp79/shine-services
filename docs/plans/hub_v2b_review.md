# Code review: `feature/hub_v2b` vs `main`

Non-SDP investigation notes. Not kept in sync with the code.

Scope: builder crate, hub and command streams first. Shared infra and identity changes that affect builder come next. The web client is low priority and sits at the end.

Method: a first `/code-review high` pass, a separate read-only re-check of each of those findings, and three further bug hunts (builder, game and web, infra and identity). Nothing was compiled or run. Status tags:

- **[validated]**: re-checked independently against the code.
- **[hunt]**: found by a bug hunt that read the code path. Not re-checked by a second reader.
- **[spot-checked]**: I confirmed the specific claim directly.

---

## High

### H1. `check_postgres_tls` rejects pool params, so prod startup fails [validated]
- `crates/shine-infra/src/db/tls_policy.rs:12` runs `cns.parse::<tokio_postgres::Config>()` on the raw string. tokio-postgres returns `UnknownOption` for `pool_timeout` and `max_size`. `create_postgres_pool` strips those first (`pg_connection.rs:341-344`).
- Callers pass the unstripped `sql_cns`: `services/builder/src/main.rs:39`, `services/identity/src/main.rs:83`.
- Non-prod returns early, so dev and test never hit it. Both `server_config.test.json` files already carry `pool_timeout=5`. `docs/plans/hub_issues.md:82` recommends shipping `pool_timeout` in prod strings. `tls_policy.rs` has no tests.
- Impact: with `stage=prod` and `…?sslmode=require&pool_timeout=5`, both services abort at startup. It fails closed, so it only shows up on the first prod deploy.
- Fix: run the string through `ConnectionString::parse`, `take_u64("pool_timeout")`, `take_u64("max_size")` and `into_cns()` before parsing. Better, parse once and share the config with the pool. Add a test for `stage=prod` with `sslmode=require&pool_timeout=5&max_size=3`.

### H2. No authorization or quota on streams [hunt]
- Files: `services/hub/hub_service.rs:155-181` (`subscribe_stream`) and `:190-203` (`send_stream_command`), `command_stream/stream_registry.rs:141-176` (`open`) and `:212-238` (`subscribe`), `command_stream_service.rs:57` (`load` calls `ensure_stream`), `sql_migrations/V1__command_log.sql`.
- The only checks are a valid `StreamId` (charset, at most 64 characters) and a known kind. There is no ownership or ACL, no cap on subscriptions per connection, no cap on workers or streams, and no GC of `cl_streams` rows. The worker channel is unbounded.
- Impact: one authenticated socket loops `streamSubscribe` with random ids. Each one costs a Postgres `head` query inline on the recv task and a spawned worker (`ensure_stream` INSERT, snapshot and command loads, a 10 s backstop pass, a 60 s grace). This exhausts the Postgres pool (2 s checkout timeout), so legitimate streams fail five passes in a row and go `Unsynced`. Tasks and `cl_streams` rows grow without bound. Any user can also post to chat-global or paint any pixel map, and can squat a stream id with a chosen kind.
- Repeated `streamSubscribe` to one stream makes the worker rebuild and clone a full `Sync` each time.
- Fix: cap subscriptions per connection and add a token bucket for subscribe and command messages. Authorize stream ids (allowlist, or a prefix tied to user or room). Create the `cl_streams` row only on the first accepted command, or add a global worker cap. Bound the worker channel or coalesce duplicate Subscribes.

### H3. `panic = "abort"` is now set on the plain `[profile.release]` [spot-checked]
- `Cargo.toml:141-142` adds it in this branch. `release-lto` already had it. `services/Dockerfile:38,60` ships `cargo build --release`, so both prod services abort the whole process on any panic. Before, only the task died.
- Runtime-reachable panics: `assert!`s in identity request paths (`auth_handler.rs` `authenticate_with_refresh_session`, `external_login_handler.rs` `assert!(auth_session.user_session().is_some())`), and `assert_not_blocking` in `redis_commands.rs:95`, which panics on any pooled `XREAD`, `XREADGROUP` or `B*` command, even a non-blocking `XREAD`. Any such bug becomes a prod outage.
- The `JoinError` panic handling in builder `connect.rs` ("Send task failed" and "Receive task failed") is now dead code.
- Fix: scope `panic = "abort"` to `release-lto` or to the wasm build. Make `assert_not_blocking` return an error, or use `debug_assert`.

---

## Medium

### M1. Resent pending commands become duplicates (no server dedupe) [validated]
- `shine-game/src/replica/client_stream.rs:199-206`: `on_sync` resends every pending command whose id is not in the sync's tail. The tail holds only confirmed commands after the snapshot (`stream_worker.rs:419-425`).
- The server never dedupes. `stream_state.rs:143-146` says "a command sent twice is two commands". `cl_commands` has primary key `(stream_id, version)` and no unique `command_id`. Chat `Add` uses `command_id` as the message id (`chat.rs:84-90`), so duplicates give two messages with the same id, and `Remove` or `Replace` only hit the first.
- Two paths: (a) the command is in the inbox or `state.pending` when a sync is computed (reconnect, Resync, resubscribe, server `reload()`); (b) the command was confirmed and folded into a snapshot (every 100 versions) while the client missed the broadcast. The doc's premise ("whose commandId it does not find in it") breaks for (b).
- Idempotent kinds such as pixel-map are probably unaffected. Chat is the visible case.
- Fix: dedupe on the server, with a `(stream_id, command_id)` window check that works across instances and after a reload, or put recently folded command ids in the sync so the client can drop them from pending.

### M2. Chain hash omits `command_id` and `user_id`, so the wrong author can be confirmed [validated, narrower than first stated]
- `stable_hash.rs:54-59` folds only parent hash, version and content hash. In `append.rs:108-125`, when the head covers the batch, equal hashes make the stored rows count as "mine", and `persist()` (`command_stream_service.rs:242`) calls `confirm_pending`.
- It needs a stale instance B that polls the inbox after instance A stored and trimmed entry e1, then sees a later entry e2 with identical content. B confirms e2 at the version where the log holds e1, and delivers `StreamCommand{v, e2.command_id, U2}`. U2's command is trimmed from the inbox and never persisted. Replicas diverge until a snapshot hits `SnapshotDiverged`, and only if the state hash depends on ids or user, as chat does.
- The window is milliseconds if the `notify_persisted` wake arrives, up to the 10 s backstop if not.
- Fix: treat any `head.version >= first_version` as someone else's rows. Return Diverged, or compare the stored `command_id` and `user_id`, and go through `catch_up`. Longer term, fold both ids into the stored chain hash (needs a migration).

### M3. Subscribe during the worker's final pass is dropped with no reply [validated]
- `stream_worker.rs:213-217` (grace) and `:202-205` (Stop) run `self.pass().await`. `pass()` puts `state` back, so `self.state` is `Some` after the loop and `reject_queued` (`:220-222`) is skipped. During the pass the channel is open, so `WorkerHandle::subscribe` returns true and the registry doesn't respawn.
- The client stays `waiting`. The TS resubscribe timer only runs for `unsynced` (`stream-connection.ts:117-128`).
- The window is narrow because the `biased` select handles an already-queued Subscribe first. The Stop variant only happens at hub shutdown, when clients reconnect anyway.
- Fix: after the loop, call `receiver.close()` and always drain the receiver. Answer each queued `Subscribe` with `Unsynced`, or re-queue it into a fresh worker. No test covers the race.

### M4. No rate limit, no inbox cap, no WebSocket size limits [validated and hunt]
**Status: size limits done** (32 KiB socket message and frame limit, 2000-byte legacy chat text limit, in `connect.rs`). The rate limit and the inbox cap are still open.

- Missing limit: `connect.rs:299-316` to `HubService::send_stream_command` to `StreamRegistry::push` (`stream_registry.rs:188-208`) to `inbox.rs::push`. There is no per-connection or per-user limit and no `MAXLEN`. `docs/services/builder/hub.md:189` accepts this, but its TTL wording is wrong: `EXPIRE` is re-issued on every push, so under sustained load the key never expires.
- What actually bounds the inbox is the worker: it reads at most `poll_limit=128` per sync, persists, then `XTRIM MINID`. `max_pending=512` only stops `ingest` from polling. A producer faster than the worker's persist rate grows Redis memory until pushes stop, and one user's flood delays everyone on that stream. With `noeviction`, other Redis users (sessions, hub registry) can start failing.
- `max_message_size` and `max_frame_size` are set nowhere [spot-checked], so axum's defaults apply (assumed 64 MiB and 16 MiB, not confirmed). The 16 KiB `MAX_COMMAND_BYTES` check runs only after the frame is read, parsed and re-stringified (`connect.rs:175,304`). `docs/plans/hub_issues.md:63` lists this as item 4.
- Legacy `WSMessageRequest::Chat { text }` has no length cap at all (`chat_service.rs` `append_comment`, `redis_chat_comments_store.rs:23-60`). A multi-MB text goes into `chat:global` (about 200 entries kept) and fans out to every connection. The 1000-message per-connection queue is bounded by count, not bytes.
- Fix: `WebSocketUpgrade::max_message_size` and `max_frame_size` (around 32–64 KiB). A per-connection token bucket in `recv_task`. `XADD … MAXLEN ~ N` or an `XLEN` check in `inbox::push`, with a Nack "busy". A text limit in `ChatService::append_comment` (the stream chat kind already has `CHAT_MAX_TEXT_BYTES = 2000`). Fix the TTL wording in the doc.

### M5. `registry.subscribe` ignores the requested kind when a worker exists [hunt]
- `stream_registry.rs:220-231` vs `:141-176`. `open()` approves the kind, then the hub loop and dispatcher run asynchronously before `registry.subscribe`. With an existing worker, it calls `worker.subscribe(conn)` without checking `worker.kind() == kind`.
- Two users subscribing to the same new stream id with different kinds both pass `open`. The second is attached to the first's worker, and `hub.subscriptions` records the wrong kind for that connection. Their commands are validated as one kind but queued into another stream, and the worker drops them with a warn log per command. It is harmless today only because the serde tags are disjoint.
- Fix: compare `worker.kind()` to `kind` under the lock and return `KindMismatch`. Verify the kind again at `Subscribed` time in the hub.

### M6. Migration replaced instead of added [spot-checked, impact unverified]
**Status: moot if nothing is deployed yet**, provided no shared database has applied main's old V1.

- `V1__commands.sql` is deleted and `V1__command_log.sql` added (`git diff main --name-status`). Main's V1 came from commit `04e09eca`. The repo runs refinery 0.10 (`repositories/db.rs:34-48`).
- If any environment already applied main's V1, refinery may abort at startup on the divergent or missing migration. I haven't checked refinery's behaviour or which databases ran the old V1.
- Fix: if any environment ran it, add `V2__command_log.sql` that drops the old table. Otherwise reset those databases.

### M7. Service migrations run under the new 10 s `statement_timeout` [hunt]
- `pg_connection.rs:308,320` sets `-c statement_timeout=10000` on every pooled connection. `PgConnection::migrate` lifts it (`SET statement_timeout = 0`, ~`:144`), but identity (`repositories/db.rs:42-53`) and builder (`repositories/db.rs:~38-49`) call `runner().run_async(client)` directly on a pooled connection.
- A heavy `CREATE INDEX`, `ALTER` or backfill over 10 s fails and blocks startup.
- Also: in `PgConnection::migrate`, if `RESET statement_timeout` fails after a failed run, the `?` hides the real migration error. The 10 s default also bounds builder command-log prune, snapshot and consistency scans; confirm none exceeds it.
- Fix: route service migrations through `PgConnection::migrate`, or lift the timeout in `migrate_postgres`.

### M8. TLS policy fails open for any stage other than the literal `"prod"` [hunt]
- `tls_policy.rs:5,9,23`: `stage != "prod"` returns `Ok`. `stage` is a free-form CLI argument, so `Prod`, `production`, `staging` or a typo accept `sslmode=disable` and `redis://` against real data.
- Fix: allowlist the non-TLS stages (`test`, `local`, `dev`) and require TLS for everything else. Add tests, since the file has none.

### M9. Redis pool no longer evicts dead connections [hunt]
- `redis_connection.rs:198,295`: `test_on_check_out(false)` drops the PING. `has_broken` only reflects the flag from `mark_if_broken`, which is set only for `is_connection_dropped()` or `is_unrecoverable_error()`. A `TimedOut` error is neither (redis 1.7.1 `redis_error.rs:372-405`).
- After a failover, a NAT or load-balancer idle drop, or a half-open link, commands time out but the connection is reused until the kernel kills the socket, which can take minutes. After a Redis restart, each pooled connection returns one user-visible error before being flagged.
- Fix: also flag `err.is_timeout()` or `ErrorKind::Io` as broken, or restore the PING (or an idle-time PING).

### M10. `Problem::log` dumps sensitive internals at info and error [hunt]
- `problem_detail.rs:150-166` logs `{self:#?}`, including `sensitive`: Postgres and Redis error dumps, and the attacker-controlled `Host` and `Origin` headers (builder `connect.rs:79,86`). Callers: `ProblemResponse::into_response` and `auth_page_handler.rs`.
- 4xx responses log this at `info`, so every 401, 403 and 400 emits a multi-line dump. This violates the CLAUDE.md rule that sensitive data goes at `trace` only.
- Fix: log status, type and a short safe cause label at info or error, and the `sensitive` dump only at trace. At minimum skip it for 4xx.

### M11. Pre-deploy checks for the builder service [hunt]
- `services/builder/src/repositories/db.rs:12` has `pub sql_cns: String` with no serde default, and `server_config.prod.json` has no `db` block. A missing `sqlCns` (Key Vault or env) fails deserialization at startup. If present, it must satisfy `sslmode=require` and cannot carry pool params (H1). `rediss://` is also required.
- `redis_check.rs` (`MIN_REDIS_VERSION`) hard-fails below Redis 6.2, and Azure Cache for Redis v6 can report 6.0.x. Confirm the prod version.

### M12. Agent git-deny hook removed from `.claude/settings.json` [validated]
- Removed in `ecad142f` ("Improve tracing and metrics (#192)"), and earlier in `992ed00c`, so it looks deliberate but is unexplained. `main` still has it. The same hunk re-serialises `extraKnownMarketplaces`.
- Nothing replaces it: `.claude/CLAUDE.md` has no git rule, there is no `settings.local.json`, and no plugin hook has it. Only the user's private memory note remains.
- Fix: restore the hook, or state in the PR that the removal is intentional and move the rule into `CLAUDE.md`.

---

## Low (builder, hub, stream)

| # | Finding | Where | Notes |
|---|---|---|---|
| L1 | `sync()` Err drops `outcome.confirmed` [validated, downgraded] | `stream_worker.rs:244-250`, `command_stream_service.rs:116-145,265,276` | State has advanced but adopted commands aren't delivered, so `sent_up_to` lags. The next command shows a version gap and the client resubscribes, so it self-heals. Fix: return the partial outcome with the error, or deliver from `state.tail()` above each subscriber's `sent_up_to`. |
| L2 | Per-subscriber JSON re-parse in egress [validated, downgraded] | `message.rs:101-113,128-159`, `connect.rs:350-351` | CPU only, scales with subscribers times command rate. The base64 half of the original claim is wrong: `Sync` is built per subscriber, so its encode runs once per message. Fix: cache the encoded frame on the shared message, or store the command as `RawValue`. Only if profiling shows it. |
| L3 | `ConnectUser` registry failure [validated, mostly refuted] | `hub_service.rs:407-414` | The socket does close (the sender is dropped, so `forward_egress` sends Close) and the client reconnects with backoff. What remains: the close has no code or reason, there is no `connections_dropped` metric, and no test covers the branch. |
| L4 | No write timeout or WS ping | `connect.rs:359,219-251` | A half-open peer fills its queue and the hub drops it as `SlowConsumer`, but `forward_egress` stays blocked in `send().await` and `recv_task` in `next()`. Tasks leak until TCP gives up. Fix: `timeout` around `send`, periodic Ping with Pong deadline, abort tasks after a hub-initiated drop. |
| L5 | Worker tasks unsupervised | `stream_worker.rs:140`, `stream_registry.rs:146,219,243` | The `JoinHandle` is dropped. A panic in `run` (e.g. an `expect` in `FromRow`, or a poisoned std Mutex) leaves subscribers without `Unsynced`, while `hub.subscriptions` still accepts their commands. Fix: supervise and broadcast `Unsynced`; don't panic on lock poison. Matters more with H3. |
| L6 | Grace timer can be starved | `stream_worker.rs:197-218` | `biased` select with the wake arm first. On streams with other-instance traffic the wake permit is always ready, so a zero-subscriber worker never unloads. Fix: check `grace_deadline` at the top of each iteration. |
| L7 | Inbox trim only when `stored_inbox` advances | `command_stream_service.rs` `sync`/`finish`, `stream_state.rs:146-174` | A failed trim is only warned and not retried. Malformed entries are never trimmed until a later valid command is stored. Fix: trim on every successful sync. |
| L8 | Hub lifecycle events fan out to every socket | `hub_service.rs:639-663`, `connect.rs:387` | O(N) per connect and O(N²) in a reconnect storm. A near-full queue can be dropped as `SlowConsumer` by someone else's connect. Fix: send each socket only its own `UserDisconnected`, drop the Hub topic. |
| L9 | Best-effort shutdown | `main.rs:46-50`, `web_app.rs:91-100` | The hook enqueues `Shutdown` and returns, so in-flight passes are cancelled. Commands stay in the inbox (1 h TTL), so only persistence is delayed. |
| L10 | `ensure_stream` writes on every load | `pg/stream.rs:538-546` | `ON CONFLICT DO UPDATE SET stream_id = stream_id` makes a dead tuple and a row lock. Use `DO NOTHING` plus SELECT. |
| L11 | Info-level spans and logs on hot paths | `connect.rs:177,197`, `redis_chat_comments_store.rs`, `redis_hub_registry.rs`, `otel_http.rs:113,207`, `web_app.rs:104` | Per-message `info_span!("ws message", enduser.id)` is exported to AppInsights, and pixel-map painting can be high rate. Registry heartbeat is info per connection per tick. `web_app.rs:104` logs "alive connections" every 1 s at info. User ids appear in many info lines. Rule: per-tick and per-message at debug or lower. |
| L12 | `log_events` counts startup self-test lines | `telemetry_service.rs:100,105`, `web_app.rs:207,212` | Every restart adds 2 warn and 2 error events, plus the "Trace is disabled" warn. Alerts on that metric fire on every deploy. |
| L13 | `shine-test` panic hook exits the whole binary | `crates/shine-test/src/lib.rs:~48-84` | Any panic in a spawned task calls `process::exit(-1)`, so failures can't be attributed. Tests that expect a task panic can't exist. |
| L14 | Fault-injection tests can pass without running | `Toxiproxy::from_env()` | Returns `None` with only a `tracing::warn!` when `SHINE_TEST_TOXIPROXY_URL` is unset. |
| L15 | CI and dev defaults | `.github/workflows/fuzz_ci.yml`; `.vscode/tasks.json`, `run_identity_local.*` | Fuzz job uses unpinned nightly and an uncached `cargo install cargo-fuzz`. Dev defaults export OTLP to `localhost:4317` with no listener, which spams warnings that L12 counts. |

---

## Low priority: web client and `shine-game` replica

1. **Stale `live` after reconnect** (`stream-connection.ts` onopen and `afterFrame`, `client_stream.rs:119`, `pixel-map/index.ts` `paint()`). Nothing resets the stream to `Waiting` on drop or reopen, and an unrelated `chat` frame flips the connection to `live` early. Clicks during an outage or before the sync go out and can be submitted twice (M1). Fix: have `subscribeFrame` call `stream.resync()`, or add `StreamPort.reset()` from `onclose` and `onopen`.
2. **No watchdog on `waiting` and `syncing`.** A lost or unparseable `Sync` (one bad field fails the whole message) leaves the client stuck. The `unsynced` retry timer fires once and is never re-armed. Fix: re-send the subscribe with backoff for any non-live status.
3. **Backoff resets on every successful open** (`stream-connection.ts:~87`). If the server accepts the upgrade and then closes (`SlowConsumer`, `SessionInvalid`, `HeartbeatInactive`, `DeadChannel`), the client retries every 1 s forever. Fix: reset the attempt count only after the first sync or after a minimum open time.
4. **Resync while closed or replaced** strands the stream in `Waiting` (`pixel-map/index.ts:~140`).
5. **`failed` nack drops the pending command** (`client_stream.rs:254-265`) although the server says it may have been queued.
6. **Persistent checkpoint mismatch loops forever** (`on_checkpoint`, `client_stream.rs:236-252`). Needs a counter or a transition to `Unsynced`.
7. **`apply_wire` records a hash for a skipped command** (`client_stream.rs:289-310`), which can cause a false checkpoint mismatch if a Sync tail overlaps the snapshot.
8. **`restore` accepts trailing bytes**, and the state hash comes from the same untrusted frame. The client never verifies `chain_hash`, so the "tampered or diverged" check is a no-op on the client (`client_stream.rs:281-286`, `codec.rs`).
9. **Random-UUID user id fallback** (`pixel-map/index.ts:~33`) breaks the optimistic view for user-scoped chat commands when the HttpOnly `sid` cookie is unreadable.
10. **`new WebSocket(url)` unguarded** [validated, mostly refuted, low] (`stream-connection.ts:80`). The only caller passes a fixed URL, so a throw would happen on the first `start()`. Cheap hardening: try/catch and fall into the backoff path.
11. **Lifecycle:** `start()` twice leaks the first socket and timers, `stop()` then `start()` keeps a stale attempt count, and permanent `StreamSubscribeFailed` reasons (`unknownKind`, `kindMismatch`) are retried every 5 s forever.
12. **Docs:** `engine/net/` is missing from the `client-web-architecture` skill's layout table, and `onChange: () => {}` in `index.ts` is unused.

---

## Checked and found sound
- The Rust and TypeScript wire protocols line up (tags, fields, hex hashes, the `notSubscribed` nack, the `replaced` close reason). Gap detection in `on_commands` is correct. `Reset` is always followed by `Sync`. The wasm path has no std-only APIs.
- Builder: `append` is one transaction with an INSERT and a CAS on `(version, chain_hash)`. Reload and give-up re-read the inbox from `stored_inbox`. `adopt` gap and chain checks, trim `MINID` arithmetic, and prune logic are fine. Session and user id come from the server. Origin and host checks run before the upgrade. No lock is held across an await. Subscribe and disconnect ordering is safe.
- Infra: `sslmode` parsing is case-sensitive and fail-closed, and a missing `sslmode` defaults to `Prefer`, which the check rejects in prod. No `log::` calls remain outside the allowed places. No connection strings are logged. Token, CSRF and email dumps are at `trace`. The new streams API tests have real assertions.

## Missing tests worth adding
- Rust, `shine-infra`: `tls_policy.rs` (prod with pool params, odd stage names).
- Rust, builder: the final-pass Subscribe race (M3), the kind mismatch (M5), `ConnectUser` registry failure (L3), `sync()` Err after `catch_up` adopted commands (L1), a stale instance racing an identical-content command (M2), and the `ensure_stream` and migration path against a database that has main's V1.
- Rust, `shine-game` `client_stream`: a mid-batch gap, a command that fails to decode, a checkpoint mismatch while `Waiting`, non-contiguous or overlapping Sync tails, trailing snapshot bytes, `HASH_HISTORY` eviction at 256, a `failed` nack, and a command folded into the snapshot but not in the tail.
- TypeScript, `stream-connection.test.ts`: stale `live` after reconnect, a hang in `waiting`, an immediate close after open with growing delay, `start()` twice, the `unsynced` timer re-arming, and `resync()` while closed or replaced.
