# INC-49 Pre-UAT One-Shot Operator Checklist

Owner scope: collect closure evidence for `INC-49-1`, `INC-49-4`, and `INC-49-T1` without destructive recovery steps.

## 1) Task Mapping (Direct)

| Task ID | Evidence source in one-shot script |
|---|---|
| `INC-49-1` | Scenario 16 result + command logs from `tools/inc49/run-preuat-evidence.ps1` |
| `INC-49-4` | Scenario 17 + Scenario 18 results + command logs from `tools/inc49/run-preuat-evidence.ps1` |
| `INC-49-T1` | Scenario 16 checks confirming terminal/editor focus recovery |

## 2) Safety Rules (Mandatory)

- [ ] Non-destructive recovery first: `panic-revert` before disruptive actions.
- [ ] Do not use reboot, sign-out, Explorer restart, or Task Manager force-kill unless the scenario already failed.
- [ ] If any scenario fails, stop and preserve generated evidence artifacts.
- [ ] `status` must indicate stopped before rerunning in safe mode.

## 3) Prerequisites

- [ ] Real Windows desktop host.
- [ ] Repo root terminal at `C:\dev\OpenNiri-Windows`.
- [ ] `openniri-cli` is available in `PATH` (or pass `-CliPath` explicitly).
- [ ] Two terminals available (Scenario 18 needs one dedicated long-running copy terminal).
- [ ] Tray area visible (including hidden icons).

## 4) One Command (Copy/Paste)

```powershell
pwsh -NoProfile -File tools/inc49/run-preuat-evidence.ps1
```

What the script does automatically:

- Captures preflight evidence (`git status --porcelain`, `git rev-parse --short HEAD`, `openniri-cli status`).
- Runs `cargo +stable-x86_64-pc-windows-gnu build --release` and fails fast if build fails.
- Executes Scenario 16 command flow (`run`, `apply`, `panic-revert`, `status`, safe-mode run/stop/status) and prompts for recovery checks.
- Executes Scenario 17 command flow with tray action prompts and safe-mode rerun checks.
- Executes Scenario 18 command flow (`run`, `apply`, `panic-revert`, `status`, safe-mode run/stop/status) and prompts for long-copy continuity checks.
- Collects post-run logs (`collect-logs`, daemon log tails, crash file listing).
- Tries to copy latest screenshot into evidence directory (if finder script is available).

## 5) Prompts You Must Answer During Run

- Scenario 16:
- Confirm keyboard recovery sequence restored terminal/editor focus without reboot.
- Confirm whether any destructive recovery was used.
- Scenario 17:
- Confirm tray emergency uncloak worked.
- Confirm tray Exit was used. The script verifies daemon-stop state via captured `status` checks before safe-mode rerun.
- Confirm whether any destructive recovery was required.
- Scenario 18:
- Start long-running copy in another terminal before continuing.
- Confirm copy continued/completed through recovery.
- Confirm keyboard recovery worked and no destructive recovery was required.

## 6) Evidence Artifacts (Required)

The script prints:

- `INC-49 evidence captured: <full_path>`
- `Report: <full_path>\report.md`
- `Summary: <full_path>\summary.json`
- `Gate status: PASS|FAIL`

Required files in the printed directory:

- `report.md`
- `summary.json`
- `commands\*.log`
- optional `latest-screenshot.<ext>`

## 7) Tracker Updates After PASS

Update only when gate status is `PASS`:

- [ ] `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`
- Set `INC-49-1`, `INC-49-4`, and `INC-49-T1` to `done`.
- Add exact evidence paths to `report.md` and `summary.json`.

- [ ] `docs/1_Progress and review/OPEN_ITEMS.md`
- Move `INC-49-1`, `INC-49-4`, and `INC-49-T1` from active to completed with evidence paths.

- [ ] `docs/1_Progress and review/ITERATION_LOG.md`
- Add iteration entry with date, command run, scenario outcomes, and evidence directory path.

- [ ] `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md`
- Close incident only after all three IDs are done and evidence is linked.

## 8) Completion Gate

All must be true:

- [ ] Scenario 16 passed and mapped to `INC-49-1` and `INC-49-T1`.
- [ ] Scenario 17 and Scenario 18 passed and mapped to `INC-49-4`.
- [ ] Evidence directory contains `report.md`, `summary.json`, and `commands\*.log`.
- [ ] Tracker files are updated with concrete evidence paths.
