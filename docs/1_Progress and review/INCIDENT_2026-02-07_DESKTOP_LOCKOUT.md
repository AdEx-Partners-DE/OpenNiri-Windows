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

## Release Policy Effect

No signed binaries, tagged prerelease, or publication work should proceed until this incident class is reproducibly mitigated and documented as verified closed.

## Links

- `docs/1_Progress and review/OPEN_ITEMS.md`
- `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `docs/1_Progress and review/ITERATION_LOG.md`
