# Open Items Dashboard
Last Updated: 2026-02-09 (Iteration 65 strict automation QA gate rerun + double QA loop)
Source of truth: `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`

## Active Work (Open)
- [ ] `INC-49-1` Critical incident closure validation: run and record host reproduction/recovery evidence for desktop lockout/focus-trap scenario after `openniri-cli apply`.
- [ ] `INC-49-4` Acceptance gate: host-level manual recovery scenario passes on real desktop workload before any release/tagging.
- [ ] `INC-49-T1` Required host test evidence: apply + recovery path does not leave user unable to focus terminal/editor windows.
- [ ] Host-manual evidence capture only: all code-level Critical/High issues are closed for Iteration 65 strict gate rerun; release remains blocked until host closure evidence is recorded.

## Completed
- [x] `INC-49-2` Recovery hardening implemented and extended through Iteration 62: deterministic bounded shutdown joins (no detached late worker threads), local CLI emergency visibility restore (`emergency-uncloak` + panic-revert fallback), emergency panic hotkey (`Win+Ctrl+Escape` -> `panic_revert`), `stop`/`panic-revert` shutdown confirmation waits with auto local fallback restore, `apply` timeout/disconnect/connect-timeout local fallback restore coverage, non-success stop/panic responses now trigger local restore + unconfirmed-shutdown guidance, focus/foreground WinEvent delivery hardening for cloaked-window recovery, fullscreen-clear safety during minimize/restore and toggle-floating transitions, runtime re-check on delayed focus-follows-mouse dispatch, IPC newline framing + bounded response-size guards, end-to-end `toggle-pause` kill-switch command (`toggle-pause` / `pause`) with tray/IPC resume parity plus failed-resume paused-state rollback protection, stricter doctor/status handling, and updated recovery docs (`crates/platform_win32/src/lib.rs`, `crates/cli/src/main.rs`, `crates/daemon/src/main.rs`, `crates/core_layout/src/lib.rs`, `crates/daemon/src/config.rs`, `README.md`, `docs/TROUBLESHOOTING.md`, `docs/GETTING_STARTED.md`, `docs/TESTING_GUIDE.md`, `docs/SUPPORT_PLAYBOOK.md`, `docs/CONFIGURATION.md`, `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md`)
- [x] `INC-49-3` Documentation/process guardrails implemented: non-destructive recovery-first + explicit last-resort risk warnings (`docs/SUPPORT_PLAYBOOK.md`, `docs/TROUBLESHOOTING.md`, `README.md`, `docs/GETTING_STARTED.md`)
- [x] `INC-49-5` Release governance hardened: no tagging/publish while incident remains open, and CI runs for all tags (`.github/workflows/ci.yml` `tags: ['**']`) while release-sensitive checks remain `refs/tags/v`-gated (`docs/PUBLIC_READINESS_CHECKLIST.md`, `docs/PRE_STABLE_EXECUTION_PLAN.md`, `docs/1_Progress and review/CLAUDE_FINALIZATION_CHECKLIST.md`)
- [x] `INC-49-T2` Panic-revert coverage in default suite (`crates/cli/src/main.rs`, `crates/daemon/src/main.rs`, `crates/daemon/tests/integration.rs`, `crates/ipc/src/lib.rs`)
- [x] `R32-C1` Add repeatable host smoke tests for real Win32 window-control behavior — added to TESTING_GUIDE.md (iter 41)
- [x] `R32-C2` Refactor create-event window enumeration behind injectable boundary — `lookup_window_info()`, `is_known_window()`, `injected_window_info` (iter 41)
- [x] `R32-C3` Replace ignored daemon singleton test with deterministic alternative — `test_check_already_running_with_isolated_pipe` (iter 41)
- [x] `R32-T1` Created-event test proving focus_new_windows=false preserves focus through handle_window_event — `test_created_event_uses_injected_window_info` and friends (iter 41)
- [x] `R32-T2` Daemon singleton check test no longer ignored in default CI run — deterministic pipe-name isolation (iter 41)
- [x] `V1` `cargo test --all --verbose` — 546 test-binary total, 542 passed, 4 ignored (+doc-test compile checks) (Iteration 65 double-loop revalidation)
- [x] `V2` `cargo test --workspace -- --ignored` (supplemental manual validation)
- [x] `V3` `cargo clippy --all -- -D warnings` (passed in both Iteration 65 QA loops)
- [x] `V4` `cargo clippy --workspace --all-targets --all-features -- -D warnings` (supplemental, non-CI gate; passed in both Iteration 65 QA loops)
- [x] `V5` `cargo fmt --all -- --check` (passed in both Iteration 65 QA loops)
- [x] `V6` `cargo build --release --all` (passed in both Iteration 65 QA loops)

## Incident References
- `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md`

## Housekeeping Rules
1. When an item is finished, mark it `[x]` here and set `status: "done"` in `CODEX_BLOCKER_FIX_PLAN.json`.
2. Add concrete evidence in JSON for each completed item (file refs and command summaries).
3. Keep only open work in **Active Work**.
4. Archive this file only when all active tasks and validations are complete.
