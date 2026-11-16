---
name: local-development
description: Use when starting identity or builder services locally, running Rust crate tests that need Redis/Postgres, running the Playwright API tests, or debugging a local service, test infra, or toxiproxy problem in shine-services
---

# Local Development

One PowerShell 7 script, `scripts/dev.ps1` (`pwsh` works on Windows, Linux and macOS), prepares and checks the environment: infra, services, test variables. **It never runs tests**: you call `cargo` / `pnpm` yourself once it reports the environment ready. The VSCode tasks cover the same ground (their test tasks set the variables in `tasks.json` directly).

**Default target: standalone services on the host with TLS** (identity `:8443`, builder `:8444`). The nginx/CI docker container is only for reproducing CI when the user asks (`scripts/emulate_github_server_build.ps1`, see [configuration.md](../../../docs/services/configuration.md)).

## What do you want to do?

| Goal | Command |
|---|---|
| See what is running and what is ready | `pwsh scripts/dev.ps1 status` |
| Start DB/Redis/toxiproxy | `pwsh scripts/dev.ps1 up` (`up-observe` adds the trace dashboard) |
| Run a service | `pwsh scripts/dev.ps1 run identity -Stage test` (or `builder`) |
| Mocked externals for manual poking (email, OAuth/OIDC, web client) | `pnpm --dir tests mock:services` (VSCode task "start mocked services") |
| Rust tests | `pwsh -NoProfile -Command ". scripts/dev.ps1 env rust; cargo test -p shine-builder"` |
| Playwright API tests | `pwsh -NoProfile -Command ". scripts/dev.ps1 env api; pnpm --dir tests test:local --grep 'Purge guests'"` |
| Stop infra | `pwsh scripts/dev.ps1 down` |

## Script feedback

Every command checks its preconditions and, when something is off, prints each problem with the command that fixes it, exit code 1. Act on that text. Nothing running is ever killed for you: stopping a service that runs in the wrong stage is your call (the message includes the pid and stop command; ask the user if it is not yours).
`dev.ps1 run` exits 0 with "already running in stage ... Nothing to do" when the wanted service is up; reuse it.

## Rules

- **Playwright (`dev.ps1 env api`) needs both services in stage `test`, and no mocks running.** Playwright starts the mocks it needs itself (see Mocks). `dev.ps1 env api` checks both; on a problem it prints the fixes and the chained `pnpm` does not run.
- **Agents pass `-Stage test`.** Stage `local` replaces console logging with OTLP (trace dashboard on `:4317`); without `up-observe` the console stays empty.
- **Rust DB tests skip silently without the env vars.** Dot-source `dev.ps1 env rust` before `cargo test`; it sets all five `SHINE_TEST_*` vars and `RUST_TEST_THREADS=1` (tests share one database, same as the docker build). Dot-source in the same `pwsh` call as the test command: variables do not survive into a separate call.
- **Never truncate test output** (`tail`, `head`): read it whole. `ENABLE_REQUEST_LOGGING=1` adds HTTP request/response logs.
- Writing a new test or mock → `api-test-writing` skill first.
- Run services in a background process; Ctrl+C / kill stops them. A rebuilt binary needs a restart.

## Typical agent session

```powershell
pwsh scripts/dev.ps1 status                          # what is missing?
pwsh scripts/dev.ps1 up
pwsh scripts/dev.ps1 run identity -Stage test    # run in background; wait for "Starting service on https://..."
pwsh scripts/dev.ps1 run builder  -Stage test    # run in background
pwsh -NoProfile -Command ". scripts/dev.ps1 env api; pnpm --dir tests test:local"
pwsh -NoProfile -Command ". scripts/dev.ps1 env rust; cargo test -p shine-identity"
```

Compare `tests/test-results/.last-run.json` between runs to spot new failures.

## Troubleshooting

| Symptom | Check |
|---|---|
| `Infra not reachable` | `dev.ps1 up`; is Docker running? |
| Port 8443 held by "something else" | `dev.ps1 status`; stale process, or the CI docker container (`dev.ps1 down` stops it) |
| TLS / DNS errors, `mockbox.foo` not found | One-time setup (hosts entries, `pnpm run mkcert`) in [configuration.md](../../../docs/services/configuration.md) |
| Rust tests "pass" but log `Missing SHINE_TEST_...` | `dev.ps1 env rust` not dot-sourced in the same call as `cargo test` |
| Fault-injection tests (`*_failures.ts`) fail after a crashed run | Left-over toxic: `curl -X POST localhost:8474/reset` |

Scenarios (prod / CI test / local test / local), config layers, env overrides, infra ports (toxiproxy vs direct), mocks, one-time setup (hosts, certs), CI docker mode: [docs/services/configuration.md](../../../docs/services/configuration.md).
