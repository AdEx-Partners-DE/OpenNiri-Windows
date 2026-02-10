# OpenNiri-Windows Public Readiness Checklist

**Owner**: Maintainers
**Last Updated**: 2026-02-09
**Goal**: Move from "public code repository" to "publicly usable product."

## Active Development Focus

While the project is still alpha, execute the pre-stable plan first:

- `docs/PRE_STABLE_EXECUTION_PLAN.md`
- `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md` (critical open incident)

This prevents packaging/signing work from crowding out reliability and usability work.

## Hard Release Gate (INC-49)

- No tag creation, prerelease publish, or public release publish is allowed while `INC-49` is open.
- Current gate truth: mitigation implementation is complete; required host closure evidence is still pending.
- Required closure evidence before any tagging/publish action:
  1. `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md` is updated with verified recovery evidence and closed status.
  2. `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json` shows `INC-49-1`, `INC-49-4`, and `INC-49-T1` as `done`.
  3. `docs/1_Progress and review/OPEN_ITEMS.md` and `docs/1_Progress and review/ITERATION_LOG.md` link the same closure evidence.

## Current Baseline

- Core daemon/CLI/features implemented and tested (540 test-binary tests, 536 passed, 4 ignored, plus 1 doc-test compile check).
- CI, tests, and strict clippy are in place, including GNU toolchain provisioning.
- README and GitHub About have been refreshed for public positioning.
- Project is still alpha and source-first (no tagged public release yet).

## Definition of Done (Public Usability)

The project is publicly usable when a new Windows user can:

1. Install in under 5 minutes without Rust toolchain knowledge.
2. Start/stop safely from one obvious entry point.
3. Understand default hotkeys and recover from misconfiguration quickly.
4. Report issues with reproducible data in under 2 minutes.
5. Upgrade and roll back with documented steps.

## Checklist

### 1) Distribution and Installation

- [ ] Publish signed release binaries on GitHub Releases (`openniri.exe`, `openniri-cli.exe`).
- [ ] Provide an installer package (MSI or winget-ready package).
- [ ] Add uninstall path that restores startup and leaves windows in visible state.
- [x] Add release checksums and integrity verification instructions.

### 2) First-Run Experience

- [x] Add `openniri-cli doctor` command (environment, permissions, pipe, config health).
- [x] Add first-run onboarding command (`openniri-cli setup`) with guided defaults.
- [x] Add "safe mode" launch (`--no-hotkeys` and `--no-cloak`) for troubleshooting.
- [x] Provide one-command "reset to defaults" and config backup/restore flow.

### 3) Reliability and Safety

- [x] Add end-to-end tests that start daemon and exercise real IPC command paths.
- [x] Add explicit regression tests for startup/shutdown under heavy window churn.
- [x] Add structured crash artifacts (timestamped logs + optional minidump guidance).
- [x] Add watchdog/health-check command for external supervision.

### 4) Compatibility and UX Quality

- [x] Publish compatibility matrix (Win32/WPF/Electron/UWP; elevated window behavior).
- [ ] Add tested monitor topology matrix (single, dual, ultrawide, mixed DPI) with linked host evidence in iteration/review docs.
- [x] Add documented fallback behavior for windows that refuse move/cloak APIs.
- [x] Add default profile presets (developer, laptop, ultrawide, accessibility-friendly).

### 5) Security and Trust

- [x] Add `SECURITY.md` with disclosure and response SLAs.
- [x] Add threat model summary for named-pipe access and local privilege boundaries.
- [x] Add privacy statement clarifying telemetry policy (currently none by default).
- [x] Validate that logs redact sensitive process/window content where required.

### 6) Support and Operations

- [x] Add GitHub issue templates (bug, regression, feature request, compatibility report).
- [x] Add triage labels and response workflow (`needs-logs`, `needs-repro`, `blocked`).
- [x] Add support playbook: "common failures and exact fixes."
- [x] Add pinned "Getting Help" discussion with required diagnostic commands.

### 7) Release Engineering

- [ ] `INC-49` closure evidence is present in incident + blocker + open-items/iteration docs before any tag/publish action.
- [x] Add `CHANGELOG.md` with semver and upgrade notes.
- [x] Add release checklist automation (tests, clippy, artifact build, checksums).
- [x] Add release gate checks for tag/version/changelog consistency in CI.
- [x] Add pre-release channel (`alpha`, `beta`, `stable`) and promotion policy.
- [x] Add rollback instructions for each tagged release.

### 8) Project Governance

- [x] Add `CODE_OF_CONDUCT.md`.
- [x] Add maintainer ownership map for crates and docs.
- [x] Add roadmap with milestone dates and acceptance criteria.
- [x] Define support window for previous releases.

## Suggested Execution Order

1. Distribution and first-run experience (sections 1 and 2)
2. Reliability and compatibility hardening (sections 3 and 4)
3. Security/support/release governance (sections 5 to 8)

## Immediate Next Items (Pre-Alpha Exit)

1. Resolve `INC-49` desktop lockout incident with verified recovery path evidence.
2. Add the first release section to `CHANGELOG.md` (`## [0.1.0-alpha.1]`) before tagging.
3. Run host smoke + manual monitor-matrix validation and attach evidence in iteration/review docs.
4. Tag and publish an unsigned `v0.1.0-alpha.1` prerelease only after CI release gates pass **and** `INC-49` closure evidence is linked in incident/tracker docs.
5. Keep signing/installer work deferred until the stability gate is met.

## Deferred Until Stability Gate

The following are intentionally deferred while core behavior is still stabilizing:

1. Signed binaries.
2. Installer/winget distribution.
3. Formal release-channel hardening.
