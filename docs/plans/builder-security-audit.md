# Builder service: security and robustness audit

Non-SDP investigation notes. Not kept in sync with the code.

Open findings for the builder service: the hub and WebSocket layer (`services/hub`, `routes/ws`, `repositories/hub_registry`), the command stream (Redis inbox, relay, persistence, Postgres log, worker, registry, stream access), the shared `shine-infra` / `shine-core` code it runs on, its web clients (`shine-game` replica, web hub client) and the deployment files in front of it (nginx, Dockerfile, Fly, CI). Out of scope: the identity service and unrelated crates.

**Method.** Static reading only, no live system touched, nothing exploited, no scanner or test run (`cargo audit`, `cargo deny`, `clippy` and the tests were not run). Every item was re-checked against the code at the time of writing; items that turned out fixed or wrong are listed under [No longer valid](#no-longer-valid). Line numbers drift, treat them as pointers.

**Paths** are relative to `services/builder/src/` unless they start with `crates/`, `client/`, `services/`, `.claude/` or `.github/`.

**Status:** `NEEDS ATTENTION`. No Critical or High finding. Three Medium findings (H2, C5, L18), six Low / Medium ones (H1, H3, L2, E1, H4, C1), one Low finding (L19), and a long tail of Low ones. This is not a statement that the service is secure; see [Limitations](#limitations).

**Design assumption.** Several builder instances may serve the same stream, each with its own worker reading the same Redis inbox and appending to the same Postgres log. Redis pub/sub wakes are best effort. Production serves a fixed allow-list of two streams (`chat/global`, `pixelMap/pixel-demo`) behind nginx on a single Fly machine type.

## Accepted decisions (not findings)

These were decided earlier and are kept as conventions. Several were also flagged independently in this pass; the consequence is recorded so nobody re-opens them by accident.

- **`command_id` is a client-owned placeholder; the server does not dedup by it.** Consequences: a client retry after a lost reply (`CommandFailure::Failed`) can be stored twice, and a user can reuse another user's public chat message id (`ChatCommand::Add` takes the message id from the command id, `crates/shine-game/src/replica/chat.rs:85`). Server state stays consistent: the author always comes from the session, and `Remove` / `Replace` require id and author to match. Clients must not trust message ids as unique.
- **The inbox soft backpressure limit is the only size bound** (`XADD` without `MAXLEN`, `repositories/command_inbox/redis/inbox.rs:24-46`). Between two 250 ms polls the inbox can overshoot the 1024 high watermark by the aggregate push rate; bounded and transient.
- **A missing hub registry row means inactive.** After a Redis flush or restart every connection is dropped as `HeartbeatInactive` at the next heartbeat (up to 300 s) and reconnects; the reconnect storm is accepted. (It does run through the hub loop, see H1.)
- **Only the literal `prod` stage requires TLS** for Postgres; a new stage on a hosted database needs its own TLS check.
- **`panic = "abort"` on release.** A panic is a critical bug and the platform restarts the service. Findings that only matter when a panic is survived are moot. No panic reachable from client input was found.
- **Once a V1 migration has run on a database that is not purged, change it only with a new migration.**
- **`Problem::log` dumping internals at `info` / `error`** is a documented exception to "sensitive data: trace only".
- **Every authenticated user may read and write every allow-listed stream**, and every subscriber learns every author's raw `user_id` (R22).

## Summary

| ID | Sev | Area | Finding |
|----|-----|------|---------|
| H1 | Low / Medium | availability | Hub command loop does Redis I/O inline and serially |
| H2 | Medium | session | An open socket keeps an idle session alive forever; a revoked session works for up to 15 min; no max session age |
| C5 | Medium | client | Stale state after reconnect; the web hub reconnects after every close, including "replaced", so two tabs fight forever |
| H3 | Low / Medium | authz | A dropped, replaced or revoked connection can still open streams and push commands |
| L2 | Low / Medium | multi-instance | Persisted-version wakes are ignored; a peer that missed a wake stays stale |
| E1 | Low / Medium | capacity | Fly `hard_limit` 25 is shared with identity; nginx has no connection or rate limits |
| H4 | Low / Medium | resource use | Unsubscribe is charged to the wrong bucket; re-subscribe forces a full sync; per-connection serialization |
| C1 | Low / Medium | client | No watchdog on `waiting` and `syncing` |
| C2 | Low | client | Backoff resets on every open |
| C3 | Low | client | A `failed` nack drops the pending command |
| C4 | Low | client | A persistent checkpoint mismatch loops forever |
| C6 | Low | client | Overlapping `Sync` tail records a bad hash |
| C7 | Low | client | Snapshot decode accepts trailing bytes |
| C8 | Low | client | `StreamSubscribeFailed` / `desynced` retried every 5 s forever, no jitter |
| C9 | Low | docs | `net/` missing from the `client-web-architecture` skill |
| L4 | Low | integrity | A `Corrupt` load gives the worker up; snapshot store checks only `chain_hash` |
| L5 | Low | lifecycle | `is_finished()` true while a worker drains; draining workers invisible to `shutdown` |
| L6 | Low | correctness | Inbox cursor never detects an id regression |
| L7 | Low | correctness | Inbox trim only after commands are stored |
| L8 | Low | perf | `ensure_stream` writes on every worker load |
| L9 | Low | correctness | Load/prune race yields a `Gap` |
| L12 | Low | compat | Rolling deploy: old binary cannot decode new command variants |
| L14 | Low | misc | Chat text stored and relayed raw |
| L15 | Low | lifecycle | A stale `StreamFailed` tears down a healthy successor worker |
| L16 | Low | lifecycle | A subscribe can be lost at worker idle-unload |
| L17 | Low | availability | `Contention` after 8 lost head CASes makes the worker give up |
| L18 | Medium | durability | Inbox expiry can delete commands before they are persisted |
| L19 | Low | robustness | Invalid hub intervals can panic tasks or break registry connections |
| A1 | Low | authz | Access is by stream id only; no re-check after subscribe; `any` is valid in prod |
| A6 | Low | authz | `Ping` has no role check (off by default and in prod) |
| S1 | Low | auth | The session fingerprint is a hash of the User-Agent only |
| S2 | Low | config | Trace-level reconfiguration enabled in prod |
| S3 | Low | secrets | Secrets reachable through `Debug` and startup `trace` logging |
| S4 | Low | secrets | Key Vault loader reads every secret; builder uses identity's credentials |
| S5 | Low | supply chain | Unpinned downloads, images and CI actions |
| R9 | Low | availability | Registry listener: no retry of the first subscribe, no recheck after reconnect |
| R10 | Low | correctness | `Connected` sent before registration |
| R13 | Low | perf | Lifecycle events broadcast to every socket |
| R17 | Low | security | Host check trusts client headers (not a control) |
| R18 | Low | compat | Unversioned Redis keys and channel; no environment prefix |
| R19 | Low | docs | Single-primary Redis assumed, undocumented |
| R22 | Low | design | Raw `user_id` in commands |
| R23 | Low | resource leak | No TCP keepalive / user timeout on the listener |
| O2 | Low | logging | Info-level spans/logs on hot paths |
| O5 | Low | perf | Registry notices enqueue commands for users the instance does not hold |
| O6 | Low | protocol | Protocol gaps (no version/connection id, silent drops, OpenAPI) |
| X1 | Info | misc | Control frames uncharged, public `/info/version`, zero-duration config, dead `UserAlreadyConnected`, benign leftovers |

---

## Hub and session

### H1. Hub command loop does Redis I/O inline (Low / Medium)

`routes/ws` hands every connect, disconnect and registry change to one command loop that serializes all state mutation. Four handlers await Redis from that loop (`services/hub/hub_loop.rs`):
- `ConnectUser`: `create_connection` (`:126`), and on failure `is_active` (`:130`), so a failing connect pays the pool wait twice.
- `DisconnectUser`: `remove_connection` (`:178`), on every removed connection, client disconnects included.
- `HubRegistryChanged`: `heartbeat_connection` (`:247`).
- Shutdown: bounded (32 at a time, 5 s cap).

Subscribe, unsubscribe, `StreamFailed` and publish do no Redis I/O.

Why it matters: the checkout waits `pool_timeout`, default 30 s (`crates/shine-infra/src/db/redis/redis_connection.rs:337`; the production value comes from Key Vault, so the real figure is unknown). The builder pool (`repositories/db.rs:22`, default `max_size` 10) is shared by the hub registry (loop and heartbeat) and the command inbox (pushes and workers), so stream traffic can starve the loop's checkout. (The HTTP session lookups and the session checker use a separate pool.) During a Redis stall the hub is effectively down: subscribes, failure notices and disconnects queue behind the loop, the control queue is unbounded, and recovery takes roughly the queue length times the pool wait. Sockets whose commands are still queued may already be gone. The reconnect storm after a Redis flush (accepted) runs through the same loop.

Fix, if needed: a short `pool_timeout` in the builder's `redisCns`, a separate small pool for the hub, a `tokio::time::timeout(1-2 s)` around the registry calls in the loop, or moving `remove_connection` and the heartbeat off the loop (the in-memory map is authoritative locally, so the registry write can be an async write with retry).

### H2. Session lifetime: no max age, sliding refresh, slow revocation (Medium)

- `get_current_user` runs `EXPIRE` on the sentinel and data keys on every call (`crates/shine-infra/src/session/current_user_service.rs:131-136`) and has an open `todo` for a maximum session lifetime (`:130`). `created_at` is only reported as `session_start`.
- The session checker (`services/hub/tasks/session_checker.rs:41-43`) validates sessions through that same call, so for a WebSocket-only user the checker is the only keepalive: while a socket stays open the session never reaches its 1800 s inactivity expiry (`services/config/common.prod.json`, `sessionTtl`). `session_check` must therefore stay well below `sessionTtl` (900 s in prod, `services/builder/server_config.prod.json`); no config validation enforces it.
- A revoked session (logout, admin revoke) keeps its socket and its stream access for up to 900 s plus the tick time. The check only tests that the session exists: it does not re-check the fingerprint (`SessionCompromised` is raised only by the upgrade extractor, `crates/shine-infra/src/session/current_user.rs:104`, never by the periodic lookup) or stream authorization.
- The lookups run with bounded concurrency (32), and a Redis outage logs one warning per tick with a count.

Fix, if needed: a read-only check that does not extend the lifetime (a product decision, it stops the checker keeping WebSocket-only sessions alive), plus a hard maximum age from `session_start`; revoke by event (publish on logout); or a batched lookup (one pipeline per tick). A shorter `session_check` makes the window smaller, not gone.

### H3. A dropped connection can still subscribe and push (Low / Medium)

When the hub drops a connection (replaced, session invalid, registry inactive, rate limited) it calls `registry.disconnect`, but the socket's receive task keeps handling frames until its send task finishes writing the close frame (up to the 10 s write timeout against a peer that does not read; `routes/ws/connect.rs:214-298` never checks whether the connection was closed). In that window:
1. `StreamSubscribe` reaches `StreamRegistry::open`, which never asks the hub whether the connection is live (`services/command_stream/stream_registry.rs:187-217`) and re-inserts the connection.
2. A `StreamCommand` reaches `push`, which trusts the registry map (`:277-302`) and writes to the inbox.
3. Only when the loop later handles `SubscribeStream` does `contains(connection_id)` fail and `registry.disconnect` clean up (`services/hub/hub_loop.rs:198-207`).

The window is the control-queue latency: microseconds when idle, seconds when the loop is stalled (H1). Impact: a replaced, banned or session-expired user can inject a few commands into the shared chat or pixel stream; the per-socket frame budget (60 burst) still applies. No worker leak: the dispatcher never starts a worker for an unregistered connection.

Fix: break the receive loop as soon as the connection has a close reason; have `open` and `push` consult hub liveness, or keep a tombstone of closed connection ids in the registry.

### H4. Resource use: unsubscribe charge, re-sync, serialization (Low / Medium)

- `StreamUnsubscribe` is charged only to the frames bucket (`routes/ws/message.rs:37-44`), while the limits doc comment says a subscribe or unsubscribe costs a subscribe token on top (`routes/ws/ws_config.rs:52-53`). Each unsubscribe queues a control command and takes a publish write lock inside the hub loop, at up to 30/s per socket.
- Re-subscribing to a running stream forces a full `Sync` (snapshot plus tail): up to about 1 MB for a full chat window (200 messages of up to 2 KB, plus up to a few hundred tail commands), at the subscribe budget of 10 burst then 2/s per socket.
- Every connection re-serializes the shared message (`serde_json::to_string` at `routes/ws/connect.rs:539`, base64 in `routes/ws/message.rs:122,166`), so one user sending valid chat commands at 30/s to N subscribers costs N × 30 serializations per second. CPU only.

Fix: charge unsubscribe to the subscribe bucket; cache the encoded frame per `Arc<StreamMessage>` (if profiling shows it); rate-limit re-syncs per (connection, stream) and commands per (user, stream).

### R9. Registry listener (Low)

`services/hub/hub_service.rs:232-250` spawns the listener and does not await it.
- A failed first subscribe is not retried: the task logs and calls `supervisor.task_ended`, which now ends the whole service gracefully (`crates/shine-infra/src/web/web_app.rs:113`), so the platform restarts it. A single transient subscribe error at start therefore restarts the process.
- A later reconnect is handled by the keep-alive task in `crates/shine-infra/src/db/redis/redis_listener.rs` (exponential backoff, re-subscribes every channel and nudges handlers with `None`). It never reaches the supervisor. The registry handler ignores `None` (`repositories/hub_registry/redis/redis_hub_connection_db.rs:49-54`) and does not recheck local users, so notices lost during an outage are reconciled only by the 300 s heartbeat; a cross-node takeover can leave two live sockets for up to about 300 s.

Fix: retry the first subscribe with backoff; recheck local users after a reconnect.

### R10. `Connected` precedes registration (Low)

`routes/ws/connect.rs:195-210` queues `ConnectUser` (a channel send) and then sends `Connected`. If `create_connection` fails and `is_active` is false (`services/hub/hub_loop.rs:126-133`), the connect is dropped, the egress ends as `Gone` and the client gets a close frame with no code or reason. The hub side is covered by a test; what the client sees is not. Fix: send `Connected` once registered, or close with a reasoned code on failure.

### R13. Lifecycle events fan out to every socket (Low)

Sockets subscribe to `TopicKey::Hub` (`routes/ws/connect.rs:584`) but `event_to_wire_message` drops hub events (`:146`). `HubLoop::publish` still broadcasts each lifecycle event to all sockets (`services/hub/hub_loop.rs:330`, `hub_connections.rs:179-202`: clone and `try_send` under the registry read lock) and the events use the same 1000-slot queue that decides `SlowConsumer` eviction. A connect or disconnect costs O(N); a mass drop (Redis flush) costs O(N²) inside the loop, and other users' churn pushes slow sockets toward eviction. `UserConnected` carries the session key; it only travels in-process and its `Debug` is redacted. Fix: do not subscribe sockets to `TopicKey::Hub`; keep the session key out of the broadcast.

### R17. Host check trusts client headers (Low, not a control)

`crates/shine-infra/src/web/extracts/target_host.rs:33-43` prefers `X-Forwarded-Host`, then `Forwarded host=`, then `Host`; `routes/ws/connect.rs:97` checks the result against `allowedHosts`. In the deployed setup `location = /builder/api/connect` overwrites `Host` and `X-Forwarded-Host` with `$host` (`services/docker/nginx.conf`), and port 7001 is not exposed from the container. But `$host` still comes from the client's `Host` (the prod nginx has no `server_name`, so any value is accepted), and if `$host` were empty a crafted `Forwarded` could win. The `Origin` check (required, anchored regexes) is the real defence. Fix: trust forwarded headers only from the proxy, and do not treat the host check as a control.

### R18. Unversioned Redis keys and channels (Low)

`hub-connection:`, the channel `hub-registry-changed`, `cmd-inbox:{id}` and `cmd-inbox-wake` have no version and no environment prefix (`repositories/hub_registry/redis/redis_hub_connections.rs:11-12`, `repositories/command_inbox/redis/redis_command_inbox_db.rs:33`, `.../wake.rs:13`). A mixed-version deploy or two environments on one Redis would make instances drop each other's users. The session store already has a `key_prefix`. Fix: version the keys and channels, prefix by environment, treat an unknown format as foreign. Related: L12.

### R19. Single-primary Redis assumed (Low)

The heartbeat is a multi-key Lua `EVAL` over up to 256 keys (`redis_hub_connections.rs:23-38,75-87,125`). It would hit CROSSSLOT in cluster mode, so a single-primary Redis is an undocumented assumption. Fix: document it, or make the script cluster-safe.

### R22. Raw `user_id` in commands (Low, design)

Every command sent to subscribers carries the sender's raw `user_id` (`WireStreamCommand`, `routes/ws/message.rs:68-73`). Decide and document.

### R23. No TCP keepalive on the listener (Low)

`crates/shine-infra/src/web/web_app.rs:395` is a plain `TcpListener::bind` plus `axum::serve` (the TLS branch uses `axum_server::bind_rustls`); neither sets keepalive or a user timeout. The application layer already ends half-open peers (idle 90 s, ping 30 s, write 10 s), and in the container the builder sits behind nginx on loopback, so the benefit is small. The `send_drain` abort has no test.

### A1. Access is by stream id only (Low)

Which streams may be opened is configured per kind (`streams.<kind>.access`: `allowList` with ids, or `any`, with an empty allow-list as the fail-closed default). Production uses fixed allow-lists, so the number of streams is bounded and stream creation is not an abuse vector. What remains:
- Every authenticated user may read and write the allowed streams: `AllowListStreamAccess::allows` ignores `user_id` (`services/command_stream/stream_access.rs:34-38`).
- Authorization is only checked in `StreamRegistry::open` (`stream_registry.rs:198`) and not re-evaluated after subscribe (H2, H3).
- `mode: any` is accepted in every stage: `Validate` checks only ids (`stream_config.rs:136-150`) and `create_access` has no stage check (`stream_registry.rs:146`). Only the config layers keep it out of prod (`config/mock.json` sets it). A start-up check rejecting it for the `prod` stage would close that.

### A6. `Ping` has no role check (Low; None while disabled)

With `pingEnabled` on, any logged-in user may send `{"type":"ping","id":"<255 chars>","userId":<any uuid>}` or broadcast to every connection (`routes/ws/connect.rs:459-478`). Every socket is subscribed to the ping topic, so a broadcast can push attacker-chosen strings to all clients and fill queues until victims are evicted as `SlowConsumer`. `pingEnabled` defaults to `false` (`services/hub/hub_service.rs:59,81`) and `server_config.prod.json` sets it false; `server_config.test.json` enables it. The socket's frame budget limits how often. Fix: operator-only role and reject `userId` for non-admins, or delete the feature.

### S1. The session fingerprint is a hash of the User-Agent only (Low)

`crates/shine-infra/src/web/extracts/client_fingerprint.rs:69-79`: SHA-256 of the `User-Agent`; a missing UA becomes `unknown` and passes. Compared at `crates/shine-infra/src/session/current_user.rs:100`. A stolen signed cookie plus the same UA bypasses the token-theft check. The anchored `Origin` allow-list is the real defence against cross-site WebSocket hijacking. Fix: bind the fingerprint to more than the UA, or stop treating it as a control.

### S2. Trace-level reconfiguration is enabled in prod (Low)

`services/config/common.prod.json` sets `telemetry.allowReconfigure: true`; `PUT /builder/api/telemetry/config` is guarded by `CheckedCurrentUser` and the `UPDATE_TRACE` permission (`crates/shine-infra/src/telemetry/telemetry_router.rs:26-43`). A holder of that permission can enable `trace` in prod, where this codebase allows sensitive data (for example the raw invalid client message at `routes/ws/connect.rs:291`). The startup config dump is not re-emitted at runtime. Fix: review who holds `UPDATE_TRACE`; restrict the accepted filter directives.

### S3. Secrets reachable through `Debug` and `trace` (Low)

`ServiceConfig` (`session_secret`, `captcha_secret`, `session_redis_cns`) and the builder's DB config derive `Debug` and the whole config is dumped by `tracing::trace!("Config: {cfg:#?}")` at startup (`crates/shine-infra/src/web/web_config.rs:212`), values resolved from Key Vault included. Within the trace-only convention, but fragile. `SessionKey`'s `Debug` is redacted. Fix: a redacting `Secret<T>` wrapper or a manual `Debug`.

### S4. Key Vault scope and credentials (Low)

`crates/shine-infra/src/azure/azure_keyvault_config.rs:55-85` loads every enabled secret in the vault, and `services/docker/scripts/start-builder.sh` authenticates builder with the identity service's `IDENTITY_*` Azure credentials. A builder compromise therefore exposes every identity secret. Fix: a separate vault or credential for builder, limited to the secrets it uses.

### S5. Supply chain and CI (Low)

The otelcol tarball is downloaded without a checksum, base images (`rust:bookworm`, `nginx:bookworm`) and `cargo install cargo-chef` are unpinned, and CI actions are pinned by tag (including `superfly/flyctl-actions/setup-flyctl@v1`, used with `FLY_API_TOKEN`). The deploy job runs only on `main`; the container runs as the unprivileged `nginx` user. Fix: pin by digest or commit SHA and verify checksums.

### E1. Capacity and edge limits (Low / Medium)

`services/fly.json` sets `concurrency.hard_limit: 25` (soft 20) per machine, and identity shares that container. About 25 long-lived sockets can therefore block new HTTP connections, logins included, on that machine. `services/docker/nginx.conf` has `worker_processes 1`, `worker_connections 1024` (each proxied socket uses two) and no `limit_req` / `limit_conn`. The hub's own limits (10 000 connections, connect rate 5 burst then 0.1/s per user) cannot be reached first. It needs authenticated accounts to hold the sockets, so the exposure depends on how cheap account creation is (not checked). Fix: decide the intended per-machine socket capacity, raise or retype the Fly limit, and add nginx `limit_conn` / `limit_req` on the connect route.

### O2. Info-level spans and logs on hot paths (Low)

Still `info`: the per-command `info_span!("hub command")` (`services/hub/hub_loop.rs:83`), the `#[instrument(level = "info")]` on four registry methods (`redis_hub_connections.rs:91,116,138,147`), the per-client-frame `info_span!("ws message")` (`routes/ws/connect.rs:278`), the `ws open` / `ws close` spans (`:570,598`) and per-connection info logs. The per-frame span is the hot one. Fix: `debug`.

### O5. Registry notices enqueue a command for users the instance does not hold (Low)

The listener forwards every peer notice (only self-originated ones are dropped) as `HubRegistryChanged` (`services/hub/hub_service.rs:240`); the loop then does an in-memory `find_active` and returns (`hub_loop.rs:243-245`). Cost: a queue entry plus an `info` span per notice, and a Redis heartbeat inside the loop for a locally held user. Filtering needs a lookup against `Connections` from the listener's sync handler.

### O6. Protocol gaps (Low)

`Connected` carries only `user_id`, with no version or connection id (`routes/ws/message.rs:79-81`). Invalid JSON, binary frames, bad stream ids and `Invalid` commands get no reply (`routes/ws/connect.rs:289-295,407-409,449-452`). The OpenAPI path documents only `OK` (`:79-87`). Garbage frames do cost a rate-limit token and 20 consecutive denials close the socket as `RateLimited` (1008), but a client sending garbage below the budget (burst 60, 30/s) is never closed.

---

## Command stream

### L2. Persisted wakes are ignored: stale instances never reconcile (Low with one instance, Medium with several)

Open, analysed, no code changed. With one instance it cannot happen. With several, the clients of an instance that missed a wake silently stop receiving commands (no data loss: the log append identity check rejects stale appends). The instance's own worker sends each command to its clients on the inbox wake, before it is persisted; persistence is the backup, not the delivery path. After a missed wake nothing tells those clients they are behind (checkpoints follow only the instance's own persists, the web client has no watchdog, C1). They recover on the next command reaching that instance or on resubscribe: seconds on a busy stream, indefinitely on a quiet one.

**Today**
- A push publishes `<stream>` on `cmd-inbox-wake`; after a persist the instance publishes `<stream>@<version>` (`notify_persisted`, `repositories/command_inbox/redis/wake.rs`). Every instance with a worker for the stream hears both, and the persisted version is parsed (`wake.rs:182-191`) but unused.
- The registry `listen` handler throws the version away: it calls `Notify::notify_one` on the worker (`services/command_stream/stream_registry.rs:392-401`, `stream_worker.rs:89-91`). A listener reconnect (`InboxWake::All`) wakes every worker the same way.
- A woken worker runs `process_inbox`: read the inbox after its `read_index`, version, submit to persistence. It never compares its head with the log. `PersistenceWork::Reconcile` is requested once, when `RelayLoop::run` starts (`stream_worker.rs:271`). A persist whose baseline no longer matches also triggers a reload (`stream_persistence.rs:105-113`); that is the "next command" recovery path.
- Peers trim the inbox through the entries they stored. If C misses the wake, A persists and trims; C finds an empty inbox. The 10 s recovery poll (`recoveryPollInterval`) runs the same `process_inbox`, so it does not help. The same hole exists after a reconnect.

Failure: C misses the wake of e11; A persists v11 and trims. C's subscribers never get v11; a new subscriber on C gets a `Sync` of stale state. When the next command reaches C it is versioned from the stale head, its append fails (`Behind`), C reloads and resyncs: a wrong frame, then a correction.

Not causes: the `room == 0` early return in `StreamRelay::ingest` (`settle` notifies again) and a busy loop (`notify_one` keeps one permit).

**Proposed fix**
1. The worker keeps the highest persisted version heard (`AtomicI64::fetch_max`, set by the registry handler, `wake` carries it). In the relay loop, when it is above `confirmed_version` and nothing is in flight, send `PersistenceWork::Reconcile` (one cheap `head` read per peer persist). Compare after `settle`, since an instance's own announcement arrives before its state updates.
2. On `InboxWake::All`, flag every worker for a reconcile.
3. No periodic reconcile (a `head` read per stream per instance every 10 s costs more than it adds).

**Tests**: two workers on one inbox and log, A persists and trims, C gets only the persisted wake and its subscriber must get the commands; `All` after a peer persisted and trimmed gives a reconcile and a resync; a persisted version not above `confirmed_version` sends no `Reconcile`. Open question: whether a reconcile may be sent while a persistence is in flight (the proposal waits).

### L4. A corrupt load gives the worker up (Low)

Replay recomputes `command_hash` (`repositories/command_log/command_log_types.rs:31`) and checks the restored state hash against `snapshot.hash`; a mismatch is `CommandStreamError::Corrupt` (`services/command_stream/stream_persistence.rs:199-225`) and `give_up` runs (`stream_worker.rs:310-311`). Not done: no fallback to an older snapshot, and `try_store_snapshot` checks only the stored command's `chain_hash` (`repositories/command_log/pg/snapshot.rs:109-118`; `agrees()` at `:166` compares the hash only when a row already exists).

### L5. Worker replacement overlap and shutdown blind spot (Low)

`run()` calls `receiver.close()` before `drain()` and `is_finished()` is `messages.is_closed()` (`stream_worker.rs:99-101,339-340`), so the registry replaces the worker (`stream_registry.rs:351-358`) while it still drains. Two workers for one stream then run in one process; correctness relies on the CAS and hash checks. `shutdown` waits only on current workers (`stream_registry.rs:405-409`), so a replaced worker's drain is never awaited and the process may exit with commands taken from the inbox but not persisted (they stay in Redis up to the 1 h TTL, no signal to the client). `close_stream` (`:254`) and `purge` (`:385`) drop worker handles the same way. `drain()` also ingests one inbox page only (`stream_worker.rs:376`).

Fix: report finished only when both tasks ended; keep draining workers in a list that `shutdown` awaits; make `drain` loop until the page is short.

### L6. Inbox cursor never detects a regression (Low)

`ingest` skips every entry with `inbox_id <= stored_inbox` (`services/command_stream/stream_relay.rs:211-220`) and `read_index` is reset only on reload (`:395-396`). The scenario is narrower than "failover with a slower clock": `XADD *` ids are monotonic within one stream key even across failover, because the last id is replicated. A regression needs a restored older dump, or a flushed or expired key combined with a clock that went backwards; new commands are then skipped silently until ids pass the stored value (and the unique `(kind, id, inbox_id)` constraint would reject any that slip through). Fix: compare the stream's newest id with the cursor and reset on regression, or alert on repeated skipped-only polls.

### L7. Inbox trim only after commands are stored (Low)

Trimming happens only in `finish`, after a successful append, through the highest stored `inbox_id` (`stream_persistence.rs:146,319-324`, `stream_log_service.rs:198-205`); a failed trim is only warned. A malformed entry waits for a later valid store (`stream_relay.rs:221-229`). The stored-position filter makes a missed trim harmless, and `push` validates commands first (`stream_registry.rs:288`), so malformed entries come only from version skew (L12) or writes made outside the service. Fix: trim on every successful pass.

### L8. `ensure_stream` writes on every worker load (Low)

`repositories/command_log/pg/stream.rs:222-230` is an upsert (`ON CONFLICT DO UPDATE SET id = cl_streams.id`): a dead tuple and a row write lock per reload, contending with append CAS. It is called unconditionally from `load_once` (`stream_log_service.rs:106`). Fix: `DO NOTHING` plus a `SELECT` first, not on read-only loads.

### L9. Load/prune race (Low, practically None at defaults)

`StreamLogService::load_once` reads head, latest snapshot and tail in separate queries (`stream_log_service.rs:105-119`). `prune` deletes commands below the `keep_snapshots`-th newest snapshot (`repositories/command_log/pg/prune.rs:9-24`), so the race needs `keep_snapshots` newer snapshots between two queries: three at the default (about 300 commands with `snapshotEvery` 100), one if `keepSnapshots` is 1. The tail then starts above the snapshot, `apply_next` fails with `Gap` (`Corrupt`) and the worker gives up; the log is not damaged and a resubscribe heals it. Fix: one snapshot-isolated transaction.

### L12. Rolling deploys and the command codec (Low)

There is no codec versioning (`decode_command` is postcard plus a canonical-form check, `crates/shine-game/src/replica/codec.rs:35-41`). An older binary meeting a command variant added by a newer one behaves differently by path: a command read back from the log fails `to_envelope` with `Corrupt` (`command_log_types.rs:29-30`) and the worker gives up repeatedly for that stream; an undecodable inbox entry is dropped as "malformed" (`stream_relay.rs:221-229`) and not trimmed, so the old instance silently ignores the new commands. Fix: version the command codec, gate new variants. Related: R18.

### L14. Chat text is raw (Low)

Stored and relayed raw; `is_well_formed` checks only `!text.trim().is_empty()` and `text.len() <= 2000` (`crates/shine-game/src/replica/chat.rs:109-115`). Clients must escape. No `innerHTML` use in a chat renderer was found in `client/web/src`.

### L15. A stale `StreamFailed` tears down a healthy successor worker (Low)

`notify_failed` queues `StreamFailed { stream_id }` with no worker identity (`services/hub/hub_gateway.rs:103`); the loop calls `close_stream(&stream_id)` unconditionally (`hub_loop.rs:216-233`), which unsubscribes every connection and removes the stream entry, dropping the worker handle (`stream_registry.rs:242-256`). Interleaving: worker W1 calls `give_up` and queues the command; W1's receiver drops, so `is_finished()` is true; a subscribe starts W2 (`stream_registry.rs:351-358`) before the loop handles the command; the loop then closes W2's stream, W2's subscribers get `Desynced` and W2 stops. No data loss (the log CAS and chain-hash checks prevent forks); extra reload and resync cycles, widened by H1, with no jitter on the client (C8). Fix: carry a worker generation in `StreamFailed` and ignore it if the current worker differs.

### L16. A subscribe can be lost at worker idle-unload (Low)

When the grace timer fires (`stream_worker.rs:326-329`) the worker breaks, calls `receiver.close()` (`:339`) and `drain()`; a `Subscribe` accepted into the channel between the select's `recv()` poll and `close()` is never read, and `subscribe()` keeps returning true while the receiver is open (`:79-81`). The registry then believes the connection is subscribed, the worker is gone and nobody sends it a `Sync`; a later subscribe starts a new worker, but that connection stays in `waiting` (C1). The window is very small (the `biased` select polls `recv()` first). Fix: after `close()`, drain pending `Subscribe` messages and re-queue them or answer them with a failure.

### L17. `Contention` makes the worker give up (Low)

After `MAX_ATTEMPTS = 8` lost head CASes (backoff 100 µs to 100 ms) `append` returns `Contention` (`repositories/command_log/pg/append.rs:22,117-193`). Only `ServiceUnavailable` is transient (`models/command_stream_error.rs:61-63`), so there is no retry, `persist_commands` returns `Failed` (`stream_persistence.rs:135`) and the worker gives up; its subscribers get `Desynced` and recover by resubscribing. It only matters with several instances writing one busy stream. Fix: treat `Contention` as reload-and-retry.

### L18. Inbox expiry can delete commands before persistence (Medium)

`push` appends to the Redis stream and refreshes the key's TTL in one transaction (`repositories/command_inbox/redis/inbox.rs:24-43`). The configured TTL is one hour (`app_state.rs:16-18`). Trimming occurs only after successful persistence (`stream_persistence.rs:146,319-324`; L7), so while commands remain unpersisted their only copy may be the expiring Redis stream. If all workers are down or unable to persist for longer than the TTL after the last push, Redis deletes the stream and its queued commands; when workers recover, there is nothing to replay. This is separate from L5's drain/shutdown window, where entries remain recoverable only until this TTL elapses.

Fix: do not expire inbox entries that have not been persisted, or ensure an independent durable recovery path. If the expiry is retained, surface and alert on commands approaching expiry while still unpersisted.

### L19. Invalid hub intervals can panic tasks or break registry connections (Low)

`HubConfig` deserializes `heartbeat` and `session_check` durations without validators (`services/hub/hub_service.rs:52-56`); nested config validation therefore does not reject zero. `run_background_task` passes both values to `tokio::time::interval` (`hub_service.rs:178-193`; `tasks/background_task.rs:141`), which panics for a zero duration. Separately, `app_state.rs:35` derives the registry TTL as `heartbeat.as_secs().saturating_mul(3)`: any heartbeat under one second truncates to zero, and `RedisHubConnectionDb::new` does not reject it (`repositories/hub_registry/redis/redis_hub_connection_db.rs:30-37`). `SET ... EX 0` on connection registration returns a Redis error, so registration fails; a heartbeat's `EXPIRE` with zero deletes an existing key, after which that connection is treated as inactive.

Fix: validate heartbeat and session-check as non-zero; require heartbeat to be at least one whole second (or derive a positive TTL with suitable rounding) before constructing the registry.

---

## Web client and replica

| ID | Sev | Finding | Where | Fix |
|---|---|---|---|---|
| C1 | Low / Medium | **No watchdog on `waiting` and `syncing`.** A lost `streamSync` strands the client; only `desynced` has a retry timer, armed only inside `onStreamFrame`, so it re-arms only when another frame arrives. | `client/web/src/experiments/pixel-map/index.ts:94-104`; `crates/shine-game/src/replica/client_stream.rs:173-176` | Re-send the subscribe with backoff for any non-live status. |
| C2 | Low | **Backoff resets on every open.** `onOpen` sets `reconnectAttempt = 0`, so a server that accepts the upgrade and then closes is retried every second forever. | `client/web/src/engine/net/websocket-stream-hub.ts:129-131,166` | Reset after the first sync or a minimum open time. |
| C3 | Low | **A `failed` nack drops the pending command**, although the server says it may have been queued. `on_nack` removes the pending command for every reason and re-subscribes only for `notSubscribed`. | `client_stream.rs:253-261` | Keep it pending and resend (the server does not dedup, so a possible repeat is accepted), or mark it unknown. |
| C4 | Low | **A persistent checkpoint mismatch loops forever.** Every mismatch calls `resubscribe()` with no counter. | `client_stream.rs:235-251` | A counter, then a move to `Desynced`. |
| C5 | Medium | **Stale state after a reconnect, and a reconnect loop between tabs.** See below. | `client/web/src/experiments/pixel-map/index.ts:45-52,96-97,163-190`; `client/web/src/engine/net/websocket-stream-hub.ts:116,159-172`; `client/web/src/engine/net/stream-hub.ts:1` | See below. |
| C6 | Low | **Overlapping `Sync` tail records a bad hash.** `apply_wire` discards `Ok(ApplyOutcome::Skipped)` and still pushes `(version, hash)` into the hash history. Only `on_sync` tails are exposed; the server sends non-overlapping tails today. | `client_stream.rs:189-193,306-309`; `crates/shine-game/src/replica/replica.rs:130-131` | Honour `ApplyOutcome::Skipped`. |
| C7 | Low | **Snapshot decode accepts trailing bytes.** `decode_snapshot` uses `postcard::from_bytes`; only commands get the canonical re-encode check. State and chain hashes come from the same frame, and the chain hash is never checked against a checkpoint. | `crates/shine-game/src/replica/codec.rs:15-17,33-39`; `client_stream.rs:281-287` | Canonical check for snapshots; verify the chain hash against a checkpoint. |
| C8 | Low | **`StreamSubscribeFailed` is retried every 5 s forever**, including permanent reasons; a `desynced` resubscribe has no jitter, so after a server-side give-up every client of an instance comes back at once; `new WebSocket` is unguarded and a constructor throw skips the backoff path. | `client_stream.rs:155-160`; `websocket-stream-hub.ts:50,168-171` | Stop on permanent reasons; jittered backoff; try/catch into the backoff path. |
| C9 | Low | **`net/` is missing from the `client-web-architecture` skill's layout table** (`client/web/src/engine/net/` holds `stream-hub.ts`, `websocket-stream-hub.ts` and tests). | `.claude/skills/client-web-architecture/SKILL.md:14-25` | Add a row. |

**C5 in detail.**
- The server tells clients which closes must not be retried: 4001 (replaced by a newer connection, "two tabs would replace each other forever") and 4002 (session invalid, sign in again); 4000, 1001 and 1008 may be retried after a back-off.
- The web hub's `onClose` takes no `CloseEvent` (`websocket-stream-hub.ts:159`), so close codes are never read and it reconnects after any close, 4001 included. Two tabs of one user therefore replace each other in a loop; each round is a connect-rate token (5 burst, then 0.1/s), a `ConnectUser` / `DisconnectUser` pair with Redis round trips in the hub loop (H1) and a full re-sync.
- `'replaced'` exists only as a type (`stream-hub.ts:1`) and in the UI text and its test (`pixel-map-ui.ts:14`); nothing ever sets it.
- After a reconnect the wasm stream is never reset to `Waiting`: the `connected()` callback only sets `connectionState = 'syncing'`, the hub reuses the listener (`entry.listener ??=`, `:116`), and `connectionState` flips to `live` on any frame while `stream.status()` is still `live` (`index.ts:96-97`), so the UI shows live before the new sync arrives. `paint()` is not gated by `connectionState` (it only fails when the stream is not live, `:179-189`); `onUpdate` gates only the dimming (`:69-70`).
- Fix: read the close code; do not reconnect on 4001 / 4002 and surface `replaced`; reset or resync the stream on reconnect; gate paints on the connection state.

### Desynced is a permanent error (client policy, not implemented)

What the server sends: when a worker gives up (`StreamRelay::give_up` → `notify_failed` → the hub's `StreamFailed`), every connection that had the stream open gets `streamDesynced` (`StreamMessage::Desynced`) and the stream is closed for it. A failed subscribe request (`streamSubscribeFailed`) and a stream that stopped serving are different events. A successful reload is not a desync: the subscribers get a fresh `streamSync` (`accept_reload` → `reset_subscribers`).

Why it is permanent: the worker gives up only after the retries of the inbox read and of the log calls are used up, or on a permanent error (`Corrupt`, `Diverged`). A client that resubscribes at once or after 5 s meets the same failure.

What the client does today: `ClientStream` has three statuses (`Waiting`, `Live`, `Desynced`; `client_stream.rs:15-23`). `streamDesynced` and any `streamSubscribeFailed` map to `Desynced` (`:155-160`), and the pixel-map experiment resubscribes after a fixed 5 s, forever (C8). There is no doubling, no jitter and no `Rejected` status anywhere in `crates/shine-game/src` or `client/web/src`.

Intended policy (decided in discussion, no code yet): treat `Desynced` as a permanent-ish error. The client waits much longer between tries, with growing delays and jitter, and eventually gives up in a terminal state that the UI shows as unavailable. A sync resets the counter.

Open:
- The delays, the cap and the number of tries before giving up.
- The terminal status and what ends it (a hub reconnect, a user action).
- Whether the logic lives in `ClientStream` (Rust, testable, the TS side only runs the timer) or in the TS client.
- Whether `streamSubscribeFailed` reasons share the policy: `rateLimited` needs a short wait, `failed` the long one, `unauthorized` none (a `Rejected` status).

---

## Informational (X1)

- **Control frames are uncharged.** Ping, Pong and Close frames skip the budget and every frame resets the 90 s idle timer (`routes/ws/connect.rs:234-235`). CPU cost only: tungstenite keeps one pending Pong, so memory is bounded.
- **Public `/info/version`** returns the app version and environment without authentication; `/info/status` and the telemetry config need `READ_TRACE` (`crates/shine-infra/src/health/health_router.rs`). Swagger and `x-powered-by` default to off and prod does not enable them.
- **Zero `streamShutdownTimeout` duration in `HubConfig`** has no validator (`services/hub/hub_service.rs:57-58`). It is config-only, not client-reachable. Zero heartbeat and session-check durations, plus a sub-second heartbeat yielding a zero registry TTL, are tracked in L19. `SocketTimeouts` and the stream configs are validated.
- **Benign leftovers.** The grace timer is the last arm of a `biased` select (`stream_worker.rs:285,326-329`): it can wait while the other arms stay ready, but its deadline is kept and it fires once the load pauses, and unloading a stream that is still being fed is not obviously wanted. Targeted sends skip the topic check that `broadcast` applies (`services/hub/hub_connections.rs:145,163,185`): inconsistent but harmless. `UserAlreadyConnected` is never constructed (`models/hub_error.rs:9,35`). `serve_socket` has no drop guard, but no code path cancels the future (`on_upgrade` spawns it).
- **Deliberate ordering.** The connection tracker polls its ticker before lifecycle events on purpose, to keep TTL refreshes from being starved (`services/hub/tasks/background_task.rs:157-181`). Events are not drained only while `on_tick` runs, and `MissedTickBehavior::Delay` spaces ticks by a full interval.
- **`ping` and `any` are config-gated.** See A6 and A1.

---

## No longer valid

Items from earlier notes that were re-checked and are fixed or were wrong.

- **No PG `statement_timeout`: fixed.** Every Postgres connection now gets `-c statement_timeout=10000` by default, plus `tcp_user_timeout` and keepalives (`crates/shine-infra/src/db/postgres/pg_connection.rs:365-415`, applied in `create_postgres_pool` at `:458`); migrations lift it (`:152-159`). `append` still sets its own `lock_timeout` and `idle_in_transaction_session_timeout`.
- **`list_connections` and its `SCAN` are gone**, and there is no `MGET` or `SCAN` left (R19 now concerns only the Lua heartbeat).
- **The registry listener no longer "silently" fails:** a failed first subscribe now ends the service (R9).
- **A tick starving lifecycle events is deliberate**, not a defect (see Informational); the proposed "drain events before ticking" would reverse the intended priority.
- **Session lookups do not share the hub's Redis pool:** they use a separate pool built from `session_redis_cns` (`crates/shine-infra/src/session/current_user_service.rs:66-71`). The shared one is the builder pool (H1).
- **The `ConnectUser` registry-failure hub case is tested** (`services/hub/tests/fault.rs:62`, `connect_is_rejected_when_the_registry_write_did_not_land`); the missing part is what the client sees (R10).

---

## Tests

| Gap | Where |
|---|---|
| Stress tests are small (12 to 48 users) and the test hub runs with heartbeat and session check at 3600 s, so scale, cascades, a command loop that ends early and a Redis flush are untested; 12 fixed sleeps and `tick * N` waits remain. | `testing/hub/test_hub.rs:33,50-51`; `services/hub/tests/contention.rs`, `fault.rs`, `lifecycle.rs`, `streams.rs` |
| Missing fault and race cases: listener failure (supervisor reporting is tested, a listener failure is not), half-open or oversized frames, id regression. | `services/hub/tests/lifecycle.rs:575-599`; `routes/ws/connect_test.rs`; `services/command_stream/stream_worker_test.rs` |
| New in this pass, none covered: H1 (Redis latency or cut through the existing toxiproxy setup, then connect and disconnect a batch and measure how long `SubscribeStream` waits), H2 (delete the session key in Redis and measure how long `streamCommand` keeps succeeding; keep a socket open past `sessionTtl` and check the session still exists), H3 (stall the hub loop in the test hub, drop the connection, send subscribe plus command from the old socket), L15, L16, L17. | `services/hub/tests/`, `testing/hub/test_hub.rs` |
| Mutation blind spots: `a_close_reason_ends_the_socket_with_it` and `the_close_reason_is_not_lost_when_the_queue_is_full` assert the `CLOSE_CODE_*` constants against the same constants production uses (the table test uses literals and is fine); no test drives `AppState` wiring; the `timeouts.send_drain` abort is untested (`connect_test.rs:558` only configures it). | `routes/ws/connect_test.rs:74-83,105-120,143-161`; `app_state.rs`; `routes/ws/connect.rs:339-341` |
| Rust, `shine-infra`: the prod TLS check is tested for rejections only (`prod_rejects_non_require_sslmode_even_with_pool_params`, `prod_check_runs_after_pool_params_are_validated`); no pool-level test that `sslmode=require` is accepted. `apply_session_timeouts` and the `statement_timeout` default have no tests in that module. Migrations under the timeout need a database and have no test. | `crates/shine-infra/src/db/postgres/pg_connection.rs:392-415,519-539` |
| Rust, builder: what the client sees when `ConnectUser` registry creation fails (R10), and `ensure_stream` (L8). | `services/command_stream/` |
| Rust, `shine-game` `client_stream`: a mid-batch gap (only a first-command gap is covered), a command that fails to decode, a checkpoint mismatch while `Waiting` (only `Live` is covered), overlapping `Sync` tails, trailing snapshot bytes, `HASH_HISTORY` eviction at 256, a `failed` nack on a known pending command, a command folded into the snapshot but absent from the tail. | `crates/shine-game/tests/replica/client_stream.rs` |
| TypeScript: only two tests exist for the hub (subscribe routing, reconnect and restore). Missing: stale live after reconnect, a hang in waiting, an immediate close after open with growing delay, no reconnect on a 4001 close, `connect()` twice, the `desynced` timer re-arming, `resync()` while closed or replaced. Nothing tests the pixel-map experiment's stream handling. | `client/web/src/engine/net/websocket-stream-hub.test.ts` |
| `shine-test` panic hook exits the whole run on a panic in a background thread (`std::process::exit(-1)`, exit code `0xffffffff` on Windows, no summary); a test that expects a spawned-task panic cannot exist. The fuzz CI job uses an unpinned nightly (`dtolnay/rust-toolchain@nightly`) and the cache covers only `~/.cargo/registry` and `~/.cargo/git`, so `cargo install cargo-fuzz` is repeated. | `crates/shine-test/src/lib.rs:60-84`; `.github/workflows/fuzz_ci.yml:26-38` |

---

## Verified as sound

No action needed.

**Connect path and sockets**
- Authentication runs first: signed cookie (HMAC, key of at least 64 bytes), fingerprint match, Redis-backed session; the session key is 128 bits from `ring` and Redis holds only its SHA-256. Then the host check, `Origin` (required, anchored regexes, empty allow-list denies everything), the per-user connect rate and the global connection cap, all before the upgrade. Allow-lists fail closed.
- Admission permits do not leak: the permit is bound for the whole socket task, released if the upgrade fails, and every exit path of `serve_socket` finishes or aborts both child tasks. Rate buckets are per authenticated user, refused attempts cost a token, idle buckets are swept every 60 s.
- Size limits: `max_message_size` and `max_frame_size` are 32 KB; the base64 text is length-checked before decoding; commands are re-checked against 16 KB, a canonical postcard re-encode check and per-kind well-formedness; ping id is capped at 255; binary frames are ignored. Per-socket token buckets close floods after 20 consecutive denials. Time going backwards in the token bucket is a no-op and the rates are validated finite and positive.
- Slow consumers: egress is bounded (1000) with `try_send`; a full queue drops the subscription; writes have a 10 s timeout, pings go every 30 s and the receive side times out after 90 s. The broadcaster never blocks. Internal consumers are unbounded by design.
- Identity: `user_id` and `connection_id` always come from the server-side session and the hub, never from the client payload; the broadcast `user_id` comes from the Redis entry written by `push`; `CommandFailed` goes only to the sender's connection.
- Hub lifecycle events are not forwarded to clients.

**Hub invariants**
- `users` and `connections` are mutated only on the loop; replacement removes the old entry before registering; a stale `DisconnectUser` is keyed by connection id and cannot remove a fresh reconnect (this also holds for heartbeat, session check and registry changes); lock scopes in `Connections` contain no awaits and only non-blocking `try_send`; no `std` lock is held across an await in the registry.
- Ordering: `ConnectUser`, `SubscribeStream` and `DisconnectUser` are FIFO on one channel; a subscribe that lands after the connection is gone is undone. A subscribe registered before an abort never ends half-registered.
- No supervised task ends on a client-triggerable event (no remote kill switch): only the hub loop, the dispatcher, the heartbeat, the session checker and the registry listener are supervised, and the listener ends only on a failed first subscribe.
- Shutdown: the `Shutdown` event is queued ahead of `subscribers.clear()`, the dispatcher is joined with a 10 s worker timeout and a 5 s registry cleanup cap, and commands queued after `Shutdown` are dropped and their sockets get `Gone`.

**Command stream**
- Identity of streams: `StreamId` accepts only an exact lowercase kind and an ASCII id of `[A-Za-z0-9_.:-]`, 1 to 64 bytes, no normalization, so no case or unicode aliases and no kind collisions; the allow-list, the Redis key and the DB row all use the same string. Prod allow-lists exactly `chat/global` and `pixelMap/pixel-demo`.
- Injection: every Redis and SQL statement is static or parameterized, there is no `format!` into SQL and no Lua taking user text; the inbox push is one atomic `MULTI` (`XADD` + `EXPIRE` + `PUBLISH`); `XRANGE` exclusive ranges, `MINID` trim (checked overflow) and numeric `InboxId` ordering are right.
- No poisoned stream: `is_well_formed` runs at push and again at ingest, `ReplayState::apply` has no error path and cannot panic, `to_envelope` recomputes `command_hash` and the chain hash is verified on every apply; a malformed entry is dropped, never persisted.
- Append: dense-run assumptions are guaranteed by the relay, index arithmetic is guarded (`repositories/command_log/pg/append.rs`), `ON CONFLICT DO NOTHING` plus a CAS on `(version, chain_hash)` serialize concurrent writers and roll back losing inserts; the chain hash binds command, user and inbox identity. Pruning keeps at least `keep_snapshots` snapshots, snapshot bytes come only from the server replica, and `page_size`, `snapshot_every` and `keep_snapshots` are validated as at least 1.
- Exactly-once across a crash: inbox entries are trimmed only after a successful append and replay, and a reload resets `read_index` to the last stored inbox id, so entries sequenced optimistically but not persisted are re-read, not lost. Overlapping workers are serialized by the log CAS and the chain-hash checks, so there are no forked or duplicated versions.
- Worker select arms, `SocketReceiver::next`, `GraceTimer::wait` and the background loops are cancel-safe.

**Configuration and deployment**
- `fullProblemResponse` is `false`, Swagger and `x-powered-by` are off, `Problem` values carry sensitive data only via `with_sensitive_str`, and client payloads appear only at `trace`.
- There is no `unsafe` in `services/builder`, `shine-infra` or `shine-core`.
- The container runs as the unprivileged `nginx` user and TLS terminates at the platform; the deploy job runs only on `main`; port 7001 is not exposed from the container.

---

## Limitations

- Findings come from manual reading; scanner silence would prove nothing and none was run. Several items were traced by a read-only reviewer and then spot-checked, not independently re-derived.
- Production Key Vault values (Redis and Postgres pool sizes and timeouts, TLS and network ACLs, whether Redis is shared with the session store) are unknown, so the real figures for H1, E1 and the stall windows are unknown.
- Cookie flags (`HttpOnly`, `Secure`, `SameSite`) are set by the identity service and were not reviewed; the conclusion that cross-site WebSocket hijacking is closed rests on the `Origin` check and on those flags.
- Not read: the pixel-map `apply` / `is_well_formed`, `pg_listener.rs` and `pg_connection.rs` beyond a skim, `event_bus` / `topic_bus`, metrics label cardinality, `command_log/pg/consistency.rs`, `security_headers.rs`, `validated.rs`, the `testing/` helpers, and the identity service. Dependency advisories were not assessed.
- Redis pool checkout timeout in production (affects how long a `push` can park with Redis down) remains unverified.

A human owns any release decision on this.
