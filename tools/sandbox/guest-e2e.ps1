<#
.SYNOPSIS
    End-to-end test that runs INSIDE Windows Sandbox (started by run-sandbox-e2e.ps1).

.DESCRIPTION
    Opens test windows, runs the real daemon and drives it with the CLI and
    injected input. Records window geometry, cloak state and the foreground
    window at each step, and writes results.json, screenshots and logs to the
    output folder. Never run this on a real desktop: it starts the tiling
    daemon and kills it on purpose.
#>
param(
    [string]$In = 'C:\e2e\in',
    [string]$Out = 'C:\e2e\out'
)

$ErrorActionPreference = 'Continue'
$results = New-Object System.Collections.Generic.List[object]
$work = 'C:\e2e\work'

function Log([string]$message) {
    Add-Content -Path (Join-Path $Out 'guest.log') -Value ("{0} {1}" -f (Get-Date -Format o), $message)
}

function Check([string]$name, [bool]$pass, [string]$detail) {
    $results.Add([pscustomobject]@{ name = $name; pass = $pass; detail = $detail })
    Log ("CHECK {0} pass={1} {2}" -f $name, $pass, $detail)
}

function Save-Results {
    $results | ConvertTo-Json -Depth 6 | Set-Content -Path (Join-Path $Out 'results.json') -Encoding UTF8
}

function Write-Config([string]$content) {
    $dir = Join-Path $env:APPDATA 'openniri\config'
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    Set-Content -Path (Join-Path $dir 'config.toml') -Value $content -Encoding ASCII
    Copy-Item (Join-Path $dir 'config.toml') (Join-Path $Out ("config-{0}.toml" -f $script:phase)) -Force
}

function Invoke-Cli([string[]]$cliArgs) {
    $output = & $script:cli @cliArgs 2>&1 | Out-String
    $code = $LASTEXITCODE
    Add-Content -Path (Join-Path $Out 'cli.log') -Value ("== [{0}] openniri-cli {1} -> exit {2}`n{3}" -f $script:phase, ($cliArgs -join ' '), $code, $output)
    return [pscustomobject]@{ Code = $code; Output = $output }
}

function Start-Daemon {
    $process = Start-Process -FilePath $script:daemon -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $Out ("daemon-{0}.out.log" -f $script:phase)) `
        -RedirectStandardError (Join-Path $Out ("daemon-{0}.err.log" -f $script:phase))
    $ready = $false
    for ($i = 0; $i -lt 60; $i++) {
        Start-Sleep -Milliseconds 250
        $status = Invoke-Cli @('status')
        if ($status.Code -eq 0) { $ready = $true; break }
        if ($process.HasExited) { break }
    }
    Check ("{0}: daemon starts and answers status" -f $script:phase) $ready ("pid={0} exited={1}" -f $process.Id, $process.HasExited)
    Start-Sleep -Seconds 1
    return $process
}

function Stop-Daemon($process) {
    $stop = Invoke-Cli @('stop')
    $exited = $process.WaitForExit(10000)
    Check ("{0}: daemon stops cleanly" -f $script:phase) ($exited) ("stop exit={0} processExited={1}" -f $stop.Code, $exited)
    if (-not $exited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
}

function Get-TestWindows {
    @([E2E]::TopLevelWindows() | Where-Object { $_.Title -like 'E2E-*' })
}

# True when at least 60% of the window's area lies on the virtual screen.
function Test-OnScreen($window) {
    $vs = [E2E]::VirtualScreen()
    if ($window.W -le 0 -or $window.H -le 0) { return $false }
    $left = [Math]::Max($window.X, $vs[0]); $right = [Math]::Min($window.X + $window.W, $vs[0] + $vs[2])
    $top = [Math]::Max($window.Y, $vs[1]); $bottom = [Math]::Min($window.Y + $window.H, $vs[1] + $vs[3])
    if ($right -le $left -or $bottom -le $top) { return $false }
    return ((($right - $left) * ($bottom - $top)) / ($window.W * $window.H)) -ge 0.6
}

# Tiled windows fill the work-area height and visible ones do not overlap.
function Test-Tiled($windows) {
    $vs = [E2E]::VirtualScreen()
    $problems = New-Object System.Collections.Generic.List[string]
    foreach ($w in $windows) {
        if ($w.Cloaked -eq 0 -and (Test-OnScreen $w) -and $w.H -lt ($vs[3] * 0.8)) { $problems.Add(("{0} height {1} not tiled" -f $w.Title, $w.H)) }
    }
    $onScreen = @($windows | Where-Object { $_.Cloaked -eq 0 -and (Test-OnScreen $_) } | Sort-Object X)
    for ($i = 1; $i -lt $onScreen.Count; $i++) {
        if ($onScreen[$i].X -lt ($onScreen[$i - 1].X + $onScreen[$i - 1].W - 16)) { $problems.Add(("{0} overlaps {1}" -f $onScreen[$i].Title, $onScreen[$i - 1].Title)) }
    }
    return $problems
}

function Snap([string]$tag) {
    $name = "{0}-{1}" -f $script:phase, $tag
    $windows = Get-TestWindows
    [pscustomobject]@{
        tag = $name
        foreground = [E2E]::Foreground()
        virtualScreen = [E2E]::VirtualScreen()
        windows = $windows
    } | ConvertTo-Json -Depth 5 | Set-Content -Path (Join-Path $Out ("windows-{0}.json" -f $name)) -Encoding UTF8
    try { [E2E]::Screenshot((Join-Path $Out ("screen-{0}.png" -f $name))) } catch { Log ("screenshot failed: {0}" -f $_) }
    return $windows
}

function Test-AllRecovered([string]$tag) {
    $windows = Snap $tag
    $bad = @($windows | Where-Object { -not (Test-OnScreen $_) -or $_.Cloaked -ne 0 })
    $detail = ($bad | ForEach-Object { "{0}@({1},{2}) cloaked={3}" -f $_.Title, $_.X, $_.Y, $_.Cloaked }) -join '; '
    Check ("{0}: all {1} test windows on screen and uncloaked after {2}" -f $script:phase, $windows.Count, $tag) ($bad.Count -eq 0 -and $windows.Count -eq $script:titles.Count) $detail
}

# Click a window's caption and confirm it becomes and stays the foreground window.
# Windows Sandbox occasionally pops up a short-lived system console window
# (rgnupdt.exe) that takes the foreground; one retry is allowed when the window
# that took focus is not a test window.
function Test-ClickActivates($window, [string]$tag) {
    $result = Invoke-ClickActivation $window
    if (-not $result.Pass -and -not $result.LaterIsTestWindow) {
        Log ("click on {0} lost focus to non-test window {1}; retrying once" -f $window.Title, $result.Later)
        $retry = Invoke-ClickActivation $window
        $retry.Detail = "retry after focus went to non-test window {0}; {1}" -f $result.Later, $retry.Detail
        $result = $retry
    }
    Check ("{0}: click activates {1} and focus stays ({2})" -f $script:phase, $window.Title, $tag) $result.Pass $result.Detail
}

function Invoke-ClickActivation($window) {
    $x = $window.X + [int]($window.W / 2)
    $y = $window.Y + 12
    $hit = [E2E]::WindowAt($x, $y)
    if ($hit -ne $window.Hwnd) {
        return [pscustomobject]@{ Pass = $false; LaterIsTestWindow = $true; Later = $hit; Detail = ("point ({0},{1}) hits hwnd {2}, not the window" -f $x, $y, $hit) }
    }
    [E2E]::Click($x, $y)
    Start-Sleep -Milliseconds 400
    $first = [E2E]::Foreground()
    Start-Sleep -Milliseconds 1500
    $later = [E2E]::Foreground()
    $laterIsTest = @(Get-TestWindows | Where-Object { $_.Hwnd -eq $later }).Count -gt 0
    return [pscustomobject]@{
        Pass = ($first -eq $window.Hwnd -and $later -eq $window.Hwnd)
        LaterIsTestWindow = $laterIsTest
        Later = $later
        Detail = ("fg after 0.4s={0} after 1.9s={1} expected={2}" -f $first, $later, $window.Hwnd)
    }
}

function Measure-CursorLatency([string]$tag) {
    $vs = [E2E]::VirtualScreen()
    $samples = New-Object System.Collections.Generic.List[double]
    $timeouts = 0
    for ($i = 0; $i -lt 40; $i++) {
        $x = $vs[0] + 50 + (($i * 97) % [Math]::Max(100, $vs[2] - 100))
        $y = $vs[1] + 50 + (($i * 61) % [Math]::Max(100, $vs[3] - 100))
        $ms = [E2E]::CursorLatencyMs($x, $y, 2000)
        if ($ms -lt 0) { $timeouts++ } else { $samples.Add($ms) }
    }
    $sorted = @($samples | Sort-Object)
    $p95 = if ($sorted.Count -gt 0) { $sorted[[Math]::Min($sorted.Count - 1, [int]($sorted.Count * 0.95))] } else { -1 }
    $max = if ($sorted.Count -gt 0) { $sorted[-1] } else { -1 }
    Check ("{0}: cursor latency with hooks ({1})" -f $script:phase, $tag) ($timeouts -eq 0 -and $p95 -ge 0 -and $p95 -lt 50) ("p95={0:N1}ms max={1:N1}ms timeouts={2}" -f $p95, $max, $timeouts)
}

$VK_ALT = 0x12; $VK_SHIFT = 0x10; $VK_H = 0x48; $VK_L = 0x4C
# Enough windows that the strip is wider than any sandbox viewport.
$script:titles = @('E2E-A', 'E2E-B', 'E2E-C', 'E2E-D', 'E2E-E', 'E2E-F', 'E2E-G', 'E2E-H', 'E2E-I', 'E2E-J')

try {
    New-Item -ItemType Directory -Force -Path $work | Out-Null
    $script:phase = 'setup'
    Add-Type -Path (Join-Path $In 'E2EHelpers.cs') -ReferencedAssemblies System.Drawing
    [E2E]::InitDpi()
    Add-Type -Path (Join-Path $In 'TestWindow.cs') -OutputAssembly (Join-Path $work 'TestWindow.exe') `
        -OutputType WindowsApplication -ReferencedAssemblies System.Windows.Forms, System.Drawing
    Copy-Item (Join-Path $In 'bin\*.exe') $work -Force
    $script:cli = Join-Path $work 'openniri-cli.exe'
    $script:daemon = Join-Path $work 'openniri.exe'
    Log ("virtual screen: {0}" -f ([E2E]::VirtualScreen() -join ','))

    # ------------------------------------------------------------------
    # Phase A: default settings (cloaking on, gestures and focus-follows-mouse off)
    # ------------------------------------------------------------------
    $script:phase = 'A-default'
    Write-Config "[behavior]`nlog_level = `"debug`"`n"
    foreach ($title in $script:titles) {
        Start-Process -FilePath (Join-Path $work 'TestWindow.exe') -ArgumentList $title, (Join-Path $Out 'activations.log')
        Start-Sleep -Milliseconds 300
    }
    Start-Sleep -Seconds 2
    $before = Snap 'before-daemon'
    Check 'setup: test windows open' ($before.Count -eq $script:titles.Count) ("found {0} of {1}" -f $before.Count, $script:titles.Count)

    $daemonProcess = Start-Daemon
    $tiled = Snap 'tiled'
    $managed = Invoke-Cli @('query', 'all')
    Check 'A-default: daemon manages all test windows' (($tiled.Count -eq $script:titles.Count) -and (($managed.Output -split 'E2E-').Count - 1) -ge $script:titles.Count) ("query all exit={0}" -f $managed.Code)
    $tileProblems = Test-Tiled $tiled
    Check 'A-default: windows are tiled at startup (full height, no overlap)' ($tileProblems.Count -eq 0) ($tileProblems -join '; ')
    $offscreen = @($tiled | Where-Object { -not (Test-OnScreen $_) -or $_.Cloaked -ne 0 })
    Check 'A-default: strip extends beyond the viewport (some windows hidden)' ($offscreen.Count -ge 1) ("hidden={0}" -f $offscreen.Count)
    $cloakedHidden = @($tiled | Where-Object { $_.Cloaked -ne 0 })
    $parked = @($tiled | Where-Object { $_.X -le -29000 -and $_.Y -le -29000 })
    Check 'A-default: hidden windows are cloaked by the shell, not parked off-screen' ($cloakedHidden.Count -ge 1 -and $parked.Count -eq 0) ("cloaked={0} parked={1}" -f $cloakedHidden.Count, $parked.Count)

    # Hotkeys: walk left to the first column, then right through all columns.
    for ($i = 0; $i -lt ($script:titles.Count + 1); $i++) { [E2E]::Chord($VK_ALT, $VK_H); Start-Sleep -Milliseconds 350 }
    Start-Sleep -Milliseconds 500
    $seen = @{}
    $badSteps = New-Object System.Collections.Generic.List[string]
    for ($i = 0; $i -lt $script:titles.Count; $i++) {
        $fg = [E2E]::Foreground()
        $windows = Get-TestWindows
        $focused = $windows | Where-Object { $_.Hwnd -eq $fg } | Select-Object -First 1
        if ($null -eq $focused) { $badSteps.Add(("step {0}: foreground {1} is not a test window" -f $i, $fg)) }
        elseif (-not (Test-OnScreen $focused) -or $focused.Cloaked -ne 0) { $badSteps.Add(("step {0}: focused {1} not visible at ({2},{3}) cloaked={4}" -f $i, $focused.Title, $focused.X, $focused.Y, $focused.Cloaked)) }
        else { $seen[$focused.Title] = $true }
        [E2E]::Chord($VK_ALT, $VK_L)
        Start-Sleep -Milliseconds 600
    }
    Snap 'after-hotkey-walk' | Out-Null
    Check 'A-default: Alt+H/Alt+L focus walk reaches every window, each visible when focused' ($seen.Count -eq $script:titles.Count -and $badSteps.Count -eq 0) (("seen={0}; {1}" -f ($seen.Keys -join ','), ($badSteps -join ' | ')))

    # Focus must not be stolen back: click every on-screen window.
    $visibleNow = @(Get-TestWindows | Where-Object { (Test-OnScreen $_) -and $_.Cloaked -eq 0 })
    foreach ($window in $visibleNow) { Test-ClickActivates $window 'while tiling' }

    # CLI commands used in the INC-49 scenario.
    foreach ($cmd in @(@('apply'), @('focus', 'left'), @('scroll', 'right', '--pixels', '400'), @('toggle-pause'), @('toggle-pause'), @('equalize-widths'), @('refresh'))) {
        $r = Invoke-Cli $cmd
        Start-Sleep -Milliseconds 500
    }
    $afterCli = Snap 'after-cli'
    $fgAfterCli = [E2E]::Foreground()
    Check 'A-default: foreground is a visible test window after CLI scenario' (@($afterCli | Where-Object { $_.Hwnd -eq $fgAfterCli -and (Test-OnScreen $_) -and $_.Cloaked -eq 0 }).Count -eq 1) ("fg={0}" -f $fgAfterCli)

    Stop-Daemon $daemonProcess
    Start-Sleep -Seconds 1
    Test-AllRecovered 'stop'
    foreach ($window in @(Get-TestWindows | Select-Object -First 3)) { Test-ClickActivates $window 'after stop' }

    $daemonLog = Get-Content -Raw -ErrorAction SilentlyContinue (Join-Path $Out 'daemon-A-default.err.log')
    $daemonLog += Get-Content -Raw -ErrorAction SilentlyContinue (Join-Path $Out 'daemon-A-default.out.log')
    $cloakErrors = ([regex]::Matches([string]$daemonLog, 'CLOAK=0\) failed')).Count
    $applyFailures = ([regex]::Matches([string]$daemonLog, 'apply_placements completed with side-effect failures')).Count
    Check 'A-default: no uncloak error spam and no failed layout passes' ($cloakErrors -eq 0 -and $applyFailures -eq 0) ("uncloak errors={0} failed passes={1}" -f $cloakErrors, $applyFailures)
    Check 'A-default: daemon log has no panic' (-not ([string]$daemonLog -match 'panicked')) ''

    # ------------------------------------------------------------------
    # Phase B: low-level mouse hooks on (focus-follows-mouse + gestures)
    # ------------------------------------------------------------------
    $script:phase = 'B-hooks'
    Write-Config "[behavior]`nlog_level = `"debug`"`nfocus_follows_mouse = true`nfocus_follows_mouse_delay_ms = 100`n`n[gestures]`nenabled = true`n"
    $daemonProcess = Start-Daemon
    Measure-CursorLatency 'focus-follows-mouse + gesture hooks'
    $visibleNow = @(Get-TestWindows | Where-Object { (Test-OnScreen $_) -and $_.Cloaked -eq 0 })
    $target = $visibleNow | Where-Object { $_.Hwnd -ne [E2E]::Foreground() } | Select-Object -First 1
    if ($null -ne $target) {
        [void][E2E]::CursorLatencyMs($target.X + [int]($target.W / 2), $target.Y + [int]($target.H / 2), 500)
        Start-Sleep -Milliseconds 900
        Check 'B-hooks: focus follows mouse onto a visible window' ([E2E]::Foreground() -eq $target.Hwnd) ("target={0} fg={1}" -f $target.Title, [E2E]::Foreground())
    } else {
        Check 'B-hooks: focus follows mouse onto a visible window' $false 'no unfocused visible window to hover'
    }
    Stop-Daemon $daemonProcess
    Start-Sleep -Seconds 1
    Test-AllRecovered 'stop'

    # ------------------------------------------------------------------
    # Phase C: hard kill while windows are hidden, then CLI recovery
    # ------------------------------------------------------------------
    $script:phase = 'C-crash'
    Write-Config "[behavior]`nlog_level = `"debug`"`n"
    $daemonProcess = Start-Daemon
    # Alt+2 (half width) on several columns guarantees that windows are hidden.
    for ($i = 0; $i -lt 4; $i++) {
        [E2E]::Chord($VK_ALT, 0x32); Start-Sleep -Milliseconds 400
        [E2E]::Chord($VK_ALT, $VK_L); Start-Sleep -Milliseconds 500
    }
    $hidden = @(Snap 'before-kill' | Where-Object { -not (Test-OnScreen $_) -or $_.Cloaked -ne 0 })
    Stop-Process -Id $daemonProcess.Id -Force
    Start-Sleep -Seconds 1
    Snap 'after-kill' | Out-Null
    Check 'C-crash: windows are hidden when the daemon is killed (precondition)' ($hidden.Count -ge 1) ("hidden={0}" -f $hidden.Count)
    $recover = Invoke-Cli @('emergency-uncloak')
    Start-Sleep -Seconds 1
    Check 'C-crash: emergency-uncloak succeeds after hard kill' ($recover.Code -eq 0) ("exit={0}" -f $recover.Code)
    Test-AllRecovered 'kill + emergency-uncloak'

    # ------------------------------------------------------------------
    # Phase D: hard kill while windows are cloaked, then just start the daemon again
    # ------------------------------------------------------------------
    $script:phase = 'D-restart'
    $daemonProcess = Start-Daemon
    for ($i = 0; $i -lt 4; $i++) {
        [E2E]::Chord($VK_ALT, 0x32); Start-Sleep -Milliseconds 400
        [E2E]::Chord($VK_ALT, $VK_L); Start-Sleep -Milliseconds 500
    }
    $hidden = @(Snap 'before-kill' | Where-Object { $_.Cloaked -ne 0 })
    Check 'D-restart: windows are cloaked when the daemon is killed (precondition)' ($hidden.Count -ge 1) ("cloaked={0}" -f $hidden.Count)
    Stop-Process -Id $daemonProcess.Id -Force
    Start-Sleep -Seconds 1
    $daemonProcess = Start-Daemon
    $managed = Invoke-Cli @('query', 'all')
    $managedCount = ($managed.Output -split 'E2E-').Count - 1
    Check 'D-restart: restarted daemon recovers and manages every window' ($managedCount -ge $script:titles.Count) ("managed={0}" -f $managedCount)
    Stop-Daemon $daemonProcess
    Start-Sleep -Seconds 1
    Test-AllRecovered 'restart + stop'

    # ------------------------------------------------------------------
    # Phase E: Explorer restarts while the daemon runs (shell reconnect)
    # ------------------------------------------------------------------
    $script:phase = 'E-explorer'
    $daemonProcess = Start-Daemon
    Get-Process explorer -ErrorAction SilentlyContinue | Stop-Process -Force
    $explorerBack = $false
    for ($i = 0; $i -lt 40; $i++) {
        Start-Sleep -Milliseconds 500
        if (Get-Process explorer -ErrorAction SilentlyContinue) { $explorerBack = $true; break }
    }
    if (-not $explorerBack) { Start-Process explorer.exe; Start-Sleep -Seconds 3 }
    Start-Sleep -Seconds 4
    for ($i = 0; $i -lt 4; $i++) {
        [E2E]::Chord($VK_ALT, 0x32); Start-Sleep -Milliseconds 400
        [E2E]::Chord($VK_ALT, $VK_L); Start-Sleep -Milliseconds 500
    }
    $afterRestart = Snap 'after-explorer-restart'
    $cloakedNow = @($afterRestart | Where-Object { $_.Cloaked -ne 0 })
    $parkedNow = @($afterRestart | Where-Object { $_.X -le -29000 -and $_.Y -le -29000 })
    Check 'E-explorer: windows are still cloaked through the shell after Explorer restarts' ($cloakedNow.Count -ge 1 -and $parkedNow.Count -eq 0) ("explorerRestartedBySystem={0} cloaked={1} parked={2}" -f $explorerBack, $cloakedNow.Count, $parkedNow.Count)
    Stop-Daemon $daemonProcess
    Start-Sleep -Seconds 1
    Test-AllRecovered 'explorer restart + stop'
}
catch {
    Check 'guest script completed without exception' $false ($_ | Out-String)
}
finally {
    Get-Process openniri -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Save-Results
    Set-Content -Path (Join-Path $Out 'done.txt') -Value (Get-Date -Format o)
}
