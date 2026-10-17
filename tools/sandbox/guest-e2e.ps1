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

function Start-Daemon([string[]]$DaemonArgs = @()) {
    $startParams = @{
        FilePath = $script:daemon
        PassThru = $true
        WindowStyle = 'Hidden'
        RedirectStandardOutput = (Join-Path $Out ("daemon-{0}.out.log" -f $script:phase))
        RedirectStandardError = (Join-Path $Out ("daemon-{0}.err.log" -f $script:phase))
    }
    if ($DaemonArgs.Count -gt 0) { $startParams.ArgumentList = $DaemonArgs }
    $process = Start-Process @startParams
    $ready = $false
    for ($i = 0; $i -lt 60; $i++) {
        Start-Sleep -Milliseconds 250
        $status = Invoke-Cli @('status')
        if ($status.Code -eq 0) { $ready = $true; break }
        if ($process.HasExited) { break }
    }
    if (-not $ready) { Check ("{0}: daemon starts and answers status" -f $script:phase) $false ("pid={0} exited={1}" -f $process.Id, $process.HasExited) }
    Start-Sleep -Seconds 1
    return $process
}

function Stop-Daemon($process) {
    $stop = Invoke-Cli @('stop')
    $exited = $process.WaitForExit(10000)
    if (-not $exited) { Check ("{0}: daemon stops cleanly" -f $script:phase) $false ("stop exit={0}" -f $stop.Code) }
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
    Check ("all windows visible after {0}" -f $tag) ($bad.Count -eq 0 -and $windows.Count -eq $script:titles.Count) $detail
}

# Click a window's caption and confirm it becomes and stays the foreground window.
# Windows Sandbox occasionally pops up a short-lived system console window
# (rgnupdt.exe) that takes the foreground; one retry is allowed when the window
# that took focus is not a test window.
function Test-ClickActivates($window) {
    $result = Invoke-ClickActivation $window
    if (-not $result.Pass -and -not $result.LaterIsTestWindow) {
        Log ("click on {0} lost focus to non-test window {1}; retrying once" -f $window.Title, $result.Later)
        $result = Invoke-ClickActivation $window
    }
    if (-not $result.Pass) { return ("{0}: {1}" -f $window.Title, $result.Detail) }
    return $null
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

# Alt+2 (half width) on several columns, so that some windows leave the viewport.
function Hide-SomeWindows {
    for ($i = 0; $i -lt 4; $i++) {
        [E2E]::Chord($VK_ALT, 0x32); Start-Sleep -Milliseconds 400
        [E2E]::Chord($VK_ALT, $VK_L); Start-Sleep -Milliseconds 500
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
    Check ("cursor latency p95 < 50 ms ({0})" -f $tag) ($timeouts -eq 0 -and $p95 -ge 0 -and $p95 -lt 50) ("p95={0:N1}ms max={1:N1}ms timeouts={2}" -f $p95, $max, $timeouts)
}

function Test-MouseFocus {
    $vs = [E2E]::VirtualScreen()
    $targets = @(Get-TestWindows | Where-Object { $_.Cloaked -eq 0 -and (Test-OnScreen $_) } | Sort-Object X)
    $failures = New-Object System.Collections.Generic.List[string]
    $visited = 0
    foreach ($window in $targets) {
        $x = [int](([Math]::Max($window.FX, $vs[0]) + [Math]::Min($window.FX + $window.FW, $vs[0] + $vs[2])) / 2)
        $y = $window.FY + 60
        $hit = [E2E]::WindowAt($x, $y)
        if ($hit -ne $window.Hwnd) { $failures.Add(("{0}: hover point hits {1}" -f $window.Title, $hit)); continue }
        $result = [E2E]::ObserveHover($x, $y, $false, 0, 0)
        $visited++
        if (-not $result.CursorArrived -or $result.AfterDelay -ne $window.Hwnd -or $result.Later -ne $window.Hwnd -or $result.UnderCursor -ne $window.Hwnd) {
            $failures.Add(("{0}: after delay={1} later={2} under cursor={3}" -f $window.Title, $result.AfterDelay, $result.Later, $result.UnderCursor))
        }
    }
    Check 'mouse hover focuses windows and focus stays with stationary cursor' ($visited -ge 2 -and $failures.Count -eq 0) ("visited={0}; {1}" -f $visited, ($failures -join ' | '))

    # A move into the desktop's outer gap does not emit MouseEnterWindow for a
    # managed window. Stay away from the remote session's bottom screen edge.
    # An old hover must therefore be rejected by the live hit test at expiry.
    $leaveX = $vs[0] + 5
    $leaveY = $vs[1] + 5
    $leaveHit = [E2E]::WindowAt($leaveX, $leaveY)
    $foreground = [E2E]::Foreground()
    $target = $targets | Where-Object { $_.Hwnd -ne $foreground } | Select-Object -First 1
    $leaveIsManaged = @(Get-TestWindows | Where-Object { $_.Hwnd -eq $leaveHit }).Count -gt 0
    if ($null -eq $target -or $leaveIsManaged) {
        Check 'leaving a hovered window before the delay preserves focus' $false 'Missing nonfocused hover target or unmanaged exit point'
    } else {
        $x = [int](([Math]::Max($target.FX, $vs[0]) + [Math]::Min($target.FX + $target.FW, $vs[0] + $vs[2])) / 2)
        $y = $target.FY + 60
        $hit = [E2E]::WindowAt($x, $y)
        $result = [E2E]::ObserveHover($x, $y, $true, $leaveX, $leaveY)
        $pass = $hit -eq $target.Hwnd -and $result.CursorArrived -and $result.Before -eq $foreground -and $result.AfterDelay -eq $foreground -and $result.Later -eq $foreground -and $result.UnderCursor -eq $leaveHit
        Check 'leaving a hovered window before the delay preserves focus' $pass ("before={0} after delay={1} later={2} under cursor={3} expected hit={4} arrived={5} cursor=({6},{7}) exit=({8},{9})" -f $result.Before, $result.AfterDelay, $result.Later, $result.UnderCursor, $leaveHit, $result.CursorArrived, $result.CursorX, $result.CursorY, $leaveX, $leaveY)
    }
    Snap 'mouse-focus' | Out-Null
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
    # A: default settings
    # ------------------------------------------------------------------
    $script:phase = 'A'
    Write-Config "[behavior]`nlog_level = `"debug`"`n"
    foreach ($title in $script:titles) {
        Start-Process -FilePath (Join-Path $work 'TestWindow.exe') -ArgumentList $title, (Join-Path $Out 'activations.log')
        Start-Sleep -Milliseconds 300
    }
    # On a busy host .NET window startup can take much longer than two seconds.
    # Wait for the actual test windows before starting the daemon under test.
    $windowsDeadline = (Get-Date).AddSeconds(30)
    while (@(Get-TestWindows).Count -ne $script:titles.Count -and (Get-Date) -lt $windowsDeadline) {
        Start-Sleep -Milliseconds 250
    }
    $before = Snap 'before-daemon'
    if ($before.Count -ne $script:titles.Count) { throw ("setup: only {0} of {1} test windows opened" -f $before.Count, $script:titles.Count) }

    $daemonProcess = Start-Daemon
    $tiled = Snap 'tiled'

    # Layout at startup: all managed, full height, no overlap, 10 px visible gaps,
    # windows outside the viewport cloaked in place (not parked off-screen).
    $managed = Invoke-Cli @('query', 'all')
    $problems = New-Object System.Collections.Generic.List[string]
    $managedCount = ($managed.Output -split 'E2E-').Count - 1
    if ($managedCount -lt $script:titles.Count) { $problems.Add(("managed {0}" -f $managedCount)) }
    foreach ($p in (Test-Tiled $tiled)) { $problems.Add($p) }
    $frames = @($tiled | Where-Object { $_.Cloaked -eq 0 -and (Test-OnScreen $_) } | Sort-Object FX)
    for ($i = 1; $i -lt $frames.Count; $i++) {
        $gap = $frames[$i].FX - ($frames[$i - 1].FX + $frames[$i - 1].FW)
        if ([Math]::Abs($gap - 10) -gt 2) { $problems.Add(("gap {0}" -f $gap)) }
    }
    foreach ($f in $frames) { if ([Math]::Abs($f.FY - 10) -gt 2) { $problems.Add(("{0} top {1}" -f $f.Title, $f.FY)) } }
    $cloakedCount = @($tiled | Where-Object { $_.Cloaked -ne 0 }).Count
    $parkedCount = @($tiled | Where-Object { $_.X -le -29000 -and $_.Y -le -29000 }).Count
    if ($cloakedCount -lt 1 -or $parkedCount -gt 0) { $problems.Add(("cloaked={0} parked={1}" -f $cloakedCount, $parkedCount)) }
    Check 'layout at startup: tiled, 10 px gaps, hidden windows cloaked' ($problems.Count -eq 0) ($problems -join '; ')

    # The IPC pipe admits only this user and SYSTEM.
    $pipeName = [System.IO.Directory]::GetFiles('\\.\pipe\') | Where-Object { $_ -match 'openniri' } | Select-Object -First 1
    $pipeSddl = [E2E]::SecuritySddl($pipeName)
    $broad = @('WD', 'AU', 'BU', 'AN', 'S-1-1-0', 'S-1-5-11', 'S-1-5-32-545') | Where-Object { $pipeSddl -match (";;;{0}\)" -f [regex]::Escape($_)) }
    Check 'IPC pipe ACL: only this user and SYSTEM' ($pipeSddl -like 'O:*D:P*' -and @($broad).Count -eq 0) $pipeSddl

    # Hotkeys: Alt+H to the first column, then Alt+L through all of them.
    for ($i = 0; $i -lt ($script:titles.Count + 1); $i++) { [E2E]::Chord($VK_ALT, $VK_H); Start-Sleep -Milliseconds 350 }
    Start-Sleep -Milliseconds 500
    $seen = @{}
    $badSteps = New-Object System.Collections.Generic.List[string]
    for ($i = 0; $i -lt $script:titles.Count; $i++) {
        $fg = [E2E]::Foreground()
        $focused = Get-TestWindows | Where-Object { $_.Hwnd -eq $fg } | Select-Object -First 1
        if ($null -eq $focused) { $badSteps.Add(("step {0}: foreground {1} is not a test window" -f $i, $fg)) }
        elseif (-not (Test-OnScreen $focused) -or $focused.Cloaked -ne 0) { $badSteps.Add(("step {0}: {1} not visible" -f $i, $focused.Title)) }
        else { $seen[$focused.Title] = $true }
        [E2E]::Chord($VK_ALT, $VK_L)
        Start-Sleep -Milliseconds 600
    }
    Snap 'after-hotkey-walk' | Out-Null
    Check 'hotkey walk focuses every window, each visible when focused' ($seen.Count -eq $script:titles.Count -and $badSteps.Count -eq 0) ($badSteps -join ' | ')

    # Lockout check (INC-49): clicking a window activates it and nothing steals focus back.
    $clickFailures = @(Get-TestWindows | Where-Object { (Test-OnScreen $_) -and $_.Cloaked -eq 0 } | ForEach-Object { Test-ClickActivates $_ } | Where-Object { $_ })
    Check 'click activates every visible window and focus stays' ($clickFailures.Count -eq 0) ($clickFailures -join ' | ')

    foreach ($cmd in @(@('apply'), @('focus', 'left'), @('scroll', 'right', '--pixels', '400'), @('toggle-pause'), @('toggle-pause'), @('equalize-widths'), @('refresh'))) {
        Invoke-Cli $cmd | Out-Null
        Start-Sleep -Milliseconds 500
    }
    $afterCli = Snap 'after-cli'
    $fg = [E2E]::Foreground()
    Check 'CLI scenario leaves a visible test window focused' (@($afterCli | Where-Object { $_.Hwnd -eq $fg -and (Test-OnScreen $_) -and $_.Cloaked -eq 0 }).Count -eq 1) ("fg={0}" -f $fg)

    Stop-Daemon $daemonProcess
    Start-Sleep -Seconds 1
    Test-AllRecovered 'stop'

    $daemonLog = [string](Get-Content -Raw -ErrorAction SilentlyContinue (Join-Path $Out 'daemon-A.err.log'))
    $daemonLog += [string](Get-Content -Raw -ErrorAction SilentlyContinue (Join-Path $Out 'daemon-A.out.log'))
    $logProblems = @('CLOAK=0\) failed', 'side-effect failures', 'panicked') | Where-Object { $daemonLog -match $_ }
    Check 'daemon log: no uncloak errors, failed passes or panics' (@($logProblems).Count -eq 0) (@($logProblems) -join ', ')

    # ------------------------------------------------------------------
    # B: low-level mouse hooks installed (focus-follows-mouse + gestures)
    # Regression coverage for stale delayed focus (issue #63).
    # ------------------------------------------------------------------
    $script:phase = 'B'
    Write-Config "[layout]`ndefault_column_width = 400`nmin_column_width = 300`nmax_column_width = 400`n`n[behavior]`nlog_level = `"debug`"`nfocus_follows_mouse = true`nfocus_follows_mouse_delay_ms = 100`n`n[gestures]`nenabled = true`n"
    $daemonProcess = Start-Daemon
    Measure-CursorLatency 'mouse hooks installed'
    # Start at the left edge so at least two narrow columns are visible,
    # regardless of the scroll offset saved by phase A.
    Invoke-Cli @('scroll', 'left', '--pixels', '100000') | Out-Null
    Start-Sleep -Milliseconds 500
    Test-MouseFocus
    Stop-Daemon $daemonProcess
    Start-Sleep -Seconds 1

    # ------------------------------------------------------------------
    # C: hard kill while windows are hidden, then `emergency-uncloak`
    # ------------------------------------------------------------------
    $script:phase = 'C'
    Write-Config "[behavior]`nlog_level = `"debug`"`n"
    $daemonProcess = Start-Daemon
    Hide-SomeWindows
    $hidden = @(Snap 'before-kill' | Where-Object { -not (Test-OnScreen $_) -or $_.Cloaked -ne 0 }).Count
    Stop-Process -Id $daemonProcess.Id -Force
    Start-Sleep -Seconds 1
    $recover = Invoke-Cli @('emergency-uncloak')
    Start-Sleep -Seconds 1
    if ($hidden -lt 1 -or $recover.Code -ne 0) { Check 'kill + emergency-uncloak: preconditions' $false ("hidden={0} exit={1}" -f $hidden, $recover.Code) }
    Test-AllRecovered 'kill + emergency-uncloak'

    # ------------------------------------------------------------------
    # D: hard kill while windows are cloaked, then start the daemon again
    # ------------------------------------------------------------------
    $script:phase = 'D'
    $daemonProcess = Start-Daemon
    Hide-SomeWindows
    $cloaked = @(Snap 'before-kill' | Where-Object { $_.Cloaked -ne 0 }).Count
    Stop-Process -Id $daemonProcess.Id -Force
    Start-Sleep -Seconds 1
    $daemonProcess = Start-Daemon
    $managedCount = ((Invoke-Cli @('query', 'all')).Output -split 'E2E-').Count - 1
    Check 'restart after kill recovers cloaked windows and manages all' ($cloaked -ge 1 -and $managedCount -ge $script:titles.Count) ("cloaked before kill={0} managed after restart={1}" -f $cloaked, $managedCount)
    Stop-Daemon $daemonProcess
    Start-Sleep -Seconds 1
    Test-AllRecovered 'restart + stop'

    # ------------------------------------------------------------------
    # F: launch options survive repeated IPC reloads.
    # ------------------------------------------------------------------
    $reloadConfig = "[behavior]`nlog_level = `"debug`"`n`n[appearance]`nuse_cloaking = true`n`n[hotkeys]`n`"Alt+Ctrl+F8`" = `"reload`"`n"
    foreach ($flag in @('--safe-mode', '--no-hotkeys', '--no-cloak')) {
        $script:phase = 'F-' + $flag.TrimStart('-')
        Write-Config $reloadConfig
        $keyWasFree = [E2E]::HotkeyAvailable(3, 0x77)
        $daemonProcess = Start-Daemon @($flag)
        $failures = New-Object System.Collections.Generic.List[string]
        if (-not $keyWasFree) { $failures.Add('test hotkey already registered before daemon start') }
        for ($attempt = 0; $attempt -lt 2; $attempt++) {
            $reload = Invoke-Cli @('reload')
            if ($reload.Code -ne 0) { $failures.Add(("reload {0} failed: {1}" -f $attempt, $reload.Output)) }
            $keyFree = [E2E]::HotkeyAvailable(3, 0x77)
            $expectFree = $flag -ne '--no-cloak'
            if ($keyFree -ne $expectFree) { $failures.Add(("hotkey available={0}, expected={1}" -f $keyFree, $expectFree)) }
            Invoke-Cli @('scroll', 'right', '--pixels', '100000') | Out-Null
            Start-Sleep -Milliseconds 400
            $windows = Get-TestWindows
            $cloaked = @($windows | Where-Object { $_.Cloaked -ne 0 }).Count
            $parked = @($windows | Where-Object { $_.X -le -29000 -and $_.Y -le -29000 }).Count
            $expectCloak = $flag -eq '--no-hotkeys'
            if (($expectCloak -and ($cloaked -eq 0 -or $parked -ne 0)) -or (-not $expectCloak -and ($cloaked -ne 0 -or $parked -eq 0))) {
                $failures.Add(("cloaked={0} parked={1} expected cloaking={2}" -f $cloaked, $parked, $expectCloak))
            }
        }
        Check ("{0} survives repeated config reloads" -f $flag) ($failures.Count -eq 0) ($failures -join ' | ')
        Stop-Daemon $daemonProcess
        Start-Sleep -Seconds 1
        Test-AllRecovered ("{0} reload + stop" -f $flag)
    }

    # ------------------------------------------------------------------
    # G: a reload hotkey replaces its own binding; invalid TOML preserves it.
    # ------------------------------------------------------------------
    $script:phase = 'G'
    Write-Config $reloadConfig
    $keysWereFree = [E2E]::HotkeyAvailable(3, 0x77) -and [E2E]::HotkeyAvailable(3, 0x78)
    $daemonProcess = Start-Daemon
    Write-Config "[behavior]`nlog_level = `"debug`"`n`n[hotkeys]`n`"Alt+Ctrl+F9`" = `"toggle_pause`"`n"
    [E2E]::Chord(0x12, 0x11, 0x77)
    $rebound = $false
    for ($i = 0; $i -lt 40; $i++) {
        Start-Sleep -Milliseconds 100
        if ([E2E]::HotkeyAvailable(3, 0x77) -and -not [E2E]::HotkeyAvailable(3, 0x78)) { $rebound = $true; break }
    }
    Check 'hotkey-triggered reload replaces OS shortcut registrations' ($keysWereFree -and $rebound) ("old key released and new key registered={0}" -f $rebound)
    [E2E]::Chord(0x12, 0x11, 0x78)
    Start-Sleep -Milliseconds 400
    $health = Invoke-Cli @('health')
    Check 'new shortcut executes its reloaded command' ($health.Code -eq 0 -and $health.Output -match 'Status: PAUSED') $health.Output.Trim()

    Write-Config '[hotkeys'
    $reload = Invoke-Cli @('reload')
    [E2E]::Chord(0x12, 0x11, 0x78)
    Start-Sleep -Milliseconds 400
    $health = Invoke-Cli @('health')
    $retained = -not [E2E]::HotkeyAvailable(3, 0x78) -and [E2E]::HotkeyAvailable(3, 0x77)
    Check 'invalid config preserves bindings and their commands' ($reload.Code -ne 0 -and $retained -and $health.Code -eq 0 -and $health.Output -notmatch 'Status: PAUSED') ("reload exit={0}; {1}" -f $reload.Code, $health.Output.Trim())
    Stop-Daemon $daemonProcess
    Start-Sleep -Seconds 1
    Test-AllRecovered 'hotkey reload + stop'

    # ------------------------------------------------------------------
    # E: Explorer restarts while the daemon runs (shell reconnect).
    # Run this destructive shell scenario last: failed recovery here must not
    # contaminate the reload tests with windows hidden by an earlier phase.
    # ------------------------------------------------------------------
    $script:phase = 'E'
    Write-Config "[behavior]`nlog_level = `"debug`"`n"
    $daemonProcess = Start-Daemon
    Get-Process explorer -ErrorAction SilentlyContinue | Stop-Process -Force
    $explorerBack = $false
    for ($i = 0; $i -lt 40; $i++) {
        Start-Sleep -Milliseconds 500
        if (Get-Process explorer -ErrorAction SilentlyContinue) { $explorerBack = $true; break }
    }
    if (-not $explorerBack) { Start-Process explorer.exe; Start-Sleep -Seconds 3 }
    Start-Sleep -Seconds 4
    Hide-SomeWindows
    $after = Snap 'after-explorer-restart'
    $cloakedNow = @($after | Where-Object { $_.Cloaked -ne 0 }).Count
    $parkedNow = @($after | Where-Object { $_.X -le -29000 -and $_.Y -le -29000 }).Count
    Check 'shell cloaking still works after Explorer restarts' ($cloakedNow -ge 1 -and $parkedNow -eq 0) ("cloaked={0} parked={1}" -f $cloakedNow, $parkedNow)
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
