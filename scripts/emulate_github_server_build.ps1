# Resolve the repo root from the script location so the script works regardless of the caller's cwd.
$repoRoot = Split-Path -Parent $PSScriptRoot

# Push-Location records the caller's directory; the finally block restores it even on error/exit.
Push-Location $repoRoot
# CI drives the URL/port selection in playwright.config.ts; restore the prior value on exit.
$prevCI = $env:CI
try {
    # The dockerized service stamps cookie expiries with the Docker VM clock; if it drifts behind the
    # host (common after the host sleeps) freshly-issued cookies look already-expired and the auth
    # cookie tests fail. Catch that here instead of deep in a confusing assertion.
    Write-Host "Check Docker VM clock skew"
    $maxSkewSeconds = 120
    $hostEpoch = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    $vmEpoch = [int64](docker run --rm busybox date -u +%s | Select-Object -Last 1)
    $skew = [Math]::Abs($hostEpoch - $vmEpoch)
    Write-Host "  clock skew: ${skew}s"
    if ($skew -gt $maxSkewSeconds) {
        throw "Docker VM clock differs from host by ${skew}s (max ${maxSkewSeconds}s). Resync with 'wsl --shutdown' then restart Docker Desktop."
    }

    Write-Host "Setup buildx"
    docker network create shine
    docker buildx create --name shine-build --driver=docker-container --driver-opt=network=shine --use

    Write-Host "Reset environment"
    docker compose -f services/docker-compose.yml -p shine --profile test down

    Write-Host "Setup environment"
    docker compose -f services/docker-compose.yml -p shine up -d
    $pg_host=docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' shine-postgres-1
    $redis_host=docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' shine-redis-1
    Write-Host "  pg: $pg_host"
    Write-Host "  redis: $redis_host"

    Write-Host "Build action"
    # --load option is added only for local test to cache the layers for the next build
    docker buildx build --progress=plain -f services/Dockerfile --target test --add-host postgres.mockbox.foo:$pg_host --add-host redis.mockbox.foo:$redis_host -t gzp79/shine-services:test --load .

    Write-Host "Start service in docker"
    docker compose -f services/docker-compose.yml -p shine --profile test up -d --no-recreate

    Write-Host "Run tests"
    $env:CI = "true"
    pnpm --dir tests run test:local
}
finally {
    if ($null -eq $prevCI) { Remove-Item Env:CI -ErrorAction SilentlyContinue } else { $env:CI = $prevCI }
    Pop-Location
}
