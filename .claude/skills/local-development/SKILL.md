---
name: local-development
description: Starting development sessions, running tests after code changes, debugging service issues, or verifying implementations - handles service startup, test execution, and troubleshooting
---

# Local Development Workflow

## Quick Start

**Start service** (choose one):

```bash
# Recommended: Use startup script (from repo root)
cd services/identity && powershell.exe -ExecutionPolicy Bypass -File run_identity_local.ps1

# Alternative: VSCode task "identity: local" (auto-configures env vars)
```

**Start Docker dev environment** (databases — required before service):

```bash
docker compose -f services/docker-compose.yml -p shine up -d
# Stop: docker compose -f services/docker-compose.yml -p shine down
```

**Run tests:**

```bash
# Full suite
cd tests && pnpm test:local

# Filtered by test name
cd tests && pnpm test:local --grep "Purge guests"

# Single file
cd tests && pnpm exec playwright test api-tests/identity/purge_guests.ts
```

**Enable request logging** (for debugging / agent analysis):

```bash
cd tests && ENABLE_REQUEST_LOGGING=1 pnpm test:local --grep "test name"
```

**Run Rust crate tests** (unit/integration tests inside the workspace):

Tests that touch Redis/Postgres **skip themselves** unless the `SHINE_TEST_*_CNS`
env vars are set (you'll see `Missing SHINE_TEST_REDIS_CNS/SHINE_REDIS_CNS, skipping test`
warnings). This mirrors the Docker build (`services/Dockerfile`). To run them locally,
start the Docker dev environment (above) and point the CNS vars at the toxiproxy ports
it exposes on the host:

```bash
# toxiproxy exposes redis on host :6379 and postgres on host :5432 (see services/docker-compose.yml)
export SHINE_TEST_REDIS_CNS="redis://localhost:6379"
export SHINE_TEST_PG_CNS="postgres://username:password@localhost:5432/database-test?sslmode=disable"

cargo test -p shine-builder    # or -p shine-identity, or the whole workspace
```

- Direct (un-proxied) DBs are also available: redis `:26379`, postgres `:25432`.
- Without these vars the tests still "pass" but only because they skip — always set them
  when validating DB-backed code, otherwise you're testing nothing.

**Agent test guidelines:**

- NEVER pipe test output through `tail`, `head`, or other truncation — full output is needed for analysis
- Use `--grep` to scope runs to relevant tests when possible
- Set `ENABLE_REQUEST_LOGGING=1` when debugging failures to see HTTP request/response details
- Read full output directly; the `list` reporter shows pass/fail per test
- **Writing a new test or mock?** Use the `api-test-writing` skill first — covers folder layout, available mocks, session minting vs a real user, and service isolation rules.

**Verify service is ready:**

```bash
curl -k https://localhost:8443/identity/info/ready  # Should return "Ok"
```

## Configuration

**Service requirements:**

- Port 8443 (HTTPS), config `server_config.test.json`, certs at `../../certs/scytta.{crt,key}`
- URL: `https://cloud.local.scytta.com:8443/identity`

**Test requirements:**

- Service running at above URL
- Mock services (SMTP:2525, OAuth2:8090, OIDC:8091) auto-start with tests

**Test outputs:**

- Report: `tests/reports/index.html`
- Status: `tests/test-results/.last-run.json` (compare for regressions)

## Development Cycle

1. Start service (background terminal)
2. Run tests → establish baseline
3. Make changes
4. `cd services/identity && cargo build -p shine-identity --release`
5. Restart service (Ctrl+C, rerun script)
6. Re-run tests → compare `.last-run.json` for new failures

## Troubleshooting

**Service won't start:**

- Wrong directory? `pwd` must show `services/identity` (config file location)
- Missing certs? Check `ls ../../certs/scytta.{crt,key}` from services/identity
- Port conflict? `netstat -ano | findstr :8443` (Windows) or `lsof -ti:8443` (Linux/Mac)

**Tests failing:**

1. Service running? `curl -k https://localhost:8443/identity/info/ready`
2. Correct port? Logs should show "Starting service on https://0.0.0.0:8443"
3. New failures? Compare `tests/test-results/.last-run.json` vs current run

**Windows env var errors:**
Bash doesn't support `--` in variable names. Use PowerShell:

```powershell
${env:SHINE--SERVICE--PORT} = "8443"
```

Or use VSCode task (handles env vars automatically).

**Manual service start** (if scripts fail):

```bash
cd services/identity
cargo run -p shine-identity --release -- test
```

Requires env vars set in shell (use PowerShell on Windows).
