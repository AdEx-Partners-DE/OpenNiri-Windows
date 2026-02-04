# CLAUDE.md

This file is a thin wrapper. The canonical instructions for this repo are in `AGENTS.md`.

## How to Work Here
- Read `AGENTS.md` first and follow it as the source of truth.
- Review `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` for feedback and open items.
- For non-trivial changes, produce a short plan before editing.
- Run the relevant checks listed in `AGENTS.md` before finishing.
- Prefer reuse before adding new code.
- If `tasks/lessons.md` exists, update it after corrections.

## Commands
- Build: `cargo build --release`
- Test: `cargo test --all`

## Notes
*None*

<!-- PORTFOLIO_BASELINE_START -->
## Portfolio Baseline (Revertability)
- Leave the repo in a clean git state, or document outstanding changes in HANDOFF.md.
- For risky operations, ensure a rollback path (backup, version history, or scripted revert).
- Keep changes small and scoped.
<!-- PORTFOLIO_BASELINE_END -->

