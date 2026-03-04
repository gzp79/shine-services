---
name: local-development
description: Use when starting development sessions, running tests after code changes, debugging service issues, or verifying bug fixes and refactoring
---

# Local Development

## When to Use This Skill
- Starting local development session
- Running tests after code changes
- Debugging service issues
- Verifying bug fixes or refactoring

## Running the Identity Service

**PowerShell (Recommended on Windows):**
```powershell
# From services/identity directory
cd services/identity
powershell.exe -ExecutionPolicy Bypass -File run_identity_local.ps1
```

**Bash (Linux/Mac or Git Bash on Windows):**
```bash
# From services/identity directory
cd services/identity
./run_identity_local.sh
```

**VSCode Task:**
- Use task: "identity: local"
- Sets correct env vars: `SHINE--SERVICE--TLS--CERT`, `SHINE--SERVICE--TLS--KEY`, `SHINE--SERVICE--PORT=8443`

**Manual (from services/identity):**
```bash
cd services/identity
cargo run -p shine-identity --release -- test
```

**Service expects:**
- Port: 8443 (HTTPS)
- Config: `server_config.test.json` in working directory
- Certs: `../../certs/scytta.crt` and `../../certs/scytta.key`

## Running Tests

**From tests directory:**
```bash
cd tests
pnpm test:local
```

**Requirements:**
- Service must be running on https://cloud.local.scytta.com:8443
- Mock services start automatically (SMTP on 2525, OAuth2/OIDC on 8090/8091)

**Test results:**
- HTML report: `tests/reports/index.html`
- Last run status: `tests/test-results/.last-run.json`

## Common Issues

### Service won't start
- Check working directory (must be `services/identity` for config)
- Verify certs exist at relative path `../../certs/`
- Check port 8443 not already in use

### Tests failing
1. Verify service is running: `curl https://cloud.local.scytta.com:8443/identity/info/ready` (should return "Ok")
2. Check service logs for errors
3. Ensure service is on port 8443, not 7000

### Environment variable errors on Windows
- Use PowerShell, not bash, for env vars with `--` in names
- Syntax: `${env:SHINE--SERVICE--PORT} = "8443"`
- Bash doesn't support `--` in env var names

## Verification Steps

After starting service:
1. Check startup log shows: "Starting service on https://0.0.0.0:8443"
2. Curl readiness endpoint: `curl -k https://localhost:8443/identity/info/ready`
3. Run tests to verify functionality

## Quick Development Cycle

1. Start service in background
2. Run tests
3. Make code changes
4. Rebuild: `cargo build -p shine-identity --release`
5. Stop old service (Ctrl+C)
6. Restart service
7. Re-run tests
