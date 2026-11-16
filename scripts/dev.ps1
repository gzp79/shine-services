<#
.SYNOPSIS
    Local dev helper: backing infra, running services, and test-environment checks. It never runs tests itself.

.DESCRIPTION
    Usage: pwsh scripts/dev.ps1 <command> [options]

      up | up-observe        start postgres, redis, toxiproxy (docker compose, project 'shine') and wait until
                             reachable; up-observe adds the trace dashboard (http://localhost:18888, OTLP :4317)
      down                   stop everything incl. dashboard and the CI 'shine' container
      status                 what is running (infra, services + stage, mocks) and whether the tests can run
      run <identity|builder> [-Stage test|local]
                             run the service in the foreground on the host, with TLS (identity 8443, builder 8444).
                             Stage 'test' (as CI: compact console logs) is what Playwright needs; 'local' logs to
                             OTLP only (needs up-observe).
                             Exit 0: ran, or already running in that stage. Exit 1: cannot start, message says why.
      env <rust|api>         check that the environment is ready for the tests and set their variables. Variables
                             only stay when the script is dot-sourced, in the same call as the test command:
                               pwsh -NoProfile -Command ". scripts/dev.ps1 env rust; cargo test -p shine-builder"
                               pwsh -NoProfile -Command ". scripts/dev.ps1 env api; pnpm --dir tests test:local"
                             rust: infra up. Sets SHINE_TEST_* and RUST_TEST_THREADS=1.
                             api:  rust checks + identity and builder running in stage 'test', no mocks running.
                                   Sets CI=false (standalone layout, readable reporter).

    Problems are printed with the command that fixes them; nothing running is ever killed automatically.
    Standalone services are the default target; the nginx/CI docker container is only for reproducing CI on request:
    scripts/emulate_github_server_build.ps1.
#>
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$isWindowsHost = $IsWindows -or $PSVersionTable.PSVersion.Major -lt 6
$dotSourced = $MyInvocation.InvocationName -eq '.'
$compose = @('compose', '-f', 'services/docker-compose.yml', '-p', 'shine')

$servicePorts = [ordered]@{ identity = 8443; builder = 8444 }
$mockPorts = @{ 2525 = 'smtp'; 8090 = 'oauth2'; 8091 = 'oidc' } # plain hashtables: [ordered] treats int keys as positions
$infraPorts = @{ 8474 = 'toxiproxy api'; 5432 = 'postgres (toxiproxy)'; 6379 = 'redis (toxiproxy)'; 25432 = 'postgres (direct)'; 26379 = 'redis (direct)' }

function Test-Port([int]$Port) {
    $c = [Net.Sockets.TcpClient]::new()
    try { return $c.ConnectAsync('127.0.0.1', $Port).Wait(500) -and $c.Connected } catch { return $false } finally { $c.Dispose() }
}

function Get-PortOwnerPid([int]$Port) {
    try {
        if ($isWindowsHost) { return (Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction Stop | Select-Object -First 1).OwningProcess }
        return [int](& lsof -ti "tcp:$Port" -sTCP:LISTEN 2>$null | Select-Object -First 1)
    }
    catch { return $null }
}

function Get-StopHint([int]$ProcessId) { if ($isWindowsHost) { "Stop-Process -Id $ProcessId" } else { "kill $ProcessId" } }

function Get-MissingInfra {
    @($infraPorts.Keys | Sort-Object | Where-Object { -not (Test-Port $_) } | ForEach-Object { "$_ ($($infraPorts[$_]))" })
}

function Wait-Infra([int]$TimeoutSeconds = 60) {
    for ($i = 0; $i -lt $TimeoutSeconds; $i++) {
        if ((Get-MissingInfra).Count -eq 0) { return }
        Start-Sleep -Seconds 1
    }
    throw "Infra not reachable: $((Get-MissingInfra) -join ', ')"
}

# What is behind a service port: nothing, one of our services (and the stage it runs in), or something else.
function Get-ServiceState([string]$Name) {
    $port = $servicePorts[$Name]
    $state = [pscustomobject]@{ Name = $Name; Port = $port; Listening = $false; App = $null; Stage = $null; Pid = $null }
    if (-not (Test-Port $port)) { return $state }
    $state.Listening = $true
    $state.Pid = Get-PortOwnerPid $port
    $json = & curl.exe -sk --max-time 5 "https://cloud.local.scytta.com:$port/$Name/info/version" 2>$null
    try { $v = $json | ConvertFrom-Json; $state.App = $v.app_name; $state.Stage = $v.environment } catch { }
    return $state
}

function Get-MockState {
    $mockPorts.Keys | Sort-Object | Where-Object { Test-Port $_ } | ForEach-Object {
        [pscustomobject]@{ Port = $_; Mock = $mockPorts[$_]; Pid = Get-PortOwnerPid $_ }
    }
}

# Describes how a service state relates to the wanted stage; $null when it is fine.
function Get-ServiceProblem($State, [string]$WantedStage) {
    if (-not $State.Listening) {
        return "$($State.Name) is not running. Start it (background): pwsh scripts/dev.ps1 run $($State.Name) -Stage $WantedStage"
    }
    if ($State.App -ne $State.Name) {
        return "Port $($State.Port) is held by something that is not $($State.Name) (pid $($State.Pid))."
    }
    if ($State.Stage -ne $WantedStage) {
        return "$($State.Name) runs in stage '$($State.Stage)' (pid $($State.Pid)) but '$WantedStage' is needed. Stop it ($(Get-StopHint $State.Pid)) and run: pwsh scripts/dev.ps1 run $($State.Name) -Stage $WantedStage"
    }
    return $null
}

function Get-RustTestProblems {
    $missing = Get-MissingInfra
    if ($missing.Count -gt 0) { return @("Infra not reachable: $($missing -join ', '). Run: pwsh scripts/dev.ps1 up") }
    return @()
}

function Get-ApiTestProblems {
    $problems = @(Get-RustTestProblems)
    foreach ($name in $servicePorts.Keys) {
        $p = Get-ServiceProblem (Get-ServiceState $name) 'test'
        if ($p) { $problems += $p }
    }
    # Playwright starts the SMTP/OAuth2/OIDC mocks itself; running ones would hold the ports.
    foreach ($m in Get-MockState) {
        $problems += "Mock '$($m.Mock)' already listens on $($m.Port) (pid $($m.Pid)); Playwright needs the port. Stop it: $(Get-StopHint $m.Pid)"
    }
    return $problems
}

# Prints the message (plain, no PowerShell error decoration) and fails: `throw` when dot-sourced (the caller's
# command chain stops), exit code 1 otherwise.
function Fail([string[]]$Lines) {
    $Lines | ForEach-Object { [Console]::Error.WriteLine($_) }
    if ($dotSourced) { throw $Lines[0] }
    exit 1
}

function Invoke-Run([string[]]$RunArgs) {
    $service = if ($RunArgs.Count -gt 0) { $RunArgs[0] } else { '' }
    if ($service -notin $servicePorts.Keys) { Fail 'Usage: dev.ps1 run <identity|builder> [-Stage test|local]' }
    $stage = 'local'
    for ($i = 1; $i -lt $RunArgs.Count; $i++) {
        switch -Regex ($RunArgs[$i]) {
            '^--?stage$' { $stage = $RunArgs[++$i] }
            default { Fail "Unknown option: $($RunArgs[$i])" }
        }
    }
    if ($stage -notin 'local', 'test') { Fail "Stage must be local or test, got '$stage'." }

    $missing = Get-MissingInfra
    if ($missing.Count -gt 0) { Fail "Dev infra not reachable: $($missing -join ', '). Run: pwsh scripts/dev.ps1 up" }

    $state = Get-ServiceState $service
    if ($state.Listening) {
        $problem = Get-ServiceProblem $state $stage
        if ($problem) { Fail $problem }
        Write-Host "$service is already running in stage '$stage' on port $($state.Port) (pid $($state.Pid)). Nothing to do."
        return
    }

    if ($stage -eq 'local' -and -not (Test-Port 4317)) {
        Write-Warning "Stage 'local' logs to OTLP (localhost:4317) and nothing listens there, so the console stays empty. Use -Stage test or dev.ps1 up-observe."
    }

    Set-Location (Join-Path $repoRoot "services/$service")
    $env:RUST_BACKTRACE = 'full'
    ${env:SHINE--SERVICE--PORT} = "$($servicePorts[$service])"
    ${env:SHINE--SERVICE--TLS--CERT} = '../../certificates/scytta.crt'
    ${env:SHINE--SERVICE--TLS--KEY} = '../../certificates/scytta.key'
    cargo run -p "shine-$service" --release -- $stage
    exit $LASTEXITCODE
}

function Invoke-Env([string]$Target) {
    if ($Target -notin 'rust', 'api') { Fail 'Usage: dev.ps1 env <rust|api>' }
    $problems = @(if ($Target -eq 'api') { Get-ApiTestProblems } else { Get-RustTestProblems })
    if ($problems.Count -gt 0) { Fail (@("Environment not ready for '$Target' tests:") + ($problems | ForEach-Object { "  - $_" })) }

    if ($Target -eq 'rust') {
        # *_CNS goes through toxiproxy (fault injection), *_CNS_DIRECT bypasses it; same set as the docker build
        $env:SHINE_TEST_TOXIPROXY_URL = 'http://localhost:8474'
        $env:SHINE_TEST_PG_CNS = 'postgres://username:password@localhost:5432/database-test?sslmode=disable'
        $env:SHINE_TEST_PG_CNS_DIRECT = 'postgres://username:password@localhost:25432/database-test?sslmode=disable'
        $env:SHINE_TEST_REDIS_CNS = 'redis://localhost:6379'
        $env:SHINE_TEST_REDIS_CNS_DIRECT = 'redis://localhost:26379'
        $env:RUST_TEST_THREADS = '1' # DB tests share one database
        $env:RUST_BACKTRACE = 'full'
    }
    else {
        $env:CI = 'false'
    }
    Write-Host "Environment ready for '$Target' tests."
    if (-not $dotSourced) { Write-Warning 'Not dot-sourced: the variables are gone after this script. Use ". scripts/dev.ps1 env ..." in the same call as the test command.' }
}

Set-Location $repoRoot
$command = if ($args.Count -gt 0) { $args[0] } else { '' }
$commandArgs = if ($args.Count -gt 1) { @($args[1..($args.Count - 1)]) } else { @() }

switch ($command) {
    { $_ -in 'up', 'up-observe' } {
        # the compose file declares the network as external
        if (-not (docker network ls --filter name=^shine$ -q)) { docker network create shine | Out-Null }
        $profileArgs = if ($command -eq 'up-observe') { @('--profile', 'observe') } else { @() }
        docker @compose @profileArgs up -d
        if ($LASTEXITCODE -ne 0) { throw 'docker compose up failed' }
        Wait-Infra
        Write-Host 'Infra is ready.'
    }
    'down' { docker @compose --profile '*' down }
    'status' {
        $missing = Get-MissingInfra
        Write-Host ('Infra     : ' + $(if ($missing.Count -eq 0) { 'ok' } else { "MISSING $($missing -join ', ') -> pwsh scripts/dev.ps1 up" }))
        foreach ($name in $servicePorts.Keys) {
            $s = Get-ServiceState $name
            $text = if (-not $s.Listening) { 'not running' } elseif ($s.App -ne $name) { "port held by another process (pid $($s.Pid))" } else { "running, stage '$($s.Stage)', pid $($s.Pid)" }
            Write-Host ('{0,-10}: :{1} {2}' -f $name, $s.Port, $text)
        }
        $mocks = @(Get-MockState)
        Write-Host ('Mocks     : ' + $(if ($mocks.Count -eq 0) { 'none running' } else { ($mocks | ForEach-Object { "$($_.Mock):$($_.Port)" }) -join ', ' }))
        Write-Host ('Dashboard : ' + $(if (Test-Port 4317) { 'OTLP on :4317' } else { 'not running (up-observe)' }))
        $ci = docker @compose ps --status running --services 2>$null | Where-Object { $_ -eq 'shine' }
        if ($ci) { Write-Host "CI container 'shine' is running (binds 8443/8080): stop it for standalone services (dev.ps1 down)." }
        Write-Host ''
        foreach ($t in @(@('rust tests', (Get-RustTestProblems)), @('api tests', (Get-ApiTestProblems)))) {
            if ($t[1].Count -eq 0) { Write-Host ('{0,-10}: ready' -f $t[0]) }
            else { Write-Host ('{0,-10}: NOT ready' -f $t[0]); $t[1] | ForEach-Object { Write-Host "    - $_" } }
        }
    }
    'run' { Invoke-Run $commandArgs }
    'env' { Invoke-Env ($commandArgs | Select-Object -First 1) }
    default {
        Get-Help $PSCommandPath | Out-String | Write-Host
        if (-not $dotSourced) { exit 2 }
    }
}
