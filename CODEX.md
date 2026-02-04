# CODEX.md

This file is a thin wrapper. The canonical instructions for this repo are in `AGENTS.md`.

## How to Work Here
- Follow `AGENTS.md` as the source of truth.
- Keep changes small and verifiable.
- Run the checks listed in `AGENTS.md` before finishing.
- Prefer reuse before adding new code.
- If `tasks/lessons.md` exists, update it after corrections.

## Commands
- Build: `cargo build --release`
- Test: `cargo test --all`

<!-- PORTFOLIO_BASELINE_START -->
## Portfolio Baseline (Revertability)
- Leave the repo in a clean git state, or document outstanding changes in HANDOFF.md.
- For risky operations, ensure a rollback path (backup, version history, or scripted revert).
- Keep changes small and scoped.
<!-- PORTFOLIO_BASELINE_END -->

