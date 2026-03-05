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

**Run tests:**
```bash
cd tests && pnpm test:local
```

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
