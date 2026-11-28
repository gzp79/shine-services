---
name: rust-test-writing
description: >
  How to lay out and write Rust tests in shine-services: where a test file goes (bin crate vs lib crate),
  `*_test.rs` hooks, fixtures in `src/testing` or a `*-testing` crate, test-only helpers and accessors
  (`impl` blocks in `_test.rs`, `#[cfg(test)]`), import style, and clippy with and without the test cfg.
  Use whenever adding or modifying a `*_test.rs`, a `tests/` directory, `src/testing/`, a testing crate, or a
  test-only function in services/ or crates/. For deciding whether a scenario belongs in Rust or in Playwright,
  see `api-test-writing`.
---

# Rust Test Writing

Running the tests (needs Redis/Postgres): `local-development` skill. Rust or Playwright for a scenario:
`api-test-writing` ("Playwright or Rust"). This skill is the layout and the conventions of the Rust side.

## Where the tests live

The layout follows the crate kind, not the size of the test. A test is "unit" when it is compiled into the crate,
however heavy.

- **Lib crate** (`shine-infra`): integration tests in the crate's `tests/`, one binary per domain
  (`tests/<domain>/main.rs`, a file per area). Shared fixtures go to the sibling testing crate
  (`shine-infra-testing`), which depends on the crate. Everything that uses the testing crate lives under `tests/`:
  a unit test inside the crate would see two copies of the crate's types.
- **Bin crate** (`services/builder`, `services/identity`): `tests/` cannot import a binary, so every Rust test is
  embedded in `src/`, real Postgres/Redis included. A testing crate is not possible (it would have to depend on the
  binary). Do not split a lib out of a service only to get `tests/`, and do not widen visibility to reach it.

## Embedded tests of a bin crate

- Tests of `foo.rs` are in `foo_test.rs`, hooked at the end of `foo.rs` with
  `#[cfg(test)] #[path = "foo_test.rs"] mod foo_test;` (`#[path]` because `mod foo_test;` in a non-`mod.rs` file
  looks in `foo/`). The hook is the only test code in a production file.
- A suite of several files that tests a whole module (`services/hub/tests/`: lifecycle, contention, faults) is a
  `tests/` directory under the module, declared once with `#[cfg(test)] mod tests;`.
- **Fixtures** used by tests of different modules live in the crate-level `#[cfg(test)] mod testing;`
  (`src/testing/`, a subfolder per domain). A fixture uses the public surface of the code plus the accessors below,
  never more. A helper used by the tests of one file stays in that `_test.rs`.
- **Private state access**: `foo_test.rs` is a child module of `foo`, so it reads `foo`'s private items. When a test
  or a fixture needs private state or a private step, add it as `impl Foo { pub fn ... }` in `foo_test.rs`, not as a
  free function, and do not widen the field or method (`pub(crate)` and `pub(super)` mean unwanted dependencies).
  It is plain `pub`: the module is `cfg(test)`, so production code cannot depend on it, and `crate::testing` can
  call it.
- Name such methods as production code would be named. They exist only in tests because production does not need
  them, or because they expose internals that could break the type's consistency; never use a throwaway name. They
  must not clash with the production methods.
- A helper that works through the public API only is a plain function or a fixture in `crate::testing`, not an
  `impl` or an extension trait.
- A method of a trait impl cannot live in a separate `impl` block: a test-only trait method gets `#[cfg(test)]` on
  the trait method and on its impls. Re-exports only tests use get `#[cfg(test)]` too; unused ones are deleted.

## Warnings and imports

- Both builds compile without warnings: no `allow(dead_code)` to hide an unused item. Test-only code is moved to a
  `_test.rs` or gated; dead code is deleted.
- One `use crate::{...}` block per file. A `#[cfg(test)]` import is its own statement (an attribute cannot go on a
  leaf of a use tree).
- `use super::...` only in test files (`*_test.rs`, `#[cfg(test)] mod tests`, `tests/`). Non-test code uses
  absolute `crate::` paths.

## Check

```sh
cargo clippy -p <crate>                 # production only: shows items only tests use (VSCode task "clippy")
cargo clippy -p <crate> --all-targets   # with the test cfg (VSCode task "clippy (all targets, test cfg)")
```

Run both after moving or gating test-only code.
