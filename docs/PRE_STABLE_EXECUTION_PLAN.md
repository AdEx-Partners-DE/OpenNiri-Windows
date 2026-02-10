# OpenNiri-Windows Pre-Stable Execution Plan

**Status**: Active (pre-alpha exit tracking)  
**Last Updated**: 2026-02-07  
**Intent**: Keep development focused on reliability and usability while the codebase is still unstable.

## Scope and Assumptions

- The project is still in alpha.
- The project is source-first (no tagged public release yet).
- We optimize for faster feedback and lower regression risk.
- We do **not** optimize for release packaging/signing yet.

## Completed Foundations (Iterations 35-47)

- `doctor`, safe mode, and `collect-logs` are implemented and documented.
- Compatibility matrix, support playbook, and issue templates are in place.
- Host smoke procedure and deterministic coverage for prior review gaps are in place.
- CI includes release checksums and prerelease classification.

## Current Priority (Now, Before First Tag)

### Critical Incident Gate (Must Close First)

1. `INC-49`: desktop lockout/focus-trap incident reported during local `apply` test must be resolved and verified.
2. Current gate truth: mitigation implementation is done (`panic_revert`, unified shutdown cleanup, MoveOffScreen restoration, emergency tray uncloak), but host closure evidence is still pending.
3. Evidence link: `docs/1_Progress and review/INCIDENT_2026-02-07_DESKTOP_LOCKOUT.md`.
4. Closure evidence must also be reflected in `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`, `docs/1_Progress and review/OPEN_ITEMS.md`, and `docs/1_Progress and review/ITERATION_LOG.md`.
5. No prerelease tagging/publish work proceeds until this gate is closed with linked evidence.

### Workstream A: Release-Readiness Consistency

1. Keep README/getting-started/testing docs aligned with GNU toolchain requirements.
2. Keep review + blocker + open-items trackers synchronized (no stale "open" findings).
3. Enforce release metadata checks in CI (`tag == Cargo.toml version`, changelog section exists).

### Workstream B: Alpha Exit Validation

1. Run host smoke validation on real Windows hardware and record evidence.
2. Run monitor-topology verification (single/dual/ultrawide/mixed DPI) and record outcomes.
3. Confirm no critical regressions for the agreed soak window before tagging.

### Workstream C: First Tagged Prerelease (Unsigned)

1. Add `## [0.1.0-alpha.1]` section to `CHANGELOG.md`.
2. Tag `v0.1.0-alpha.1` only after CI release gates pass and `INC-49` closure evidence is present in all required tracker docs.
3. Publish unsigned prerelease artifacts + checksums and capture evidence in iteration logs.

## Explicitly Deferred (Post-Stable / Beta Gate)

- Signed binaries.
- Installer / winget distribution.
- Formal release channels and upgrade/rollback UX hardening.

These remain in backlog and should not displace pre-stable engineering work.

## Completion Criteria for Pre-Stable Phase

- Release metadata gates pass for the first prerelease tag.
- `INC-49` closure evidence is present before any tagging/publish action.
- Host smoke and monitor-topology evidence is captured in project logs/docs.
- No active Codex blocker items remain in tracker documents.
- Signing/installer work remains explicitly deferred until stability gate.
