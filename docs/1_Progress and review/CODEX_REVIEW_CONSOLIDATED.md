# Codex Consolidated Review (Latest: Review 32)
Date: 2026-02-08
Reviewer: Codex
Scope: Full QA pass and regression review after latest Claude updates.
Status: Review-32 findings are closed. INC-49 mitigation code/docs landed, but host validation evidence is still open. Release remains blocked until incident closure evidence exists.

## Verification Evidence
- `cargo test --all --verbose` -> PASSED: **530 total** (**526 passed, 4 ignored**) (Iteration 54 rerun)
- `cargo test --workspace -- --ignored` -> PASSED: 4 ignored tests executed successfully (supplemental manual validation)
- `cargo clippy --all -- -D warnings` -> PASSED (Iteration 54 rerun)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` -> PASSED (supplemental; not part of CI-enforced gate)
- `cargo fmt --all -- --check` -> PASSED
- `cargo build --release --all` -> PASSED

## Review-32 Resolution Status
- `R32-C1` host-level Win32 validation gap: closed via documented Host Smoke Test procedure.
  - `docs/TESTING_GUIDE.md`
- `R32-C2` created-event testability gap for `focus_new_windows=false`: closed via injectable lookup boundary and deterministic tests.
  - `crates/daemon/src/main.rs`
- `R32-C3` ignored daemon singleton test gap: closed via isolated-pipe deterministic test in default CI path.
  - `crates/daemon/src/main.rs`

## Open Items
- `INC-49` (Critical): Desktop lockout/focus-trap reported after local `openniri-cli apply` test, including failed in-session recovery and workflow disruption.
  - Implemented: `panic_revert` IPC/CLI path, shutdown cleanup hardening, MoveOffScreen restoration, emergency tray uncloak, and recovery/runbook/release-gate doc updates.
  - Remaining: execute and record host-level lockout recovery acceptance evidence (`INC-49-1`, `INC-49-4`, `INC-49-T1`).
  - Incident record: `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md`
  - Tracking plan: `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`
