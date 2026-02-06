# OpenNiri-Windows Public Readiness Checklist

**Owner**: Maintainers  
**Last Updated**: 2026-02-06  
**Goal**: Move from "public code repository" to "publicly usable product."

## Current Baseline

- Core daemon/CLI/features implemented and tested.
- CI, tests, and strict clippy are in place.
- README and GitHub About have been refreshed for public positioning.
- Project is still alpha and source-first (no installer/release channel yet).

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
- [ ] Add release checksums and integrity verification instructions.

### 2) First-Run Experience

- [ ] Add `openniri-cli doctor` command (environment, permissions, pipe, config health).
- [ ] Add first-run onboarding command (`openniri-cli setup`) with guided defaults.
- [ ] Add "safe mode" launch (`--no-hotkeys` and `--no-cloak`) for troubleshooting.
- [ ] Provide one-command "reset to defaults" and config backup/restore flow.

### 3) Reliability and Safety

- [ ] Add end-to-end tests that start daemon and exercise real IPC command paths.
- [ ] Add explicit regression tests for startup/shutdown under heavy window churn.
- [ ] Add structured crash artifacts (timestamped logs + optional minidump guidance).
- [ ] Add watchdog/health-check command for external supervision.

### 4) Compatibility and UX Quality

- [ ] Publish compatibility matrix (Win32/WPF/Electron/UWP; elevated window behavior).
- [ ] Add tested monitor topology matrix (single, dual, ultrawide, mixed DPI).
- [ ] Add documented fallback behavior for windows that refuse move/cloak APIs.
- [ ] Add default profile presets (developer, laptop, ultrawide, accessibility-friendly).

### 5) Security and Trust

- [ ] Add `SECURITY.md` with disclosure and response SLAs.
- [ ] Add threat model summary for named-pipe access and local privilege boundaries.
- [ ] Add privacy statement clarifying telemetry policy (currently none by default).
- [ ] Validate that logs redact sensitive process/window content where required.

### 6) Support and Operations

- [ ] Add GitHub issue templates (bug, regression, feature request, compatibility report).
- [ ] Add triage labels and response workflow (`needs-logs`, `needs-repro`, `blocked`).
- [ ] Add support playbook: "common failures and exact fixes."
- [ ] Add pinned "Getting Help" discussion with required diagnostic commands.

### 7) Release Engineering

- [ ] Add `CHANGELOG.md` with semver and upgrade notes.
- [ ] Add release checklist automation (tests, clippy, artifact build, checksums).
- [ ] Add pre-release channel (`alpha`, `beta`, `stable`) and promotion policy.
- [ ] Add rollback instructions for each tagged release.

### 8) Project Governance

- [ ] Add `CODE_OF_CONDUCT.md`.
- [ ] Add maintainer ownership map for crates and docs.
- [ ] Add roadmap with milestone dates and acceptance criteria.
- [ ] Define support window for previous releases.

## Suggested Execution Order

1. Distribution and first-run experience (sections 1 and 2)  
2. Reliability and compatibility hardening (sections 3 and 4)  
3. Security/support/release governance (sections 5 to 8)

## Immediate Next 10 Items (Recommended)

1. Publish first binary release artifacts.
2. Add `openniri-cli doctor`.
3. Add issue templates and support playbook.
4. Add `SECURITY.md`.
5. Add `CHANGELOG.md`.
6. Add end-to-end daemon/IPC tests.
7. Add installer or winget packaging.
8. Add safe-mode launch and reset flow.
9. Publish compatibility matrix.
10. Define alpha/beta/stable release policy.
