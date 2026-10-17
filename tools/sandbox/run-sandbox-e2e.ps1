<#
.SYNOPSIS
    Run the OpenNiri end-to-end test in a disposable Windows Sandbox.

.DESCRIPTION
    Starts Windows Sandbox (networking off), maps the release binaries
    read-only and a results folder writable, runs guest-e2e.ps1 in the sandbox
    user session, waits for it, stops the sandbox and prints a summary. The
    tiling daemon only ever runs inside the sandbox, never on this desktop.

    Requires Windows 11 with the Windows Sandbox feature (wsb.exe).
    Exit code: 0 when every check passed, 1 when a check failed, 2 on harness errors.

.EXAMPLE
    cargo build --release --workspace
    pwsh tools/sandbox/run-sandbox-e2e.ps1
#>
param(
    [string]$BinDir,
    [string]$OutRoot,
    [int]$TimeoutSec = 600,
    [switch]$KeepSandbox
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not $BinDir) { $BinDir = Join-Path $repo 'target\x86_64-pc-windows-gnu\release' }
if (-not $OutRoot) { $OutRoot = Join-Path $repo 'target\sandbox-e2e' }

if (-not (Get-Command wsb -ErrorAction SilentlyContinue)) {
    Write-Error 'wsb.exe not found. Enable the Windows Sandbox feature (Windows 11 24H2 or later).'
    exit 2
}
foreach ($exe in 'openniri.exe', 'openniri-cli.exe') {
    if (-not (Test-Path (Join-Path $BinDir $exe))) {
        Write-Error "Missing $exe in $BinDir. Run: cargo build --release --workspace"
        exit 2
    }
}

$runDir = Join-Path $OutRoot (Get-Date -Format 'yyyyMMdd-HHmmss')
$stage = Join-Path $runDir 'in'
$out = Join-Path $runDir 'out'
New-Item -ItemType Directory -Force -Path (Join-Path $stage 'bin'), $out | Out-Null
Copy-Item (Join-Path $PSScriptRoot 'guest-e2e.ps1'), (Join-Path $PSScriptRoot 'E2EHelpers.cs'), (Join-Path $PSScriptRoot 'TestWindow.cs') $stage
Copy-Item (Join-Path $BinDir 'openniri.exe'), (Join-Path $BinDir 'openniri-cli.exe') (Join-Path $stage 'bin')

$config = @"
<Configuration>
  <Networking>Disable</Networking>
  <ClipboardRedirection>Disable</ClipboardRedirection>
  <PrinterRedirection>Disable</PrinterRedirection>
  <AudioInput>Disable</AudioInput>
  <VideoInput>Disable</VideoInput>
  <MappedFolders>
    <MappedFolder><HostFolder>$stage</HostFolder><SandboxFolder>C:\e2e\in</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>
    <MappedFolder><HostFolder>$out</HostFolder><SandboxFolder>C:\e2e\out</SandboxFolder><ReadOnly>false</ReadOnly></MappedFolder>
  </MappedFolders>
</Configuration>
"@

# wsb writes expected failures (e.g. "no active user session yet") to stderr;
# Windows PowerShell would turn those into terminating errors under 'Stop'.
$ErrorActionPreference = 'Continue'

Write-Host "Starting Windows Sandbox (results: $runDir)"
$startOutput = (& wsb start --config $config --raw) | Out-String
$id = $null
try { $id = ($startOutput | ConvertFrom-Json).Id } catch { }
if (-not $id) {
    $match = [regex]::Match($startOutput, '[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}')
    if ($match.Success) { $id = $match.Value }
}
if (-not $id) {
    Write-Error "Could not read sandbox id from: $startOutput"
    exit 2
}
Write-Host "Sandbox id: $id"

$exitCode = 2
try {
    # `wsb start` boots without a user logon; the session (and its desktop) only
    # exists once a client connects. The client window shows the test running.
    Start-Process -FilePath 'wsb' -ArgumentList 'connect', '--id', $id | Out-Null

    # Wait until a user session exists inside the sandbox.
    $deadline = (Get-Date).AddSeconds(180)
    $ready = Join-Path $out 'ready.txt'
    while (-not (Test-Path $ready)) {
        if ((Get-Date) -gt $deadline) { throw 'Sandbox user session did not become ready within 180 s' }
        & wsb exec --id $id -r ExistingLogin -c 'cmd.exe /c echo ready> C:\e2e\out\ready.txt' 2>&1 | Out-Null
        Start-Sleep -Seconds 3
    }
    Write-Host 'Sandbox session ready; running guest test'

    & wsb exec --id $id -r ExistingLogin -c 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\e2e\in\guest-e2e.ps1' 2>&1 | Out-Null

    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while (-not (Test-Path (Join-Path $out 'done.txt'))) {
        if ((Get-Date) -gt $deadline) { throw "Guest test did not finish within $TimeoutSec s" }
        Start-Sleep -Seconds 3
    }

    $resultsPath = Join-Path $out 'results.json'
    # Windows PowerShell returns a JSON array as one pipeline object; unroll it.
    $checks = @(Get-Content -Raw $resultsPath | ConvertFrom-Json | ForEach-Object { $_ })
    $failed = @($checks | Where-Object { -not $_.pass })
    foreach ($c in $checks) {
        $mark = if ($c.pass) { 'PASS' } else { 'FAIL' }
        Write-Host ("[{0}] {1}  {2}" -f $mark, $c.name, $c.detail)
    }
    Write-Host ("{0}/{1} checks passed. Evidence: {2}" -f ($checks.Count - $failed.Count), $checks.Count, $out)
    $exitCode = if ($failed.Count -eq 0) { 0 } else { 1 }
}
catch {
    Write-Host "Harness error: $_"
    $exitCode = 2
}
finally {
    if (-not $KeepSandbox) {
        & wsb stop --id $id 2>&1 | Out-Null
    } else {
        Write-Host "Sandbox $id left running (-KeepSandbox). Stop it with: wsb stop --id $id"
    }
}
exit $exitCode
