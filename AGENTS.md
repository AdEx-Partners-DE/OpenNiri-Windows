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
- `docs/1_Progress and review/ITERATION_LOG.md` - Development iteration tracking (update after each iteration)

## Notes
- **Codex Review**: Review `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` for feedback and open items before making changes.
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

