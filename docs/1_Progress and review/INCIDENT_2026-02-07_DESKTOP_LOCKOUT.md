# Incident Report: Desktop Lockout During Local Apply Test

Date: 2026-02-07  
Severity: Critical  
Status: Open (unresolved)  
Scope: Local desktop usability/safety regression during manual user test

## Summary

A local manual test reported severe desktop usability failure immediately after running `openniri-cli apply`.  
The user experienced loss of practical desktop control (unable to reliably return to terminal windows via Alt+Tab/Win+Tab despite visible thumbnails), severe temporary mouse lag, and inability to recover with normal in-session guidance.  

An attempted shell-level recovery sequence later included an Explorer restart, which interrupted an unrelated long-running file copy job (4 TB, approximately 60% complete at interruption time).

## User-Reported Symptoms

1. Immediately after `apply`, desktop behavior became unstable/"jumping" and unusable.
2. Existing terminal windows were visible in switcher previews but could not be focused/recovered for practical use.
3. Mouse movement became extremely slow/unresponsive for approximately 30 seconds.
4. No in-product stuck-state guidance/tooltips were available to help exit safely.
5. Multiple recovery key paths (`Win+Ctrl+Shift+B`, `Alt+Esc`, `Win+Tab`) were attempted and acknowledged by the OS/UI but did not restore sustainable control.
6. Recovery guidance involving Explorer restart interrupted an unrelated long-running data copy operation.

## Reproduction Context (as Reported)

1. User followed local test steps through daemon start.
2. User executed `openniri-cli apply`.
3. Failure state began immediately after apply.
4. User could not access prior terminals to continue guided recovery.
5. Additional recovery attempts did not resolve focus/access lockout.

## Impact

1. Critical disruption to desktop workflow.
2. Loss of active working session continuity.
3. Interruption of long-running copy process.
4. User trust and release confidence severely damaged.

## What Failed

### Product-side

1. Safe-recovery path from a bad apply state was not robust enough in this user scenario.
2. Stuck-state UX signaling was insufficient (no obvious in-product bailout guidance).
3. Focus/window-switching behavior degraded into a state where standard shell recovery shortcuts were ineffective.

### Process-side

1. Recovery guidance did not sufficiently protect unrelated long-running operations before shell restarts.
2. Manual test protocol did not enforce explicit "non-destructive recovery first" ordering before disruptive shell actions.
3. Release-readiness framing remained too optimistic relative to this severity class of field failure.

## Immediate Containment Rules (Effective Now)

1. Treat any desktop lockout/focus-trap report as a release blocker.
2. Do not recommend Explorer restart without explicit warning that it can terminate/interrupt active file operations and shell-bound tasks.
3. Prefer non-destructive containment first:
   - hard stop daemon process
   - verify daemon/IPC is down
   - attempt non-shell-reset focus recovery
4. Any shell restart/reboot guidance must be clearly flagged as disruptive.

## Required Follow-Up (Blockers)

1. Add deterministic "panic revert" command path that restores all managed windows and exits without shell restart.
2. Add explicit stuck-state fallback UX/documentation surfaced from CLI (`doctor`/`health`/`collect-logs`) and support docs.
3. Add host-level manual test case for "apply causes unusable desktop" with pass/fail recovery criteria.
4. Add process guardrail: recovery runbook must preserve unrelated long-running tasks by default.
5. Re-run local acceptance on real desktop workloads before any tagging/publishing step.

## Additional Mitigations Landed (2026-02-08, Iteration 54)

1. CLI recovery UX now prints explicit timeout/non-running guidance for `apply`, `stop`, and `panic-revert`, including tray fallback.
2. Daemon shutdown cleanup now always attempts MoveOffScreen restoration and runs delayed multi-pass visibility recovery when timed-out apply workers are detected.
3. Apply-timeout path now triggers immediate best-effort visibility recovery and clears suppression state from failed/timed-out apply attempts.
4. Win32 placement path now protects foreground visibility before hiding offscreen windows and rolls back partial hide side effects on apply failure.
5. Hotkey registration now fails fast when zero shortcuts register, preventing “running without recovery controls” mode.
6. MoveOffScreen sentinel moved away from minimized-window coordinates to reduce false restore collisions.

## Additional Mitigations Landed (2026-02-08, Iteration 55)

1. Daemon apply-worker control now blocks overlapping `apply` executions while a timed-out worker is still running and retains timed-out worker handles for deterministic re-join attempts.
2. Daemon cleanup now performs recovery when late timed-out workers are reaped and schedules detached late-worker recovery passes when workers outlive shutdown retry windows.
3. Win32 foreground safety now treats `SetForegroundWindow` denial (`Ok(false)`) as a failed handoff and explicitly keeps the current foreground window visible (skip cloak/off-screen move) when handoff is not confirmed.
4. Win32 rollback now attempts foreground repair on visible windows after partial apply failures.
5. Hotkey registration is now all-or-nothing (`N/N` required) to prevent partial shortcut state.
6. CLI `run`/`apply` now share the same structured apply-recovery path, and unconfirmed `stop`/`panic-revert` outcomes now return non-zero to avoid false-success automation.
7. `doctor` now reports daemon-probe errors explicitly instead of silently classifying probe failures as "not running."

## Additional Field Evidence Update (2026-02-08, Post-Iteration 58 Follow-up)

1. User-reported follow-up still showed focus-trap behavior after local recovery attempts: terminal windows remained visible in switcher previews but were not practically focusable.
2. Recovery key sequence (`Win+Ctrl+Shift+B`, `Alt+Esc`, `Win+Tab`) produced UI response but did not restore sustainable terminal control in-session.
3. Report confirms that host-level closure evidence is still outstanding; incident remains open and release-blocking.

## Additional Field Evidence Update (2026-02-13, Host Pre-UAT Script Run)

1. Host run of `pwsh -NoProfile -File tools/inc49/run-preuat-evidence.ps1` progressed through initial prompts, then entered a window-management state where multiple terminal windows became inaccessible (visible as thumbnails but not focusable in practice).
2. Recovery evidence captured from administrator shell:
   - `openniri-cli emergency-uncloak` returned:
     - `[openniri] Emergency uncloak of all windows complete`
     - `Executed local emergency visibility restore (best-effort).`
     - `Recovery trigger: explicit emergency-uncloak request`
   - `openniri-cli stop` returned:
     - `[openniri] Emergency uncloak of all windows complete`
     - `Executed local emergency visibility restore (best-effort).`
     - `Recovery trigger: stop requested while daemon not running`
     - `Daemon not running.`
   - `openniri-cli status` returned:
     - `Error: Daemon is not running. Start it with \`openniri-cli run\`.`
3. Explorer shell reset was attempted (`Stop-Process -Name explorer -Force` + `Start-Process explorer.exe`), but practical access to previously affected terminals remained unresolved in-session.
4. Current operator plan is to wait for unrelated running processes to finish, then reboot host as final Windows-shell recovery step.
5. Incident status remains **Open** and **release-blocking**. Host closure evidence for `INC-49-1`, `INC-49-4`, and `INC-49-T1` is still pending.

## Technical Root-Cause Signals (2026-02-13 Log Correlation)

Evidence gathered from local OpenNiri artifacts and Windows event logs indicates a partial-apply/partial-recovery failure mode with concurrent shell/graphics instability:

1. Scenario flow reached and executed (not an early script abort):
   - `docs/1_Progress and review/evidence/inc49/20260213-114332/commands/10-s16-run.*`
   - `docs/1_Progress and review/evidence/inc49/20260213-114332/commands/11-s16-apply.*`
   - `docs/1_Progress and review/evidence/inc49/20260213-114332/commands/12-s16-panic-revert.*`
   - `docs/1_Progress and review/evidence/inc49/20260213-114332/commands/13-s16-status-after-panic.*`
2. `apply` returned non-success with high-volume Win32 side-effect failures:
   - `11-s16-apply.stderr.log` shows `apply_placements completed with side-effect failures` and repeated:
     - `DwmSetWindowAttribute(CLOAK=0/1) failed`
     - `SetWindowPos ... Access is denied (0x80070005)`
3. Safe-mode rerun still reproduced the same class of failures:
   - `14-s16-safe-run.stderr.log` repeats large batches of `DwmSetWindowAttribute` failures and off-screen move access-denied errors.
4. Daemon lifecycle behaved as expected (process-level recovery did occur):
   - `12-s16-panic-revert.stdout.log` => `OK`
   - `13-s16-status-after-panic.stderr.log` => daemon not running
   - `15-s16-safe-stop.stdout.log` => `OK`
   - `16-s16-safe-status-after-stop.stderr.log` => daemon not running
5. Daemon runtime log confirms same failure pattern at timestamp level:
   - `C:\Users\stark\AppData\Local\Temp\openniri-daemon.log`
   - At `2026-02-13T10:44:04Z`: many `Failed to uncloak window ... DwmSetWindowAttribute(CLOAK=0) failed`
   - At `2026-02-13T10:44:04Z`: multiple `Failed to move off-screen window ... Access is denied. (0x80070005)`
   - At shutdown (`~10:44:08Z`): repeated uncloak failures persisted.
6. Windows Error Reporting correlation shows shell/graphics distress in the same window:
   - Application log, provider `Windows Error Reporting`, event `1001`:
     - `Event Name: WindowsBlackScreenDiagnosticsV1`
     - Signatures include `P1: DWM`, `P4: Hotkey`; `P4: Hotkey_Explorer`; and `P1: RDP`
   - Observed around `2026-02-13 11:59` and `2026-02-13 12:12` during/after recovery attempts.

Interpretation:
1. This was not a simple daemon crash. The daemon hit broad Win32 operation failures, attempted emergency restore, then exited cleanly.
2. The remaining user-visible lockout aligns with an in-session Windows shell/graphics state problem after partial window operation failures.
3. Incident remains open until post-reboot rerun produces successful host-closure evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`).

## Additional Mitigations Landed (2026-02-08, Iteration 59)

1. Daemon pause/resume toggle now rolls back to paused state if resume-time layout apply fails, preventing false "resumed" status after failed re-apply.
2. Regression test now asserts failed resume keeps tiling paused (`test_toggle_pause_resume_reports_apply_failure`).
3. Recovery docs now explicitly state that `stop`/`panic-revert` target only `openniri.exe` and are not intended to terminate unrelated user apps.

## Additional Mitigations Landed (2026-02-09, Iteration 60)

1. CLI stop/panic recovery now explicitly treats IPC connect-timeout failures as unconfirmed shutdown/recovery states and runs local emergency visibility restore before returning.
2. Added regression coverage for IPC connect-timeout detection in CLI error-chain helpers.
3. Setup output hotkey wording now consistently uses `Win+Ctrl+Escape` to match config/docs wording.

## Additional Mitigations Landed (2026-02-09, Iteration 61)

1. WinEvent manageability filtering now applies only to create/location events; focus/foreground events are no longer dropped by cloaked-window checks.
2. This allows managed cloaked windows to emit focus/foreground signals so daemon recovery/layout logic can re-surface them instead of silently ignoring the transition.
3. Added/updated regression coverage for filter scope (`test_should_filter_window_event_by_manageability_scopes_to_create_and_move`).

## Additional Mitigations Landed (2026-02-09, Iteration 62)

1. CLI `apply` recovery now treats disconnected-before-response and connect-timeout races as unconfirmed completion and triggers local emergency visibility restore before returning actionable guidance.
2. CLI `stop` and `panic-revert` now trigger local emergency visibility restore when daemon responses are non-success (`error`/`unknown`) instead of exiting immediately without local fallback.
3. Daemon minimize/restore handling now clears fullscreen safety state when needed and ensures focused-window visibility before syncing/applying, reducing trapped-focus/offscreen risk after restore transitions.
4. Daemon delayed focus-follows-mouse dispatch now re-checks runtime config (`focus_follows_mouse`) at dispatch time to avoid stale delayed focus actions.
5. Win32 event emission policy now uses event-specific filtering so lifecycle/focus/foreground transitions are retained while still filtering non-actionable noise.
6. IPC server handling now enforces newline-terminated command framing and adds bounded outbound response-size fallback behavior for oversized responses.
7. Recovery docs were synchronized to one shutdown confirmation rule: `status` must fail (`timeout` OR `Daemon is not running...`) before any rerun, and incident flow now explicitly includes `Win+Ctrl+Escape`.

## Release Policy Effect

No signed binaries, tagged prerelease, or publication work should proceed until this incident class is reproducibly mitigated and documented as verified closed.

## Links

- `docs/1_Progress and review/OPEN_ITEMS.md`
- `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `docs/1_Progress and review/ITERATION_LOG.md`
