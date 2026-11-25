# Builder test restructure: lib + testing crate + per-domain integration tests

Status: plan only, no code changed. Written after an investigation of `services/builder`.

## Problem

`shine-builder` is a bin-only crate (`main.rs` declares all modules privately). Tests therefore have to live
inside the crate, which causes:

- 19 `#[cfg(test)] #[path = "x_test.rs"] mod ...` hooks in production files.
- `src/services/hub/tests/` is a `#![cfg(test)]` module tree inside `src/`.
- `src/testing/` (~1000 lines) is compiled only under `cfg(test)`.
- Test-only re-exports (`StreamConfig`, `StreamRegistry`, `migrate_postgres`, ...).
- 28 warnings in a plain `cargo check`. About 14 are helpers only tests call (`check_consistency`,
  `last_commands`, `oldest_snapshot_version`, `ConsistencyReport`, `ConsistencyIssue`, `find_redis_keys`,
  `unlisten`, `worker_count`, `list_connections`, `spawn_connection_loop_with_wake`, `test_default`, ...).
  Six more are unused re-exports that only tests import.

## Target layout

Mirrors `shine-infra` (`src/`, `tests/`, `testing/` crate `shine-infra-testing`).

```
services/builder/
├── Cargo.toml                 # adds [lib]; dev-dep on shine-builder-testing
├── src/
│   ├── lib.rs                 # pub mod app_config, app_state, models, repositories, routes, services
│   └── main.rs                # thin: Application + main()
├── testing/                   # new crate: shine-builder-testing (depends on shine-builder)
│   ├── Cargo.toml
│   └── src/                   # TestHub, TestInbox, TestLog, TestEgress, chat_seeding, TestStreamId, ...
└── tests/                     # one binary per domain
    ├── hub.rs                 # mod lifecycle; mod contention; mod streams; mod fault; mod stream_fault;
    ├── hub/…
    ├── command_stream.rs
    ├── command_stream/…       # worker, relay, registry, config, access
    ├── repositories.rs
    └── repositories/…         # pg_command_log, redis_command_inbox, redis_hub_connections
```

Only top-level `tests/*.rs` files are separate binaries, so 3 link steps instead of ~15.

## What moves, what stays

| Group | ~Lines | Verdict |
|---|---|---|
| `repositories/**/*_test.rs` | 1360 | Move to `tests/repositories/` |
| `services/command_stream/*_test.rs` | 2300 | Move to `tests/command_stream/` after re-exports |
| `services/hub/tests/*` | 2170 | Move to `tests/hub/` |
| `src/testing/` | 1000 | Becomes `shine-builder-testing` crate |
| `hub_subscribers`, `connection_tracker`, `session_checker`, `message` tests | small | Stay as unit tests (private helpers). Need only `TestMessage` (22 lines) |
| `connect_test`, `stream_log_service_test`, `stream_persistence_test` | ~850 | Undecided, see open questions |

Roughly 85% of test lines move. Not 99%: only 7 `_test.rs` files use `use super::*`, and 3 of them also use
`TestHub`/`TestInbox`/`TestLog`.

## Gotchas

1. **Two copies of the crate.** The testing crate depends on `shine-builder`. A unit test inside
   `shine-builder` that uses testing-crate types sees two different copies of every type and the types do not
   match. This is why `shine-infra` puts everything that uses its testing crate under `tests/`. Consequence:
   any test that needs both private items and `TestHub` cannot stay a unit test. Either make the needed items
   `pub` and move the test, or keep a small `cfg(test)` fixture in the lib.
2. **Dead-code detection disappears.** In a lib, unused `pub` items do not warn. Warnings vanish without the
   code being cleaned up. Real fix = move test-only helpers into the testing crate. That works for helpers
   that only need the public surface (`find_redis_keys`). It does not work for `check_consistency`, which
   needs private prepared statements.
3. **Shared Redis/Postgres across binaries.** `serial = "hub-service"` only serializes within one process.
   Plain `cargo test` runs binaries sequentially, so it is safe. `nextest` runs binaries in parallel and could
   collide. Check whether CI / the `local-development` flow uses nextest. If so, add a nextest test group that
   limits these binaries to one at a time. Most tests already use random UUIDs and their own chat rooms.
4. **More public surface.** Many items become `pub`; some `pub(super)` items may need adjusting.

## Steps

1. **Lib split, no behavior change.** Add `lib.rs`, thin out `main.rs`, make modules `pub`, fix re-exports.
   Keep existing in-crate tests working. Verify `cargo test -p shine-builder`.
2. **Testing crate.** Create `services/builder/testing`, add to workspace, move `src/testing/**` into it
   (drop `cfg(test)`). Keep `TestMessage` available to the remaining unit tests (small `cfg(test)` module in
   the lib, or the dev-dependency if the type-duplication issue does not apply).
3. **Move tests per domain**, one domain at a time, running tests after each:
   repositories, then command_stream, then hub. Delete the `#[path]` hooks and `hub/tests/` as they empty.
4. **Handle test-only helpers.** For each warning-carrying helper, decide: keep as legitimate public API,
   move to the testing crate, or delete. Remove the unused re-exports.
5. **Docs/skills.** Update `api-test-writing` ("Rust tests: crate/service `*_test.rs`, `tests/` dirs") and
   `local-development` if test commands change (e.g. `--test hub`).

## Open questions

1. Separate `shine-builder-testing` crate (recommended; matches `shine-infra-testing`, no feature flag on the
   production lib) or `pub mod testing` in the lib behind a `testing` feature (no new crate, needs a
   self-dev-dependency to enable the feature)?
2. Test-only methods (`check_consistency`, `ConsistencyReport`, ...): keep as public repository API (has some
   ops value) or confine to the testing crate (needs accessors or duplicated queries)?
3. `connect_test` and the two stream-service tests: which private items do they actually touch? Determines
   whether they move or stay (not yet checked).
4. Does CI use nextest? (not yet checked)
