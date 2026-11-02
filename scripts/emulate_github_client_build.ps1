$ErrorActionPreference = "Stop"

# Resolve the repo root from the script location so the script works regardless of the caller's cwd.
$repoRoot = Split-Path -Parent $PSScriptRoot

function Invoke-Pnpm {
    param([Parameter(Mandatory)][string[]]$Arguments)

    & pnpm @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "pnpm $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

# Keep the local commands aligned with .github/workflows/web_client_ci.yml.
Push-Location (Join-Path $repoRoot "client/web")
try {
    Write-Host "Install dependencies"
    Invoke-Pnpm -Arguments @("install")

    Write-Host "Build"
    Invoke-Pnpm -Arguments @("build")

    Write-Host "Test"
    Invoke-Pnpm -Arguments @("test")
}
finally {
    Pop-Location
}
