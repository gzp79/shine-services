<#
.SYNOPSIS
    Local dev helper: backing infra, running services, and test-environment checks. It never runs tests itself.

.DESCRIPTION
    Usage: pwsh scripts/dev.ps1 <command> [options]

      up | up-observe        start postgres, redis, toxiproxy (docker compose, project 'shine') and wait until
                             reachable; up-observe adds the trace dashboard (http://localhost:18888, OTLP :4317)
      down                   stop everything incl. dashboard and the CI 'shine' container
      status                 what is running (infra, services + stage, mocks) and whether the tests can run
      start <identity|builder|all> [-Stage test|local]
                             start a service on the host, with TLS (identity 8443, builder 8444). One service runs in
                             the foreground. 'all' builds once, starts the services that are not running in the
                             background (logs: temp/dev-<service>.log) and waits until they listen.
                             Stage 'test' (as CI: compact console logs) is what Playwright needs; 'local' logs to
                             OTLP only (needs up-observe).
                             Exit 0: started, or already running in that stage. Exit 1: cannot start, message says why.
      stop <identity|builder|all>
                             stop the services running on the host, infra stays up. Only a process answering as the
                             service is stopped, anything else holding the port is left.
      env <rust|api>         check that the environment is ready for the tests and set their variables. Variables
                             only stay when the script is dot-sourced, in the same call as the test command:
                               pwsh -NoProfile -Command ". scripts/dev.ps1 env rust; cargo test -p shine-builder"
                               pwsh -NoProfile -Command ". scripts/dev.ps1 env api; pnpm --dir tests test:local"
                             rust: infra up. Sets SHINE_TEST_* and RUST_TEST_THREADS=1.
                             api:  rust checks + identity and builder running in stage 'test', no mocks running.
                                   Sets CI=false (standalone layout, readable reporter).

    Problems are printed with the command that fixes them; nothing running is ever killed automatically
    (only 'stop' stops services, on request).
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
        return "$($State.Name) is not running. Start it (background): pwsh scripts/dev.ps1 start $($State.Name) -Stage $WantedStage"
    }
    if ($State.App -ne $State.Name) {
        return "Port $($State.Port) is held by something that is not $($State.Name) (pid $($State.Pid))."
    }
    if ($State.Stage -ne $WantedStage) {
        return "$($State.Name) runs in stage '$($State.Stage)' (pid $($State.Pid)) but '$WantedStage' is needed. Stop it ($(Get-StopHint $State.Pid), or dev.ps1 stop $($State.Name)) and start: pwsh scripts/dev.ps1 start $($State.Name) -Stage $WantedStage"
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

function Write-StageHint([string]$Stage) {
    if ($Stage -eq 'local' -and -not (Test-Port 4317)) {
        Write-Warning "Stage 'local' logs to OTLP (localhost:4317) and nothing listens there, so the console stays empty. Use -Stage test or dev.ps1 up-observe."
    }
}

# Reports a service that is already up; fails if it runs in another stage. True when there is nothing to start.
function Test-AlreadyRunning([string]$Service, [string]$Stage) {
    $state = Get-ServiceState $Service
    if (-not $state.Listening) { return $false }
    $problem = Get-ServiceProblem $state $Stage
    if ($problem) { Fail $problem }
    Write-Host "$Service is already running in stage '$Stage' on port $($state.Port) (pid $($state.Pid)). Nothing to do."
    return $true
}

function Start-InForeground([string]$Service, [string]$Stage) {
    if (Test-AlreadyRunning $Service $Stage) { return }
    Write-StageHint $Stage

    Set-Location (Join-Path $repoRoot "services/$Service")
    $env:RUST_BACKTRACE = 'full'
    ${env:SHINE--SERVICE--PORT} = "$($servicePorts[$Service])"
    ${env:SHINE--SERVICE--TLS--CERT} = '../../certificates/scytta.crt'
    ${env:SHINE--SERVICE--TLS--KEY} = '../../certificates/scytta.key'
    cargo run -p "shine-$Service" --release -- $Stage
    exit $LASTEXITCODE
}

function Start-AllInBackground([string]$Stage) {
    $pending = @($servicePorts.Keys | Where-Object { -not (Test-AlreadyRunning $_ $Stage) })
    if ($pending.Count -eq 0) { return }
    Write-StageHint $Stage

    # Built once up front: started side by side, the services would wait on each other for the cargo locks.
    $buildArgs = @('build', '--release') + @($pending | ForEach-Object { '-p'; "shine-$_" })
    cargo @buildArgs
    if ($LASTEXITCODE -ne 0) { Fail 'cargo build failed.' }

    $logDir = Join-Path $repoRoot 'temp'
    New-Item -ItemType Directory -Force $logDir | Out-Null
    # The output goes to the log through the shell, not through Start-Process' redirection: that would hand the
    # caller's output pipe down to the service, and whoever reads it (a tool, CI) would wait for the service to end.
    $started = foreach ($name in $pending) {
        $log = Join-Path $logDir "dev-$name.log"
        $command = "pwsh -NoProfile -File `"$PSCommandPath`" start $name -Stage $Stage"
        $process = if ($isWindowsHost) {
            Start-Process cmd.exe -ArgumentList '/d', '/s', '/c', "`"$command > `"$log`" 2>&1`"" -WindowStyle Hidden -PassThru
        }
        else {
            Start-Process sh -ArgumentList '-c', "exec $command > '$log' 2>&1" -PassThru
        }
        [pscustomobject]@{ Name = $name; Log = $log; Process = $process }
    }

    $deadline = (Get-Date).AddSeconds(120)
    foreach ($service in $started) {
        $port = $servicePorts[$service.Name]
        while (-not (Test-Port $port) -and -not $service.Process.HasExited -and (Get-Date) -lt $deadline) { Start-Sleep -Seconds 1 }
        if (-not (Test-Port $port)) { Fail "$($service.Name) did not start. See $($service.Log)" }
        Write-Host "$($service.Name) started in stage '$Stage' on port $port (pid $(Get-PortOwnerPid $port)). Log: $($service.Log)"
    }
}

function Invoke-Start([string[]]$StartArgs) {
    $target = if ($StartArgs.Count -gt 0) { $StartArgs[0] } else { '' }
    if ($target -ne 'all' -and $target -notin $servicePorts.Keys) { Fail 'Usage: dev.ps1 start <identity|builder|all> [-Stage test|local]' }
    $stage = 'local'
    for ($i = 1; $i -lt $StartArgs.Count; $i++) {
        switch -Regex ($StartArgs[$i]) {
            '^--?stage$' { $stage = $StartArgs[++$i] }
            default { Fail "Unknown option: $($StartArgs[$i])" }
        }
    }
    if ($stage -notin 'local', 'test') { Fail "Stage must be local or test, got '$stage'." }

    $missing = Get-MissingInfra
    if ($missing.Count -gt 0) { Fail "Dev infra not reachable: $($missing -join ', '). Run: pwsh scripts/dev.ps1 up" }

    if ($target -eq 'all') { Start-AllInBackground $stage } else { Start-InForeground $target $stage }
}

function Invoke-Stop([string[]]$StopArgs) {
    $target = if ($StopArgs.Count -gt 0) { $StopArgs[0] } else { '' }
    if ($StopArgs.Count -ne 1 -or ($target -ne 'all' -and $target -notin $servicePorts.Keys)) { Fail 'Usage: dev.ps1 stop <identity|builder|all>' }
    $names = if ($target -eq 'all') { $servicePorts.Keys } else { @($target) }

    foreach ($name in $names) {
        $state = Get-ServiceState $name
        if (-not $state.Listening) {
            Write-Host "$name is not running."
        }
        elseif ($state.App -ne $name -or -not $state.Pid) {
            Write-Warning "Port $($state.Port) is held by something that is not $name (pid $($state.Pid)): left alone."
        }
        else {
            if ($isWindowsHost) { Stop-Process -Id $state.Pid } else { & kill $state.Pid }
            for ($i = 0; $i -lt 20 -and (Test-Port $state.Port); $i++) { Start-Sleep -Milliseconds 500 }
            if (Test-Port $state.Port) { Write-Warning "$name (pid $($state.Pid)) is still listening on port $($state.Port)." }
            else { Write-Host "$name stopped (pid $($state.Pid))." }
        }
    }
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
    'stop' { Invoke-Stop $commandArgs }
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
    'start' { Invoke-Start $commandArgs }
    'env' { Invoke-Env ($commandArgs | Select-Object -First 1) }
    default {
        Get-Help $PSCommandPath | Out-String | Write-Host
        if (-not $dotSourced) { exit 2 }
    }
}
