# Rust `use` cleanup: no `super` outside tests, one `crate` block per file

Status: plan only, no code changed. Found while restructuring the builder tests; the builder itself is clean.

## Rules

- A non-test file does not import with `use super::...`. It uses the absolute `crate::` path. `use super::*` and
  `use super::...` stay allowed in test files (`*_test.rs`, `#[cfg(test)] mod tests`, `tests/`), where a child module
  needs its parent's private items.
- One `use crate::{...}` block per file, not a `use crate::a::b;` followed by `use crate::{...}`.
  Exception: an import under `#[cfg(test)]` is its own statement, because an attribute cannot go on a leaf of a use
  tree.

## Files to change

Top-level `use super::...` in non-test files (25):

| Crate | Files |
|---|---|
| `shine-game` | `heap/counting.rs`, `heap/static_heap.rs`, `math/hex/axial_base.rs`, `math/prng/xorshift.rs` |
| `shine-infra` | `crypto/{harsh_id_encoder,optimus_id_encoder,prefixed_id_encoder}.rs`, `db/postgres/{pg_connection,pg_error,pg_listener}.rs`, `sync/{event_bus,topic_bus,wrapper}.rs`, `telemetry/otel_layer.rs`, `web/web_config.rs` |
| `identity` | `handlers/{external_login_handler,guest_login_handler}.rs`, `models/token_info.rs`, `repositories/identity/pg/{pg_identities,pg_identity_db,pg_search_identities}.rs`, `routes/auth/api/router.rs`, `routes/auth/auth_router.rs`, `routes/auth/pages/router.rs`, `routes/identity/api/router.rs` |

Split `use crate` statements (check each; a `#[cfg(...)]`-gated one is the exception above):
`shine-game/src/math/triangulation/builder/state.rs` (2), `shine-infra/src/telemetry/telemetry_service.rs` (3).

Paths are relative to each crate's `src/`.

## How

- Replace `super::x` with the crate-absolute path of the same item (`crate::<module path>::x`); merge it into the
  file's existing `use crate::{...}` block.
- Indented `use super::*` inside `#[cfg(test)] mod tests { ... }` is fine and is not part of this.
- Do it per crate, then check both builds: `cargo clippy -p <crate>` (no test cfg) and
  `cargo clippy -p <crate> --all-targets`; run the crate's tests.

## Find them

```sh
# top-level `use super` outside test files
grep -rn "^use super::" --include=*.rs services crates | grep -v "_test\.rs\|/tests/\|/testing/"
# files with more than one top-level `use crate`
grep -rc "^use crate" --include=*.rs services crates | awk -F: '$2>1'
```

## Open

- Enforce it so it does not come back? `clippy::absolute_paths` and `clippy::wildcard_imports` do not cover this;
  a grep step in CI would.
