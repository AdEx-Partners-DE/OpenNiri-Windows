# Review Housekeeping Policy
Last Updated: 2026-02-06
Purpose: Keep review/planning docs current, reduce stale noise, and surface only open work.

## Canonical Files
- Active findings: `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- Active blocker plan (human): `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.md`
- Active blocker plan (machine): `docs/1_Progress and review/CODEX_BLOCKER_FIX_PLAN.json`
- Open-work dashboard: `docs/1_Progress and review/OPEN_ITEMS.md`

## Update Protocol (Every Iteration)
1. Update `CODEX_REVIEW_CONSOLIDATED.md` with latest findings and severity.
2. Update `CODEX_BLOCKER_FIX_PLAN.json` statuses (`open`, `in_progress`, `done`) and evidence.
3. Mirror task completion in:
   - `CODEX_BLOCKER_FIX_PLAN.md` checkboxes
   - `OPEN_ITEMS.md` (move done items to Completed)
4. If test totals changed, sync:
   - `docs/SPEC.md`
   - `docs/ARCHITECTURE.md`
   - `docs/1_Progress and review/ITERATION_LOG.md`

## Archiving Rules
Move redundant/outdated files to `docs/1_Progress and review/archive/` when:
1. A review cycle is fully superseded by a newer consolidated review.
2. A blocker plan is fully complete and validated.
3. A generated status file no longer contains open work.

## Archive Naming
- `<YYYY-MM-DD>_<source_filename>`
- Example: `2026-02-06_CODEX_BLOCKER_FIX_PLAN.md`

## Done Criteria for Blocker Plan
All conditions must be true:
1. All task items are `done` in `CODEX_BLOCKER_FIX_PLAN.json`.
2. All validation-gate checks are `passed`.
3. `OPEN_ITEMS.md` has no entries under Active Work.
4. Consolidated review is updated to reflect closure.

## Archive Procedure (When Done)
1. Copy active plan files into `archive/` using archive naming.
2. Keep lightweight pointers in active files:
   - "Completed in iteration X, see archive file Y."
3. Reset active files for next unresolved work only.
