# AGENTS.md (Minimal)

## Build/runtime assumptions
- GNU/MinGW toolchain is configured in `.cargo/config.toml`; no MSVC Developer Prompt workflow is required.
- In this workstation's PowerShell, use `cargo.exe` if the `cargo` wrapper intercepts short flags such as `-p`.
- CI must run on both PR heads and pushes to `main`; a green PR is not post-merge proof. Validate workflow edits with `python tools/ci/test_workflow_contract.py`.
- Do not run desktop-control or INC-49 recovery scenarios on an active operator desktop. Green CI does not close the separate host-acceptance/release gate.

## Operational docs that must stay current
- `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`
- `docs/1_Progress and review/OPEN_ITEMS.md`
- `docs/1_Progress and review/ITERATION_LOG.md`
- Follow `docs/1_Progress and review/REVIEW_HOUSEKEEPING.md` to archive superseded review artifacts.

The role of this file is to describe common mistakes and confusion points. If you ever encounter something in this project that surprises you, or if you get stuck, please alert the developer and indicate that this is the case in this agent.md file to help prevent future agents from having the same issue.
