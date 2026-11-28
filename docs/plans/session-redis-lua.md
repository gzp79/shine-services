# Sessions in Redis as Lua scripts

Non-SDP investigation notes. Not kept in sync with the code.

**Goal**: one atomic session store per user, and a mass check of many sessions in one round trip. The sentinel key is expected to become unnecessary.

## Today

Two keys per session, `[prefix]session:<user>:<key_hash>:sentinel` and `:data` (`services/identity/src/repositories/session/redis/redis_sessions.rs`), written by identity and read by every service through `CurrentUserService` (`crates/shine-infra/src/session/current_user_service.rs`). The key format is duplicated in both places and kept in sync by hand.

- The sentinel is immutable (`created_at`, fingerprint, agent, geo) and owns the lifetime: created with `SET NX EX`, extended on every access. The data key (name, roles, flags) can be rewritten without extending the lifetime. A session is alive only if both keys exist.
- Listing or deleting every session of a user is a keyspace `SCAN` with a pattern (`find_redis_keys`).
- A check is 3 round trips (`GET` sentinel, `GET` data, `EXPIRE` both); the hub's `SessionChecker` does this once per connection per tick (R7 in [builder-hub-streams-audit.md](builder-hub-streams-audit.md)).
- Multi-step flows are not atomic: `update_session_user_by_hash` checks `EXISTS` and then writes, so a concurrent logout can leave data behind; the sentinel only keeps that data from being extended.

## Proposal

One Redis hash per user, `session:<user>`, one field per session (`<key_hash>`) holding one JSON value: the sentinel fields, the data fields and `expires_at`. All operations are Lua scripts, so each is atomic.

| Script | Replaces | Notes |
|---|---|---|
| `create(user, hash, record, ttl, max_sessions)` | `SET NX` sentinel + `SET` data | Fails on an existing field (key conflict). Purges expired sessions, then evicts the least recently used ones until there is room (see Session limit). |
| `validate_many(users[], hashes[], extend, ttl)` returns flags | `get_current_user` per connection | The mass check: expired ones are purged and reported as `0`, live ones are reported as `1` and their lifetime is extended only when `extend` is set. Chunked (about 256) per call. |
| `get(user, hash)` | `find_session_by_hash` | Returns the record, or nothing when expired. |
| `update_user(user, hash, data)` | `update_session_user_by_hash` | Keeps `created_at` and `expires_at`, does nothing for a missing or expired session (no resurrection). |
| `delete(user, hash)`, `delete_all(user)` | `DEL` of two keys, `SCAN` + `DEL` | `delete_all` is one `DEL` of the user's hash. |
| `list(user)` | `SCAN` + per-key reads | One `HGETALL` filtered by `expires_at`. |

Design points:

- **Redis version**: the minimum is fixed by `MIN_REDIS_VERSION` (6.2) in `crates/shine-infra/src/db/redis/redis_check.rs` and is not raised for this. The scripts use only what 6.2 has (`EVAL`, hashes, `TIME`); no `HEXPIRE` (7.4), no Redis functions (7.0).
- **Expiry**: a hash field has no TTL in 6.2. Store `expires_at` in the record and let every script filter and purge on access, and give the user's hash a key TTL of the latest `expires_at`.
- **Clock**: use `redis.call('TIME')` in the script, not the caller's clock, so instances cannot disagree.
- **No sentinel**: the single record carries `created_at` and `expires_at`, and one script call is atomic, so the "data outlives the sentinel" case cannot happen. A maximum session lifetime (the `todo` in `get_current_user`) becomes one comparison with `created_at`.
- **Cluster**: written to be cluster-compatible, although only a single primary runs today. The user's hash is `[prefix]session:{<shard>}:<user>`, where `<shard>` is derived from the user id (the first byte of the uuid modulo 64) and only the `{<shard>}` part is hashed, so all sessions of one user share a slot, and so do all users of a shard. Every script gets all its keys through `KEYS` and touches nothing else. The single-user scripts take one key. `validate_many` takes all the keys of one shard, so the caller groups its connections by shard and sends one call per shard (at most 64 calls per tick, each with many connections, not one per user). A fixed number of shards keeps the slots few, so batching still works, and spreads the users over the nodes. The shard count is a constant of the format: changing it needs a purge. On a single primary the braces change nothing and the grouping is only a few extra calls.
  Switching the connection from the standalone pool to a cluster client is a separate step and not part of this plan; the scripts and the key layout do not need to change for it. The other builder Redis keys (`MGET`, `SCAN` in the hub registry, R19) are not covered.
- **Session limit**: a user holds at most `max_sessions` live sessions (a config value of identity, for example 10; a bound, not a quota to tune). `create` purges the expired ones, and while the count is at the limit it removes the session with the lowest `expires_at`, the least recently used, since every access extends it. Evicting by `created_at` would log out the device in use whenever the user signs in somewhere new; refusing the new login could lock the user out. The eviction is in the same script as the insert, so concurrent logins cannot exceed the limit. An evicted session is gone like a logged-out one: its HTTP calls get `SessionExpired` and the hub's checker closes its socket as `SessionInvalid`. Log the eviction at `info` (user id and count only). The limit also bounds the size of `session:<user>` and the cost of `list`, `validate_many` and the `cjson` decoding in `create`.
- **One owner of the format**: the scripts and the record type live in `shine-infra::session`; identity's `RedisSessionDb` and `CurrentUserService` both call them. This removes the duplicated key format.

## Migration

No compatibility layer for now: deploy all services that share the session store together (identity and builder) and purge the old session keys from Redis (`session:*:*:sentinel` and `:data`). Every user logs in again once. The old format is not read, and no fallback code is written.

This holds while the services deploy together. Once they roll out separately (see L12/R18 in the audit), a new format needs a new key name and a read-both period; do that then, not now.

## Tests

Against real Redis (the shared test infrastructure), in `shine-infra`:

- Each script: happy path and every refusal (conflict, expired, missing).
- Session limit: the session over the limit evicts the least recently used one (not the oldest created); an accessed old session survives; expired sessions are purged before anything is evicted; concurrent `create` calls never leave more than `max_sessions`.
- Layout: all keys of one user, and all keys of one shard, hash to the same slot (the CRC16 slot of the `{<shard>}` tag); `validate_many` refuses or never receives keys of two shards.
- Races: `update_user` against `delete`, and `validate_many` against `delete_all`, never leave a live session behind.
- `validate_many`: all live, mixed, all gone; with `extend` the TTL grows only for live ones, without it no TTL changes.
- Builder: `SessionChecker` uses `validate_many` (one call per chunk) and the lifecycle test `session_check_drops_only_connections_with_an_invalid_session` stays the end-to-end check.
- Playwright: unchanged, identity login, logout and session list already cover the contract.

## Open questions

- Which value the checker passes for `extend`. `true` keeps today's behaviour (a WebSocket-only user has no other keepalive); `false` makes the checker read-only, and a WebSocket-only session then ends after one session TTL. A product decision, the script supports both.
- The value of `max_sessions`, and whether guests (one session each in practice) need a lower one.

## Background and assumptions

What the plan relies on, collected so the work can be picked up later without the original conversation. **Verified in the repo** items were read in the code; **from Redis knowledge** items were not checked against the Redis documentation or a running 6.2 and must be before building. Redis docs: <https://redis.io/docs/latest/commands/>, <https://redis.io/docs/latest/develop/interact/programmability/eval-intro/>, <https://redis.io/docs/latest/operate/oss_and_stack/reference/cluster-spec/>.

### In the repo (verified)

- Minimum Redis is `MIN_REDIS_VERSION = (6, 2)` in `crates/shine-infra/src/db/redis/redis_check.rs`; it is checked at start-up and its unit tests assert 6.0 and 5.0 are rejected. Local docker uses `redis:latest` (`services/docker-compose.yml`), so a newer-only command passes locally and fails against a 6.2 production: pin a 6.2 image for the script tests.
- Client: `redis = "1.0"` (features `tokio-comp`, `tokio-rustls-comp`, `json` in shine-infra) behind a `bb8` pool, `RedisConnectionPool` in `crates/shine-infra/src/db/redis/redis_connection.rs`. There is no cluster client in use; the `redis` crate has a `cluster` / `cluster-async` feature and a `Script` type (EVALSHA with an EVAL fallback on `NOSCRIPT`): look both up on docs.rs (`https://docs.rs/redis/latest/redis/`) before using them.
- Current format: `[prefix]session:<user simple uuid>:<key_hash>:sentinel` and `:data`, JSON values (`RedisJsonValue`, camelCase). Sentinel: `createdAt`, `fingerprint`, `agent`, `country`, `region`, `city`. Data: `name`, `isEmailConfirmed`, `isLinked`, `roles`. Writer: `services/identity/src/repositories/session/redis/redis_sessions.rs`; reader: `crates/shine-infra/src/session/current_user_service.rs` ("introduce any breaking change with great care", the format is shared). The `key_prefix` is configurable and must stay outside the `{<shard>}` tag.
- Session TTL: `session_ttl` (1800 s per the audit), extended on every `get_current_user` and on `update_session_user_by_hash`. The hub's `SessionChecker` calls `get_current_user` per connection every `session_check` (900 s in prod) and requests a drop with `SessionInvalid` on `SessionExpired` or `SessionCompromised`; `SessionCompromised` is never produced today. Other errors skip the connection for the tick.
- Callers that use the key layout besides identity's `RedisSessionDb`: the session tests (`services/identity/src/services/session_service_test.rs`), `SessionMint` for Playwright (`tests/src/mocks/session_mint.ts`, writes sessions straight into Redis and signs the cookie with the identity HMAC key) and the builder test helper `services/builder/src/testing/hub/test_session.rs`. All of them must move to the new format together.
- `find_redis_keys` is a `SCAN` with a pattern per call, used by list and delete-all; it is the part the per-user hash removes.

### From Redis knowledge (to verify)

- **Atomicity**: a script runs to completion without other commands interleaving; it also blocks the server meanwhile, so a script must stay short: bounded work per call (`max_sessions`, chunks of about 256 sessions in `validate_many`).
- **Scripts in 6.2**: `EVAL` / `EVALSHA` / `SCRIPT LOAD`, `cjson` (decode/encode) and `redis.call('TIME')` are available. Writing after `TIME` is allowed since 5.0 (scripts replicate by effects). Redis functions (`FUNCTION LOAD`) are 7.0, hash field expiry (`HEXPIRE`, `HGETEX`) is 7.4: not usable. Do not rely on the Lua `os` library; time comes from `TIME`.
- **JSON in Lua**: `cjson.decode` turns an empty array into an empty table that encodes back as `{}`; keep `roles` handling in mind if a script ever re-encodes a record (prefer storing the record as an opaque string and keeping `expires_at` and `created_at` in separate hash fields or a small header if scripts need them without decoding).
- **Hash TTL**: only the whole key has a TTL in 6.2. The user's hash needs `EXPIRE` set to the latest `expires_at` on every write or extend, otherwise a hash of expired sessions lives forever. An empty hash disappears by itself.
- **Cluster rules**: a script (and any multi-key command or `MULTI`) must have all its keys in one slot, passed through `KEYS`; accessing a key not declared there works on a single node but breaks in a cluster. The slot is CRC16 of the key modulo 16384, and when the key has a `{...}` with at least one character inside, only that part is hashed (the first `{` and the first `}` after it). Pipelines across slots are fine for a cluster client, which splits them per node. `SCAN` and `KEYS` cover only the node they run on.
- **Redis cluster and Pub/Sub**: not used by sessions; the hub's own channels are outside this plan.
- **Eviction (LRU by `expires_at`)** needs the `expires_at` of every record in the user's hash: decode each (at most `max_sessions`) in `create`; `validate_many` and `get` read only the records they are asked for.

### Decisions already taken

- Migration is a Redis purge plus a shared deploy of identity and builder; no compatibility code.
- Redis minimum version stays 6.2.
- `validate_many` has an `extend` flag; which value the checker passes is open.
- Cluster-compatible layout from the start, cluster client later.
- `max_sessions` per user with least-recently-used eviction inside `create`.
