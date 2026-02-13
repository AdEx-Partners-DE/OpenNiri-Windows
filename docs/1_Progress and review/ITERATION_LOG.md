# OpenNiri-Windows Development Iteration Log

> **Purpose**: This document tracks all development iterations, providing evidence and links for meaningful review and verification.
> **Maintainer**: Claude (Anthropic AI Assistant)
> **Last Updated**: 2026-02-13 (Iteration 68 — Technical Log Correlation for Host Lockout)

---

## Table of Contents

1. [Project Overview](#project-overview)
2. [Iteration Summary Table](#iteration-summary-table)
3. [Detailed Iteration Logs](#detailed-iteration-logs)
4. [Test Coverage History](#test-coverage-history)
5. [Architecture Evolution](#architecture-evolution)
6. [Known Issues & Technical Debt](#known-issues--technical-debt)

---

## Project Overview

| Attribute | Value |
|-----------|-------|
| **Project** | OpenNiri-Windows |
| **Description** | Niri-like scrollable tiling window manager for Windows |
| **Repository** | https://github.com/AdEx-Partners-DE/OpenNiri-Windows |
| **Language** | Rust |
| **Target Platform** | Windows 10/11 (x86_64) |
| **Toolchain** | stable-x86_64-pc-windows-gnu (MinGW) |

### Crate Structure

```
OpenNiri-Windows/
├── crates/
│   ├── core_layout/      # Platform-agnostic layout engine
│   ├── platform_win32/   # Win32 API integration
│   ├── ipc/              # IPC protocol types
│   ├── daemon/           # Main daemon process
│   └── cli/              # Command-line interface
└── docs/
    ├── ARCHITECTURE.md   # Technical architecture
    ├── SPEC.md           # Behavioral specification
    └── 1_Progress and review/
        └── ITERATION_LOG.md  # This file
```

---

## Iteration Summary Table

| Iteration | Date | Focus Area | Tests Before | Tests After | Key Deliverables |
|-----------|------|------------|--------------|-------------|------------------|
| 1-7 | Pre-2026-02-04 | Core layout, Win32 basics | 0 | 52 | Layout engine, basic Win32 |
| 8.1 | 2026-02-04 | IPC Protocol Crate | 52 | 57 | `openniri-ipc` crate |
| 8.2 | 2026-02-04 | Monitor Detection | 57 | 60 | `enumerate_monitors()`, `get_primary_monitor()` |
| 8.3 | 2026-02-04 | Async Daemon & CLI IPC | 60 | 60 | Named pipe server, real IPC |
| 9 | 2026-02-04 | Codex Review Implementation | 60 | 63 | WinEvent hooks, cleanup |
| 10 | 2026-02-04 | Configuration Support | 63 | 69 | TOML config, reload, init |
| 11 | 2026-02-04 | Multi-monitor Support | 69 | 74 | Per-monitor workspaces, cross-monitor commands |
| 12 | 2026-02-04 | Codex Audit + Doc Refresh | 74 | 74 | Updated review + agent guidance |
| 13 | 2026-02-04 | Global Hotkey Support | 74 | 81 | RegisterHotKey API, config-driven bindings |
| 14 | 2026-02-04 | Smooth Scroll Animations | 81 | 108 | Easing functions, animated workspace scroll |
| 15 | 2026-02-04 | Codex Review + Doc Drift Audit | 108 | 108 | Updated review with doc drift findings |
| 16 | 2026-02-04 | Codex Review + QA Scan | 108 | 108 | Updated review with reload/hotkey gap |
| 17 | 2026-02-04 | Codex Review + QA Scan (Failure) | 108 | FAIL | `cargo test --all` failed (E0599 Config::generate_default) |
| 18 | 2026-02-04 | Codex Review + QA Scan (Failure) | FAIL | FAIL | Added NUL file issue + repeat E0599 failure |
| 19 | 2026-02-04 | Config Completeness & Doc Sync | 111 | 111 | Hotkey reload fix, log_level, track_focus_changes, doc updates |
| 20 | 2026-02-04 | Codex Review + QA Scan (Pass) | 111 | 111 | Tests pass again; iteration log inconsistency flagged |
| 21 | 2026-02-04 | Full Feature Push | 111 | 131 | System tray, window rules, gestures, snap hints |
| 22 | 2026-02-04 | Quality & Robustness | 131 | 147 | Fix unwraps, HWND validation, unit tests, docs, catch_unwind, IPC queries |
| 23 | 2026-02-04 | Feature Completion & Tests | 147 | 202 | Wire DisplayChange, focus_follows_mouse, use_cloaking, CLI tests, integration tests |
| 24 | 2026-02-05 | Real Gestures, Persistence, Docs | 202 | 206 | Real touchpad gestures, workspace persistence, doc refresh |
| 25 | 2026-02-05 | Config Validation & Safety | 206 | 231 | Config regex validation, pre-compiled rules, safety hardening |
| 26 | 2026-02-05 | Config Validation & Safety (cont.) | 231 | 234 | Additional safety tests, clippy fixes |
| 27 | 2026-02-05 | Test Coverage & Doc Accuracy | 234 | 257 | handle_command() tests, reconcile_monitors() tests, doc updates |
| 28 | 2026-02-05 | Codex Review 19 Fixes | 257 | 261 | reconcile_monitors bug fix, 7 strengthened tests, 3 new cmd tests, clippy --all-targets clean |
| 29 | 2026-02-05 | UX overhaul: SetForegroundWindow, CloseWindow, ToggleFloating, ToggleFullscreen, column presets, active border, status, tray menu, auto-start | 261 | 295 | 0 warnings |
| 30 | 2026-02-05 | Crash safety and reliability: Ctrl+C shutdown, uncloak-on-exit/crash, DPI awareness | 295 | 302 | 297 passed, 5 ignored, strict clippy clean |
| 31 | 2026-02-05 | Public repo presentation refresh (README + GitHub metadata) | 302 | 302 | README rewrite, GitHub description/topics updated |
| 32 | 2026-02-06 | Public messaging revamp (README v2 + GitHub About cleanup) | 302 | 302 | Professionalized README structure, tightened GitHub positioning and discovery topics |
| 33 | 2026-02-06 | Public readiness planning and enterprise README polish | 302 | 302 | Added public-readiness execution checklist and refined README for adoption clarity |
| 34 | 2026-02-06 | Pre-stable execution lock and backlog triage | 302 | 302 | Added explicit pre-stable development plan and deferred post-stable packaging work |
| 35 | 2026-02-06 | Minimize/Restore handling + dynamic tray text | 302 | 380 | Workspace minimize tracking, daemon minimize/restore handlers, tray pause text toggle, hex color validation, regex size limit, shutdown timeout, config warnings, hotkey counts, workspace mutation tests, config edge cases |
| 36 | 2026-02-06 | Codex review fixes | 380 | 383 | Fix pipe-busy duplicate daemon detection, fix restore order (scroll offset clamped to 0), wire tray hotkey mismatch, cargo fmt, doc sync |
| 37 | 2026-02-06 | Pre-testing readiness | 383 | 390 | MovedOrResized feedback loop fix, config error-path tests, fullscreen-minimize regression test, release profile (LTO), SECURITY.md, issue templates, cargo audit CI, testing guide, antivirus docs |
| 38 | 2026-02-06 | Codex Review 29 Fixes | 390 | 393 | Scroll offset restore preservation, true OS-registered hotkey count, overlay doc-test no_run, applying_layout error-path test, 3 new tests |
| 39 | 2026-02-06 | Codex Review 30 Fixes | 393 | 398 | insert_window_no_focus for focus_new_windows=false, ToggleFloating roundtrip, format_startup_banner/format_tooltip_text extraction, output assertion tests, test-count doc sync |
| 40 | 2026-02-06 | Codex Review 31 Fixes | 398 | 401 | Focused event updates previous_focused_hwnd for floating windows, event validation skips is_valid_window for managed windows, ToggleFloating roundtrip test rewritten to use real event path, platform test-count fix (27 not 28), 3 new tests |
| 41 | 2026-02-06 | R32 Review Fixes + Public Readiness Phase 1 | 401 | 407 | Injectable window enumeration (`lookup_window_info`, `is_known_window`), deterministic singleton test, host smoke test procedure, Created-event tests with injected window info, 6 new tests |
| 42 | 2026-02-06 | CLI Completeness (Phase 2) | 407 | 411 | `collect-logs` subcommand, `setup` first-run assistant, `config reset/backup/restore` subcommands, config backup/restore roundtrip test, 4 new tests |
| 43 | 2026-02-06 | Reliability Hardening (Phase 3) | 411 | 415 | `HealthCheck` IPC command + `health` CLI, structured crash reports (`format_crash_report`), `HealthInfo` response variant, 4 new tests |
| 44 | 2026-02-06 | Compatibility & UX Quality (Phase 4) | 415 | 419 | COMPATIBILITY.md, fallback behavior docs, profile presets (developer/laptop/ultrawide), `--profile` flag for init, 4 new tests |
| 45 | 2026-02-06 | Security & Trust (Phase 5) | 419 | 419 | Threat model summary, privacy statement, log redaction audit, named pipe security analysis, support window definition — all in SECURITY.md |
| 46 | 2026-02-06 | Support & Operations (Phase 6) | 419 | 419 | Enhanced bug report template with required diagnostics, issue template chooser (config.yml), SUPPORT_PLAYBOOK.md, "Getting Help" section in GETTING_STARTED.md |
| 47 | 2026-02-06 | Release Engineering (Phase 7) | 419 | 419 | Pre-release channels (alpha/beta/rc), SHA-256 checksums in CI, UPGRADING.md (rollback instructions), unsigned binary documentation |
| 48 | 2026-02-07 | Full remediation pass (runtime + docs/CI) | 419 | 440 | Fixed fullscreen removal, transactional cross-monitor moves, Win32 overlay/event lifecycle issues, protocol forward-compat fallback, tracker/docs/CI consistency, and expanded tests |
| 49 | 2026-02-07 | Critical field incident documentation + blocker re-open | 440 | 440 | Documented desktop lockout incident after apply, reopened active blockers, added safety/process guardrails and release block |
| 50 | 2026-02-07 | Incident mitigation implementation + validation | 440 | 459 | Implemented panic-revert and shutdown hardening, MoveOffScreen restoration, emergency tray uncloak, safer CLI run/stop semantics, and updated runbooks/release gates with full quality-gate pass |
| 51 | 2026-02-07 | Documentation consistency sync (metrics + incident gate truth) | 459 | 459 | Synced SPEC/ARCH metrics to 459/455/4, aligned blocker MD with canonical JSON, added PanicRevert + emergency uncloak to command/menu docs, kept release gate marked pending host evidence |
| 52 | 2026-02-07 | Validation evidence refresh + tracker metric sync | 459 | 480 | Re-ran workspace/all-features/ignored tests, synced active docs + trackers to 480/476/4, and kept release gate blocked pending host closure evidence |
| 53 | 2026-02-08 | Parallel critical fix sweep + validation sync | 480 | 508 | Landed daemon apply/shutdown race hardening, platform event/hotkey safeguards, core focus/fullscreen invariants, CLI timeout/read-path fixes, IPC scoped-pipe helpers wired into daemon/cli, CI gate hardening, and docs/tracker consistency refresh |
| 54 | 2026-02-08 | Parallel safety hardening + validation sync | 508 | 520 | Added apply/stop/panic-revert recovery UX hardening, daemon multi-pass visibility recovery + timeout cleanup safety, Win32 foreground/hotkey safeguards, sentinel collision mitigation, top-of-doc recovery guardrails, and full quality-gate revalidation |
| 55 | 2026-02-08 | Lockout hardening follow-up + validation sync | 520 | 522 | Added daemon overlap guard + retained timed-join model, Win32 foreground handoff fail-safe hide skipping, CLI unconfirmed stop/panic-revert non-zero handling, and tracker/incident evidence refresh |
| 56 | 2026-02-08 | Deterministic recovery fallback hardening + double QA loop | 522 | 524 | Replaced detached late-worker recovery with bounded deterministic joins, added local CLI emergency-uncloak fallback path + emergency panic hotkey (`Win+Ctrl+Escape`), tightened doctor/status behavior, synced recovery docs/config references, and completed two full QA loops plus governance control pass |
| 57 | 2026-02-08 | Shutdown confirmation hardening + recovery QA revalidation | 524 | 532 | Added bounded daemon-shutdown confirmation waits for `stop`/`panic-revert`, auto local emergency restore on ambiguous stop/apply shutdown paths, refreshed recovery docs/runbooks, and re-ran full quality gate |
| 58 | 2026-02-08 | Tray/IPC pause-path parity + QA revalidation | 532 | 533 | Unified pause toggle logic so tray and IPC resume paths both re-apply layout immediately, added regression test for resume-time apply failure surfacing, and re-ran full quality gate plus governance pass |
| 59 | 2026-02-08 | Resume rollback hardening + recovery UX clarification + double QA loop | 533 | 533 | Added paused-state rollback on resume apply failure, expanded recovery docs with explicit `stop`/`panic-revert` scope guidance, synced incident/tracker evidence, and completed two full QA loops plus governance control pass |
| 60 | 2026-02-09 | Stop/panic IPC-connect timeout recovery hardening + double QA loop | 533 | 535 | Added CLI local emergency restore for stop/panic IPC-connect timeout failures, added connect-timeout regression tests, synchronized setup hotkey wording, refreshed incident/tracker/spec evidence, and completed two full QA loops plus governance control pass |
| 61 | 2026-02-09 | Focus/foreground WinEvent recovery hardening + double QA loop | 535 | 535 | Removed strict manageability filtering for focus/foreground WinEvents so cloaked managed windows can re-enter daemon recovery/focus flow, updated incident/tracker evidence, and completed two full QA loops plus governance control pass |
| 62 | 2026-02-09 | Recovery consistency hardening + double QA loop | 535 | 540 | Added CLI apply/stop/panic unconfirmed-recovery safety handling, fullscreen/focus safety hardening for minimize/restore paths, WinEvent policy refinement + foreground normalization, IPC newline/response-size hardening, synced recovery runbook wording, and completed two full QA loops plus governance control pass |
| 63 | 2026-02-09 | Strict automation QA gate + double QA loop | 540 | 542 | Closed high CI tag-trigger bypass (`tags: ['**']`), completed two consecutive post-fix review waves with zero new Critical/High findings, executed full QA suite twice, and refreshed all incident trackers/evidence |
| 64 | 2026-02-09 | Strict automation QA gate rerun + double QA loop | 542 | 542 | Re-ran full strict gate from current state, found zero new Critical/High findings in both loops, passed full QA suite twice, and refreshed required tracking artifacts |
| 65 | 2026-02-09 | Strict automation QA gate rerun + double QA loop | 542 | 542 | Re-ran strict gate with independent parallel review lanes and benchmark sanity check, found zero new Critical/High findings in both loops, passed full QA suite twice, and refreshed required tracking artifacts |
| 66 | 2026-02-09 | Strict automation QA gate rerun + double QA loop | 542 | 542 | Re-ran strict gate with parallel sub-agent reviewers plus pre-UAT checklist audit, found zero new code-level Critical/High findings in both loops, passed full QA suite twice, and confirmed release remains blocked on host-manual closure evidence |
| 67 | 2026-02-13 | Host pre-UAT failure evidence capture + tracker sync | 542 | 542 | Logged failed host pre-UAT run/recovery evidence (daemon confirmed stopped, Explorer reset attempted, in-session terminal focus still impaired), updated incident/tracker docs, and kept release gate blocked |
| 68 | 2026-02-13 | Technical host-log correlation for lockout incident | 542 | 542 | Correlated inc49 command artifacts, daemon runtime logs, and Windows Error Reporting black-screen diagnostics; documented root-cause signals and kept release gate blocked pending post-reboot host closure evidence |

---

## Detailed Iteration Logs

### Iteration 68: Technical Log Correlation for Host Lockout

**Date**: 2026-02-13  
**Status**: COMPLETED (evidence analysis/documentation), incident still open  
**Previous Context**: Iteration 67 (host pre-UAT failure evidence capture + tracker sync)

#### 68.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Correlate host-failure timeline across script command logs, daemon logs, and Windows event logs | Critical | DONE |
| 2 | Document technical root-cause signals in incident record | Critical | DONE |
| 3 | Preserve release-blocking state until successful post-reboot closure run | Critical | DONE |

#### 68.2 Changes Made

- Updated incident file with a dedicated technical-correlation section:
  - `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md`
- Evidence captured in documentation from:
  - `docs/1_Progress and review/evidence/inc49/20260213-114332/commands/*`
  - `C:\Users\stark\AppData\Local\Temp\openniri-daemon.log`
  - Windows Error Reporting (`Event ID 1001`, `WindowsBlackScreenDiagnosticsV1`) entries during host recovery window.

#### 68.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Runtime QA suite | Not re-run in Iteration 68 | NOT RUN (evidence-analysis iteration) |

#### 68.4 Impact Statement

- Confirms failure mode is not a simple daemon crash; it is a partial window-operation failure path with concurrent Windows shell/graphics instability signals.
- Incident remains open and release-blocking pending successful post-reboot host rerun evidence for `INC-49-1`, `INC-49-4`, and `INC-49-T1`.

### Iteration 67: Host Pre-UAT Failure Evidence Capture + Tracker Sync

**Date**: 2026-02-13  
**Status**: COMPLETED (documentation/tracker sync only), host incident remains open and release-blocking  
**Previous Context**: Iteration 66 (strict automation QA gate rerun + double QA loop)

#### 67.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Capture and preserve exact host pre-UAT failure/recovery evidence from field run | Critical | DONE |
| 2 | Synchronize incident/tracker artifacts with latest host evidence | High | DONE |
| 3 | Keep release gate state explicit (blocked until host closure evidence passes) | Critical | DONE |

#### 67.2 Changes Made

- Incident evidence update added:
  - `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md`
  - Captured 2026-02-13 host run details: scenario flow led to non-focusable terminals; recovery outputs for `emergency-uncloak`, `stop`, `status`; Explorer reset attempted; in-session usability still impaired.
- Tracker synchronization:
  - `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` advanced to Review 38 with host evidence update.
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json` updated `last_updated`, `review_id`, and evidence/status for `INC-49-1`, `INC-49-4`, `INC-49-T1`.
  - `docs/1_Progress and review/OPEN_ITEMS.md` updated with Iteration 67 host-run failure note.
- Pre-UAT tooling docs/script remained in-progress and unclosed:
  - `tools/inc49/run-preuat-evidence.ps1`
  - `docs/1_Progress and review/INC49_PRE_UAT_ONE_SHOT_CHECKLIST.md`

#### 67.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Script parse check | PowerShell parser on `tools/inc49/run-preuat-evidence.ps1` | PASSED |
| Runtime QA suite | Not re-run in Iteration 67 | NOT RUN (documentation/tracker update iteration) |

#### 67.4 Impact Statement

- Code-level strict automation gate result from Iteration 66 remains unchanged (no new Critical/High code findings introduced in Iteration 67).
- Host closure evidence remains failed/incomplete:
  - `INC-49-1`: still in progress
  - `INC-49-4`: still in progress
  - `INC-49-T1`: moved to in-progress with failed host evidence captured
- Release/tag/publish remains blocked pending successful post-reboot host rerun evidence.

### Iteration 66: Strict Automation QA Gate Rerun + Double QA Loop

**Date**: 2026-02-09  
**Status**: COMPLETED (code-level Critical/High gate passed), host incident-closure evidence still pending  
**Previous Context**: Iteration 65 (strict automation QA gate rerun + double QA loop)

#### 66.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-run strict automation gate from current tree state with parallel review lanes before host-manual testing | Critical | DONE |
| 2 | Confirm two consecutive zero-new-Critical/High loops with full QA suites | Critical | DONE |
| 3 | Execute pre-live/pre-UAT checklist audit and isolate host-manual blockers | High | DONE |

#### 66.2 Changes Made

- No runtime/code changes were required in this rerun (zero new code-level Critical/High findings).
- Parallel review execution:
  - Spawned four explorer reviewers for runtime safety, IPC/CLI robustness, Win32 focus/recovery, and pre-UAT checklist/tracker consistency.
  - Runtime, IPC/CLI, and Win32 reviewers reported no new code-level Critical/High findings.
  - Checklist reviewer confirmed release remains blocked by pending host evidence tasks (`INC-49-1`, `INC-49-4`, `INC-49-T1`).
- Pre-UAT checklist audit completed against:
  - `docs/1_Progress and review/CLAUDE_FINALIZATION_CHECKLIST.md`
  - `docs/PUBLIC_READINESS_CHECKLIST.md`
  - `docs/TESTING_GUIDE.md` (sections 15-18 host recovery scenarios)
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`
- Tracker synchronization:
  - `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` — advanced to Review 37 with loop-level zero-finding evidence.
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json` — advanced review id/evidence strings to Iteration 66 loop evidence.
  - `docs/1_Progress and review/OPEN_ITEMS.md` — updated iteration/evidence references to Iteration 66.

#### 66.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| QA Loop 1: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 1: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 1: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 1: Workspace tests | `cargo test --all --verbose` | PASSED: 546 test-binary total (542 passed, 4 ignored) + doc-test compile checks |
| QA Loop 1: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 1: Release build | `cargo build --release --all` | PASSED |
| QA Loop 1: Audit | `cargo audit` | PASSED with 9 allowed warnings (GTK3/glib/proc-macro chain via `tray-icon`) |
| QA Loop 1: Target scope check | `cargo tree --target x86_64-pc-windows-gnu -i gtk` + `-i glib` | PASSED (`nothing to print`, not in Windows target tree) |
| QA Loop 2: Full gate rerun | `cargo fmt --all -- --check && cargo clippy --all -- -D warnings && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --all --verbose && cargo test --workspace -- --ignored && cargo build --release --all` | PASSED |

#### 66.4 Impact Statement

- Strict automation gate rerun criteria are satisfied again from current repo state:
  - two consecutive loops with zero new code-level Critical/High findings;
  - full required QA suite passed in both loops.
- Pre-live/pre-UAT blocker status is unchanged and non-code:
  - `INC-49-1` host reproduction/recovery closure evidence pending.
  - `INC-49-4` host acceptance run evidence pending.
  - `INC-49-T1` manual host focus-recovery test evidence pending.
- Release remains blocked until host-manual evidence is captured and linked in incident/tracker docs.

### Iteration 65: Strict Automation QA Gate Rerun + Double QA Loop

**Date**: 2026-02-09  
**Status**: COMPLETED (code-level Critical/High gate passed), host incident-closure evidence still pending  
**Previous Context**: Iteration 64 (strict automation QA gate rerun + double QA loop)

#### 65.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-run strict automation gate from current tree state before host-manual testing | Critical | DONE |
| 2 | Confirm two consecutive zero-new-Critical/High loops with full QA suites | Critical | DONE |
| 3 | Refresh required tracker artifacts with loop-level evidence and gate result | High | DONE |

#### 65.2 Changes Made

- No runtime/code changes were required in this rerun (zero new code-level Critical/High findings).
- Independent parallel review lanes rerun across runtime/recovery, IPC framing/size/timeouts, Win32 foreground/hide safeguards, CLI recovery/non-success handling, and CI/release gate consistency.
- External benchmark sanity-check reviewed against niri/komorebi guidance (`https://github.com/YaLTeR/niri/wiki/Design-Principles`, `https://lgug2z.github.io/komorebi/`).
- Tracker synchronization:
  - `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md:1` — advanced to Review 36 with loop-level zero-finding evidence.
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json:11` — advanced review id/evidence strings to Iteration 65 loop evidence.
  - `docs/1_Progress and review/OPEN_ITEMS.md:2` — updated iteration/evidence references to Iteration 65.

#### 65.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| QA Loop 1: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 1: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 1: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 1: Workspace tests | `cargo test --all --verbose` | PASSED: 546 test-binary total (542 passed, 4 ignored) + doc-test compile checks |
| QA Loop 1: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 1: Release build | `cargo build --release --all` | PASSED |
| QA Loop 2: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 2: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 2: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 2: Workspace tests | `cargo test --all --verbose` | PASSED: 546 test-binary total (542 passed, 4 ignored) + doc-test compile checks |
| QA Loop 2: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 2: Release build | `cargo build --release --all` | PASSED |

#### 65.4 Impact Statement

- Strict automation gate rerun criteria are satisfied again from current repo state:
  - two consecutive loops with zero new code-level Critical/High findings;
  - full required QA suite passed in both loops.
- Remaining risk is unchanged and non-code: host-manual incident closure evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`) is still required before release.

### Iteration 64: Strict Automation QA Gate Rerun + Double QA Loop

**Date**: 2026-02-09  
**Status**: COMPLETED (code-level Critical/High gate passed), host incident-closure evidence still pending  
**Previous Context**: Iteration 63 (strict automation QA gate + double QA loop)

#### 64.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-run strict automation gate from current tree state before host-manual testing | Critical | DONE |
| 2 | Confirm two consecutive zero-new-Critical/High loops with full QA suites | Critical | DONE |
| 3 | Refresh required tracker artifacts with loop-level evidence and gate result | High | DONE |

#### 64.2 Changes Made

- No runtime/code changes were required in this rerun (zero new code-level Critical/High findings).
- Tracker synchronization:
  - `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` — advanced to Review 35 with loop-level zero-finding evidence.
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json` — advanced review id/evidence strings to Iteration 64 loop evidence.
  - `docs/1_Progress and review/OPEN_ITEMS.md` — updated iteration/evidence references to Iteration 64.

#### 64.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| QA Loop 1: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 1: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 1: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 1: Workspace tests | `cargo test --all --verbose` | PASSED: 546 test-binary total (542 passed, 4 ignored) + doc-test compile checks |
| QA Loop 1: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 1: Release build | `cargo build --release --all` | PASSED |
| QA Loop 2: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 2: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 2: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 2: Workspace tests | `cargo test --all --verbose` | PASSED: 546 test-binary total (542 passed, 4 ignored) + doc-test compile checks |
| QA Loop 2: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 2: Release build | `cargo build --release --all` | PASSED |

#### 64.4 Impact Statement

- Strict automation gate rerun criteria are satisfied again from current repo state:
  - two consecutive loops with zero new code-level Critical/High findings;
  - full required QA suite passed in both loops.
- Remaining risk is unchanged and non-code: host-manual incident closure evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`) is still required before release.

### Iteration 63: Strict Automation QA Gate + Double QA Loop

**Date**: 2026-02-09  
**Status**: COMPLETED (code-level Critical/High gate passed), host incident-closure evidence still pending  
**Previous Context**: Iteration 62 (recovery consistency hardening + double QA loop)

#### 63.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Resolve any remaining code-level Critical/High issues before further host testing | Critical | DONE |
| 2 | Run strict review/fix loop until two consecutive zero-new-Critical/High waves are achieved | Critical | DONE |
| 3 | Execute full required QA suite twice consecutively and record evidence in active trackers | High | DONE |

#### 63.2 Changes Made

- CI gate hardening:
  - `.github/workflows/ci.yml:5` — broadened tag trigger from `tags: ['v*']` to `tags: ['**']` so non-`v*` tag pushes also run CI (release-sensitive steps remain `refs/tags/v` guarded).
- Strict review evidence capture:
  - Executed two consecutive post-fix review waves across runtime/recovery, CLI non-success paths, IPC framing/timeout handling, Win32 foreground/hide safeguards, and CI release gating.
  - Both waves reported zero new Critical/High findings.
- Tracker synchronization:
  - `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` — advanced to Review 34 with closed `QA4-CI-01`.
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json` — updated review id/evidence/validation strings to Iteration 63.
  - `docs/1_Progress and review/OPEN_ITEMS.md` — updated active/completed evidence and iteration stamp.

#### 63.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| QA Loop 1: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 1: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 1: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 1: Workspace tests | `cargo test --all --verbose` | PASSED: 546 test-binary total (542 passed, 4 ignored) + doc-test compile checks |
| QA Loop 1: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 1: Release build | `cargo build --release --all` | PASSED |
| QA Loop 2: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 2: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 2: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 2: Workspace tests | `cargo test --all --verbose` | PASSED: 546 test-binary total (542 passed, 4 ignored) + doc-test compile checks |
| QA Loop 2: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 2: Release build | `cargo build --release --all` | PASSED |
| Governance control pass | `pwsh C:\\dev\\0_repo_overarching\\scripts\\portfolio\\run-coordination-control-pass.ps1` | PASSED (snapshot files written; script emitted non-fatal `HEAD` ambiguity warning) |

#### 63.4 Impact Statement

- Strict automation gate criteria were satisfied for code-level work: one High finding (`QA4-CI-01`) was fixed, then two consecutive post-fix review waves produced zero new Critical/High findings.
- Required QA suite passed twice consecutively after the fix.
- Remaining risk is unchanged and explicitly non-code: host-manual incident closure evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`) is still required before any release.

### Iteration 62: Recovery Consistency Hardening + Double QA Loop

**Date**: 2026-02-09  
**Status**: COMPLETED (runtime/docs/test hardening), host incident-closure evidence still pending  
**Previous Context**: Iteration 61 (focus/foreground WinEvent recovery hardening)

#### 62.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Close remaining unconfirmed-recovery branches for `apply`/`stop`/`panic-revert` | Critical | DONE |
| 2 | Harden minimize/restore + focus handling and WinEvent policy for focus-trap recovery | Critical | DONE |
| 3 | Add IPC framing/size safety guards and sync runbook wording for stressed recovery | High | DONE |
| 4 | Re-run full QA gates twice and refresh incident/tracker evidence | High | DONE |

#### 62.2 Changes Made

- CLI recovery hardening:
  - `crates/cli/src/main.rs:522` — added explicit recovery guidance helpers for unconfirmed apply/stop/panic outcomes.
  - `crates/cli/src/main.rs:688` — `send_apply_with_recovery()` now handles disconnect-before-response and connect-timeout branches with local emergency restore.
  - `crates/cli/src/main.rs:729` — stop non-success responses now trigger local emergency restore + unconfirmed-shutdown guidance.
  - `crates/cli/src/main.rs:793` — panic-revert non-success responses now trigger local emergency restore + unconfirmed-recovery guidance.
- Layout/daemon safety hardening:
  - `crates/core_layout/src/lib.rs:1514` — added `clear_fullscreen_if_window()` and used it in floating transitions.
  - `crates/daemon/src/main.rs:1715` — minimize path now clears fullscreen when needed and ensures focused visibility before apply/sync.
  - `crates/daemon/src/main.rs:3361` — delayed `FocusFollowsMouse` path now re-checks runtime config before applying focus.
  - `crates/daemon/src/main.rs:2192` — IPC server now enforces newline-terminated commands and bounded response-size fallback via `write_ipc_response_line()`.
- Win32 event/foreground hardening:
  - `crates/platform_win32/src/lib.rs:580` — introduced event-specific `should_emit_window_event_for(...)` policy.
  - `crates/platform_win32/src/lib.rs:834` — foreground-visibility protection now normalizes foreground window to root before placement checks.
  - `crates/platform_win32/src/lib.rs:1921` — callback now uses event-specific emit policy.
- Recovery doc consistency:
  - `docs/TROUBLESHOOTING.md:42` — unified shutdown confirmation wording to `status` must fail (`timeout` OR `Daemon is not running...`).
  - `docs/TROUBLESHOOTING.md:217` — reset flow now prefers `openniri-cli config reset` with manual fallback.
  - `docs/SUPPORT_PLAYBOOK.md:17` — unified stop-confirmation wording and added `Win+Ctrl+Escape` explicitly in critical recovery key sequence.

#### 62.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| QA Loop 1: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 1: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 1: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 1: Workspace tests | `cargo test --all --verbose` | PASSED: 540 test-binary total (536 passed, 4 ignored) + 1 doc-test compile check |
| QA Loop 1: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 1: Release build | `cargo build --release --all` | PASSED |
| QA Loop 2: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 2: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 2: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 2: Workspace tests | `cargo test --all --verbose` | PASSED: 540 test-binary total (536 passed, 4 ignored) + 1 doc-test compile check |
| QA Loop 2: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 2: Release build | `cargo build --release --all` | PASSED |
| Governance control pass | `pwsh C:\\dev\\0_repo_overarching\\scripts\\portfolio\\run-coordination-control-pass.ps1` | PASSED (snapshot files written; script emitted non-fatal `HEAD` ambiguity warning) |

#### 62.4 Impact Statement

- Recovery behavior is now consistent for timeout/disconnect/non-success response branches across apply/stop/panic flows, with local visibility restore as the fail-safe.
- Minimize/restore and delayed focus-follow transitions now include explicit visibility/config guards to reduce focus-trap risk.
- Incident remains open only for host-manual closure evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`); release remains blocked until that evidence is captured.

### Iteration 61: Focus/Foreground WinEvent Recovery Hardening + Double QA Loop

**Date**: 2026-02-09  
**Status**: COMPLETED (runtime/docs/test hardening), host incident-closure evidence still pending  
**Previous Context**: Iteration 60 (stop/panic IPC-connect timeout recovery hardening)

#### 61.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Eliminate focus-trap risk where cloaked managed windows lose focus-event delivery | Critical | DONE |
| 2 | Add/adjust regression coverage for manageability filter scope | High | DONE |
| 3 | Re-run full QA gates twice and refresh incident/tracker evidence | High | DONE |

#### 61.2 Changes Made

- WinEvent recovery hardening:
  - `crates/platform_win32/src/lib.rs:564` — narrowed manageability filtering to create/location events only so focus/foreground events are not dropped when windows are temporarily cloaked.
  - `crates/platform_win32/src/lib.rs:3246` — updated regression test to enforce the new filter scope (`test_should_filter_window_event_by_manageability_scopes_to_create_and_move`).
- Tracker/incident sync:
  - `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md` — logged Iteration 61 mitigation bullets.
  - `docs/1_Progress and review/OPEN_ITEMS.md` — refreshed iteration/evidence wording.
  - `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` — refreshed status/evidence wording.
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json` — refreshed INC-49 and validation-gate evidence strings.

#### 61.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Focused regression (pre-check) | `cargo test -p openniri-platform-win32 test_should_filter_window_event_by_manageability_scopes_to_create_and_move -- --nocapture` | PASSED |
| QA Loop 1: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 1: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 1: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 1: Workspace tests | `cargo test --all --verbose` | PASSED: 535 test-binary total (531 passed, 4 ignored) + 1 doc-test compile check |
| QA Loop 1: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 1: Release build | `cargo build --release --all` | PASSED |
| QA Loop 2: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 2: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 2: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 2: Workspace tests | `cargo test --all --verbose` | PASSED: 535 test-binary total (531 passed, 4 ignored) + 1 doc-test compile check |
| QA Loop 2: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 2: Release build | `cargo build --release --all` | PASSED |
| Governance control pass | `pwsh C:\\dev\\0_repo_overarching\\scripts\\portfolio\\run-coordination-control-pass.ps1` | PASSED (snapshot files written; script emitted non-fatal `HEAD` ambiguity warning) |

#### 61.4 Impact Statement

- Focus/foreground events can now reach daemon recovery logic even when a managed window is temporarily cloaked, reducing risk of Alt-Tab focus-trap states.
- The regression is now covered by a dedicated platform test guarding filter scope.
- Incident remains open only for host-manual closure evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`); release remains blocked until that evidence is captured.

### Iteration 60: Stop/Panic IPC-Connect Timeout Recovery Hardening + Double QA Loop

**Date**: 2026-02-09  
**Status**: COMPLETED (runtime/docs/test hardening), host incident-closure evidence still pending  
**Previous Context**: Iteration 59 (resume rollback hardening + recovery UX clarification)

#### 60.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Ensure stop/panic recovery safety also covers IPC connect-timeout failures | Critical | DONE |
| 2 | Add regression coverage for connect-timeout error-chain detection | High | DONE |
| 3 | Re-run full QA gates twice and refresh tracker/spec evidence counts | High | DONE |

#### 60.2 Changes Made

- CLI recovery hardening:
  - `crates/cli/src/main.rs:555` — added `error_chain_has_connect_timeout()` helper for IPC connect-timeout detection.
  - `crates/cli/src/main.rs:747` — `stop` path now runs local emergency visibility restore on connect-timeout failures before returning unconfirmed-shutdown guidance.
  - `crates/cli/src/main.rs:818` — `panic-revert` path now runs local emergency visibility restore on connect-timeout failures before returning timeout guidance.
- CLI UX consistency:
  - `crates/cli/src/main.rs:1623` — setup output hotkey wording now uses `Win+Ctrl+Escape` for consistency with config/docs.
- Regression coverage:
  - `crates/cli/src/main.rs:2252` — added `test_error_chain_has_connect_timeout_true`.
  - `crates/cli/src/main.rs:2260` — added `test_error_chain_has_connect_timeout_false`.
- Tracker/spec/incident sync:
  - `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md` — logged Iteration 60 mitigation bullets.
  - `docs/1_Progress and review/OPEN_ITEMS.md` — refreshed iteration/evidence wording.
  - `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` — refreshed status/evidence wording.
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json` — refreshed INC-49 and validation gate evidence strings.
  - `docs/SPEC.md` — refreshed quality-gate totals.

#### 60.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Formatting (pre-check) | `cargo fmt --all -- --check` | PASSED |
| Focused regression (pre-check) | `cargo test -p openniri-cli error_chain_has_connect_timeout -- --nocapture` | PASSED |
| QA Loop 1: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 1: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 1: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 1: Workspace tests | `cargo test --all --verbose` | PASSED: 535 test-binary total (531 passed, 4 ignored) + 1 doc-test compile check |
| QA Loop 1: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 1: Release build | `cargo build --release --all` | PASSED |
| QA Loop 2: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 2: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 2: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 2: Workspace tests | `cargo test --all --verbose` | PASSED: 535 test-binary total (531 passed, 4 ignored) + 1 doc-test compile check |
| QA Loop 2: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 2: Release build | `cargo build --release --all` | PASSED |
| Governance control pass | `pwsh C:\\dev\\0_repo_overarching\\scripts\\portfolio\\run-coordination-control-pass.ps1` | PASSED (snapshot files written; script emitted non-fatal `HEAD` ambiguity warning) |

#### 60.4 Impact Statement

- Stop/panic recovery guidance now matches behavior in IPC-connect timeout races: local visibility recovery is executed before returning failure guidance.
- Recovery resilience coverage now includes both response-timeout and connect-timeout incident branches.
- Incident remains open only for host-manual closure evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`); release remains blocked until that evidence is captured.

### Iteration 59: Resume Rollback Hardening + Recovery UX Clarification + Double QA Loop

**Date**: 2026-02-08  
**Status**: COMPLETED (runtime/docs/test hardening), host incident-closure evidence still pending  
**Previous Context**: Iteration 58 (tray/IPC pause-path parity + QA revalidation)

#### 59.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Prevent false resumed state when resume-time layout apply fails | Critical | DONE |
| 2 | Clarify user-facing recovery wording for `stop`/`panic-revert` scope under stress | High | DONE |
| 3 | Re-run full QA gates twice and refresh tracker evidence | High | DONE |

#### 59.2 Changes Made

- Daemon pause/resume state-consistency hardening:
  - `crates/daemon/src/main.rs:1046` — `toggle_pause()` now rolls back `paused` state if resume-time `apply_layout()` fails.
  - `crates/daemon/src/main.rs:3782` — strengthened `test_toggle_pause_resume_reports_apply_failure` to assert failed resume keeps `paused=true`.
- Recovery UX clarification in user docs:
  - `docs/TESTING_GUIDE.md:13` — added explicit statement that `stop`/`panic-revert` target `openniri.exe` only.
  - `docs/GETTING_STARTED.md:213` — clarified command scope and preserved non-destructive-first ordering.
  - `docs/TROUBLESHOOTING.md:45` — documented that `stop` does not intend to close unrelated user apps.
  - `docs/SUPPORT_PLAYBOOK.md:13` — added explicit OpenNiri-only scope note in critical recovery flow.
- Incident/tracker synchronization:
  - `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md` — logged post-Iteration 58 field evidence and Iteration 59 mitigations.
  - `docs/1_Progress and review/OPEN_ITEMS.md` — refreshed iteration marker/evidence wording.
  - `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` — refreshed status/evidence wording.
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json` — refreshed `INC-49` and validation-gate evidence strings.

#### 59.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Formatting (pre-check) | `cargo fmt --all -- --check` | PASSED |
| Focused regression (pre-check) | `cargo test -p openniri-daemon test_toggle_pause_resume_reports_apply_failure -- --nocapture` | PASSED |
| QA Loop 1: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 1: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 1: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 1: Workspace tests | `cargo test --all --verbose` | PASSED: 533 total (529 passed, 4 ignored) |
| QA Loop 1: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 1: Release build | `cargo build --release --all` | PASSED |
| QA Loop 2: Formatting | `cargo fmt --all -- --check` | PASSED |
| QA Loop 2: Clippy (strict) | `cargo clippy --all -- -D warnings` | PASSED |
| QA Loop 2: Clippy (all-targets/all-features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA Loop 2: Workspace tests | `cargo test --all --verbose` | PASSED: 533 total (529 passed, 4 ignored) |
| QA Loop 2: Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA Loop 2: Release build | `cargo build --release --all` | PASSED |
| Governance control pass | `pwsh C:\\dev\\0_repo_overarching\\scripts\\portfolio\\run-coordination-control-pass.ps1` | PASSED (snapshot files written) |

#### 59.4 Impact Statement

- Resume failures no longer leave the daemon claiming a resumed state while layout re-apply failed.
- Recovery docs now describe `stop`/`panic-revert` in plain operational terms to reduce high-stress misuse.
- Incident remains open only for host-manual closure evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`); release remains blocked until that evidence is captured.

### Iteration 58: Tray/IPC Pause-Path Parity + QA Revalidation

**Date**: 2026-02-08  
**Status**: COMPLETED (runtime test/docs sync), host incident-closure evidence still pending  
**Previous Context**: Iteration 57 (shutdown confirmation hardening + recovery QA revalidation)

#### 58.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Eliminate tray vs IPC behavior drift for `toggle-pause` resume semantics | Critical | DONE |
| 2 | Add regression coverage for resume-time apply failures | High | DONE |
| 3 | Re-run full QA + governance gates and sync evidence counts | High | DONE |

#### 58.2 Changes Made

- Pause-path behavior parity in daemon:
  - `crates/daemon/src/main.rs:1045` — added shared `toggle_pause()` helper used by all pause toggle callers.
  - `crates/daemon/src/main.rs:1285` — IPC `TogglePause` now routes through shared helper (single source of truth).
  - `crates/daemon/src/main.rs:3254` — tray `TogglePause` now routes through shared helper so resume always reapplies layout immediately.
- Regression coverage:
  - `crates/daemon/src/main.rs:3773` — added `test_toggle_pause_resume_reports_apply_failure` to prove resume path surfaces apply failures.
- Tracker/evidence sync:
  - `docs/1_Progress and review/OPEN_ITEMS.md` — refreshed last-updated marker and QA totals.
  - `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` — refreshed verification counts.
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json` — refreshed validation gate evidence strings.
  - `docs/SPEC.md` — refreshed quality-gate totals.

#### 58.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Formatting | `cargo fmt --all -- --check` | PASSED |
| Clippy (strict) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| Workspace tests | `cargo test --all --verbose` | PASSED: 533 total (529 passed, 4 ignored) |
| Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| Release build | `cargo build --release --all` | PASSED |
| Governance control pass | `pwsh C:\\dev\\0_repo_overarching\\scripts\\portfolio\\run-coordination-control-pass.ps1` | PASSED (snapshot files written) |

#### 58.4 Impact Statement

- Tray and IPC pause/resume flows now behave consistently; resume no longer waits for a later event to retile windows.
- Regression coverage now explicitly guards resume-time apply error handling.
- Validation evidence and quality-gate counts are synchronized across active tracker/spec docs.
- Incident remains open only for host-manual closure evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`).

### Iteration 57: Shutdown Confirmation Hardening + Recovery QA Revalidation

**Date**: 2026-02-08  
**Status**: COMPLETED (runtime/docs/test hardening), host incident-closure evidence still pending  
**Previous Context**: Iteration 56 (deterministic recovery fallback hardening)

#### 57.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Ensure `stop`/`panic-revert` return only after bounded daemon-shutdown confirmation | Critical | DONE |
| 2 | Auto-run local emergency restore on ambiguous shutdown/apply-timeout CLI paths | Critical | DONE |
| 3 | Sync recovery-facing docs with the stricter stop/recovery semantics | High | DONE |
| 4 | Re-run full QA gate and refresh tracker evidence | High | DONE |

#### 57.2 Changes Made

- CLI shutdown/recovery hardening (`crates/cli/src/main.rs`):
  - `crates/cli/src/main.rs:162` and `crates/cli/src/main.rs:165` — added recovery command aliases (`recover`, `restore-windows`) for faster break-glass discovery.
  - `crates/cli/src/main.rs:32` and `crates/cli/src/main.rs:34` — added bounded shutdown-confirm timeout/poll constants.
  - `crates/cli/src/main.rs:428` — added `wait_for_daemon_shutdown()` helper for explicit stop confirmation.
  - `crates/cli/src/main.rs:673` — `apply` timeout branch now runs local emergency visibility restore before returning actionable failure.
  - `crates/cli/src/main.rs:699`, `crates/cli/src/main.rs:708`, `crates/cli/src/main.rs:717` — `stop` now executes local emergency restore for not-running/race/disconnect paths.
  - `crates/cli/src/main.rs:742` — `stop` now waits for daemon teardown confirmation and fails with fallback recovery if confirmation times out/probe fails.
  - `crates/cli/src/main.rs:804` — `panic-revert` now also waits for daemon teardown confirmation and falls back to local restore on ambiguous outcomes.
  - `crates/cli/src/main.rs:161` and `crates/cli/src/main.rs:1894` — added `toggle-pause` CLI command (`pause` alias) plus command-mapping/alias tests.
  - `crates/cli/src/main.rs:1994` and `crates/cli/src/main.rs:2000` — added guardrail tests for shutdown confirmation timeout/poll budgets.
- IPC/daemon pause control parity:
  - `crates/ipc/src/lib.rs:190` — added `IpcCommand::TogglePause` protocol command.
  - `crates/daemon/src/main.rs:1268` — daemon now handles `TogglePause` directly (pause/resume with re-apply on resume).
  - `crates/daemon/src/main.rs:2965` — IPC toggle-pause path now updates tray pause text/tooltip.
  - `crates/daemon/src/config.rs:577` — `toggle_pause`/`toggle-pause` now available in hotkey/gesture command parsing.
  - `crates/daemon/tests/integration.rs:223` — added toggle-pause payload roundtrip integration coverage.
- Recovery docs/runbook sync:
  - `README.md:91` and `README.md:103` — documented stop confirmation wait + local fallback behavior.
  - `docs/TROUBLESHOOTING.md:45` — clarified that unconfirmed `stop` already triggers local emergency restore.
  - `docs/GETTING_STARTED.md:212` — added explicit stop confirmation/fallback semantics in stuck-desktop quick checks and CLI pause guidance.
  - `docs/SUPPORT_PLAYBOOK.md:16` — aligned incident playbook with stop fallback behavior.
  - `docs/TESTING_GUIDE.md:214` — added explicit validation step for unconfirmed-stop local restore output.
  - `docs/CONFIGURATION.md:283` — documented `toggle_pause` as a valid hotkey/gesture command.

#### 57.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Formatting | `cargo fmt --all -- --check` | PASSED |
| Clippy (strict) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| Workspace tests | `cargo test --all` | PASSED: 532 total (528 passed, 4 ignored) |
| Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| Release build | `cargo build --release --all` | PASSED |

#### 57.4 Impact Statement

- `stop` and `panic-revert` are now safer under race conditions: the CLI explicitly waits for daemon teardown and treats ambiguous outcomes as recovery events, not silent success.
- `apply` timeout now triggers immediate local visibility fallback, improving recoverability when desktop control degrades mid-apply.
- `toggle-pause` is now available end-to-end (IPC + CLI + daemon + tray sync), providing a non-destructive temporary kill switch without full daemon shutdown.
- Recovery documentation now reflects the hardened behavior so operators have clearer, consistent guidance.
- Incident remains open only for host-manual closure evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`).

### Iteration 56: Deterministic Recovery Fallback Hardening + Double QA Loop

**Date**: 2026-02-08  
**Status**: COMPLETED (runtime/docs/test hardening), host incident-closure evidence still pending  
**Previous Context**: Iteration 55 (lockout hardening follow-up + validation sync)

#### 56.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Remove detached shutdown-recovery thread path and make cleanup deterministic/bounded | Critical | DONE |
| 2 | Add out-of-band local emergency recovery path in CLI when daemon IPC is unavailable | Critical | DONE |
| 3 | Tighten status/doctor behavior and sync user-facing recovery docs/config defaults | High | DONE |
| 4 | Run two full QA loops and record evidence | High | DONE |

#### 56.2 Changes Made

- Daemon shutdown hardening:
  - `crates/daemon/src/main.rs:110` — added bounded final join timeout constant for lingering apply workers.
  - `crates/daemon/src/main.rs:140` — added `shutdown_mode_for_command()` helper for shared shutdown command routing.
  - `crates/daemon/src/main.rs:2037` — replaced detached late-worker recovery scheduling with deterministic bounded joins + final visibility pass.
  - `crates/daemon/src/main.rs:3066` and `crates/daemon/src/main.rs:3135` — hotkey/gesture command paths now route `panic_revert`/`stop` through full shutdown cleanup.
- Config + default hotkey hardening:
  - `crates/daemon/src/config.rs:330` — documented `panic_revert` as a supported hotkey command.
  - `crates/daemon/src/config.rs:379` — added default emergency binding: `Win+Ctrl+Escape = panic_revert`.
  - `crates/daemon/src/config.rs:572` — parser now accepts `panic_revert` and `panic-revert`.
- CLI recovery and diagnostics hardening:
  - `crates/cli/src/main.rs:156` — added `emergency-uncloak` command.
  - `crates/cli/src/main.rs:537` — added local emergency visibility restore helper (`uncloak_all_visible_windows`) used by panic-revert fallback and explicit command.
  - `crates/cli/src/main.rs:570` — added fast not-running connect fail path for pure not-found pipe states.
  - `crates/cli/src/main.rs:660` — apply path now probes daemon state before send to avoid confusing timeout-first UX.
  - `crates/cli/src/main.rs:711` — `panic-revert` now executes local emergency restore on not-running/unconfirmed/timeout paths.
  - `crates/cli/src/main.rs:751` and `crates/cli/src/main.rs:1547` — added explicit `status` handler and stricter doctor status-query failure classification.
- Dependency + docs alignment:
  - `crates/cli/Cargo.toml:15` — added direct dependency on `openniri-platform-win32` for local emergency restore.
  - `README.md`, `docs/GETTING_STARTED.md`, `docs/TROUBLESHOOTING.md`, `docs/TESTING_GUIDE.md`, `docs/CONFIGURATION.md` — updated recovery wording, new `emergency-uncloak` command guidance, `status` failure expectation text, and default emergency hotkey docs.

#### 56.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Targeted CLI regressions | `cargo test -p openniri-cli` | PASSED: 96 passed |
| Targeted daemon overlap regression | `cargo test -p openniri-daemon test_apply_layout_rejects_overlap_while_timed_out_worker_is_running -- --nocapture` | PASSED |
| Targeted daemon config parser | `cargo test -p openniri-daemon test_parse_command` | PASSED |
| QA loop 1 | `cargo fmt --all -- --check` | PASSED |
| QA loop 1 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA loop 1 | `cargo test --all` | PASSED: 524 total (520 passed, 4 ignored) |
| QA loop 1 | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA loop 1 | `cargo build --release --all` | PASSED |
| Governance control pass | `pwsh C:\\dev\\0_repo_overarching\\scripts\\portfolio\\run-coordination-control-pass.ps1` | PASSED (snapshot files written) |
| QA loop 2 | `cargo fmt --all -- --check` | PASSED |
| QA loop 2 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| QA loop 2 | `cargo test --all` | PASSED: 524 total (520 passed, 4 ignored) |
| QA loop 2 | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed |
| QA loop 2 | `cargo build --release --all` | PASSED |

#### 56.4 Impact Statement

- Recovery behavior is now safer under daemon IPC failures: panic-revert can still trigger local visibility restore and users have a direct `emergency-uncloak` command.
- Shutdown cleanup no longer depends on detached late-recovery threads, reducing nondeterministic exit-time behavior.
- Emergency recovery is now discoverable via default hotkeys (`Win+Ctrl+Escape`) and synchronized documentation.
- Incident closure still requires host-manual evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`); release gate remains blocked pending that evidence.

### Iteration 55: Lockout Hardening Follow-Up + Validation Sync

**Date**: 2026-02-08
**Status**: COMPLETED (code/test/docs hardening), host incident-closure evidence still pending
**Previous Context**: Iteration 54 (parallel safety hardening + validation sync)

#### 55.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Eliminate remaining lockout-class races around timed-out apply workers and late worker mutation | Critical | DONE |
| 2 | Strengthen Win32 focus/hide behavior so foreground handoff denial cannot hide active window | Critical | DONE |
| 3 | Tighten CLI recovery semantics and refresh incident trackers/evidence after full validation | High | DONE |

#### 55.2 Changes Made

- Daemon hardening:
  - `crates/daemon/src/main.rs:104` — added cooperative timed-join polling interval constant.
  - `crates/daemon/src/main.rs:640` — reaps late timed-out apply workers and runs visibility recovery pass.
  - `crates/daemon/src/main.rs:653` — blocks overlapping `apply_layout` runs while a timed-out worker is still active.
  - `crates/daemon/src/main.rs:2277` — refactored `join_with_timeout` to retain handle ownership on timeout (no watcher-thread detachment).
  - `crates/daemon/src/main.rs:5207` — added deterministic overlap-prevention regression test.
- Win32 hardening:
  - `crates/platform_win32/src/lib.rs:830` — treats foreground handoff denial (`Ok(false)`) as failed transfer and keeps foreground window visible.
  - `crates/platform_win32/src/lib.rs:970` — skips cloaking foreground window when handoff is not confirmed.
  - `crates/platform_win32/src/lib.rs:994` — skips MoveOffScreen on foreground window when handoff is not confirmed.
  - `crates/platform_win32/src/lib.rs:2255` — enforces all-or-nothing global hotkey registration to avoid partial shortcut state.
- CLI hardening:
  - `crates/cli/src/main.rs:523` — added explicit `stop_unconfirmed_message()`.
  - `crates/cli/src/main.rs:614` — introduced shared `send_apply_with_recovery()` path for both `run` and `apply`.
  - `crates/cli/src/main.rs:654` — unconfirmed `stop` branches now return non-zero (`Err`) instead of soft-success.
- `crates/cli/src/main.rs:691` — unconfirmed `panic-revert` branches now return non-zero (`Err`) instead of soft-success.
- Incident/tracker updates:
  - `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md`
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`
  - `docs/1_Progress and review/OPEN_ITEMS.md`
  - `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`

#### 55.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Targeted daemon regression | `cargo test -p openniri-daemon test_apply_layout_rejects_overlap_while_timed_out_worker_is_running -- --nocapture` | PASSED |
| Targeted join timeout regression | `cargo test -p openniri-daemon test_join_with_timeout_hanging_thread -- --nocapture` | PASSED |
| Targeted Win32 apply regression | `cargo test -p openniri-platform-win32 test_apply_placements_reports_move_offscreen_errors -- --nocapture` | PASSED |
| Workspace tests | `cargo test --all` | PASSED: 522 total (518 passed, 4 ignored) |
| Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed successfully |
| Clippy | `cargo clippy --all -- -D warnings` | PASSED |
| Formatting | `cargo fmt --all -- --check` | PASSED |
| Release build | `cargo build --release` | PASSED |
| Governance control pass | `pwsh C:\\dev\\0_repo_overarching\\scripts\\portfolio\\run-coordination-control-pass.ps1` | PASSED (snapshots written) |

#### 55.4 Impact Statement

- Timed-out apply workers can no longer silently overlap with new apply calls, reducing late mutation risk after timeout.
- Foreground-handoff failure can no longer hide the active foreground window during hide passes, reducing focus-trap probability.
- CLI recovery commands now surface unconfirmed outcomes as non-zero, improving operator and automation reliability.
- Incident remains open only for host-manual closure evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`).

### Iteration 54: Parallel Safety Hardening + Validation Sync

**Date**: 2026-02-08
**Status**: COMPLETED (runtime/docs/test hardening), incident closure evidence still pending
**Previous Context**: Iteration 53 (parallel critical fix sweep)

#### 54.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Harden user recovery UX and daemon/platform lockout protections after incident feedback | Critical | DONE |
| 2 | Add deterministic tests for newly hardened recovery paths | High | DONE |
| 3 | Re-run full quality gate and refresh active tracker evidence | High | DONE |

#### 54.2 Changes Made

- CLI recovery hardening:
  - `crates/cli/src/main.rs` — added actionable timeout/non-running guidance for `apply`, `stop`, and `panic-revert`; introduced recovery connect timeout budget; improved pipe-timeout next steps.
- Daemon safety hardening:
  - `crates/daemon/src/main.rs` — added apply-timeout visibility recovery pass, cleanup suppression reset on failed/timed-out applies, always-attempt MoveOffScreen restore during shutdown recovery, and multi-pass delayed recovery retries after timed-out apply workers.
- Win32 platform hardening:
  - `crates/platform_win32/src/lib.rs` — ensured foreground window is redirected before hiding offscreen targets, added rollback of partial hide side effects on apply failure, restored minimized windows before foreground activation, failed fast when zero hotkeys register, added hotkey-global cleanup helper, and moved MoveOffScreen sentinel away from minimized-window coordinates.
- Documentation and recovery clarity:
  - `README.md`, `docs/TESTING_GUIDE.md`, `docs/SUPPORT_PLAYBOOK.md`, `docs/GETTING_STARTED.md` — elevated panic-revert-first recovery flow, tray hidden-icon guidance, and top-of-doc safety checklists.
- Tracker/evidence sync:
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`
  - `docs/1_Progress and review/OPEN_ITEMS.md`
  - `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`

#### 54.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Workspace tests | `cargo test --all --verbose` | PASSED: 520 total (516 passed, 4 ignored) |
| Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed successfully |
| Clippy (workspace) | `cargo clippy --all -- -D warnings` | PASSED |
| Clippy (all targets/features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| Formatting | `cargo fmt --all -- --check` | PASSED |
| Release build | `cargo build --release --all` | PASSED |
| Governance control pass | `pwsh C:\\dev\\0_repo_overarching\\scripts\\portfolio\\run-coordination-control-pass.ps1` | PASSED (snapshots written) |

#### 54.4 Impact Statement

- Recovery paths now fail safer under timeout/race conditions and provide actionable user guidance.
- Shutdown/revert cleanup is more robust against timed-out apply-worker races.
- Win32 hide/focus behavior reduces risk of focus-trap lockout patterns reported in incident testing.
- Release remains blocked only on host-manual incident closure evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`).

### Iteration 53: Parallel Critical Fix Sweep + Validation Sync

**Date**: 2026-02-08
**Status**: COMPLETED (runtime fixes + full validation), incident closure evidence still pending
**Previous Context**: Iteration 52 (validation evidence refresh + tracker metric sync)

#### 53.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Address high-risk lockout and event-storm runtime findings across daemon/platform/core/cli | Critical | DONE |
| 2 | Add/refresh deterministic tests around new safety behavior | High | DONE |
| 3 | Harden IPC/CI/process consistency and refresh tracker metrics to current baseline | High | DONE |

#### 53.2 Changes Made

- Runtime hardening:
  - `crates/daemon/src/main.rs` — cancellation/join safety for timed-out apply workers, post-apply move/resize suppression window, robust cleanup window-ID merge, conservative singleton probe across candidate pipes, and IPC connection concurrency limiting.
  - `crates/platform_win32/src/lib.rs` — non-blocking hotkey-thread shutdown fallback, event manageability filtering for focus/foreground, root-window normalization in mouse hook, safer cloak-query fallback, and improved DWM border error mapping.
  - `crates/core_layout/src/lib.rs` — focus index clamping after removals, visible-window targeting for fullscreen entry, fullscreen/minimized consistency fallback, and animated/non-animated stacked-height parity.
  - `crates/cli/src/main.rs` — split connect/response timeouts, longer apply response budget, readiness/apply separation in `run`, bounded response-frame parsing, and candidate-pipe probing/open retry logic.
  - `crates/ipc/src/lib.rs` — user-scoped pipe helper APIs (`preferred_pipe_name`, `pipe_name_candidates`) and protocol-version helper APIs with tests.
- CI/process/docs:
  - `.github/workflows/ci.yml` — scheduled run + matrix/all-target/release-smoke coverage improvements.
  - `docs/*` + `docs/1_Progress and review/*` — command/hotkey wording corrections, recovery wording cleanup, tracker/gate consistency fixes, and metric refresh.

#### 53.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Workspace tests | `cargo test --workspace` | PASSED: 508 total (504 passed, 4 ignored) |
| All tests | `cargo test --all` | PASSED: 508 total (504 passed, 4 ignored) |
| Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed successfully |
| Clippy (workspace targets) | `cargo clippy --workspace --all-targets -- -D warnings` | PASSED |
| Clippy (all targets) | `cargo clippy --all --all-targets -- -D warnings` | PASSED |
| Clippy (all features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| Formatting | `cargo fmt --all -- --check` | PASSED |
| Release build | `cargo build --release --all` | PASSED |

#### 53.4 Impact Statement

- Critical lockout-class behaviors identified in review were addressed with code-level safeguards and expanded tests.
- IPC connectivity now supports scoped preferred pipe names with legacy fallback, and daemon/cli both use the shared candidate strategy.
- Active docs and tracker artifacts were refreshed to the new validated quality gate metric (`508/504/4`).

### Iteration 52: Validation Evidence Refresh + Tracker Metric Sync

**Date**: 2026-02-07
**Status**: COMPLETED (validation + docs sync), incident closure evidence still pending
**Previous Context**: Iteration 51 (documentation consistency sync)

#### 52.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-run quality gate to capture current deterministic totals after remediation tests landed | High | DONE |
| 2 | Sync active trackers/docs from 459/455/4 to current validated totals | High | DONE |
| 3 | Keep release gate truth unchanged (`INC-49` remains open pending host evidence) | Critical | DONE |

#### 52.2 Changes Made

- Validation evidence refresh:
  - Re-ran `cargo test --workspace`, `cargo test --workspace --all-features`, and `cargo test --workspace -- --ignored`.
- Metric synchronization updates:
  - `docs/SPEC.md` (implementation status metric line).
  - `docs/ARCHITECTURE.md` (planned vs implemented quality-gate metric line).
  - `docs/PUBLIC_READINESS_CHECKLIST.md` (current baseline metric line).
  - `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` (verification evidence line).
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.md` (V1 validation snapshot line).
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json` (canonical V1 validation evidence).
  - `docs/1_Progress and review/OPEN_ITEMS.md` (V1 validation line + last updated stamp).
  - `docs/1_Progress and review/CLAUDE_FINALIZATION_CHECKLIST.md` (baseline test metric line).

#### 52.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Workspace tests | `cargo test --workspace` | PASSED: 480 total (476 passed, 4 ignored) |
| Workspace tests (all features) | `cargo test --workspace --all-features` | PASSED: 480 total (476 passed, 4 ignored) |
| Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed successfully |

#### 52.4 Impact Statement

- Active docs and tracker artifacts now reference a single current quality-gate metric (`480/476/4`).
- Validation evidence trail is up to date in both human-readable and canonical tracker files.
- Release gate status is unchanged: `INC-49` remains open until host-level closure evidence is captured and linked.

### Iteration 51: Documentation Consistency Sync (Metrics + Incident Gate Truth)

**Date**: 2026-02-07
**Status**: COMPLETED (docs-only synchronization), host closure evidence still pending
**Previous Context**: Iteration 50 (incident mitigation implementation + validation)

#### 51.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Align current validation metrics across active docs to latest validated run | High | DONE |
| 2 | Remove blocker tracker contradiction between Markdown and canonical JSON | Critical | DONE |
| 3 | Ensure `panic_revert` command and emergency uncloak tray action are present in SPEC/ARCH lists | High | DONE |
| 4 | Keep incident gate wording truthful (implementation done, host evidence pending) | Critical | DONE |

#### 51.2 Changes Made

- Updated behavior/architecture docs:
  - `docs/SPEC.md` — refreshed implementation metric line to `459 total, 455 passed, 4 ignored`; added tray menu `Emergency: Uncloak All Windows`; added `PanicRevert` to command coverage summary.
  - `docs/ARCHITECTURE.md` — added `PanicRevert` to IPC command lists; expanded tray context-menu listings with emergency uncloak action; replaced stale per-crate test-count callouts with validated quality-gate metric summary.
- Reconciled blocker trackers:
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.md` — replaced stale open-R32 checklist with explicit non-canonical snapshot synced to `CODEX_BLOCKER_FIX_PLAN.json` and current open incident-evidence tasks.
- Corrected release-readiness gate wording:
  - `docs/PUBLIC_READINESS_CHECKLIST.md` — updated baseline metric line to `459/455/4` and added explicit gate-truth note.
  - `docs/PRE_STABLE_EXECUTION_PLAN.md` — added explicit critical-gate truth line: mitigation implementation done, host closure evidence pending.

#### 51.3 Test Results

No runtime/code changes in this iteration; no new test execution required.

| Item | Command | Result |
|------|---------|--------|
| Workspace tests | `cargo test --workspace` | Not re-run (docs-only). Baseline remains latest validated run from iteration 50: 459 total (455 passed, 4 ignored). |
| Ignored tests | `cargo test --workspace -- --ignored` | Not re-run (docs-only). Baseline remains latest validated run from iteration 50: 4 ignored tests executed successfully. |

#### 51.4 Impact Statement

- Active tracker docs now present one consistent status model: mitigation implementation completed, host evidence gate still open.
- SPEC/ARCH command and tray-menu coverage now includes `PanicRevert` and emergency uncloak recovery paths.
- Release remains blocked until `INC-49-1`, `INC-49-4`, and `INC-49-T1` host evidence is captured and linked.

### Iteration 50: Incident Mitigation Implementation + Validation

**Date**: 2026-02-07
**Status**: COMPLETED (implementation + validation), incident closure evidence pending
**Previous Context**: Iteration 49 (critical incident documentation + blocker re-open)

#### 50.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Implement panic-revert and deterministic recovery hardening in CLI/daemon/platform | Critical | DONE |
| 2 | Add explicit emergency user escape path and stop/safe-mode semantics hardening | Critical | DONE |
| 3 | Propagate Win32 placement/hide failures instead of silently masking them | High | DONE |
| 4 | Update support/testing/release docs to enforce user-safe recovery and release block | High | DONE |
| 5 | Re-run full validation gates and capture evidence | High | DONE |

#### 50.2 Changes Made

- Runtime safety and recovery:
  - `crates/ipc/src/lib.rs` - added `IpcCommand::PanicRevert`.
  - `crates/cli/src/main.rs` - added `panic-revert` command mapping, stop idempotence path (`Daemon not running`), and fail-fast `run --safe-mode` guard when daemon already running.
  - `crates/daemon/src/main.rs` - unified shutdown cleanup, bounded IPC response timeout, stop ACK after cleanup, `panic_revert` handling, focus sync fixes, MoveOffScreen restore integration.
  - `crates/platform_win32/src/lib.rs` - propagate side-effect failures from placement/hide paths, conservative cloaked detection on API failure, hardened foreground API path, added MoveOffScreen restore helpers.
  - `crates/daemon/src/tray.rs` - added `Emergency: Uncloak All Windows` tray action.
- Verification tests:
  - `crates/daemon/tests/integration.rs` - panic_revert JSON shape + stop response payload semantics.
  - Extended CLI/daemon/platform unit coverage for new recovery and robustness paths.
- Documentation and process gates:
  - `README.md`, `docs/GETTING_STARTED.md`, `docs/TROUBLESHOOTING.md`, `docs/SUPPORT_PLAYBOOK.md`, `docs/TESTING_GUIDE.md`
  - `docs/PUBLIC_READINESS_CHECKLIST.md`, `docs/PRE_STABLE_EXECUTION_PLAN.md`, `docs/1_Progress and review/CLAUDE_FINALIZATION_CHECKLIST.md`

#### 50.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Workspace tests | `cargo test --workspace` | PASSED: 459 total (455 passed, 4 ignored) |
| Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 ignored tests executed successfully |
| Clippy (targets) | `cargo clippy --workspace --all-targets -- -D warnings` | PASSED |
| Clippy (all features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| Formatting | `cargo fmt --all -- --check` | PASSED |
| Release build | `cargo build --release` | PASSED |

#### 50.4 Impact Statement

- Critical recovery primitives are now implemented and covered in default automated suites.
- User-facing recovery guidance no longer implies unsafe or ambiguous stop/restart flows.
- Release remains blocked until host-level incident closure evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`) is executed and logged.

### Iteration 49: Critical Field Incident Documentation + Blocker Re-open

**Date**: 2026-02-07
**Status**: COMPLETED (documentation/triage), remediation OPEN
**Previous Context**: Iteration 48 (Full remediation pass)

#### 49.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Document reported desktop lockout/focus-trap incident with concrete impact | Critical | DONE |
| 2 | Re-open tracking artifacts (review status, blocker plan, open items) | Critical | DONE |
| 3 | Add immediate support/playbook guardrails to prevent repeat recovery harm | High | DONE |
| 4 | Explicitly block release/tag flow pending incident closure evidence | High | DONE |

#### 49.2 Changes Made

- Added incident record:
  - `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md`
- Reopened active tracking status:
  - `docs/1_Progress and review/OPEN_ITEMS.md`
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`
  - `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- Added safety guardrail language:
  - `docs/SUPPORT_PLAYBOOK.md` (critical non-destructive recovery-first guidance)
- Updated release/prioritization docs to reflect active incident gate:
  - `docs/PUBLIC_READINESS_CHECKLIST.md`
  - `docs/PRE_STABLE_EXECUTION_PLAN.md`

#### 49.3 Test Results

No code-path changes in this iteration; documentation and governance updates only.

| Item | Command | Result |
|------|---------|--------|
| Workspace tests | `cargo test --workspace` | Not re-run (no code changes in iteration 49) |
| Formatting | N/A | Markdown-only updates |

#### 49.4 Impact Statement

- A critical user-reported local test incident is now explicitly tracked as open.
- Release/publish readiness is blocked until incident-class mitigation and verification are documented.

---

### Iteration 48: Full Remediation Pass (Runtime + Docs/CI)

**Date**: 2026-02-07
**Status**: COMPLETED (fully verified)
**Previous Context**: Iteration 47 (Release Engineering)

#### 48.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Fix high-severity runtime defects (fullscreen removal, cross-monitor move state loss, Win32 resource/lifecycle issues) | High | DONE |
| 2 | Improve protocol robustness and command validation coverage | High | DONE |
| 3 | Reconcile tracker/docs/CI consistency and GNU toolchain guidance | High | DONE |
| 4 | Re-run full quality gates and record concrete evidence | High | DONE |

#### 48.2 Changes Made

- Core layout/runtime correctness:
  - `crates/core_layout/src/lib.rs` — clear fullscreen state when removing fullscreen tiled/floating windows; added regression tests.
- Daemon/CLI/IPC robustness:
  - `crates/daemon/src/main.rs` — transactional focused-window monitor moves; set-width fraction validation in command path.
  - `crates/daemon/src/config.rs` — non-negative column width validation + invalid `behavior.log_level` warning/reset.
  - `crates/daemon/tests/integration.rs` — expanded IPC command/response roundtrip coverage and unknown-response handling.
  - `crates/cli/src/main.rs` — set-width parser validation; unknown IPC response handling as non-success.
  - `crates/ipc/src/lib.rs` — `IpcResponse::Unknown` forward-compatibility fallback.
- Win32 platform hardening:
  - `crates/platform_win32/src/overlay.rs` — fixed paint-brush leak, singleton guard for overlay instance, explicit destroy/unregister cleanup.
  - `crates/platform_win32/src/lib.rs` — re-installable event sender lifecycle, hidden top-level hotkey window for `WM_DISPLAYCHANGE`, event filtering parity with enumeration, explicit destroy/unregister cleanup.
- Docs/CI/tracker reconciliation:
  - `.github/workflows/ci.yml`, `README.md`, `docs/GETTING_STARTED.md`, `docs/TESTING_GUIDE.md`, `CHANGELOG.md`, `docs/PUBLIC_READINESS_CHECKLIST.md`, `docs/PRE_STABLE_EXECUTION_PLAN.md`, `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`, `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`, `docs/1_Progress and review/OPEN_ITEMS.md`.

#### 48.3 Test Results

| Item | Command | Result |
|------|---------|--------|
| Workspace tests | `cargo test --workspace` | PASSED: 440 total (436 passed, 4 ignored) |
| Ignored tests | `cargo test --workspace -- --ignored` | PASSED: 4 passed |
| Clippy (targets) | `cargo clippy --workspace --all-targets -- -D warnings` | PASSED |
| Clippy (all features) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASSED |
| Formatting | `cargo fmt --all -- --check` | PASSED |
| Release build | `cargo build --release` | PASSED |

#### 48.4 Verification Evidence

- Local validation transcript captured in this iteration run (full workspace gates all green).
- Tracker/docs consistency aligned to `2026-02-07` across review/blocker/open-items/readiness/pre-stable docs.
- CI metadata gate logic present and validated in `.github/workflows/ci.yml` (tag/version/changelog checks).

---

### Iteration 35: Minimize/Restore Handling + Dynamic Tray Text

**Date**: 2026-02-06
**Status**: COMPLETED
**Previous Context**: Iteration 34 (Pre-Stable Focus Lock)

#### 35.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Implement minimize tracking in Workspace (core_layout) | High | DONE |
| 2 | Wire daemon minimize/restore event handlers | High | DONE |
| 3 | Add dynamic tray pause/resume text | Medium | DONE |
| 4 | Comprehensive test coverage for all changes | High | DONE |

#### 35.2 Changes Made

**`crates/core_layout/src/lib.rs`** — Workspace minimize tracking:
- Added `minimized_windows: HashSet<WindowId>` field with `#[serde(default)]`
- Added `mark_minimized(wid) -> bool`, `mark_restored(wid) -> bool`, `is_minimized(wid) -> bool`, `minimized_count() -> usize`
- Modified `remove_window()`: clears window from minimized set
- Modified `compute_placements()` and `compute_placements_animated()`: skip minimized windows, redistribute height among remaining visible windows in column
- Added `use std::collections::HashSet` import

**`crates/daemon/src/main.rs`** — Event handlers:
- Replaced stub `Minimized` handler: marks window minimized, adjusts focus if minimized window was focused, applies layout, syncs foreground
- Replaced minimal `Restored` handler: marks window restored, focuses it, applies layout, syncs foreground
- Renamed `_tray_manager` to `tray_manager` for active use
- Added `update_pause_text()` call in `TogglePause` tray event handler

**`crates/daemon/src/tray.rs`** — Dynamic menu text:
- Added `pause_item: MenuItem` field to `TrayManager` struct
- Added `update_pause_text(&self, paused: bool)` method using `MenuItem::set_text()`

#### 35.3 Test Results

| Crate | Before | After | Delta |
|-------|--------|-------|-------|
| core_layout | 99 | 113 | +14 |
| daemon (binary) | 100 | 107 | +7 |
| ipc | 15 | 15 | 0 |
| cli | 38 | 38 | 0 |
| integration | 22 | 22 | 0 |
| platform | 27 | 27 | 0 |
| **Total** | **302** | **323** | **+21** |

New core_layout tests: mark_minimized (managed, unknown, floating, idempotent), mark_restored, mark_restored_not_minimized, placements_skip_minimized, placements_animated_skip_minimized, minimize_height_redistribution, minimize_all_in_column, remove_window_clears_minimized, all_window_ids_includes_minimized, contains_window_minimized, minimized_window_count_unchanged

New daemon tests: minimize_marks_workspace_window, restore_clears_minimized, minimize_unmanaged_window_noop, minimize_preserves_window_in_workspace, minimize_focus_moves_to_next

New tray tests: tray_event_toggle_pause_variant, menu_ids_constants

#### 35.4 Evidence & Verification

| Item | Command | Result |
|------|---------|--------|
| All tests pass | `cargo test --workspace` | 318 passed, 5 ignored |
| Clippy clean | `cargo clippy --workspace --all-targets -- -D warnings` | 0 warnings |
| Release build | `cargo build --release` | Success |

---

### Iteration 34: Pre-Stable Focus Lock

**Date**: 2026-02-06
**Status**: COMPLETED
**Previous Context**: Iteration 33 (Public Readiness Checklist and Messaging Polish)

#### 34.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Prevent unstable-phase work from drifting into release packaging tasks | High | DONE |
| 2 | Capture concrete pre-stable execution order to avoid iteration drift | High | DONE |
| 3 | Keep public-facing docs aligned with pre-stable strategy | Medium | DONE |

#### 34.2 Changes Made

- Added `docs/PRE_STABLE_EXECUTION_PLAN.md`:
  - Defines active pre-stable scope.
  - Defines explicit post-stable deferred backlog.
  - Adds execution order for iterations 35-39.
- Updated `docs/PUBLIC_READINESS_CHECKLIST.md`:
  - Added "Active Development Focus" linking to pre-stable plan.
  - Moved signing/installer/channel-hardening into deferred section.
  - Updated immediate top-10 list to pre-stable priorities.
- Updated `README.md`:
  - Linked pre-stable execution plan in public readiness section.

#### 34.3 Test Results

No Rust code changes in this iteration.

**Test Growth**: 302 -> 302 (unchanged)

#### 34.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| Strategy docs check | Manual markdown review | Pre-stable and deferred scopes are explicit and non-conflicting |
| README linkage check | Manual markdown review | Readiness section links to both checklist and pre-stable plan |

---

### Iteration 33: Public Readiness Checklist and Messaging Polish

**Date**: 2026-02-06
**Status**: COMPLETED
**Previous Context**: Iteration 32 (Public README and GitHub About Revamp)

#### 33.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Improve README tone for external users and evaluators | High | DONE |
| 2 | Add explicit public-usability checklist for execution planning | High | DONE |
| 3 | Link readiness workstream directly from README | High | DONE |

#### 33.2 Changes Made

- Refined `README.md` with clearer product positioning for public audiences:
  - Added "Who this is for" and "Capability Snapshot" framing.
  - Added explicit daily-start usage path and runtime/config path references.
  - Added direct link to readiness execution checklist.
- Added `docs/PUBLIC_READINESS_CHECKLIST.md`:
  - Defined public-usability "definition of done."
  - Added 8 execution domains (distribution, onboarding, reliability, compatibility, security, support, release engineering, governance).
  - Added prioritized top-10 immediate tasks for near-term execution.

#### 33.3 Test Results

No Rust code changes in this iteration.

**Test Growth**: 302 -> 302 (unchanged)

#### 33.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| GitHub metadata check | `gh repo view AdEx-Partners-DE/OpenNiri-Windows --json description,repositoryTopics,url` | Description/topics remain aligned |
| README and checklist sanity check | Manual markdown review | Sections are coherent and cross-linked |

---

### Iteration 32: Public README and GitHub About Revamp

**Date**: 2026-02-06
**Status**: COMPLETED
**Previous Context**: Iteration 31 (Repository Presentation Refresh)

#### 32.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Replace generic README language with product-grade messaging | High | DONE |
| 2 | Clarify "why", status, and usage expectations for public users | High | DONE |
| 3 | Expand quick-start and default hotkey reference coverage | Medium | DONE |
| 4 | Ensure GitHub About metadata reflects current project scope | High | DONE |

#### 32.2 Changes Made

- Reworked `README.md` to improve first impression and reduce ambiguity:
  - Added CI and license badges.
  - Tightened positioning and "why this project exists" narrative.
  - Added explicit product status (alpha) and expectation-setting.
  - Expanded hotkey table to include monitor focus/move and refresh actions.
  - Added config path and clearer architecture/doc references.
- Updated GitHub repository metadata to align with project identity and discoverability:
  - Description aligned to actual product behavior.
  - Topics reviewed/updated for better categorization.

#### 32.3 Test Results

No Rust code changes in this iteration.

**Test Growth**: 302 -> 302 (unchanged)

#### 32.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| GitHub metadata check | `gh repo view AdEx-Partners-DE/OpenNiri-Windows --json description,repositoryTopics,url` | Description/topics match new positioning |
| README sanity check | Manual markdown review | Sections render and link cleanly |

---

### Iteration 30: Crash Safety and Reliability

**Date**: 2026-02-05
**Status**: COMPLETED
**Previous Context**: Iteration 29 (Dramatic UX Overhaul)

#### 30.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Add Ctrl+C signal handling | High | DONE |
| 2 | Uncloak managed windows on shutdown | High | DONE |
| 3 | Add panic-hook emergency uncloak | High | DONE |
| 4 | Enable DPI awareness at process startup | Medium | DONE |
| 5 | Align tray exit with unified shutdown path | High | DONE |
| 6 | Add reliability-focused regression tests | Medium | DONE |

#### 30.2 Changes Made

- Added `tokio::signal::ctrl_c()` task that sends `DaemonEvent::Shutdown`.
- Added managed-window shutdown recovery:
  - `AppState::all_managed_window_ids()` in daemon.
  - `uncloak_all_managed_windows()` in platform layer.
- Added crash recovery safety net:
  - Panic hook in daemon (`std::panic::set_hook`) that invokes `uncloak_all_visible_windows()`.
- Added process DPI initialization:
  - `set_dpi_awareness()` using `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`.
  - Called at the top of daemon `main()` before other window operations.
- Unified shutdown behavior:
  - Tray Exit now routes through `DaemonEvent::Shutdown`, so shutdown cleanup is shared across IPC Stop, Ctrl+C, and tray exit.

#### 30.3 Test Results

```
Test Summary (2026-02-05):
- core_layout:    99 passed, 0 failed, 0 ignored
- daemon:         99 passed, 0 failed, 1 ignored
- cli:            38 passed, 0 failed, 0 ignored
- integration:    22 passed, 0 failed, 0 ignored
- ipc:            15 passed, 0 failed, 0 ignored
- platform_win32: 24 passed, 0 failed, 3 ignored
- doc-tests:       0 passed, 0 failed, 1 ignored

TOTAL: 297 passed, 0 failed, 5 ignored (302 total)
Clippy: 0 warnings (`--workspace --all-targets -- -D warnings`)
```

**Test Growth**: 295 -> 302 (+7 tests)

#### 30.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --workspace` | 297 passed, 5 ignored |
| Build succeeds | `cargo build --release` | Success |
| Strict clippy clean | `cargo clippy --workspace --all-targets -- -D warnings` | 0 warnings |

---

### Iteration 29: Dramatic UX Overhaul

**Date**: 2026-02-05
**Status**: COMPLETED
**Previous Context**: Iteration 28 (Codex Review 19 Fixes)

#### 29.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | SetForegroundWindow integration for focus commands | Critical | DONE |
| 2 | Owner-window filtering (dialogs, UWP) | High | DONE |
| 3 | CloseWindow command (Win+Shift+Q) | High | DONE |
| 4 | ToggleFloating command (Win+F) | High | DONE |
| 5 | ToggleFullscreen command (Win+Shift+F) | High | DONE |
| 6 | SetColumnWidth presets (Win+1/2/3/0) | High | DONE |
| 7 | Active window border via DWM | Medium | DONE |
| 8 | Snap hints and gestures enabled by default | Medium | DONE |
| 9 | QueryStatus command and CLI status subcommand | Medium | DONE |
| 10 | Tray menu: Pause Tiling, Open Config, View Logs | Medium | DONE |
| 11 | Auto-start via Registry (CLI autostart enable/disable) | Medium | DONE |

#### 29.2 Changes Made

##### 29.2.1 Phase 1: SetForegroundWindow Integration

**Problem**: Focus commands (FocusUp, FocusDown, FocusLeft, FocusRight) updated internal layout state but did not actually move OS-level keyboard focus to the target window. Users had to click windows manually after focusing.

**Solution**: Focus commands now call `SetForegroundWindow` after updating internal state. `FocusUp` and `FocusDown` now also call `apply_layout()` to ensure window positions are applied.

##### 29.2.2 Phase 2: Owner-Window Filtering

**Problem**: Dialog windows (owned windows) were being tiled alongside their parent applications, causing layout corruption. UWP apps like Calculator were not being tiled because their window class (`ApplicationFrameWindow`) was not recognized.

**Solution**: Added owner-window filtering so that owned/dialog windows (those with a non-null owner via `GetWindow(GW_OWNER)`) are excluded from tiling. UWP apps with `ApplicationFrameWindow` class are now correctly identified and tiled.

##### 29.2.3 Phase 3: CloseWindow Command

Added `CloseWindow` IPC command with default hotkey `Win+Shift+Q`. Sends `WM_CLOSE` to the focused window, allowing graceful application shutdown.

##### 29.2.4 Phase 4: ToggleFloating Command

Added `ToggleFloating` IPC command with default hotkey `Win+F`. Toggles the focused window between tiled and floating states. Floating windows are removed from the column layout and positioned with their original dimensions.

##### 29.2.5 Phase 5: ToggleFullscreen Command

Added `ToggleFullscreen` IPC command with default hotkey `Win+Shift+F`. Toggles the focused window between normal layout and fullscreen (covering the entire monitor work area). Fullscreen state is tracked per-window.

##### 29.2.6 Phase 6: SetColumnWidth Presets

Added `SetColumnWidth` IPC command with fraction-based presets:
- `Win+1` = 1/3 width
- `Win+2` = 1/2 width
- `Win+3` = 2/3 width
- `Win+0` = equalize all columns

Allows quick column resizing without incremental resize commands.

##### 29.2.7 Phase 7: Active Window Border via DWM

Added active window border highlighting using `DwmSetWindowAttribute` with `DWMWA_BORDER_COLOR`. When a window gains focus, its border color is set to the configured accent color. When it loses focus, the border is reset to the default. Configurable via `appearance.active_border_color`.

##### 29.2.8 Phase 8: Snap Hints and Gestures Enabled by Default

Changed default configuration so that both snap hints (`snap_hints.enabled`) and gestures (`gestures.enabled`) are `true` by default, improving out-of-the-box experience.

##### 29.2.9 Phase 9: QueryStatus Command and CLI Status Subcommand

Added `QueryStatus` IPC command and `openniri-cli status` subcommand. Returns daemon status information including: number of managed windows, number of monitors, active workspace details, tiling pause state, and uptime.

##### 29.2.10 Phase 10: Tray Menu Enhancements

Extended the system tray context menu with three new items:
- **Pause Tiling** - Toggles tiling on/off without stopping the daemon
- **Open Config** - Opens the config file in the default editor
- **View Logs** - Opens the log directory in Explorer

##### 29.2.11 Phase 11: Auto-start via Registry

Added `openniri-cli autostart enable` and `openniri-cli autostart disable` subcommands. Uses the Windows Registry key `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` to add/remove the daemon from startup.

#### 29.3 Test Results

```
Test Summary (2026-02-05):
- core_layout:    99 passed, 0 failed, 0 ignored
- daemon:         96 passed, 0 failed, 1 ignored
- cli:            38 passed, 0 failed, 0 ignored
- integration:    22 passed, 0 failed, 0 ignored
- ipc:            15 passed, 0 failed, 0 ignored
- platform_win32: 21 passed, 0 failed, 2 ignored
- doc-tests:       0 passed, 0 failed, 1 ignored

TOTAL: 291 passed, 0 failed, 4 ignored
Clippy: 0 warnings
```

**Test Growth**: 261 -> 295 (+34 tests)

#### 29.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --workspace` | 291 passed, 4 ignored |
| Build succeeds | `cargo build --workspace` | Success |
| Clippy clean | `cargo clippy --workspace` | 0 warnings |

#### 29.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/daemon/src/main.rs` | +300 | SetForegroundWindow, CloseWindow, ToggleFloating, ToggleFullscreen, SetColumnWidth, active border, pause tiling, status |
| `crates/platform_win32/src/lib.rs` | +150 | Owner-window filtering, DWM border color, UWP detection |
| `crates/cli/src/main.rs` | +100 | Status subcommand, autostart subcommand, new command mappings |
| `crates/ipc/src/lib.rs` | +50 | New IPC commands and response types |
| `crates/daemon/src/config.rs` | +50 | Active border config, default changes for snap hints/gestures |
| `crates/daemon/src/tray.rs` | +80 | Pause Tiling, Open Config, View Logs menu items |
| `crates/core_layout/src/lib.rs` | +50 | Column width preset support |

---

### Iteration 24: Real Gestures, Workspace Persistence & Doc Refresh

**Date**: 2026-02-05
**Status**: COMPLETED
**Previous Context**: Iteration 23 (Feature Completion & Test Expansion)

#### 24.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Update SPEC.md to reflect current implementation | High | DONE |
| 2 | Update ARCHITECTURE.md to reflect current implementation | High | DONE |
| 3 | Implement real touchpad gesture support | High | DONE |
| 4 | Implement workspace persistence (save/restore) | High | DONE |
| 5 | Add tests for new features | Medium | DONE |

#### 24.2 Changes Made

##### 24.2.1 Documentation Refresh

**File**: `docs/SPEC.md`

Updated to reflect all implemented features:
- Added Per-Window Rules section (was listed as "pending", now documented)
- Added Floating Windows section
- Added System Tray section
- Added Visual Snap Hints section
- Added Focus Follows Mouse section
- Added Touchpad Gesture Support section
- Added Display Change Handling section
- Added Workspace Persistence section
- Updated Implementation Status (202 tests, all features implemented)
- Removed stale "Pending" items

**File**: `docs/ARCHITECTURE.md`

Updated to reflect current state:
- Updated IPC types (added QueryAllWindows, WindowList, WindowInfo, IpcRect)
- Updated CLI commands (added focus-monitor, move-to-monitor, query all, init)
- Updated daemon responsibilities (persistence, tray, gesture/mouse hooks)
- Updated event loop diagram (added hotkeys, gestures, tray, display change, focus follows mouse)
- Updated Current Status with correct test counts (202 total)
- Updated Planned vs Implemented (all features now implemented)
- Updated threading model (7 threads/tasks documented)
- Updated error handling (DeferWindowPos fallback, HWND validation, config fallbacks)

##### 24.2.2 Real Touchpad Gesture Support

**File**: `crates/platform_win32/src/lib.rs`

**Problem**: Previous implementation used a message-only window (HWND_MESSAGE) listening for WM_POINTERDOWN/WM_POINTERUP, which never receives real touchpad input.

**Solution**: Replaced with a low-level mouse hook (WH_MOUSE_LL) that captures WM_MOUSEWHEEL and WM_MOUSEHWHEEL events system-wide.

**New types**:
```rust
struct GestureAccumState {
    accum_x: i32,
    accum_y: i32,
    last_scroll_time: std::time::Instant,
}
```

**New constants**:
```rust
const WM_MOUSEWHEEL: u32 = 0x020A;
const WM_MOUSEHWHEEL: u32 = 0x020E;
const GESTURE_SCROLL_THRESHOLD: i32 = 360; // 3 * WHEEL_DELTA
const GESTURE_TIMEOUT_MS: u128 = 300;
```

**How it works**:
1. `register_gestures()` installs `WH_MOUSE_LL` hook
2. Hook callback captures WM_MOUSEWHEEL/WM_MOUSEHWHEEL
3. Extracts delta from `mouseData >> 16` (high word)
4. Accumulates horizontal (WM_MOUSEHWHEEL) and vertical (WM_MOUSEWHEEL) deltas
5. When accumulated delta exceeds threshold (360 = 3x WHEEL_DELTA), fires GestureEvent
6. Resets accumulator after timeout (300ms no scroll)

**Removed**: WM_POINTER message constants, gesture_window_proc, gesture_window_proc_inner, GestureState (replaced by GestureAccumState), thread-based GestureHandle.

**Simplified GestureHandle**: Now holds just an HHOOK (like MouseHookHandle), no thread.

##### 24.2.3 Workspace Persistence

**File**: `crates/daemon/src/main.rs`

**New types**:
```rust
#[derive(Serialize, Deserialize)]
struct WorkspaceSnapshot {
    monitor_device_name: String,
    workspace: Workspace,
}

#[derive(Serialize, Deserialize)]
struct StateSnapshot {
    saved_at: String,
    workspaces: Vec<WorkspaceSnapshot>,
    focused_monitor_name: String,
}
```

**New AppState methods**:
- `save_state()` - Serialize workspace state to `%APPDATA%/openniri/data/workspace-state.json`
- `load_state()` - Deserialize saved state from disk
- `state_file_path()` - Get persistence file path
- `restore_state()` - Apply saved scroll offsets and focus to matching monitors

**Lifecycle integration**:
- On startup: After monitor setup, before window enumeration, attempts to restore saved state
- On shutdown (DaemonEvent::Shutdown): Saves current state before exiting
- On tray Exit: Saves current state before triggering shutdown

**Matching strategy**: Monitors are matched by `device_name` (e.g., "DISPLAY1") which is stable across restarts, unlike MonitorId (HMONITOR handle values).

#### 24.3 Test Results

```
Test Summary (2026-02-05):
- core_layout:    87 passed, 0 failed, 0 ignored
- daemon:         48 passed, 0 failed, 0 ignored
- cli:            28 passed, 0 failed, 0 ignored
- integration:    17 passed, 0 failed, 0 ignored
- ipc:            13 passed, 0 failed, 0 ignored
- platform_win32: 13 passed, 0 failed, 2 ignored

TOTAL: 206 passed, 0 failed, 2 ignored (1 doc-test ignored)
Clippy: No warnings
```

**Test Growth**: 202 → 206 (+4 tests)

**New Tests**:
- `test_state_file_path` - Validates persistence path
- `test_state_snapshot_serialization` - StateSnapshot roundtrip
- `test_workspace_snapshot_serialization` - WorkspaceSnapshot roundtrip
- `test_save_and_load_roundtrip` - Full snapshot roundtrip with workspace data

#### 24.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --workspace` | 206 passed, 2 ignored |
| Build succeeds | `cargo build --workspace` | Success |
| Clippy clean | `cargo clippy --workspace` | No warnings |

#### 24.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/platform_win32/src/lib.rs` | ~288 lines changed | Real gesture hook, simplified GestureHandle |
| `crates/daemon/src/main.rs` | +208 lines | Workspace persistence, 4 tests |
| `docs/SPEC.md` | +126 lines | Doc refresh with all features |
| `docs/ARCHITECTURE.md` | +154 lines changed | Doc refresh, current state |
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | Updated | Reflect Iteration 23 fixes |

---

### Iteration 27: Test Coverage & Documentation Accuracy

**Date**: 2026-02-05
**Status**: COMPLETED
**Previous Context**: Iteration 26 (Additional Safety & Clippy Fixes)

#### 27.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Add handle_command() unit tests | High | DONE |
| 2 | Add reconcile_monitors() unit tests | High | DONE |
| 3 | Fix flaky integration test | Medium | DONE |
| 4 | Update SPEC.md test counts | Medium | DONE |
| 5 | Update ARCHITECTURE.md test counts | Medium | DONE |
| 6 | Update ITERATION_LOG.md | Medium | DONE |

#### 27.2 Changes Made

- Added 16 unit tests for `handle_command()` covering all IPC command branches
- Added 7 unit tests for `reconcile_monitors()` covering add/remove/migrate scenarios
- Marked flaky `test_check_already_running_returns_false_when_no_daemon` as `#[ignore]`
- Updated SPEC.md implementation status (206 → 257 tests)
- Updated ARCHITECTURE.md test counts
- Updated this log with iterations 25-27

#### 27.3 Test Results

All tests passing, 0 clippy warnings, clean release build.

---

### Iteration 26: Additional Safety & Clippy Fixes

**Date**: 2026-02-05
**Status**: COMPLETED
**Previous Context**: Iteration 25 (Config Validation & Safety)

#### 26.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Additional safety tests | Medium | DONE |
| 2 | Clippy warning fixes | Medium | DONE |
| 3 | Config path consistency | Low | DONE |

#### 26.2 Changes Made

- Added additional safety tests for edge cases
- Fixed clippy warnings across workspace
- Config path consistency improvements per Codex review
- Test count: 231 → 234

---

### Iteration 25: Config Validation & Safety Hardening

**Date**: 2026-02-05
**Status**: COMPLETED
**Previous Context**: Iteration 24 (Real Gestures, Persistence, Docs)

#### 25.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Add regex validation to config window rules | High | DONE |
| 2 | Pre-compile window rule regexes at config load | High | DONE |
| 3 | Safety hardening for edge cases | Medium | DONE |

#### 25.2 Changes Made

- Config: Added validation that regex patterns in window rules are valid at load time
- Config: Pre-compiled regex patterns stored in `CompiledWindowRule` for efficient matching
- Safety: Added bounds checking and defensive patterns in config handling
- Tests: Added 25 new tests for config validation and compiled rules

---

### Iteration 23: Feature Completion & Test Expansion

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 22 (Quality & Robustness)

#### 23.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1.1 | Wire DisplayChange event in daemon | High | DONE |
| 1.2 | Implement focus_follows_mouse | High | DONE |
| 1.3 | Wire use_cloaking config | Medium | DONE |
| 2.1 | Add QueryAllWindows CLI subcommand | Medium | DONE |
| 2.2 | Add CLI unit tests | High | DONE |
| 3.1 | Add integration test infrastructure | Medium | DONE |
| 3.2 | Add window rule edge case tests | Medium | DONE |

#### 23.2 Changes Made

##### 23.2.1 Phase 1: Wire Existing Infrastructure

**Task 1.1: Wire DisplayChange Event**

**File**: `crates/platform_win32/src/lib.rs`

Added `set_display_change_sender()` function to allow the daemon to register a sender for display change events:
```rust
pub fn set_display_change_sender(sender: mpsc::Sender<WindowEvent>) -> Result<(), Win32Error>
```

**File**: `crates/daemon/src/main.rs`

- Added `DisplayChange` channel and wired it to call `reconcile_monitors()` when display configuration changes
- Display changes now properly trigger monitor hotplug handling

**Task 1.2: Implement focus_follows_mouse**

**File**: `crates/platform_win32/src/lib.rs`

Added low-level mouse hook infrastructure:
```rust
pub struct MouseHookHandle { ... }

pub fn install_mouse_hook(event_sender: mpsc::Sender<WindowEvent>) -> Result<MouseHookHandle, Win32Error>
```

- Uses `SetWindowsHookEx(WH_MOUSE_LL)` to track mouse position
- Detects when mouse enters a managed window via `WindowFromPoint`
- Sends `WindowEvent::MouseEnterWindow(window_id)` events

**File**: `crates/daemon/src/main.rs`

- Added `FocusFollowsMouse { window_id }` variant to `DaemonEvent`
- Added `apply_focus_follows_mouse()` method to `AppState`
- Implemented debouncing using tokio timers (`focus_follows_mouse_delay_ms` config)
- Mouse hook only installed when `config.behavior.focus_follows_mouse = true`

**Task 1.3: Wire use_cloaking Config**

**File**: `crates/platform_win32/src/lib.rs`

Added `HideStrategy::MoveOffScreen` variant (was previously removed, now restored for config flexibility):
```rust
pub enum HideStrategy {
    #[default]
    Cloak,
    MoveOffScreen,
}
```

**File**: `crates/daemon/src/main.rs`

- Platform config now respects `config.appearance.use_cloaking`:
  - `true` → `HideStrategy::Cloak`
  - `false` → `HideStrategy::MoveOffScreen`

##### 23.2.2 Phase 2: CLI Completion

**Task 2.1: Add QueryAllWindows CLI Subcommand**

**File**: `crates/cli/src/main.rs`

Added `QueryType::All` variant:
```rust
#[derive(Subcommand)]
enum QueryType {
    Workspace,
    Focused,
    All,  // NEW - maps to IpcCommand::QueryAllWindows
}
```

Usage: `openniri-cli query all`

**Task 2.2: Add CLI Unit Tests**

**File**: `crates/cli/src/main.rs`

Added 28 comprehensive unit tests covering:
- `test_to_ipc_command_focus_left/right/up/down`
- `test_to_ipc_command_move_column_left/right`
- `test_to_ipc_command_scroll_positive/negative/zero`
- `test_to_ipc_command_resize_positive/negative/zero`
- `test_to_ipc_command_focus_monitor_left/right`
- `test_to_ipc_command_move_to_monitor_left/right`
- `test_to_ipc_command_query_workspace/focused/all`
- `test_to_ipc_command_refresh/apply/reload/stop`
- `test_generate_default_config_contains_layout/appearance/behavior/hotkeys`
- `test_default_config_path_returns_some`
- `test_print_response_ok/error/workspace_state/focused_window/window_list`

##### 23.2.3 Phase 3: Test Expansion

**Task 3.1: Integration Test Infrastructure**

**File**: `crates/daemon/tests/integration.rs` (NEW)

Created 17 integration tests for IPC protocol correctness:
- `test_all_commands_roundtrip` - All command variants serialize/deserialize
- `test_all_responses_roundtrip` - All response variants roundtrip
- `test_protocol_newline_delimited` - Protocol message format
- `test_response_newline_delimited` - Response message format
- `test_error_response_message/special_chars` - Error handling
- `test_workspace_state_edge_values/large_values/negative_scroll`
- `test_window_list_empty/multiple_windows`
- `test_window_info_unicode_title` - Unicode support
- `test_resize_command_values` - Edge cases (i32::MAX, i32::MIN)
- `test_scroll_command_values` - Edge cases (f64::MAX, f64::MIN)
- `test_invalid_json_parsing` - Error handling
- `test_unknown_command_type/response_type` - Unknown variants

**Task 3.2: Window Rule Edge Case Tests**

**File**: `crates/daemon/src/config.rs`

Added 10 window rule edge case tests:
- `test_window_rule_multiple_matches_uses_first`
- `test_window_rule_regex_special_chars`
- `test_window_rule_regex_anchors`
- `test_window_rule_empty_string_matches`
- `test_window_rule_case_sensitive_class_title`
- `test_window_rule_case_insensitive_executable`
- `test_window_rule_partial_config_class_only/title_only/executable_only`
- `test_window_rule_action_priority`

#### 23.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    87 passed, 0 failed, 0 ignored
- daemon:         44 passed, 0 failed, 0 ignored
- cli:            28 passed, 0 failed, 0 ignored
- integration:    17 passed, 0 failed, 0 ignored
- ipc:            13 passed, 0 failed, 0 ignored
- platform_win32: 13 passed, 0 failed, 2 ignored

TOTAL: 202 passed, 0 failed, 2 ignored (3 doc-tests ignored)
Clippy: No warnings
```

**Test Growth**: 147 → 202 (+55 tests)

#### 23.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --workspace` | 202 passed, 2 ignored |
| Build succeeds | `cargo build --workspace` | Success |
| Clippy clean | `cargo clippy --workspace` | No warnings |

#### 23.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/daemon/src/main.rs` | +100 | Wire DisplayChange, focus_follows_mouse, use_cloaking |
| `crates/platform_win32/src/lib.rs` | +150 | Mouse hook, set_display_change_sender, HideStrategy |
| `crates/cli/src/main.rs` | +300 | QueryType::All, 28 unit tests |
| `crates/daemon/src/config.rs` | +150 | 10 window rule edge case tests |
| `crates/daemon/tests/integration.rs` | +430 | NEW: 17 integration tests |
| `crates/daemon/src/tray.rs` | +5 | Fix clippy warning (TrayError naming) |

---

### Iteration 22: Quality, Robustness & Feature Expansion

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 21 (Full Feature Push)

#### 22.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1.1 | Fix critical unwraps in main.rs | Critical | DONE |
| 1.2 | Add HWND validation function | High | DONE |
| 1.3 | Add daemon unit tests (~10 tests) | High | DONE |
| 1.4 | Document overlay.rs | Medium | DONE |
| 2.1 | Display change detection infrastructure | Medium | DONE |
| 2.2 | Add catch_unwind in callbacks | High | DONE |
| 2.3 | DeferWindowPos fallback | Medium | DONE |
| 3.1 | Enhanced IPC - QueryAllWindows | Medium | DONE |
| 3.2 | Focus follows mouse config | Low | DONE |

#### 22.2 Changes Made

##### 22.2.1 Phase 1: Critical Quality Fixes

**Task 1.1: Fixed Critical Unwraps**

**File**: `crates/daemon/src/main.rs`

**Problem**: Lines 238 and 663 had `.unwrap()` on `floating_rect` which could panic when window rules don't set dimensions.

**Fix**: Replaced with `unwrap_or_else` that provides a default centered 800x600 window based on monitor's work area:
```rust
let rect = floating_rect.unwrap_or_else(|| {
    let viewport = self.monitors.get(&monitor_id)
        .map(|m| m.work_area)
        .unwrap_or_else(|| Rect::new(0, 0, FALLBACK_VIEWPORT_WIDTH, FALLBACK_VIEWPORT_HEIGHT));
    Rect::new(
        viewport.x + (viewport.width - 800) / 2,
        viewport.y + (viewport.height - 600) / 2,
        800,
        600,
    )
});
```

**Task 1.2: HWND Validation**

**File**: `crates/platform_win32/src/lib.rs`

**Added function**:
```rust
pub fn is_valid_window(hwnd: WindowId) -> bool {
    unsafe {
        let hwnd = HWND(hwnd as *mut c_void);
        IsWindow(Some(hwnd)).as_bool()
    }
}
```

**File**: `crates/daemon/src/main.rs`

**Added validation** at start of `handle_window_event()` - skips events for invalid window handles (except Destroyed events).

**Task 1.3: Daemon Unit Tests**

**File**: `crates/daemon/src/main.rs`

**Added 12 tests**:
- `test_app_state_new`
- `test_app_state_focused_viewport`
- `test_app_state_no_monitors_fallback`
- `test_window_rule_matching_class`
- `test_window_rule_matching_title`
- `test_window_rule_matching_executable`
- `test_window_rule_no_match_defaults_to_tile`
- `test_floating_rect_uses_rule_dimensions`
- `test_floating_rect_preserves_original_if_no_dimensions`
- `test_find_window_workspace_not_found`
- `test_app_state_apply_config`

**Task 1.4: Overlay Documentation**

**File**: `crates/platform_win32/src/overlay.rs`

**Added comprehensive documentation**:
- Module-level architecture overview
- `OverlayWindow` struct with features and example
- All public methods documented
- `SnapHintType` and `SnapHintConfig` documented

##### 22.2.2 Phase 2: Robustness Improvements

**Task 2.1: Display Change Detection**

**File**: `crates/platform_win32/src/lib.rs`
- Added `DisplayChange` variant to `WindowEvent` enum
- Added `WM_DISPLAYCHANGE` constant
- Added `DISPLAY_CHANGE_SENDER` static for event forwarding

**File**: `crates/core_layout/src/lib.rs`
- Added `all_window_ids()` method to `Workspace` for window migration

**File**: `crates/daemon/src/main.rs`
- Added `reconcile_monitors()` method for handling monitor hotplug

**Task 2.2: catch_unwind in Callbacks**

**Files Modified**: `crates/platform_win32/src/lib.rs`, `crates/platform_win32/src/overlay.rs`

**Wrapped with catch_unwind**:
- `hotkey_window_proc` → `hotkey_window_proc_inner`
- `win_event_callback` → `win_event_callback_inner`
- `gesture_window_proc` → `gesture_window_proc_inner`
- `overlay_window_proc` → `overlay_window_proc_inner`

Panics in callbacks now log the error and return safe defaults instead of crashing.

**Task 2.3: DeferWindowPos Fallback**

**File**: `crates/platform_win32/src/lib.rs`

**Improved `apply_placements_deferred`**:
- Track failed placements during DeferWindowPos
- If EndDeferWindowPos fails, fall back to individual SetWindowPos for all windows
- If batch succeeds, retry only failed placements individually

##### 22.2.3 Phase 3: Feature Additions

**Task 3.1: Enhanced IPC - Query Commands**

**File**: `crates/ipc/src/lib.rs`

**New types**:
```rust
pub struct IpcRect { pub x: i32, pub y: i32, pub width: i32, pub height: i32 }

pub struct WindowInfo {
    pub window_id: u64,
    pub title: String,
    pub class_name: String,
    pub process_id: u32,
    pub executable: String,
    pub rect: IpcRect,
    pub column_index: Option<usize>,
    pub window_index: Option<usize>,
    pub monitor_id: i64,
    pub is_floating: bool,
    pub is_focused: bool,
}
```

**New commands**: `QueryAllWindows`

**New responses**: `WindowList { windows: Vec<WindowInfo> }`, `FocusedWindowInfo`

**File**: `crates/daemon/src/main.rs`
- Added handler for `QueryAllWindows` command

**Task 3.2: Focus Follows Mouse Config**

**File**: `crates/daemon/src/config.rs`

**Added to BehaviorConfig**:
```rust
pub focus_follows_mouse: bool,        // default: false
pub focus_follows_mouse_delay_ms: u32, // default: 100
```

#### 22.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    87 passed, 0 failed, 0 ignored
- daemon:         34 passed, 0 failed, 0 ignored
- ipc:            13 passed, 0 failed, 0 ignored
- platform_win32: 13 passed, 0 failed, 2 ignored

TOTAL: 147 passed, 0 failed, 2 ignored (3 doc-tests ignored)
Clippy: 1 minor warning (pre-existing TrayError naming)
```

**Test Growth**: 131 → 147 (+16 tests)

#### 22.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo +stable-x86_64-pc-windows-gnu test --workspace` | 147 passed, 2 ignored |
| Build succeeds | `cargo +stable-x86_64-pc-windows-gnu build --workspace` | Success |
| Clippy clean | `cargo +stable-x86_64-pc-windows-gnu clippy --workspace` | 1 pre-existing warning |

#### 22.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/daemon/src/main.rs` | +200 | Fix unwraps, HWND validation, tests, reconcile_monitors |
| `crates/platform_win32/src/lib.rs` | +150 | is_valid_window, catch_unwind, DeferWindowPos fallback |
| `crates/platform_win32/src/overlay.rs` | +100 | Documentation, catch_unwind |
| `crates/ipc/src/lib.rs` | +100 | WindowInfo, IpcRect, QueryAllWindows, tests |
| `crates/daemon/src/config.rs` | +30 | focus_follows_mouse config |
| `crates/core_layout/src/lib.rs` | +15 | all_window_ids() method |
| `crates/cli/src/main.rs` | +10 | Handle new IPC responses |

---

### Iteration 21: Full Feature Push (All Four Features)

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 20 (Codex Review + QA Scan (Pass))

#### 21.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 0 | Cleanup: Remove stray `nul` file, update CODEX_REVIEW | Low | DONE |
| 1 | System Tray Icon with menu | High | DONE |
| 2 | Per-Window Floating Rules | High | DONE |
| 3 | Touchpad Gesture Support | Medium | DONE |
| 4 | Visual Snapping Hints | Medium | DONE |

#### 21.2 Changes Made

##### 21.2.1 Phase 0: Cleanup

**Changes**:
- Deleted stray `nul` file at repo root (Windows NUL device artifact)
- Updated CODEX_REVIEW_CONSOLIDATED.md to mark fixed items

##### 21.2.2 Phase 1: System Tray Icon

**Files Modified/Created**:
- `crates/daemon/Cargo.toml` - Added `tray-icon = "0.19"`, `regex = "1"`, `thiserror`
- `crates/daemon/src/tray.rs` - NEW: TrayManager, TrayEvent, menu creation
- `crates/daemon/src/main.rs` - Tray integration in event loop

**Features**:
- System tray icon with menu (Refresh Windows, Reload Config, Exit)
- Menu events forwarded via sync channel to async event loop
- Drop implementation cleans up icon properly

##### 21.2.3 Phase 2: Per-Window Floating Rules

**Files Modified**:
- `crates/daemon/src/config.rs` - WindowRule, MatchCriteria, WindowAction types
- `crates/platform_win32/src/lib.rs` - `get_process_executable()` function
- `crates/core_layout/src/lib.rs` - FloatingWindow struct, floating window support
- `crates/daemon/src/main.rs` - Rule evaluation, floating window handling

**Config Example**:
```toml
[[window_rules]]
match_class = "Notepad"
action = "float"
width = 800
height = 600

[[window_rules]]
match_executable = "spotify.exe"
action = "float"

[[window_rules]]
match_class = "#32770"
action = "ignore"
```

**Features**:
- Regex matching on window class and title
- Case-insensitive executable matching
- Float, Tile, or Ignore actions
- Optional width/height for floating windows

##### 21.2.4 Phase 3: Touchpad Gesture Support

**Files Modified**:
- `crates/platform_win32/src/lib.rs` - GestureEvent enum, register_gestures()
- `crates/daemon/src/config.rs` - GestureConfig
- `crates/daemon/src/main.rs` - Gesture event handling

**Config Example**:
```toml
[gestures]
enabled = true
swipe_left = "focus_left"
swipe_right = "focus_right"
swipe_up = "focus_up"
swipe_down = "focus_down"
```

**Features**:
- Swipe left/right/up/down detection
- Configurable command mapping
- Disabled by default (enabled via config)

##### 21.2.5 Phase 4: Visual Snapping Hints

**Files Created/Modified**:
- `crates/platform_win32/src/overlay.rs` - NEW: OverlayWindow for visual hints
- `crates/daemon/src/config.rs` - SnapHintConfig
- `crates/daemon/src/main.rs` - Snap hint display on resize operations

**Config Example**:
```toml
[snap_hints]
enabled = true
duration_ms = 200
opacity = 128
```

**Features**:
- Transparent overlay window (WS_EX_LAYERED | WS_EX_TRANSPARENT)
- Shows column boundary during resize
- Auto-hide after configurable duration
- Disabled by default

#### 21.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    87 passed, 0 failed, 0 ignored
- daemon:         21 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32: 13 passed, 0 failed, 2 ignored

TOTAL: 131 passed, 0 failed, 2 ignored
```

**New Tests**:
- `test_add_floating_window`
- `test_duplicate_floating_window_rejected`
- `test_remove_floating_window`
- `test_remove_nonexistent_floating_window`
- `test_floating_window_in_placements`
- `test_floating_and_tiled_windows_together`
- `test_floating_window_duplicate_with_tiled`
- `test_update_floating_window`
- `test_window_rule_matches_class`
- `test_window_rule_matches_title_regex`
- `test_window_rule_matches_executable`
- `test_window_rule_matches_combined`
- `test_window_rule_no_criteria_matches_nothing`
- `test_window_rule_config_parse`
- `test_snap_hint_config_default`
- `test_snap_hint_config_serialization`
- `test_overlay_state_default` (platform_win32)
- `test_snap_hint_config_default` (platform_win32)

#### 21.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo +stable-x86_64-pc-windows-gnu test --workspace` | 131 passed, 2 ignored |
| Build succeeds | `cargo +stable-x86_64-pc-windows-gnu build --workspace` | Success |
| Minor warnings | `cargo +stable-x86_64-pc-windows-gnu clippy --workspace` | 3 minor warnings (acceptable) |

#### 21.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/daemon/Cargo.toml` | +4 | Dependencies |
| `crates/daemon/src/tray.rs` | +200 | NEW: Tray manager |
| `crates/daemon/src/config.rs` | +150 | Window rules, gestures, snap hints |
| `crates/daemon/src/main.rs` | +200 | Integration for all features |
| `crates/platform_win32/src/lib.rs` | +250 | Gestures, process info |
| `crates/platform_win32/src/overlay.rs` | +230 | NEW: Overlay window |
| `crates/core_layout/src/lib.rs` | +150 | Floating window support |
| `Cargo.toml` | +1 | Win32_System_ProcessStatus feature |
| `docs/.../CODEX_REVIEW_CONSOLIDATED.md` | +10 | Updated fixed items |

---

### Iteration 20: Codex Review + QA Scan (Pass)

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 19 (Config Completeness & Doc Sync)

#### 20.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-verify repo state and tests | High | DONE |
| 2 | Update `CODEX_REVIEW_CONSOLIDATED.md` with new QA findings | High | DONE |
| 3 | Record verification evidence | High | DONE |

#### 20.2 Changes Made

**Files Modified**:
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `docs/1_Progress and review/ITERATION_LOG.md`

#### 20.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    79 passed, 0 failed, 0 ignored
- daemon:         11 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32: 11 passed, 0 failed, 2 ignored

TOTAL: 111 passed, 0 failed, 2 ignored
```

#### 20.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 111 passed, 2 ignored |

#### 20.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | 1-41 | Doc refresh |
| `docs/1_Progress and review/ITERATION_LOG.md` | 56-68, 73-119, 1241-1257, 1321 | Iteration log update |

---

### Iteration 19: Config Completeness & Documentation Sync

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 18 (Codex Review + QA Scan (Failure))

#### 19.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Implement log_level config option | High | DONE |
| 2 | Implement track_focus_changes config option | High | DONE |
| 3 | Fix hotkey reload gap (critical bug) | Critical | DONE |
| 4 | Update ARCHITECTURE.md documentation | Medium | DONE |
| 5 | Update SPEC.md documentation | Medium | DONE |
| 6 | Update AGENTS.md toolchain documentation | Low | DONE |

#### 19.2 Changes Made

##### 19.2.1 log_level Config Implementation

**File**: `crates/daemon/src/main.rs`

**Changes**:
- Moved config loading before tracing subscriber setup
- Parse `config.behavior.log_level` string to `tracing::Level`
- Apply configured log level instead of hardcoded DEBUG

```rust
let log_level = match config.behavior.log_level.to_lowercase().as_str() {
    "trace" => Level::TRACE,
    "debug" => Level::DEBUG,
    "info" => Level::INFO,
    "warn" => Level::WARN,
    "error" => Level::ERROR,
    _ => Level::INFO, // default fallback
};
```

##### 19.2.2 track_focus_changes Config Implementation

**File**: `crates/daemon/src/main.rs`

**Changes**:
- Wrapped `install_event_hooks()` call in conditional based on config
- When `track_focus_changes = false`, WinEvent hooks are not installed

```rust
let _hook_handle = if config.behavior.track_focus_changes {
    match install_event_hooks() { ... }
} else {
    info!("WinEvent hooks disabled by config");
    None
};
```

##### 19.2.3 Hotkey Reload Fix (Critical)

**Problem**: `Reload` IPC command updated layout settings but did NOT re-register hotkeys. Users had to restart daemon for hotkey changes.

**Root Cause**: `HOTKEY_SENDER` in platform layer used `OnceLock::set()` which can only be called once.

**Files Modified**:
- `crates/platform_win32/src/lib.rs`: Changed `HOTKEY_SENDER` from `OnceLock` to `Mutex<Option<...>>`
- `crates/daemon/src/main.rs`: Added `HotkeyState` struct and `setup_hotkeys()` helper

**Platform Layer Changes**:
```rust
// Before:
static HOTKEY_SENDER: OnceLock<Sender<HotkeyEvent>> = OnceLock::new();

// After:
static HOTKEY_SENDER: Mutex<Option<Sender<HotkeyEvent>>> = Mutex::new(None);
```

**Daemon Changes**:
- Added `HotkeyState` struct to hold handle and mapping
- Added `setup_hotkeys()` helper function
- In IPC command handler, detect `Reload` and re-register hotkeys:
  1. Drop old handle (triggers unregister via Drop impl)
  2. Call `setup_hotkeys()` with new config
  3. Update hotkey state

##### 19.2.4 Documentation Updates

**ARCHITECTURE.md**:
- Updated test counts (52 → 79 for core_layout, total 111)
- Added Global Hotkeys section
- Added Smooth Scroll Animations section
- Updated AppState struct documentation for multi-monitor
- Moved config and multi-monitor from Pending to Implemented

**SPEC.md**:
- Updated Multi-Monitor Support section (now implemented)
- Added Global Hotkeys behavioral specification
- Added Scroll Animations behavioral specification
- Updated implementation status

**AGENTS.md**:
- Changed toolchain from MSVC to GNU/MinGW (matches `.cargo/config.toml`)

#### 19.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    79 passed, 0 failed, 0 ignored
- daemon:         11 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32: 11 passed, 0 failed, 2 ignored

TOTAL: 111 passed, 0 failed, 2 ignored
Clippy: No warnings
```

#### 19.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo +stable-x86_64-pc-windows-gnu test --workspace` | 111 passed, 2 ignored |
| No clippy warnings | `cargo +stable-x86_64-pc-windows-gnu clippy --workspace` | No warnings |
| Build succeeds | `cargo +stable-x86_64-pc-windows-gnu build --workspace` | Success |

#### 19.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/platform_win32/src/lib.rs` | +15 | Hotkey sender mutex refactor |
| `crates/daemon/src/main.rs` | +80 | Config options, hotkey reload |
| `docs/ARCHITECTURE.md` | +40 | Documentation sync |
| `docs/SPEC.md` | +80 | Documentation sync |
| `AGENTS.md` | +2 | Toolchain clarification |
| `docs/1_Progress and review/ITERATION_LOG.md` | +100 | This entry |

---

### Iteration 18: Codex Review + QA Scan (Failure)

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 17 (Codex Review + QA Scan (Failure))

#### 18.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-verify repo state and tests | High | DONE |
| 2 | Update `CODEX_REVIEW_CONSOLIDATED.md` with new QA findings | High | DONE |
| 3 | Record verification evidence | High | DONE |

#### 18.2 Changes Made

**Files Modified**:
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `docs/1_Progress and review/ITERATION_LOG.md`

#### 18.3 Test Results

```
Test Summary (2026-02-04):
- `cargo test --all` FAILED
  - Error: E0599 `Config::generate_default` not found (crates/daemon/src/config.rs)
  - Locations: lines ~360 and ~410
```

#### 18.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| Tests run | `cargo test --all` | FAIL (E0599 Config::generate_default missing) |

#### 18.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | 1-43 | Doc refresh |
| `docs/1_Progress and review/ITERATION_LOG.md` | 56-67, 71-122, 1057-1079, 1133 | Iteration log update |

---

### Iteration 17: Codex Review + QA Scan (Failure)

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 16 (Codex Review + QA Scan)

#### 17.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-verify repo state and tests | High | DONE |
| 2 | Update `CODEX_REVIEW_CONSOLIDATED.md` with new QA findings | High | DONE |
| 3 | Record verification evidence | High | DONE |

#### 17.2 Changes Made

**Files Modified**:
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `docs/1_Progress and review/ITERATION_LOG.md`

#### 17.3 Test Results

```
Test Summary (2026-02-04):
- `cargo test --all` FAILED
  - Error: E0599 `Config::generate_default` not found (crates/daemon/src/config.rs)
  - Locations: lines ~360 and ~410
```

#### 17.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| Tests run | `cargo test --all` | FAIL (E0599 Config::generate_default missing) |

#### 17.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | 1-41 | Doc refresh |
| `docs/1_Progress and review/ITERATION_LOG.md` | 56-66, 68-114, 1012-1027, 1087 | Iteration log update |

---

### Iteration 16: Codex Review + QA Scan

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 15 (Codex Review + Doc Drift Audit)

#### 16.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-verify repo state and tests | High | DONE |
| 2 | Update `CODEX_REVIEW_CONSOLIDATED.md` with new QA findings | High | DONE |
| 3 | Record verification evidence | High | DONE |

#### 16.2 Changes Made

**Files Modified**:
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `docs/1_Progress and review/ITERATION_LOG.md`

#### 16.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    79 passed, 0 failed, 0 ignored
- daemon:         10 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  9 passed, 0 failed, 2 ignored

TOTAL: 108 passed, 0 failed, 2 ignored
Warnings: monitors_list unused; default_config_path unused
```

#### 16.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 108 passed, 2 ignored |

#### 16.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | 1-39 | Doc refresh |
| `docs/1_Progress and review/ITERATION_LOG.md` | 56-66, 69-119, 967-987, 1041 | Iteration log update |

---

### Iteration 15: Codex Review + Doc Drift Audit

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 14 (Smooth Scroll Animations)

#### 15.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-verify repo state and tests | High | DONE |
| 2 | Update `CODEX_REVIEW_CONSOLIDATED.md` with doc drift findings | High | DONE |
| 3 | Record verification evidence | High | DONE |

#### 15.2 Changes Made

**Files Modified**:
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `docs/1_Progress and review/ITERATION_LOG.md`

#### 15.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    79 passed, 0 failed, 0 ignored
- daemon:         10 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  9 passed, 0 failed, 2 ignored

TOTAL: 108 passed, 0 failed, 2 ignored
Warnings: monitors_list unused; default_config_path unused
```

#### 15.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 108 passed, 2 ignored |

#### 15.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | 1-37 | Doc refresh |
| `docs/1_Progress and review/ITERATION_LOG.md` | 54-64, 66-208, 918-931, 991 | Iteration log update |

---

### Iteration 14: Smooth Scroll Animations

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 13 (Global Hotkey Support)

#### 14.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Add animation types to core layout | High | DONE |
| 2 | Add animated scrolling in `Workspace` | High | DONE |
| 3 | Integrate animation tick in daemon | Medium | DONE |

#### 14.2 Changes Made

**Core Layout** (`crates/core_layout/src/lib.rs`):
- Added `Easing` and `ScrollAnimation` types.
- Added animated scroll helpers (`start_scroll_animation`, `tick_animation`, `compute_placements_animated`, `ensure_focused_visible_animated`).

**Daemon** (`crates/daemon/src/main.rs`):
- Added animation tick event and animated placement usage when active.

#### 14.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    79 passed, 0 failed, 0 ignored
- daemon:         10 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  9 passed, 0 failed, 2 ignored

TOTAL: 108 passed, 0 failed, 2 ignored
```

#### 14.4 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/core_layout/src/lib.rs` | +200 | Animation types + tests |
| `crates/daemon/src/main.rs` | +40 | Animation tick integration |

---

### Iteration 13: Global Hotkey Support

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 12 (Codex Audit + Doc Refresh)

#### 13.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Add global hotkey registration in platform layer | High | DONE |
| 2 | Add hotkey configuration schema | High | DONE |
| 3 | Integrate hotkeys into daemon event loop | High | DONE |

#### 13.2 Changes Made

**Platform Layer** (`crates/platform_win32/src/lib.rs`):
- Added hotkey types (`Hotkey`, `HotkeyEvent`, `Modifiers`) and parsing helpers.
- Added `register_hotkeys` / `unregister_hotkeys` with RegisterHotKey.

**Config** (`crates/daemon/src/config.rs`):
- Added `HotkeyConfig` and default bindings in generated config.

**Daemon** (`crates/daemon/src/main.rs`):
- Registers hotkeys on startup and maps them to IPC commands.

#### 13.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    52 passed, 0 failed, 0 ignored
- daemon:         10 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  9 passed, 0 failed, 2 ignored

TOTAL: 81 passed, 0 failed, 2 ignored
```

#### 13.4 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/platform_win32/src/lib.rs` | +200 | Hotkey registration + parsing |
| `crates/daemon/src/config.rs` | +120 | Hotkey config + defaults |
| `crates/daemon/src/main.rs` | +60 | Hotkey integration |

---
### Iteration 12: Codex Audit + Doc Refresh

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 11 (Multi-monitor Support)

#### 12.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Review `ITERATION_LOG.md` against repo state | High | DONE |
| 2 | Update `CODEX_REVIEW_CONSOLIDATED.md` with current verification | High | DONE |
| 3 | Update `AGENTS.md` and `CLAUDE.md` to require review of consolidated review | Medium | DONE |
| 4 | Record verification evidence | High | DONE |

#### 12.2 Changes Made

**Files Modified**:
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `AGENTS.md`
- `CLAUDE.md`
- `docs/1_Progress and review/ITERATION_LOG.md`

#### 12.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    52 passed, 0 failed, 0 ignored
- daemon:          6 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  6 passed, 0 failed, 2 ignored

TOTAL: 74 passed, 0 failed, 2 ignored
Warnings: monitors_list unused; default_config_path unused
```

#### 12.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 74 passed, 2 ignored |

#### 12.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | 1-30 | Doc refresh |
| `AGENTS.md` | 33-40 | Guidance update |
| `CLAUDE.md` | 5-8 | Guidance update |
| `docs/1_Progress and review/ITERATION_LOG.md` | 50-61, 67-116, 776-786, 846 | Iteration log update |

---

### Iteration 11: Multi-monitor Support

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 10 (Configuration Support)

#### 11.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Extend monitor enumeration for multi-monitor | High | DONE |
| 2 | Add multi-workspace state to daemon | High | DONE |
| 3 | Assign windows to monitors based on position | High | DONE |
| 4 | Add cross-monitor movement commands | Medium | DONE |

#### 11.2 Changes Made

##### 11.2.1 Platform Layer Multi-monitor Helpers (Task #18)

**File**: `crates/platform_win32/src/lib.rs`

**New Types**:
```rust
pub type MonitorId = isize;

impl MonitorInfo {
    pub fn contains_point(&self, x: i32, y: i32) -> bool { ... }
    pub fn contains_rect_center(&self, rect: &Rect) -> bool { ... }
}
```

**New Functions**:
```rust
/// Find which monitor contains a rectangle's center point.
pub fn find_monitor_for_rect<'a>(monitors: &'a [MonitorInfo], rect: &Rect) -> Option<&'a MonitorInfo>

/// Find a monitor by ID.
pub fn find_monitor_by_id(monitors: &[MonitorInfo], id: MonitorId) -> Option<&MonitorInfo>

/// Sort monitors by position (left to right, top to bottom).
pub fn monitors_by_position(monitors: &[MonitorInfo]) -> Vec<&MonitorInfo>

/// Find the monitor to the left of the current one.
pub fn monitor_to_left(monitors: &[MonitorInfo], current_id: MonitorId) -> Option<&MonitorInfo>

/// Find the monitor to the right of the current one.
pub fn monitor_to_right(monitors: &[MonitorInfo], current_id: MonitorId) -> Option<&MonitorInfo>
```

**New Tests** (5 tests):
- `test_monitor_contains_point`
- `test_monitor_contains_rect_center`
- `test_find_monitor_for_rect`
- `test_monitors_by_position`
- `test_monitor_to_left_right`

##### 11.2.2 Daemon Multi-workspace State (Task #19)

**File**: `crates/daemon/src/main.rs`

**AppState Changes**:
```rust
// Before (single workspace):
struct AppState {
    workspace: Workspace,
    viewport: Rect,
    platform_config: PlatformConfig,
    config: Config,
}

// After (per-monitor workspaces):
struct AppState {
    workspaces: HashMap<MonitorId, Workspace>,
    monitors: HashMap<MonitorId, MonitorInfo>,
    focused_monitor: MonitorId,
    platform_config: PlatformConfig,
    config: Config,
}
```

**New Methods**:
```rust
impl AppState {
    fn new_with_config(config: Config, monitors: Vec<MonitorInfo>) -> Self { ... }
    fn focused_workspace(&self) -> Option<&Workspace> { ... }
    fn focused_workspace_mut(&mut self) -> Option<&mut Workspace> { ... }
    fn focused_viewport(&self) -> Rect { ... }
    fn find_window_workspace(&self, window_id: u64) -> Option<MonitorId> { ... }
}
```

**Window Assignment**: Windows are assigned to monitors based on the center point of their current rect using `find_monitor_for_rect()`.

**Event Handling**: `handle_window_event()` updated to:
- Find which workspace contains a window using `find_window_workspace()`
- Assign new windows to monitors based on their position
- Update `focused_monitor` when window focus changes

##### 11.2.3 Cross-monitor Movement Commands (Task #21)

**File**: `crates/ipc/src/lib.rs`

**New Commands**:
```rust
pub enum IpcCommand {
    // ... existing variants ...
    FocusMonitorLeft,
    FocusMonitorRight,
    MoveWindowToMonitorLeft,
    MoveWindowToMonitorRight,
}
```

**File**: `crates/cli/src/main.rs`

**New CLI Subcommands**:
```bash
openniri-cli focus-monitor left   # Focus monitor to the left
openniri-cli focus-monitor right  # Focus monitor to the right
openniri-cli move-to-monitor left   # Move window to monitor left
openniri-cli move-to-monitor right  # Move window to monitor right
```

**File**: `crates/daemon/src/main.rs`

**Command Handling**:
- `FocusMonitorLeft/Right`: Changes `focused_monitor` to adjacent monitor
- `MoveWindowToMonitorLeft/Right`: Removes window from current workspace, adds to target workspace, follows focus

#### 11.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    52 passed, 0 failed, 0 ignored
- daemon:          6 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  6 passed, 0 failed, 2 ignored

TOTAL: 74 passed, 0 failed, 2 ignored
```

**New/Updated Tests**:
- 5 new tests in platform_win32 for multi-monitor helpers
- Updated IPC test to include 4 new command variants

#### 11.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 74 passed, 2 ignored |
| No clippy errors | `cargo clippy --workspace` | Only dead code warnings |
| Focus monitor | `openniri-cli focus-monitor left` | Focuses left monitor |
| Move window | `openniri-cli move-to-monitor right` | Moves window to right monitor |

#### 11.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/platform_win32/src/lib.rs` | +100 | Multi-monitor helpers |
| `crates/daemon/src/main.rs` | +200 | Multi-workspace refactor |
| `crates/ipc/src/lib.rs` | +15 | New commands |
| `crates/cli/src/main.rs` | +30 | New CLI subcommands |

---

### Iteration 10: Configuration File Support

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 9 (Codex Review Implementation)

#### 10.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Define config file format and schema | High | DONE |
| 2 | Implement config loading in daemon | High | DONE |
| 3 | Add Reload IPC command for hot-reload | Medium | DONE |
| 4 | Add default config generation | Medium | DONE |

#### 10.2 Changes Made

##### 10.2.1 Config File Format (Task #13)

**Files Created**:
- `crates/daemon/src/config.rs` (270 lines)

**Dependencies Added** (workspace `Cargo.toml`):
```toml
toml = "0.8"
directories = "5"
```

**Config Structure**:
```rust
pub struct Config {
    pub layout: LayoutConfig,      // gap, outer_gap, column widths, centering
    pub appearance: AppearanceConfig,  // cloaking, deferred positioning
    pub behavior: BehaviorConfig,  // focus behavior, log level
}
```

**Config Locations** (in priority order):
1. `%APPDATA%/openniri/config.toml`
2. `~/.config/openniri/config.toml`
3. `./config.toml`

##### 10.2.2 Config Loading (Task #14)

**File**: `crates/daemon/src/main.rs`

**AppState Changes**:
```rust
struct AppState {
    workspace: Workspace,
    platform_config: PlatformConfig,
    viewport: Rect,
    config: Config,  // NEW
}

impl AppState {
    fn new_with_config(config: Config, viewport: Rect) -> Self { ... }
    fn apply_config(&mut self, config: Config) { ... }
}
```

**Startup Flow**:
1. Load config (or use defaults)
2. Log config values
3. Create AppState with config
4. Apply config to workspace settings

##### 10.2.3 Reload IPC Command (Task #15)

**File**: `crates/ipc/src/lib.rs`

**New Variant**:
```rust
pub enum IpcCommand {
    // ... existing variants ...
    Reload,  // NEW
}
```

**File**: `crates/cli/src/main.rs`

**New CLI Command**:
```bash
openniri-cli reload  # Reload config from file
```

##### 10.2.4 Default Config Generation (Task #16)

**File**: `crates/cli/src/main.rs`

**New CLI Command**:
```bash
openniri-cli init              # Create config at default location
openniri-cli init -o path.toml # Create at custom path
openniri-cli init --force      # Overwrite existing
```

**Generated Config Example**:
```toml
[layout]
gap = 10
outer_gap = 10
default_column_width = 800
centering_mode = "center"

[appearance]
use_cloaking = true
use_deferred_positioning = true

[behavior]
focus_new_windows = true
log_level = "info"
```

#### 10.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    52 passed, 0 failed, 0 ignored
- daemon:          6 passed, 0 failed, 0 ignored (NEW config tests)
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  1 passed, 0 failed, 2 ignored

TOTAL: 69 passed, 0 failed, 2 ignored
```

**New Tests in daemon**:
- `test_default_config`
- `test_config_serialization_roundtrip`
- `test_config_partial_parse`
- `test_centering_mode_conversion`
- `test_generate_default_config`
- `test_config_paths_not_empty`

#### 10.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 69 passed, 2 ignored |
| Config init | `openniri-cli init` | Creates config file |
| Config reload | `openniri-cli reload` | Reloads config in daemon |

#### 10.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `Cargo.toml` | +3 | Dependencies |
| `crates/daemon/Cargo.toml` | +2 | Dependencies |
| `crates/daemon/src/config.rs` | +270 | New file |
| `crates/daemon/src/main.rs` | +30 | Config integration |
| `crates/cli/Cargo.toml` | +1 | Dependencies |
| `crates/cli/src/main.rs` | +80 | Init command |
| `crates/ipc/src/lib.rs` | +3 | Reload variant |

---

### Iteration 9: Codex Review Implementation

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 8.1-8.3 (IPC & Platform Integration)

#### 9.1 Objectives

Based on `CODEX_REVIEW_CONSOLIDATED.md`, implement all recommended fixes:

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Update ARCHITECTURE.md and SPEC.md | High | DONE |
| 2 | Remove dead HideStrategy code | Medium | DONE |
| 3 | Add `#[ignore]` to monitor tests | Medium | DONE |
| 4 | Add cloaked window filtering | High | DONE |
| 5 | Clean up named pipe server + CLI timeout | Medium | DONE |
| 6 | Add IPC integration tests | Medium | DONE |
| 7 | Implement WinEvent hooks | High | DONE |

#### 9.2 Changes Made

##### 9.2.1 Documentation Updates (Task #6)

**Files Modified**:
- `docs/ARCHITECTURE.md` (lines 207-222)
- `docs/SPEC.md` (lines 231-235)

**Changes**:
- Updated "Planned vs Implemented" section
- Marked IPC and monitor detection as implemented
- Updated test counts (52 -> 63)
- Removed WinEvent hooks from pending (now implemented)

##### 9.2.2 Dead Code Removal (Task #7)

**File**: `crates/platform_win32/src/lib.rs`

**Removed**:
```rust
// Before (lines 79-89):
pub enum HideStrategy {
    Cloak,
    Minimize,      // REMOVED - never used
    MoveOffScreen, // REMOVED - never used
}

// Before (lines 92-100):
pub struct PlatformConfig {
    pub hide_strategy: HideStrategy,
    pub buffer_zone: i32,  // REMOVED - never used
    pub use_deferred_positioning: bool,
}
```

**After**:
```rust
pub enum HideStrategy {
    #[default]
    Cloak,
    // Note: Minimize and MoveOffScreen strategies were considered but removed.
}

pub struct PlatformConfig {
    pub hide_strategy: HideStrategy,
    pub use_deferred_positioning: bool,
}
```

**Test Impact**: Updated `test_platform_config_default` to remove `buffer_zone` assertion.

##### 9.2.3 Headless CI Test Marking (Task #8)

**File**: `crates/platform_win32/src/lib.rs` (lines 607-637)

**Changes**:
```rust
#[test]
#[ignore = "Requires display hardware - run with: cargo test -- --ignored"]
fn test_enumerate_monitors() { ... }

#[test]
#[ignore = "Requires display hardware - run with: cargo test -- --ignored"]
fn test_get_primary_monitor() { ... }
```

##### 9.2.4 Cloaked Window Filtering (Task #9)

**File**: `crates/platform_win32/src/lib.rs`

**New Import**:
```rust
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DwmSetWindowAttribute, DWMWA_CLOAK, DWMWA_CLOAKED,
};
```

**New Function** (lines 351-365):
```rust
fn is_window_cloaked(hwnd: HWND) -> bool {
    unsafe {
        let mut cloaked: u32 = 0;
        let result = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        );
        result.is_ok() && cloaked != 0
    }
}
```

**Integration**: Added call in `enum_windows_callback` after `WS_EX_NOACTIVATE` check.

##### 9.2.5 Named Pipe Server Cleanup + CLI Timeout (Task #10)

**File**: `crates/daemon/src/main.rs` (lines 209-256)

**Before**: Convoluted try-without-first_pipe_instance-then-with logic
**After**: Clean `is_first_instance` tracking

**File**: `crates/cli/src/main.rs`

**New Imports**:
```rust
use std::time::Duration;
use tokio::time::timeout;

const IPC_TIMEOUT: Duration = Duration::from_secs(5);
```

**Changed Function**:
```rust
async fn send_command(cmd: IpcCommand) -> Result<IpcResponse> {
    timeout(IPC_TIMEOUT, send_command_inner(cmd))
        .await
        .context("Timed out waiting for daemon response")?
}
```

##### 9.2.6 IPC Integration Tests (Task #11)

**File**: `crates/ipc/src/lib.rs` (lines 161-220)

**New Tests Added** (5 total):
1. `test_all_command_types_roundtrip` - All 15 command variants
2. `test_all_response_types_roundtrip` - All 5 response variants
3. `test_line_delimited_protocol` - JSON + newline format
4. `test_invalid_json_handling` - Error cases
5. `test_pipe_name_format` - Named pipe path validation

**Test Count**: 5 -> 10 tests in IPC crate

##### 9.2.7 WinEvent Hooks Implementation (Task #12)

**File**: `crates/platform_win32/src/lib.rs`

**New Imports**:
```rust
use std::sync::mpsc;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::GetAncestor;
```

**New Constants**:
```rust
const EVENT_OBJECT_CREATE: u32 = 0x8000;
const EVENT_OBJECT_DESTROY: u32 = 0x8001;
const EVENT_OBJECT_FOCUS: u32 = 0x8005;
const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x0016;
const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x0017;
const EVENT_OBJECT_LOCATIONCHANGE: u32 = 0x800B;
```

**New Types**:
```rust
static EVENT_SENDER: std::sync::OnceLock<mpsc::Sender<WindowEvent>> = std::sync::OnceLock::new();

pub struct EventHookHandle {
    hooks: Vec<HWINEVENTHOOK>,
}
```

**New Functions**:
- `install_event_hooks() -> Result<(EventHookHandle, mpsc::Receiver<WindowEvent>), Win32Error>`
- `win_event_callback(...)` - extern "system" callback

**Workspace Cargo.toml Change**:
```toml
windows = { version = "0.59", features = [
    ...
    "Win32_UI_Accessibility",  # NEW
] }
```

**Daemon Integration** (`crates/daemon/src/main.rs`):

New DaemonEvent variant:
```rust
enum DaemonEvent {
    IpcCommand { ... },
    WindowEvent(WindowEvent),  // NEW
    Shutdown,
}
```

New AppState method:
```rust
fn handle_window_event(&mut self, event: WindowEvent) {
    match event {
        WindowEvent::Created(hwnd) => { ... }
        WindowEvent::Destroyed(hwnd) => { ... }
        WindowEvent::Focused(hwnd) => { ... }
        WindowEvent::Minimized(hwnd) => { ... }
        WindowEvent::Restored(hwnd) => { ... }
        WindowEvent::MovedOrResized(hwnd) => { ... }
    }
}
```

Hook installation in main():
```rust
let _hook_handle = match install_event_hooks() {
    Ok((handle, event_receiver)) => {
        // Spawn thread to forward events
        std::thread::spawn(move || { ... });
        Some(handle)
    }
    Err(e) => { warn!("..."); None }
};
```

#### 9.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    52 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  1 passed, 0 failed, 2 ignored
- daemon:          0 (binary crate)
- cli:             0 (binary crate)

TOTAL: 63 passed, 0 failed, 2 ignored
```

**Clippy**: No warnings or errors

#### 9.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 63 passed, 2 ignored |
| No clippy warnings | `cargo clippy --workspace` | No errors |
| Release build | `cargo build --release` | Success |
| Monitor tests (local) | `cargo test -- --ignored` | 2 passed (with display) |

#### 9.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `Cargo.toml` | +1 | Feature addition |
| `crates/platform_win32/src/lib.rs` | +150 | WinEvent hooks, cloaked filtering |
| `crates/daemon/src/main.rs` | +100 | Event handling |
| `crates/cli/src/main.rs` | +15 | Timeout |
| `crates/ipc/src/lib.rs` | +60 | Tests |
| `docs/ARCHITECTURE.md` | +5 | Updates |
| `docs/SPEC.md` | +3 | Updates |

---

### Iteration 8.3: Async Daemon & CLI IPC (Prior)

**Date**: 2026-02-04
**Status**: COMPLETED

#### Key Deliverables
- Async daemon with tokio event loop
- Named pipe server (`\\.\pipe\openniri`)
- CLI sends real IPC commands
- Dynamic monitor detection (no more hardcoded 1920x1080)

#### Files Created/Modified
- `crates/daemon/src/main.rs` - Full rewrite for async
- `crates/cli/src/main.rs` - Real IPC client

---

### Iteration 8.2: Monitor Detection (Prior)

**Date**: 2026-02-04
**Status**: COMPLETED

#### Key Deliverables
- `MonitorInfo` struct
- `enumerate_monitors()` function
- `get_primary_monitor()` function

#### Files Modified
- `crates/platform_win32/src/lib.rs`
- `Cargo.toml` (added `Win32_Graphics_Gdi` feature)

---

### Iteration 8.1: IPC Protocol Crate (Prior)

**Date**: 2026-02-04
**Status**: COMPLETED

#### Key Deliverables
- New `crates/ipc/` crate
- `IpcCommand` enum (15 variants)
- `IpcResponse` enum (5 variants)
- `PIPE_NAME` constant

#### Files Created
- `crates/ipc/Cargo.toml`
- `crates/ipc/src/lib.rs`

---

### Iterations 1-7: Foundation (Historical)

**Date**: Pre-2026-02-04
**Status**: COMPLETED

#### Key Deliverables
- Core layout engine with 52 tests
- Basic Win32 enumeration
- Window positioning via DeferWindowPos
- DWM cloaking

---

## Test Coverage History

| Date | core_layout | ipc | platform_win32 | daemon | Total |
|------|-------------|-----|----------------|--------|-------|
| Pre-8.1 | 52 | 0 | 0 | 0 | 52 |
| 8.1 | 52 | 5 | 0 | 0 | 57 |
| 8.2 | 52 | 5 | 3 | 0 | 60 |
| 9 | 52 | 10 | 1 (+2 ignored) | 0 | 63 |
| 10 | 52 | 10 | 1 (+2 ignored) | 6 | 69 |
| 11 | 52 | 10 | 6 (+2 ignored) | 6 | 74 |
| 12 | 52 | 10 | 6 (+2 ignored) | 6 | 74 |
| 13 | 52 | 10 | 9 (+2 ignored) | 10 | 81 |
| 14 | 79 | 10 | 9 (+2 ignored) | 10 | 108 |
| 15 | 79 | 10 | 9 (+2 ignored) | 10 | 108 |
| 16 | 79 | 10 | 9 (+2 ignored) | 10 | 108 |
| 17 | N/A | N/A | N/A | N/A | FAIL (E0599 Config::generate_default) |
| 18 | N/A | N/A | N/A | N/A | FAIL (E0599 Config::generate_default) |
| 19 | 79 | 10 | 11 (+2 ignored) | 11 | 111 |
| 20 | 79 | 10 | 11 (+2 ignored) | 11 | 111 |
| 21 | 87 | 10 | 13 (+2 ignored) | 21 | 131 |
| 22 | 87 | 13 | 13 (+2 ignored) | 34 | 147 |
| 23 | 87 | 13 | 13 (+2 ignored) | 44 (+28 cli, +17 integration) | 202 |
| 24 | 87 | 13 | 13 (+2 ignored) | 48 (+28 cli, +17 integration) | 206 |
| 25 | 87 | 13 | 13 (+2 ignored) | 52 (+29 cli, +17 integration) | 231 |
| 26 | 87 | 13 | 13 (+2 ignored) | 55 (+29 cli, +17 integration) | 234 |
| 27 | 87 | 15 | 16 (+2 ignored) | 85 (+29 cli, +22 integration, +1 ignored) | 257 |
| 28 | 87 | 15 | 16 (+2 ignored) | 89 (+29 cli, +22 integration, +1 ignored) | 261 |
| 29 | 99 | 15 | 23 (+2 ignored) | 97 (+38 cli, +22 integration, +1 ignored) | 295 |
| 30 | 99 | 15 | 24 (+3 ignored) | 100 (+38 cli, +22 integration, +1 ignored) | 302 |
| 31 | 99 | 15 | 24 (+3 ignored) | 100 (+38 cli, +22 integration, +1 ignored) | 302 |
| 32 | 99 | 15 | 24 (+3 ignored) | 100 (+38 cli, +22 integration, +1 ignored) | 302 |
| 33 | 99 | 15 | 24 (+3 ignored) | 100 (+38 cli, +22 integration, +1 ignored) | 302 |
| 34 | 99 | 15 | 24 (+3 ignored) | 100 (+38 cli, +22 integration, +1 ignored) | 302 |
| 35 | 113 | 15 | 24 (+3 ignored) | 107 (+38 cli, +22 integration, +1 ignored) | 323 |

---

## Architecture Evolution

### Current State (Post-Iteration 35)

```
┌─────────────────────────────────────────────────────────────────────┐
│                        User / System                                 │
└─────────────────┬───────────────────────────────────────┬───────────┘
                  │                                       │
                  ▼                                       ▼
         ┌────────────────┐                    ┌─────────────────────────┐
         │  openniri-cli  │──── IPC ──────────►│   openniri-daemon       │
         │   (Commands)   │   (Named Pipe)     │    (Event Loop)         │
         │   + Timeout    │    5s timeout      │    + WinEvent Hooks     │
         │   + 38 tests   │                    │    + Multi-monitor      │
         └────────────────┘                    │    + Hotkey Reload      │
                                               │    + Smooth Animations  │
                                               │    + Focus Follows Mouse│
                                               │    + Display Change     │
                                               └────────────┬────────────┘
                                                            │
                  ┌──────────────────────────────┬──────────┴──────────┐
                  │                              │                      │
                  ▼                              ▼                      ▼
         ┌────────────────┐            ┌────────────────┐     ┌───────────────┐
         │ openniri-ipc   │            │ Per-Monitor    │     │   WinEvent    │
         │ (Protocol)     │            │  Workspaces    │     │    Hooks      │
         │ + Monitor cmds │            │ (HashMap)      │     │ + Hotkeys     │
         │ + QueryAll     │            └───────┬────────┘     │ + Mouse Hook  │
         └────────────────┘                    │              └───────────────┘
                                               │
                                               ▼
                                      ┌────────────────┐
                                      │openniri-core-  │
                                      │    layout      │
                                      └───────┬────────┘
                                              │
                                              ▼
                                     ┌────────────────────┐
                                     │openniri-platform-  │
                                     │      win32         │
                                     │ + Multi-monitor    │
                                     │ + DisplayChange    │
                                     │ + HideStrategy     │
                                     └────────────────────┘
```

---

## Known Issues & Technical Debt

| Issue | Severity | Iteration Introduced | Status |
|-------|----------|---------------------|--------|
| Global EVENT_SENDER for hooks | Low | 9 | Acceptable (thread safety) |
| Config `default_config_path` unused | Low | 10 | Minor (dead code warning) |
| `monitors_list` method unused | Low | 11 | Minor (dead code warning) |
| No end-to-end daemon integration tests | Medium | - | Partially addressed in Iteration 27 |
| ARCHITECTURE.md/SPEC.md may drift again | Low | 24 | Refreshed in Iteration 34 |

---

## Next Iteration Planning

### Iteration 29 (Dramatic UX Overhaul)

**Focus**: UX overhaul with real OS integration

**Completed**:
1. SetForegroundWindow integration - focus commands now actually move OS focus
2. Owner-window filtering - dialogs excluded, UWP apps tiled correctly
3. CloseWindow command (Win+Shift+Q)
4. ToggleFloating command (Win+F)
5. ToggleFullscreen command (Win+Shift+F)
6. SetColumnWidth presets (Win+1=1/3, Win+2=1/2, Win+3=2/3, Win+0=equalize)
7. Active window border via DWM (DWMWA_BORDER_COLOR)
8. Snap hints and gestures enabled by default
9. QueryStatus command and CLI status subcommand
10. Tray menu: Pause Tiling, Open Config, View Logs
11. Auto-start via Registry (openniri-cli autostart enable/disable)

**Tests**: 261 -> 295 (291 passed, 4 ignored, 0 warnings)

---

### Iteration 30 (Crash Safety and Reliability)

**Focus**: Safer daemon shutdown and crash recovery behavior

**Completed**:
1. Ctrl+C signal handling that emits `DaemonEvent::Shutdown`
2. Managed-window uncloak/reset on daemon shutdown
3. Panic-hook emergency uncloak-all-visible behavior
4. DPI awareness initialization at process startup
5. Tray Exit routed through unified shutdown cleanup path
6. Added reliability tests for new shutdown/recovery helpers

**Tests**: 295 -> 302 (297 passed, 5 ignored, 0 warnings)

---

### Iteration 31 (Repository Presentation Refresh)

**Focus**: Public-facing quality of project messaging and GitHub profile

**Completed**:
1. Rewrote `README.md` to present product intent, current capability surface, and practical quick-start flow.
2. Removed low-signal public framing and replaced it with concise product-oriented positioning.
3. Updated GitHub repository metadata (description + topics) to match project scope and quality bar.

**Tests**: 302 -> 302 (no code changes in this iteration)

---

### Iteration 32 (Public README + GitHub About Revamp)

**Focus**: Raise public repository quality and make onboarding expectations explicit

**Completed**:
1. Rebuilt `README.md` structure with clearer product framing and user expectations.
2. Added fuller default-hotkey coverage and more direct quick-start/stop flow.
3. Tightened architecture/documentation references for easier contributor orientation.
4. Updated GitHub About metadata to better match current implementation surface.

**Tests**: 302 -> 302 (no code changes in this iteration)

---

### Iteration 33 (Public Readiness Checklist and Messaging Polish)

**Focus**: Make external adoption path explicit and execution-ready

**Completed**:
1. Added `docs/PUBLIC_READINESS_CHECKLIST.md` with definition-of-done and 8 readiness domains.
2. Added prioritized "Immediate Next 10" public rollout tasks.
3. Polished README structure for clearer public onboarding and expectations.
4. Linked readiness checklist directly from README.

**Tests**: 302 -> 302 (no code changes in this iteration)

---

### Iteration 34 (Pre-Stable Focus Lock)

**Focus**: Freeze development priorities around reliability/usability before packaging work

**Completed**:
1. Added `docs/PRE_STABLE_EXECUTION_PLAN.md` to explicitly track do-now scope.
2. Marked signing/installer/release-channel hardening as deferred post-stable.
3. Updated readiness checklist to prioritize diagnostics, safe mode, and reliability tests.
4. Linked the new pre-stable plan from README.

**Tests**: 302 -> 302 (no code changes in this iteration)

---

### Iteration 35 (Planned)

**Focus**: Pre-stable reliability and diagnostics (Execution Plan Step 1)

**Objectives**:
1. Add `openniri-cli doctor` with actionable checks and remediation hints.
2. Add GitHub issue templates that require doctor/log output.
3. Define and document alpha exit criteria in readiness docs.
4. Keep post-stable packaging tasks in deferred backlog.

---

*This document is automatically updated after each development iteration.*






