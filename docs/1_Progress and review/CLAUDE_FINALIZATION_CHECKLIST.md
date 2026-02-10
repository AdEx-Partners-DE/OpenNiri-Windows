# Claude Finalization Checklist
Date: 2026-02-07  
Owner: Claude (implementation), Codex (review)  
Purpose: Concrete remaining work to make OpenNiri-Windows ready for public use and first formal release.
Status: **Blocked for release interpretation while `INC-49` remains open.**

This checklist is not a release-go signal until `INC-49` closure evidence is recorded in incident and tracker docs.

## Baseline (Verified)
- `cargo test --all --verbose` -> 540 test-binary total, 536 passed, 4 ignored (+1 doc-test compile check)
- `cargo clippy --all -- -D warnings` -> clean
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` -> clean (supplemental, non-CI gate)
- `cargo fmt --all -- --check` -> clean
- `cargo build --release --all` -> clean

## Release Blocker Gate (Must Be Cleared First)
- [ ] `INC-49` incident record is closed with verified recovery evidence:
  - `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md`
- [ ] `INC-49` closure state is reflected in blocker tracking (`INC-49-1`, `INC-49-4`, `INC-49-T1` all `done`) in:
  - `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`
- [ ] Closure evidence is linked from:
  - `docs/1_Progress and review/OPEN_ITEMS.md`
  - `docs/1_Progress and review/ITERATION_LOG.md`
- [ ] No tagging/publish tasks in section C are started before all gate items above are complete.

## A) Stabilization and Quality Gates
- [ ] Define one canonical CI/release gate and enforce it in docs and workflow (`cargo test --all`, `cargo clippy --all -D warnings`, `cargo fmt --all -- --check`, `cargo build --release --all`; treat all-features clippy as supplemental unless CI-enforced).
- [ ] Complete a 7-day stabilization run with no P0/P1 regressions (track in `ITERATION_LOG.md`).
- [ ] Execute full host acceptance matrix and capture results:
  - [ ] single monitor
  - [ ] dual monitor
  - [ ] mixed DPI
  - [ ] ultrawide
  - [ ] suspend/resume
  - [ ] display unplug/replug
  - [ ] elevated/protected windows present
- [ ] Classify ignored tests explicitly as hardware/manual and define execution cadence.

## B) User Experience Completion
- [ ] Validate clean-machine first-run flow: `openniri-cli setup` -> `openniri-cli run` succeeds without source-build assumptions.
- [ ] Confirm hotkey conflict UX is actionable (requested vs registered shown, clear remediation steps).
- [ ] Validate troubleshooting path end-to-end (`doctor`, `collect-logs`, `safe-mode`) from a fresh user perspective.

## C) Distribution and Release (Blocked Until INC-49 Gate Closes)
- [ ] Publish signed binaries in GitHub Releases (`openniri.exe`, `openniri-cli.exe`).
- [ ] Add installer path (MSI or winget package).
- [ ] Implement and verify uninstall behavior:
  - [ ] autostart cleanup
  - [ ] config/data handling policy
  - [ ] visible-window restoration behavior is validated and documented (including limits/exceptions)
- [ ] Validate upgrade and rollback path across at least two tagged versions.
- [ ] Create and publish first public release tag (`v0.1.0-alpha.1` or chosen equivalent) with checksums and release notes, only after INC-49 closure evidence exists.

## D) Documentation and Repo Consistency
- [ ] Final consistency sweep so these files report the same status, counts, and scope:
  - [ ] `README.md`
  - [ ] `docs/SPEC.md`
  - [ ] `docs/ARCHITECTURE.md`
  - [ ] `docs/TESTING_GUIDE.md`
  - [ ] `docs/PUBLIC_READINESS_CHECKLIST.md`
  - [ ] `docs/1_Progress and review/ITERATION_LOG.md`
  - [ ] `docs/1_Progress and review/OPEN_ITEMS.md`

## Definition of Done
- [ ] New user can install, start, recover, and uninstall in under 5 minutes without Rust knowledge.
- [ ] Signed release artifacts are published and verifiable.
- [ ] No open P0/P1 items remain.
- [ ] All checklist items above are checked with evidence links in docs/iteration log.
