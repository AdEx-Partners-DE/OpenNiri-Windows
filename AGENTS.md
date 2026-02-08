# Agent Guide

> Repo: OpenNiri-Windows
> Generated: 2026-02-04
> Canonical instructions for AI coding agents in this repository.

## Project Summary
Scrollable tiling window manager for Windows (Rust workspace).

## Stack
Rust

## Ports
*None*

## Commands
- Build: `cargo build --release`
- Test: `cargo test --all`

## Workflow
- Uses GNU/MinGW toolchain (configured in `.cargo/config.toml`)
- Build commands work from any terminal (no need for MSVC Developer Prompt)

## Standard Workflow (Recommended)
- Plan first for non-trivial changes (3+ steps or architectural decisions).
- If something goes sideways, stop and re-plan.
- Prefer reuse: search for existing functions and patterns before adding new code.
- Verify before done (tests, logs, diffs).
- If `tasks/todo.md` or `tasks/lessons.md` exists, use them for plans and self-improvement.

## Boundaries
*None*

## Key Files
- `docs/ARCHITECTURE.md` - Technical architecture and crate responsibilities
- `docs/SPEC.md` - Behavioral specification
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` - Codex review findings and open items
- `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json` - Machine-readable blocker/task status
- `docs/1_Progress and review/OPEN_ITEMS.md` - Human-readable open-work dashboard
- `docs/1_Progress and review/ITERATION_LOG.md` - Development iteration tracking (update after each iteration)

## Notes
- **Codex Review**: Review `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` for feedback and open items before making changes.
- **Blocker Tracking**: Update both `CODEX_BLOCKER_FIX_PLAN.json` and `OPEN_ITEMS.md` when tasks move from open -> done.
- **Housekeeping**: Follow `docs/1_Progress and review/REVIEW_HOUSEKEEPING.md` to archive superseded files and keep only open work active.
- **Iteration Log**: After completing each development iteration, update `docs/1_Progress and review/ITERATION_LOG.md` with:
  - Iteration number and date
  - Objectives and status
  - Files modified with line numbers
  - Test results (before/after counts)
  - Evidence for verification

## Global Policies
- Do not add or expose secrets or credentials.
- Do not edit generated files or vendor folders unless explicitly asked.
- Check git status before destructive operations (deletes, resets, clean).
- For non-trivial work, produce a short plan before editing.
- Verify changes before done (tests, logs, or other evidence).
- Prefer reuse before adding new code (search for existing patterns first).
- If tasks/lessons.md exists, update it after corrections.
- Prefer minimal, scoped changes; avoid unrelated refactors.
- Update docs when behavior changes.


## Agent-Specific Guides
- `CLAUDE.md` - Claude Code wrapper for this repo
- `GEMINI.md` - Gemini CLI wrapper for this repo
- `CODEX.md` - Codex wrapper for this repo
- `OpenCode.md` - OpenCode wrapper for this repo

<!-- PORTFOLIO_BASELINE_START -->
## Portfolio Baseline (Revertability)
- Leave the repo in a clean git state, or document outstanding changes in HANDOFF.md.
- For risky operations, ensure a rollback path (backup, version history, or scripted revert).
- Avoid unrelated refactors; keep changes scoped.
<!-- PORTFOLIO_BASELINE_END -->
## Portfolio Governance Standard
- `C:\dev\0_repo_overarching\docs\agents\golden_repo_governance_proposal_v1.md`
- `C:\dev\0_repo_overarching\docs\agents\golden_repo_rollout_playbook_v1.md`
- `C:\dev\0_repo_overarching\docs\agents\agent_contract_snippet.md`
- `C:\dev\0_repo_overarching\docs\agents\multi_agent_concurrency_policy_v1.md`
- `C:\dev\0_repo_overarching\docs\agents\review_role_policy_v1.md`
- `C:\dev\0_repo_overarching\docs\agents\cross_repo_collaboration_protocol_v1.md`
## Mandatory Preflight (Parallel Agents)
- If same-repo parallel work exists or is expected, create a dedicated work tree before editing.
- Shared-tree mode is single-lane only; do not attach a second lane to the shared tree.
- In single-lane shared-tree mode, capture baseline first:
  - `git status --porcelain`
  - `git rev-parse --short HEAD`
- For dependency-affecting changes, require:
  - `docs/projects/portfolio_integration_board.md` row
  - `docs/projects/change_packets/<change-id>.md`
  - `docs/projects/cross_repo_handoff_queue.md` row
  - if uncertain, treat change as dependency-affecting
  - consumer acknowledgment before `done`
  - consumer acknowledgment recorded on integration board
  - producer+consumer tests + rollback artifact linked
- Operational check command:
  - `pwsh C:\dev\0_repo_overarching\scripts\portfolio\run-coordination-control-pass.ps1`


