# Service Configuration

How `identity` and `builder` are configured in each environment: which config files are layered, which ports and TLS setup apply, and how to override a value.

## Scenarios

| Scenario | Stage | Root config | Ports (identity / builder) | Entry point for clients | TLS terminated by | Backing services | Telemetry |
|---|---|---|---|---|---|---|---|
| **Prod** | `prod` | `server_config.prod.json` | 7000 / 7001 (inside the container) | nginx `:80`, hosting platform adds TLS on `:443` | hosting platform | real, secrets from Azure Key Vault | console logs + OTLP metrics |
| **CI test** | `test` | `server_config.test.json` | 7000 / 7001 (inside the container) | nginx `:443` (identity and builder on the same port) | nginx, repo dev certs | mocks on the host (`mockbox.foo`), docker infra | console logs only |
| **Local test** | `test` | `server_config.test.json` | 8443 / 8444 | direct, no nginx | the service, repo dev certs | same as CI test | console logs only |
| **Local** | `local` | `server_config.local.json` | 8443 / 8444 | direct, no nginx | the service, repo dev certs | same as CI test | OTLP only (trace dashboard) |

*Local test* is *CI test* without the container: the same `test` config with the port and TLS overridden by environment variables. It is what agents and Playwright use by default. *Local* adds developer-friendly telemetry on top of the mock setup and is meant for humans with the trace dashboard running.

```mermaid
flowchart LR
    subgraph prod_ci["Prod / CI test (one container)"]
        N[nginx :80 / :443] -->|/identity| I1[identity :7000]
        N -->|/builder| B1[builder :7001]
    end
    subgraph local["Local test / Local (host processes)"]
        I2[identity :8443 TLS]
        B2[builder :8444 TLS]
    end
```

## Stages

The stage is the required first argument of the service binary (`shine-identity <stage>`). It selects the root config file `server_config.<stage>.json` and is reported as `environment` by `GET /info/version`; every difference between stages comes from the config files.

| Stage | Meaning | Differences from the others |
|---|---|---|
| `prod` | Deployed service. | Real secrets from Azure Key Vault, no mocks, full error details off (`fullProblemResponse: false`), metrics to OTLP, real OAuth/OIDC providers. Runs behind the hosting platform's TLS termination. |
| `test` | Reproducible setup used by CI and the API tests. | Mocked external services and test secrets (`common.mock.json`), short timings (heartbeat, inbox recovery), console logging, no metrics or tracing. Ports `7000/7001` for the container; locally overridden to `8443/8444` with TLS. |
| `local` | Developer setup, derived from `test`. | Same mocks and secrets as `test`, plus verbose log levels per crate and logs, metrics and traces going to the OTLP dashboard (`localhost:4317`) instead of the console. Ports `8443/8444` with TLS set in the file. |

Pick `test` when running the API tests, in CI and for agents (readable console output, the exact setup CI uses). Pick `local` when you want to inspect traces in the dashboard (`dev.ps1 up-observe`). A new stage is just a new `server_config.<stage>.json`.

## Layer system

A root file `server_config.<stage>.json` (selected by the stage argument: `shine-identity <stage>`) lists its layers. Later layers win:

```text
beforeLayers (in order)  →  root file itself  →  afterLayers (in order)
```

| Layer | Meaning |
|---|---|
| `file://<path>` | JSON file, must exist; relative to the service directory |
| `file?://<path>` | same, skipped when missing (e.g. `../../temp/server_config.json` for personal overrides, gitignored) |
| `azk://<vault-host>/` | all secrets of an Azure Key Vault (credentials from `AZURE_*` env or the Azure CLI login) |
| `environment` | environment variables, see below |

Layers per scenario (paths relative to `services/<service>/`):

| Scenario | beforeLayers | afterLayers |
|---|---|---|
| Prod | `../config/common.prod.json`, `azk://shine-keyvault.vault.azure.net/` | `environment` |
| CI test / Local test | `../config/common.mock.json`, `../config/common.test.json`, `config/mock.json` | `environment` (builder also `file?://../../temp/server_config.json`) |
| Local | `../config/common.mock.json`, `config/mock.json`, `../config/telemetry.local.json` | `environment` (builder also `file?://../../temp/server_config.json`) |

What the shared files hold:

| File | Content |
|---|---|
| `config/common.prod.json` | allowed origins, prod telemetry |
| `config/common.mock.json` | test secrets, `*.local.scytta.com` origins, session store (`redis.mockbox.foo`) |
| `config/common.test.json` | port `0`, console logging, no metrics/tracing |
| `config/telemetry.local.json` | OTLP logging/metrics/tracing to `localhost:4317` |
| `<service>/config/mock.json` | DB connection strings (via toxiproxy), mocked SMTP/OAuth2/OIDC providers (identity), WS hosts (builder) |

The `local` root files also set the service port and the dev certificate paths; `test` root files only set the container ports (7000/7001).

## Environment overrides

The `environment` layer reads variables with the prefix `SHINE--`. `--` separates nesting levels, `_` inside a segment becomes camelCase:

| Variable | Config path |
|---|---|
| `SHINE--SERVICE--PORT=8443` | `service.port` |
| `SHINE--SERVICE--TLS--CERT=../../certificates/scytta.crt` | `service.tls.cert` (setting `tls` enables TLS) |
| `SHINE--TELEMETRY--DEFAULT_LEVEL=debug` | `telemetry.defaultLevel` |

Variable names with `--` are not valid shell identifiers: use PowerShell (`${env:SHINE--SERVICE--PORT}`) or `env "SHINE--...=..." cmd` in bash. The old `SHINE__` / `TLS__CERT` forms no longer work.

## Backing services and mocks

Test scenarios reach the docker infra (`services/docker-compose.yml`) and the mocked external services through the hostnames `mockbox.foo`, `postgres.mockbox.foo`, `redis.mockbox.foo`. On the host these resolve through the hosts file (`127.0.0.1`); in the CI container through `extra_hosts` / `links` in the compose file.

| Host port | What | Used by |
|---|---|---|
| 5432 | postgres **via toxiproxy** | services (`postgres.mockbox.foo`), `SHINE_TEST_PG_CNS` |
| 6379 | redis **via toxiproxy** | services (`redis.mockbox.foo`), `SHINE_TEST_REDIS_CNS` |
| 25432 / 26379 | postgres / redis **direct** (no fault injection) | `SHINE_TEST_PG_CNS_DIRECT` / `SHINE_TEST_REDIS_CNS_DIRECT`: tests that must keep a working connection while the proxied one is broken |
| 8474 | toxiproxy API | `SHINE_TEST_TOXIPROXY_URL`, Playwright `infraOutage` fixture |
| 18888 / 4317 | trace dashboard UI / OTLP ingest (`up-observe` only) | stage `local` telemetry |
| 2525 / 8090 / 8091 | SMTP / OAuth2 / OIDC mocks (`mockbox.foo:<port>`) | identity config; `tests/` mock servers |

- Mocks: Playwright starts the ones it needs per suite. `pnpm mock:services` (in `tests/`) starts all of them (+ DNS :53, game/assets static servers) for manual flows; never run both at once, the ports clash.
- Leftover fault injection after a crashed test run: `curl -X POST http://localhost:8474/reset`.

### Rust test environment

DB-backed Rust tests skip silently (or panic, for helpers that require them) without these variables. `scripts/dev.ps1 env rust` sets them for the host; `services/Dockerfile` sets the in-network equivalents for the CI build (`cargo test --workspace -- --test-threads=1`, tests share one database):

| Variable | Host (`dev.ps1 env rust`) | CI docker build |
|---|---|---|
| `SHINE_TEST_PG_CNS` | `postgres://username:password@localhost:5432/database-test?sslmode=disable` | `...@postgres.mockbox.foo:5432/...` |
| `SHINE_TEST_PG_CNS_DIRECT` | `...@localhost:25432/...` | `...@postgres-direct.mockbox.foo:5432/...` |
| `SHINE_TEST_REDIS_CNS` | `redis://localhost:6379` | `redis://redis.mockbox.foo:6379` |
| `SHINE_TEST_REDIS_CNS_DIRECT` | `redis://localhost:26379` | `redis://redis-direct.mockbox.foo:6379` |
| `SHINE_TEST_TOXIPROXY_URL` | `http://localhost:8474` | `http://toxiproxy.mockbox.foo:8474` |

A missing `_DIRECT` variable falls back to the proxied URL with a warning, which hides outage-test bugs.

## Redis ACL users (prod, Fly.io / Upstash)

The prod Redis is the managed Upstash instance on Fly.io. Each credential gets its own ACL user, scoped to the keys and commands it needs, so a leaked connection string exposes only one feature's data. Locally the docker Redis has no users and needs none.

| User | Config key | Keys | Used for |
|---|---|---|---|
| `identity` | `identity.db.redisCns` | `session:*` | session create / extend / delete (see [session lifecycle](identity/session-lifecycle.md)) |
| `shine` | `service.sessionRedisCns` | `session:*` | read-only current-user check, shared by identity and builder |
| `builder` | `builder.db.redisCns` | `cmd-inbox:*`, `hub-connection:*` | command inbox streams, hub connection registry |

Upstash manages ACL passwords itself: `ACL SETUSER … >password` with a self-chosen password is not accepted. Create a user in two steps, `ACL GENTOKEN`, then `ACL SETUSER` with that token as the password. The default user stays under Upstash's control; there is no `aclfile` or `redis.conf` to mount on a managed instance.

<details data-agent="security">
<summary>ACL commands per user</summary>

```text
ACL GENTOKEN
> "<identity_token>"
ACL SETUSER identity on ><identity_token> ~session:* \
  +ping +get +set +setnx +expire +del +scan +ttl

ACL GENTOKEN
> "<shine_token>"
ACL SETUSER shine on ><shine_token> ~session:* \
  +ping +get +exists +scan +ttl +expire

ACL GENTOKEN
> "<builder_token>"
ACL SETUSER builder on ><builder_token> ~cmd-inbox:* ~hub-connection:* \
  +@all -@admin -@dangerous +info +config|get
```

The `builder` rule is deliberately general: any command on its own keys is allowed, so a new Redis operation in the builder code needs no ACL change. What it cannot do is administrative or whole-keyspace work: `-@admin` (`CONFIG`, `ACL`, `SHUTDOWN`, …), and `-@dangerous` (`FLUSHALL`, `FLUSHDB`, `KEYS`, `DEBUG`, …). Lua scripts (`EVAL`, `EVALSHA`, `FUNCTION`) stay allowed: Redis checks every `redis.call` in a script against the user's ACL, so a script cannot touch keys or commands the user itself cannot. `+info` and `+config|get` come last because `INFO` is in `@dangerous` and `CONFIG` in `@admin`, and the startup check (`check_redis`) needs `INFO server` and `CONFIG GET maxmemory-policy` (rules apply in order; `+config|get` allows only the read-only subcommand, `CONFIG SET` stays denied). Without `config|get` the service still starts, but logs "Could not check Redis maxmemory-policy". If the builder ever needs a command from a removed category, add just that command (`+<command>`) instead of the whole category. The `identity` and `shine` users stay on explicit lists: they are tiny, and a new operation there should be a conscious decision.

Connection strings carry the credential inline: `redis://<user>:<token>@<host>:6379?timeout=3000&pool_timeout=5000`.

What the builder uses today (`services/builder/src/repositories/`); the rule above covers all of it:

| Feature | Keys / channels | Commands |
|---|---|---|
| Command inbox (`command_inbox/redis`) | stream `cmd-inbox:<stream_id>`, channel `cmd-inbox-wake` | `XADD`, `XRANGE`, `XTRIM`, `XLEN`, `EXPIRE`, `PUBLISH`, `SUBSCRIBE`, `UNSUBSCRIBE` |
| Hub registry (`hub_registry/redis`) | `hub-connection:<user_id>`, channel `hub-registry-changed` | `SET … EX`, `GET`, `MGET`, `EXPIRE`, `DEL`, `SCAN MATCH`, `EVALSHA`/`EVAL` (the heartbeat and removal scripts), `PUBLISH`, `SUBSCRIBE`, `UNSUBSCRIBE` |
| Startup (`shine_infra::db::redis`) | none | `INFO server`, `CONFIG GET maxmemory-policy` (`check_redis`), `PING` |

`SCAN` and the pub/sub commands take no key arguments, so the key pattern does not restrict them: the `builder` user can still scan the keyspace (it sees names, not values) and use any channel. Channel rules (`resetchannels &cmd-inbox-wake &hub-registry-changed`) would close that, but whether Upstash accepts them is untested.

**Untested:** none of these users exist yet. Before switching a service over, check each with `ACL DRYRUN <user> <command> <args>`: an allowed command on an own key must pass, any command on a `session:*` key (for `builder`) or a `cmd-inbox:*` key (for `identity` / `shine`) must be denied. A `NOPERM` in the service log after the switch means a command is missing from the list (`identity` / `shine`) or sits in a removed category (`builder`). Whether Upstash accepts `@category` rules is untested too; if it rejects them, list the commands from the table above explicitly.

</details>

## One-time setup

| Need | How |
|---|---|
| PowerShell 7 (`pwsh`) | the helper scripts are `.ps1` only |
| Hosts entries → `127.0.0.1` | `local.scytta.com`, `cloud.local.scytta.com`, `ws.local.scytta.com`, `game.local.scytta.com`, `assets.local.scytta.com`, `mockbox.foo`, `postgres.mockbox.foo`, `redis.mockbox.foo` |
| TLS certs in `certificates/` | `cd tests && pnpm install && pnpm run mkcert` (trust `ca.crt` in the browser if you use one) |
| Docker network `shine` | created by `dev.ps1 up` when missing (the compose file declares it `external`) |

## Running locally

Use the helper scripts (details in the [local-development skill](/.claude/skills/local-development/SKILL.md)):

```powershell
pwsh scripts/dev.ps1 up                                  # postgres, redis, toxiproxy
pwsh scripts/dev.ps1 start all -Stage test               # Local test: both services in the background
pwsh scripts/dev.ps1 start builder -Stage local          # Local, one service in the foreground (needs `dev.ps1 up-observe` to see logs)
pwsh scripts/dev.ps1 status                              # what runs in which stage
pwsh scripts/dev.ps1 stop all                            # stop the services (identity | builder for one), infra stays up
```

### Reproducing CI (only on request)

CI runs both services in one container behind nginx (`:8443` for both, `:8080` http) and Playwright with `CI=true`. That conflicts with standalone services on 8443. To debug CI itself run `pwsh scripts/emulate_github_server_build.ps1` (Docker clock-skew check, image build with the right `--add-host` entries, container start, test run); stop the container with `dev.ps1 down`.
