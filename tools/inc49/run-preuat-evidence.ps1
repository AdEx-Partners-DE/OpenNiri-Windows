[CmdletBinding()]
param(
    [string]$CliPath = "openniri-cli",
    [int]$StartupWaitSeconds = 4,
    [int]$CommandTimeoutSeconds = 30
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ($PSVersionTable.PSVersion.Major -ge 7) {
    $PSNativeCommandUseErrorActionPreference = $false
}

function New-RequiredDirectory {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -ItemType Directory -Path $Path -Force | Out-Null
    }
}

function Convert-ToEvidenceRelativePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Root,
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $rootWithSlash = $Root.TrimEnd("\", "/") + "\"
    if ($Path.StartsWith($rootWithSlash, [System.StringComparison]::OrdinalIgnoreCase)) {
        return ($Path.Substring($rootWithSlash.Length) -replace "\\", "/")
    }

    return ($Path -replace "\\", "/")
}

function Get-FilePreview {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [int]$MaxLines = 20
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return ""
    }

    $lines = @(Get-Content -LiteralPath $Path)
    if ($lines.Count -eq 0) {
        return ""
    }

    if ($lines.Count -le $MaxLines) {
        return ($lines -join [Environment]::NewLine)
    }

    $preview = $lines[0..($MaxLines - 1)] -join [Environment]::NewLine
    return "$preview$([Environment]::NewLine)...(truncated)..."
}

function Resolve-CliExecutable {
    param(
        [Parameter(Mandatory = $true)]
        [string]$NameOrPath
    )

    $resolved = Get-Command -Name $NameOrPath -ErrorAction SilentlyContinue
    if ($null -eq $resolved) {
        throw "Unable to find '$NameOrPath'. Ensure OpenNiri CLI is installed and available in PATH."
    }

    return $resolved.Source
}

function Start-CapturedRunCheck {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,
        [Parameter(Mandatory = $true)]
        [string]$CliExecutable,
        [Parameter(Mandatory = $true)]
        [string]$CommandLogDirectory,
        [Parameter(Mandatory = $true)]
        [string]$EvidenceDirectory,
        [Parameter(Mandatory = $true)]
        [int]$StartupWaitSeconds
    )

    $stdoutFile = Join-Path $CommandLogDirectory "$Name.stdout.log"
    $stderrFile = Join-Path $CommandLogDirectory "$Name.stderr.log"
    $commandText = "$CliExecutable $($Arguments -join ' ')"

    $startedAt = Get-Date
    $process = Start-Process -FilePath $CliExecutable `
        -ArgumentList $Arguments `
        -NoNewWindow `
        -PassThru `
        -RedirectStandardOutput $stdoutFile `
        -RedirectStandardError $stderrFile

    Start-Sleep -Seconds $StartupWaitSeconds
    $checkedAt = Get-Date

    $runningAfterWait = $false
    $exitCode = $null
    try {
        $process.Refresh()
        $runningAfterWait = -not $process.HasExited
        if ($process.HasExited) {
            $exitCode = $process.ExitCode
        }
    }
    catch {
        $runningAfterWait = $false
    }

    return [pscustomobject]@{
        name                       = $Name
        command                    = $commandText
        started_at                 = $startedAt.ToString("o")
        completed_at               = $checkedAt.ToString("o")
        timed_out                  = $false
        exit_code                  = $exitCode
        process_id                 = $process.Id
        process_running_after_wait = $runningAfterWait
        stdout_file                = Convert-ToEvidenceRelativePath -Root $EvidenceDirectory -Path $stdoutFile
        stderr_file                = Convert-ToEvidenceRelativePath -Root $EvidenceDirectory -Path $stderrFile
        stdout_preview             = Get-FilePreview -Path $stdoutFile
        stderr_preview             = Get-FilePreview -Path $stderrFile
        note                       = "Run-start check only; no process termination performed by this script."
    }
}

function Invoke-CapturedCliCommand {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,
        [Parameter(Mandatory = $true)]
        [string]$CliExecutable,
        [Parameter(Mandatory = $true)]
        [string]$CommandLogDirectory,
        [Parameter(Mandatory = $true)]
        [string]$EvidenceDirectory,
        [Parameter(Mandatory = $true)]
        [int]$TimeoutSeconds
    )

    $stdoutFile = Join-Path $CommandLogDirectory "$Name.stdout.log"
    $stderrFile = Join-Path $CommandLogDirectory "$Name.stderr.log"
    $commandText = "$CliExecutable $($Arguments -join ' ')"

    $startedAt = Get-Date
    $process = Start-Process -FilePath $CliExecutable `
        -ArgumentList $Arguments `
        -NoNewWindow `
        -PassThru `
        -RedirectStandardOutput $stdoutFile `
        -RedirectStandardError $stderrFile

    $timeoutMs = [Math]::Max(1, $TimeoutSeconds) * 1000
    $completedInTime = $process.WaitForExit($timeoutMs)
    $completedAt = Get-Date

    $timedOut = -not $completedInTime
    $exitCode = $null
    if (-not $timedOut) {
        $exitCode = $process.ExitCode
    }

    return [pscustomobject]@{
        name                       = $Name
        command                    = $commandText
        started_at                 = $startedAt.ToString("o")
        completed_at               = $completedAt.ToString("o")
        timed_out                  = $timedOut
        exit_code                  = $exitCode
        process_id                 = $process.Id
        process_running_after_wait = (-not $process.HasExited)
        stdout_file                = Convert-ToEvidenceRelativePath -Root $EvidenceDirectory -Path $stdoutFile
        stderr_file                = Convert-ToEvidenceRelativePath -Root $EvidenceDirectory -Path $stderrFile
        stdout_preview             = Get-FilePreview -Path $stdoutFile
        stderr_preview             = Get-FilePreview -Path $stderrFile
        note                       = if ($timedOut) { "Command timed out. No process termination performed by this script." } else { "" }
    }
}

function Read-PassFailResult {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Prompt
    )

    while ($true) {
        $raw = (Read-Host -Prompt $Prompt).Trim().ToLowerInvariant()
        switch ($raw) {
            "pass" { return "pass" }
            "p" { return "pass" }
            "fail" { return "fail" }
            "f" { return "fail" }
            default {
                Write-Host "Please enter 'pass' or 'fail'."
            }
        }
    }
}

function Read-YesNoResult {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Prompt
    )

    while ($true) {
        $raw = (Read-Host -Prompt $Prompt).Trim().ToLowerInvariant()
        switch ($raw) {
            "yes" { return $true }
            "y" { return $true }
            "no" { return $false }
            "n" { return $false }
            default {
                Write-Host "Please enter 'yes' or 'no'."
            }
        }
    }
}

function Test-StatusIndicatesStopped {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$CommandRecord
    )

    if ($null -eq $CommandRecord) {
        return $false
    }

    $output = "{0}`n{1}" -f $CommandRecord.stdout_preview, $CommandRecord.stderr_preview
    if ($output -match "(?i)daemon is not running|not running|timeout") {
        return $true
    }

    if (($null -ne $CommandRecord.exit_code) -and ($CommandRecord.exit_code -ne 0)) {
        return $true
    }

    return $false
}

function Try-DiscoverLatestScreenshot {
    param(
        [Parameter(Mandatory = $true)]
        [string]$EvidenceDirectory
    )

    $finderScript = "C:\dev\0_repo_overarching\scripts\portfolio\find-latest-screenshot.ps1"
    $result = [ordered]@{
        attempted   = $false
        found       = $false
        finder      = $finderScript
        source_path = $null
        copied_path = $null
        message     = $null
    }

    if (-not (Test-Path -LiteralPath $finderScript -PathType Leaf)) {
        $result.message = "Finder script not present; screenshot discovery skipped."
        return [pscustomobject]$result
    }

    $pwsh = Get-Command -Name "pwsh" -ErrorAction SilentlyContinue
    if ($null -eq $pwsh) {
        $result.message = "pwsh not available; screenshot discovery skipped."
        return [pscustomobject]$result
    }

    $result.attempted = $true

    try {
        $rawOutput = & $pwsh.Source -NoProfile -File $finderScript -MaxAgeMinutes 10 2>&1
        $lines = @($rawOutput | ForEach-Object { "$_".Trim() } | Where-Object { $_ -ne "" })

        $candidate = $null
        foreach ($line in $lines) {
            if (Test-Path -LiteralPath $line -PathType Leaf) {
                $candidate = $line
                continue
            }

            $regex = [regex]'([A-Za-z]:\\[^:*?"<>|]+?\.(png|jpg|jpeg|bmp))'
            $match = $regex.Match($line)
            if ($match.Success) {
                $possiblePath = $match.Groups[1].Value
                if (Test-Path -LiteralPath $possiblePath -PathType Leaf) {
                    $candidate = $possiblePath
                }
            }
        }

        if ($null -eq $candidate) {
            $result.message = "Finder script ran, but no screenshot path was detected."
            return [pscustomobject]$result
        }

        $extension = [System.IO.Path]::GetExtension($candidate)
        if ([string]::IsNullOrWhiteSpace($extension)) {
            $extension = ".png"
        }

        $copiedPath = Join-Path $EvidenceDirectory "latest-screenshot$extension"
        Copy-Item -LiteralPath $candidate -Destination $copiedPath -Force

        $result.found = $true
        $result.source_path = $candidate
        $result.copied_path = $copiedPath
        $result.message = "Latest screenshot copied into evidence directory."
        return [pscustomobject]$result
    }
    catch {
        $result.message = "Screenshot discovery failed: $($_.Exception.Message)"
        return [pscustomobject]$result
    }
}

try {
    if ($StartupWaitSeconds -lt 1) {
        throw "StartupWaitSeconds must be >= 1."
    }
    if ($CommandTimeoutSeconds -lt 1) {
        throw "CommandTimeoutSeconds must be >= 1."
    }

    $repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
    $timestamp = Get-Date -Format "yyyyMMdd-HHmmss"

    $evidenceBaseDir = Join-Path $repoRoot "docs\1_Progress and review\evidence\inc49"
    $evidenceDir = Join-Path $evidenceBaseDir $timestamp
    $commandDir = Join-Path $evidenceDir "commands"

    New-RequiredDirectory -Path $evidenceDir
    New-RequiredDirectory -Path $commandDir

    $cliExecutable = Resolve-CliExecutable -NameOrPath $CliPath
    $gitExecutable = Resolve-CliExecutable -NameOrPath "git"
    $cargoExecutable = Resolve-CliExecutable -NameOrPath "cargo"
    $pwshCommand = Get-Command -Name "pwsh" -ErrorAction SilentlyContinue
    $pwshExecutable = if ($null -ne $pwshCommand) { $pwshCommand.Source } else { $null }

    $commandRecords = New-Object "System.Collections.Generic.List[object]"
    $scenarioResults = New-Object "System.Collections.Generic.List[object]"

    Write-Host "Running preflight captures (git, status, release build)..."
    $commandRecords.Add((Invoke-CapturedCliCommand -Name "00-git-status-porcelain" -Arguments @("status", "--porcelain") -CliExecutable $gitExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))
    $commandRecords.Add((Invoke-CapturedCliCommand -Name "01-git-rev-parse-head" -Arguments @("rev-parse", "--short", "HEAD") -CliExecutable $gitExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))
    $commandRecords.Add((Invoke-CapturedCliCommand -Name "02-preflight-status" -Arguments @("status") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))

    $buildRecord = Invoke-CapturedCliCommand -Name "03-release-build" -Arguments @("+stable-x86_64-pc-windows-gnu", "build", "--release") -CliExecutable $cargoExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds 3600
    $commandRecords.Add($buildRecord)
    if ($buildRecord.timed_out -or ($null -ne $buildRecord.exit_code -and $buildRecord.exit_code -ne 0)) {
        throw "Release build failed or timed out. See commands/03-release-build.*.log"
    }

    Write-Host "Scenario 16: Focus-Lockout Recovery (INC-49-1 / INC-49-T1)"
    Write-Host "Reproduce lockout path when prompted, then continue with panic-revert as first recovery step."
    Read-Host -Prompt "Press Enter to execute Scenario 16 command sequence"

    $commandRecords.Add((Start-CapturedRunCheck -Name "10-s16-run" -Arguments @("run") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -StartupWaitSeconds $StartupWaitSeconds))
    $commandRecords.Add((Invoke-CapturedCliCommand -Name "11-s16-apply" -Arguments @("apply") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))
    $commandRecords.Add((Invoke-CapturedCliCommand -Name "12-s16-panic-revert" -Arguments @("panic-revert") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))
    $s16StatusAfterPanic = Invoke-CapturedCliCommand -Name "13-s16-status-after-panic" -Arguments @("status") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds
    $commandRecords.Add($s16StatusAfterPanic)
    $s16StoppedBeforeSafe = Test-StatusIndicatesStopped -CommandRecord $s16StatusAfterPanic

    $s16KeyboardRecovery = Read-YesNoResult -Prompt "Scenario 16: keyboard recovery (Win+Ctrl+Shift+B, Alt+Esc, Win+Tab click) restored terminal/editor focus without reboot? [yes/no]"
    $s16UsedDestructive = Read-YesNoResult -Prompt "Scenario 16: did you use reboot/sign-out/Explorer restart/Task Manager kill? [yes/no]"

    $commandRecords.Add((Start-CapturedRunCheck -Name "14-s16-safe-run" -Arguments @("run", "--safe-mode") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -StartupWaitSeconds $StartupWaitSeconds))
    $commandRecords.Add((Invoke-CapturedCliCommand -Name "15-s16-safe-stop" -Arguments @("stop") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))
    $s16StatusAfterStop = Invoke-CapturedCliCommand -Name "16-s16-safe-status-after-stop" -Arguments @("status") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds
    $commandRecords.Add($s16StatusAfterStop)

    $s16Passed = $s16StoppedBeforeSafe -and $s16KeyboardRecovery -and (-not $s16UsedDestructive) -and (Test-StatusIndicatesStopped -CommandRecord $s16StatusAfterStop)
    $s16Notes = Read-Host -Prompt "Scenario 16 notes"
    $scenarioResults.Add([pscustomobject]@{
            scenario_id    = 16
            scenario_title = "Focus-Lockout Recovery"
            result         = if ($s16Passed) { "pass" } else { "fail" }
            checks         = [ordered]@{
                status_stopped_before_safe_mode = $s16StoppedBeforeSafe
                keyboard_recovery_worked        = $s16KeyboardRecovery
                used_destructive_recovery       = $s16UsedDestructive
                status_stopped_after_safe_stop  = (Test-StatusIndicatesStopped -CommandRecord $s16StatusAfterStop)
            }
            notes          = $s16Notes
            captured_at    = (Get-Date).ToString("o")
        })

    Write-Host "Scenario 17: Tray Emergency Uncloak + Tray Exit (INC-49-4)"
    Write-Host "Keep recovery non-destructive. Use tray emergency action first when terminal becomes unavailable."
    Read-Host -Prompt "Press Enter to execute Scenario 17 command sequence"

    $commandRecords.Add((Start-CapturedRunCheck -Name "20-s17-run" -Arguments @("run") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -StartupWaitSeconds $StartupWaitSeconds))
    $s17TrayEmergencyWorked = Read-YesNoResult -Prompt "Scenario 17: tray 'Emergency: Uncloak All Windows' restored visibility? [yes/no]"
    $s17TrayExitUsed = Read-YesNoResult -Prompt "Scenario 17: tray 'Exit' was used to terminate daemon? [yes/no]"
    $s17StatusAfterTray = Invoke-CapturedCliCommand -Name "21-s17-status-after-tray-exit" -Arguments @("status") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds
    $commandRecords.Add($s17StatusAfterTray)
    $s17StoppedBeforeSafe = Test-StatusIndicatesStopped -CommandRecord $s17StatusAfterTray

    $commandRecords.Add((Start-CapturedRunCheck -Name "22-s17-safe-run" -Arguments @("run", "--safe-mode") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -StartupWaitSeconds $StartupWaitSeconds))
    $commandRecords.Add((Invoke-CapturedCliCommand -Name "23-s17-safe-stop" -Arguments @("stop") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))
    $s17StatusAfterStop = Invoke-CapturedCliCommand -Name "24-s17-safe-status-after-stop" -Arguments @("status") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds
    $commandRecords.Add($s17StatusAfterStop)

    $s17UsedDestructive = Read-YesNoResult -Prompt "Scenario 17: did recovery require reboot/sign-out/Explorer restart/Task Manager kill? [yes/no]"
    $s17Passed = $s17TrayEmergencyWorked -and $s17TrayExitUsed -and $s17StoppedBeforeSafe -and (-not $s17UsedDestructive) -and (Test-StatusIndicatesStopped -CommandRecord $s17StatusAfterStop)
    $s17Notes = Read-Host -Prompt "Scenario 17 notes"
    $scenarioResults.Add([pscustomobject]@{
            scenario_id    = 17
            scenario_title = "Tray Emergency Uncloak Path"
            result         = if ($s17Passed) { "pass" } else { "fail" }
            checks         = [ordered]@{
                tray_emergency_uncloak_worked   = $s17TrayEmergencyWorked
                tray_exit_used                  = $s17TrayExitUsed
                status_stopped_before_safe_mode = $s17StoppedBeforeSafe
                used_destructive_recovery       = $s17UsedDestructive
                status_stopped_after_safe_stop  = (Test-StatusIndicatesStopped -CommandRecord $s17StatusAfterStop)
            }
            notes          = $s17Notes
            captured_at    = (Get-Date).ToString("o")
        })

    Write-Host "Scenario 18: Long-Running File Operation Continuity (INC-49-4)"
    Write-Host "Start copy in terminal A first, then continue here. Keep panic-revert as first recovery action."
    Read-Host -Prompt "Press Enter once long-running file copy is active in another terminal"

    $commandRecords.Add((Start-CapturedRunCheck -Name "30-s18-run" -Arguments @("run") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -StartupWaitSeconds $StartupWaitSeconds))
    $commandRecords.Add((Invoke-CapturedCliCommand -Name "31-s18-apply" -Arguments @("apply") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))
    $commandRecords.Add((Invoke-CapturedCliCommand -Name "32-s18-panic-revert" -Arguments @("panic-revert") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))
    $s18StatusAfterPanic = Invoke-CapturedCliCommand -Name "33-s18-status-after-panic" -Arguments @("status") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds
    $commandRecords.Add($s18StatusAfterPanic)
    $s18StoppedBeforeSafe = Test-StatusIndicatesStopped -CommandRecord $s18StatusAfterPanic

    $s18CopyContinued = Read-YesNoResult -Prompt "Scenario 18: long-running copy continued or completed without interruption? [yes/no]"
    $s18KeyboardRecovery = Read-YesNoResult -Prompt "Scenario 18: keyboard recovery sequence restored access to terminals? [yes/no]"
    $s18UsedDestructive = Read-YesNoResult -Prompt "Scenario 18: did recovery require reboot/sign-out/Explorer restart/Task Manager kill? [yes/no]"

    $commandRecords.Add((Start-CapturedRunCheck -Name "34-s18-safe-run" -Arguments @("run", "--safe-mode") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -StartupWaitSeconds $StartupWaitSeconds))
    $commandRecords.Add((Invoke-CapturedCliCommand -Name "35-s18-safe-stop" -Arguments @("stop") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))
    $s18StatusAfterStop = Invoke-CapturedCliCommand -Name "36-s18-safe-status-after-stop" -Arguments @("status") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds
    $commandRecords.Add($s18StatusAfterStop)

    $s18Passed = $s18StoppedBeforeSafe -and $s18CopyContinued -and $s18KeyboardRecovery -and (-not $s18UsedDestructive) -and (Test-StatusIndicatesStopped -CommandRecord $s18StatusAfterStop)
    $s18Notes = Read-Host -Prompt "Scenario 18 notes (include copy source/destination summary)"
    $scenarioResults.Add([pscustomobject]@{
            scenario_id    = 18
            scenario_title = "Long-Running File Operation Continuity"
            result         = if ($s18Passed) { "pass" } else { "fail" }
            checks         = [ordered]@{
                status_stopped_before_safe_mode = $s18StoppedBeforeSafe
                copy_continued_or_completed     = $s18CopyContinued
                keyboard_recovery_worked        = $s18KeyboardRecovery
                used_destructive_recovery       = $s18UsedDestructive
                status_stopped_after_safe_stop  = (Test-StatusIndicatesStopped -CommandRecord $s18StatusAfterStop)
            }
            notes          = $s18Notes
            captured_at    = (Get-Date).ToString("o")
        })

    Write-Host "Collecting post-run evidence logs..."
    $commandRecords.Add((Invoke-CapturedCliCommand -Name "90-collect-logs" -Arguments @("collect-logs") -CliExecutable $cliExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))

    if ($null -ne $pwshExecutable) {
        $daemonLogPath = Join-Path $env:TEMP "openniri-daemon.log"
        $daemonErrLogPath = Join-Path $env:TEMP "openniri-daemon.err.log"
        $crashPattern = Join-Path $env:TEMP "openniri-crash-*.txt"

        $commandRecords.Add((Invoke-CapturedCliCommand -Name "91-tail-daemon-log" -Arguments @("-NoProfile", "-Command", "if (Test-Path -LiteralPath '$daemonLogPath') { Get-Content -LiteralPath '$daemonLogPath' -Tail 200 } else { 'daemon log not found' }") -CliExecutable $pwshExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))
        $commandRecords.Add((Invoke-CapturedCliCommand -Name "92-tail-daemon-err-log" -Arguments @("-NoProfile", "-Command", "if (Test-Path -LiteralPath '$daemonErrLogPath') { Get-Content -LiteralPath '$daemonErrLogPath' -Tail 200 } else { 'daemon err log not found' }") -CliExecutable $pwshExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))
        $commandRecords.Add((Invoke-CapturedCliCommand -Name "93-list-recent-crash-files" -Arguments @("-NoProfile", "-Command", "Get-ChildItem -Path '$crashPattern' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 3 FullName,LastWriteTime,Length | Format-Table -AutoSize") -CliExecutable $pwshExecutable -CommandLogDirectory $commandDir -EvidenceDirectory $evidenceDir -TimeoutSeconds $CommandTimeoutSeconds))
    }

    $screenshot = Try-DiscoverLatestScreenshot -EvidenceDirectory $evidenceDir
    $failedScenarios = @($scenarioResults | Where-Object { $_.result -eq "fail" })
    $gatePass = $failedScenarios.Count -eq 0

    $summary = [ordered]@{
        generated_at_utc = (Get-Date).ToUniversalTime().ToString("o")
        evidence_dir     = $evidenceDir
        cli_executable   = $cliExecutable
        testing_guide    = "docs/TESTING_GUIDE.md"
        gate_pass        = $gatePass
        commands         = @($commandRecords)
        scenarios        = @($scenarioResults)
        screenshot       = $screenshot
    }

    $summaryJsonPath = Join-Path $evidenceDir "summary.json"
    $reportPath = Join-Path $evidenceDir "report.md"
    $summary | ConvertTo-Json -Depth 9 | Set-Content -LiteralPath $summaryJsonPath -Encoding UTF8

    $reportLines = New-Object "System.Collections.Generic.List[string]"
    $reportLines.Add("# INC-49 Pre-UAT Evidence")
    $reportLines.Add("")
    $reportLines.Add("- Generated (UTC): $($summary.generated_at_utc)")
    $reportLines.Add("- Evidence Directory: $($summary.evidence_dir)")
    $reportLines.Add("- CLI Executable: $($summary.cli_executable)")
    $reportLines.Add("- Testing Guide Reference: $($summary.testing_guide)")
    $reportLines.Add("- Gate Result: $(if ($gatePass) { 'PASS' } else { 'FAIL' })")
    $reportLines.Add("")
    $reportLines.Add("## Command Captures")
    $reportLines.Add("")

    foreach ($record in $commandRecords) {
        $exitText = if ($null -ne $record.exit_code) { "$($record.exit_code)" } else { "n/a" }
        $reportLines.Add("### $($record.name)")
        $reportLines.Add("- Command: $($record.command)")
        $reportLines.Add("- Started: $($record.started_at)")
        $reportLines.Add("- Completed: $($record.completed_at)")
        $reportLines.Add("- Exit Code: $exitText")
        $reportLines.Add("- Timed Out: $($record.timed_out)")
        $reportLines.Add("- PID: $($record.process_id)")
        $reportLines.Add("- Running After Wait: $($record.process_running_after_wait)")
        $reportLines.Add("- Stdout Log: $($record.stdout_file)")
        $reportLines.Add("- Stderr Log: $($record.stderr_file)")
        if (-not [string]::IsNullOrWhiteSpace($record.note)) {
            $reportLines.Add("- Note: $($record.note)")
        }
        $reportLines.Add("")
    }

    $reportLines.Add("## Scenario Results (TESTING_GUIDE 16/17/18)")
    $reportLines.Add("")
    foreach ($scenario in $scenarioResults) {
        $reportLines.Add("### Scenario $($scenario.scenario_id): $($scenario.scenario_title)")
        $reportLines.Add("- Result: $($scenario.result)")
        foreach ($check in $scenario.checks.GetEnumerator()) {
            $reportLines.Add("- $($check.Key): $($check.Value)")
        }
        $reportLines.Add("- Notes: $($scenario.notes)")
        $reportLines.Add("- Captured At: $($scenario.captured_at)")
        $reportLines.Add("")
    }

    $reportLines.Add("## Screenshot Discovery")
    $reportLines.Add("")
    $reportLines.Add("- Attempted: $($screenshot.attempted)")
    $reportLines.Add("- Found: $($screenshot.found)")
    if ($null -ne $screenshot.source_path) {
        $reportLines.Add("- Source Screenshot: $($screenshot.source_path)")
    }
    if ($null -ne $screenshot.copied_path) {
        $copiedRel = Convert-ToEvidenceRelativePath -Root $evidenceDir -Path $screenshot.copied_path
        $reportLines.Add("- Copied Screenshot: $copiedRel")
    }
    if ($null -ne $screenshot.message) {
        $reportLines.Add("- Message: $($screenshot.message)")
    }
    $reportLines.Add("")
    $reportLines.Add("## Artifacts")
    $reportLines.Add("")
    $reportLines.Add("- JSON Summary: summary.json")
    $reportLines.Add("- Markdown Report: report.md")
    $reportLines.Add("- Command Logs: commands/*.log")
    $reportLines | Set-Content -LiteralPath $reportPath -Encoding UTF8

    Write-Host "INC-49 evidence captured: $evidenceDir"
    Write-Host "Report: $reportPath"
    Write-Host "Summary: $summaryJsonPath"
    Write-Host "Gate status: $(if ($gatePass) { 'PASS' } else { 'FAIL' })"
}
catch {
    throw "INC-49 pre-UAT evidence capture failed: $($_.Exception.Message)"
}
