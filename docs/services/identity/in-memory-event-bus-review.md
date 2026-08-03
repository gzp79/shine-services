# In-Memory Event Handling Review (`EventBus` / `TopicBus`)

**Scope:** Identity service's in-memory event handling used to communicate user-data changes to other domains.
**Files:** `crates/shine-infra/src/sync/event_bus.rs`, `crates/shine-infra/src/sync/topic_bus.rs`, and usage in `services/identity/src/main.rs`, `services/identity/src/services/{user_service,role_service}.rs`.
**Not in scope:** `pg_event_db` (durable PG event source) — this review is only the in-process bus.
**Level:** medium. Treated as new code (no git diff).

---

## Summary

`EventBus`/`TopicBus` is a purely **in-process** broadcaster: an `Arc<Inner>` wrapping a `HashMap` of boxed handlers; `publish` awaits `join_all` over every handler under a read lock. Unlike `pg_listener`/`redis_listener`, **nothing ever leaves the process** — there is no cross-node transport.

### Is it safe for multi-instance services?

**The design is intentional, not accidental.** One instance owns a mutation and drives it to completion: PG write → `refresh_user_session` → `session_service.update_all/remove_all`. The in-mem bus is just a shortcut for the intra-process domain call (identity → session). The cross-instance *effect* is real and by design because the sessions live in the **shared Redis** store — so a mutation served by instance A refreshes everyone's sessions.

Two things are worth stating as invariants this design rests on:

1. Publisher and subscriber are co-located (subscribe happens at startup in `main.rs`) — the bus is in-process only and never crosses nodes.
2. The refresh writes to shared Redis, which is what makes it visible to all instances.

Consequence: any identity mutation path that bypasses these services (admin/CLI direct DB write, a background migration, a future second writer service) publishes nothing and refreshes no sessions. This is a constraint to document, not a defect in the current flow.

### Can another instance intervene? (concurrency)

This is the substantive question, and the answer is: **a single mutation is safe; concurrent mutations of the same user are exposed.**

- **Login-during-update race — handled.** `get_user_info` (`user_session_handler.rs:55-72`) reads identity/version *first*, then roles, so the role set is at least as fresh as the identity. A session created concurrently with a refresh won't be stamped with something older. The comment at lines 56-57 / 109-112 documents this intent.

- **Concurrent-mutation race — NOT handled.** There is **no version stored in the Redis session and no compare-and-set on write.** `Identity` (`identity.rs:15`) has no version field; `RedisSessionUser` has none; `update_session_user_by_hash` (`redis_sessions.rs:291-326`) is a **blind, unconditional last-writer-wins write**. If instance A (add role X) and instance B (remove role Y) refresh the same user concurrently, the session ends up reflecting whichever `update_all` *finished last on Redis* — not necessarily the DB's final state. The session can be left stamped with a strictly older role set than PG holds.

The read-ordering trick only orders reads *within* one refresh; it does not order two blind writes against each other, and without a stored version the stale write cannot be rejected. It self-heals only when a later mutation (or a version-triggered refresh) re-runs — the mitigation the comment alludes to, but that relies on a version guard that is not enforced at the session write.

**Bottom line:** safe for a single mutation and for the login race; concurrent mutations of the same user can leave a session reflecting an older DB state until the next refresh. Distinct from the acknowledged crash-before-delivery gap. A monotonic version on the session (reject writes older than the stored version) would close it.

---

## Findings (beyond the known crash-before-delivery gap)

### 1. DB connection held across `publish()` — pool exhaustion / deadlock (most serious)
**`services/identity/src/services/role_service.rs:26`, `services/identity/src/services/user_service.rs:78,91`**

`add_role`/`delete_role` and `update`/`delete` keep the pooled PG `ctx` alive through `self.events.publish(...).await`. The handler then opens **3+ more contexts** (`get_user_info` → `find_by_id`, `is_linked`, `get_roles`) plus `update_all`. With N concurrent mutations and a pool of size N, every request holds one connection while waiting for another → deadlock.

`create()`/`create_linked_user()` explicitly `drop(ctx)` **before** publish — the drop-first pattern was intended, but the update/delete/role/link paths don't follow it.

**Fix:** drop `ctx` before `publish`, matching the `create` pattern.

### 2. Session-refresh failure swallowed — stale elevated privileges
**`services/identity/src/main.rs:40`**

A failed session refresh is only `log::error!`'d; the mutation still returns `Ok`. A role/privilege revocation whose refresh fails (e.g. transient Redis outage) leaves **stale elevated privileges** in existing sessions until natural expiry — no retry, no error surfaced to the caller.

### 3. Handler panic not isolated
**`crates/shine-infra/src/sync/event_bus.rs:70`**

`join_all` propagates a handler panic up through `publish().await` into the mutation request — a committed DB write becomes a 500 and the event effect is lost. Handlers guard the `Result` path but not panics (no `catch_unwind`/`spawn` isolation).

### 4. Read lock held across all handler futures — re-entrancy deadlock
**`crates/shine-infra/src/sync/event_bus.rs:68`, `crates/shine-infra/src/sync/topic_bus.rs:174`**

`publish` holds a read lock over all handler futures. Any future handler that (re)subscribes/unsubscribes during `handle()` deadlocks (tokio `RwLock` is write-preferring, non-reentrant). Latent — safe only because current handlers never touch the bus.

### 5. Latency coupling — synchronous publish on the request path
**`services/identity/src/services/user_service.rs:54`**

`publish` is awaited synchronously on the request path, so every mutation blocks on `get_user_info` (3 queries) + `update_all` (a Redis write per session). Slow Redis = slow mutations. A spawned task would decouple it (at the cost of the acknowledged crash-before-delivery gap).

### 6. Wasted work for new users
**`services/identity/src/services/user_service.rs:198`**

Brand-new users have no sessions, yet `UserEvent::Created` → full `get_user_info` + `update_all` runs anyway on every signup; `create_linked_user` fires `Created` then `Linked`, doing the full refresh twice for one user.

### 7. Concurrent mutations — blind last-writer-wins on the session (no version guard)
**`services/identity/src/repositories/session/redis/redis_sessions.rs:291-326`, `services/identity/src/models/identity.rs:15`**

`update_session_user_by_hash` writes the session user data unconditionally — there is no version stored in the Redis session and no compare-and-set. Two concurrent mutations of the same user both refresh; the session reflects whichever `update_all` finished last on Redis, not the DB's final state, and can be left with a strictly older role set than PG holds.

**This is not multi-instance-specific.** Each `publish` call runs its own `join_all` over the handlers, so two concurrent HTTP requests *on the same node* (e.g. `add_role B` and `delete_role admin`) spawn two independent `refresh_user_session` → two `update_all` loops that interleave their per-session Redis writes with no ordering. Multi-instance is just the same race without even shared in-process scheduling. So the exposure exists in a single-instance deployment too.

The read-ordering in `get_user_info` only protects reads *within one* refresh; it does not order two blind writes against each other, and without a stored version the stale write cannot be rejected. Self-heals only on the next mutation / version-triggered refresh. **Fix:** store a monotonic version on the session and reject writes older than the stored version.

### 8. Login in flight during a revocation keeps stale elevated privileges
**`services/identity/src/handlers/user_session_handler.rs:56-57,109-112` (comment) vs `create_user_session:74-104` + `session_service.rs:74-88`**

The comment claims a session created during a refresh "will contain the information not older than the user info just requested." That is **false when the login read its roles *before* the mutation committed.** Timeline: (T0) login reads `roles=[user, admin]`; (T1) admin `delete_role admin` commits and `update_all` runs — but `find_all_session_hashes_by_user` lists only existing `:data` keys, and the in-flight session's data key doesn't exist yet, so it is skipped; (T2) login stores the session with `roles=[user, admin]`. Result: a **revoked privilege survives** in a freshly minted session until natural expiry. `store_session` writes sentinel-then-data non-atomically, widening the window. The read-ordering trick cannot fix this — the stale read predates the commit. Needs the same version guard as #7 (login must reject/re-read if its snapshot predates the user's current version).

### 9. `add_external_link` / `delete_extern_link` also hold the DB context across publish
**`services/identity/src/services/link_service.rs:39-42,52-56`**

Same hazard as finding 1: `ctx` from `create_context()` is alive through `self.events.publish(&UserLinkEvent::...).await`, and the handler reopens multiple contexts (`get_user_info` + `update_all`). These paths were missed in finding 1's list; the drop-before-publish fix applies here too. (Note: `create_linked_user` and `create` correctly `drop(ctx)` first — the inconsistency is the tell.)

### 10. Delete/login race can leave an orphan session for a deleted user
**`services/identity/src/services/user_service.rs:90-96` + `user_session_handler.rs:118-121`**

`delete` cascades the DB delete, then publishes `Deleted` → handler `get_user_info` returns `None` → `remove_all`. A login already in flight (identity read before the delete committed) can call `store_session` *after* `remove_all` has run, leaving a live session for a user that no longer exists in PG. No later event ever targets that user again, so nothing cleans it before TTL. Same root cause as #8 (no version/existence fence at session creation).

### 11. `update_all` is non-atomic and partial failures are swallowed
**`services/identity/src/services/session_service.rs:77-88` + `main.rs:40`**

`update_all` loops per session with a separate `await` each and no transaction. If a Redis write fails mid-loop, some sessions are refreshed and some are not, `update_all` returns `Err`, `refresh_user_session` propagates it, and the handler only `log::error!`s (finding 2). Net effect on a revocation: a subset of the user's sessions keep the old (elevated) roles with no retry and no signal to the caller.

---

## Suggested priority

1. Findings 1 + 9 (connection-held-across-publish deadlock) — drop `ctx` before publish on **all** mutating service paths (user update/delete, role add/delete, link add/delete).
2. Finding 3 (panic isolation).
3. Findings 7 + 8 + 10 (session version/existence fence) — the core correctness cluster: no stored version means concurrent mutations, in-flight logins during revocation, and delete-during-login all leave stale/orphan sessions. A monotonic version stamped on identity and checked at every session write/create is the single fix that closes all three. **Present in single-instance deployments too**, not just multi-instance.
4. Finding 2 + 11 (surface/handle refresh failures; `update_all` is non-atomic and partial failures are silently logged).
5. Findings 4–6 (latent re-entrancy / efficiency).

## Correctness cluster summary

Findings 7, 8, 10, and 11 share one root cause: **the session store has no version/generation fence.** Writes (`update_session_user_by_hash`), session creation (`store_session`), and the fan-out loop (`update_all`) all operate on last-writer-wins semantics with no way to reject a write built from a stale identity snapshot. The event bus faithfully delivers the *notification*; the data race lives entirely in the un-versioned session writes downstream of it. Adding a monotonic identity version — stamped into the session at create and compared on every update, rejecting older — is the one change that makes the whole scheme safe under concurrency and across instances.
