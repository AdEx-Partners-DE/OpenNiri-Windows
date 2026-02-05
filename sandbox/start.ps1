$repo = "$env:USERPROFILE\Desktop\OpenNiri-Windows"
$cli = Join-Path $repo "target\release\openniri-cli.exe"
$daemon = Join-Path $repo "target\release\openniri.exe"

Write-Host "OpenNiri Sandbox startup"
Write-Host "Repo: $repo"

if (!(Test-Path $cli) -or !(Test-Path $daemon)) {
    Write-Host "Release binaries not found. On the host, run: cargo build --release" -ForegroundColor Yellow
    Write-Host "Expected: $cli" -ForegroundColor Yellow
    Write-Host "Expected: $daemon" -ForegroundColor Yellow
    Pause
    exit 1
}

# Open a couple of windows so the WM has something to manage
Start-Process notepad.exe
Start-Process explorer.exe
Start-Sleep -Seconds 1

# One-command start + apply
& $cli run

Write-Host ""
Write-Host "Daemon started. Try:"
Write-Host "  $cli focus left"
Write-Host "  $cli focus right"
Write-Host "  $cli scroll left --pixels 200"
Write-Host "  $cli scroll right --pixels 200"
Write-Host "  $cli stop"
Write-Host ""
Write-Host "Logs: $env:TEMP\openniri-daemon.log"
Write-Host "Logs: $env:TEMP\openniri-daemon.err.log"
Pause
