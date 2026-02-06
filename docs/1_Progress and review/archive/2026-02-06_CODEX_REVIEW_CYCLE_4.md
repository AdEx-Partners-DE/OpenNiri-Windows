# Codex Review Cycle 4
Date: 2026-02-03
Reviewer: Codex
Scope: Claude transcripts `docs/1_Progress and review/OpenNiri-Windows-Claude-Code_2.txt` and `_3.txt` + current `crates/core_layout/src/lib.rs`
Status: Review complete. No code changes made.

## Summary
The Cycle 2 changes landed cleanly and tests pass for `openniri-core-layout`. The main remaining risks are around maintainability and invariant enforcement: new constants aren’t used, public fields bypass the new checks, and gaps/overflow handling are still inconsistent in some paths. No critical blockers identified.

## Findings
1. [Medium] DEFAULT_* constants are defined but not used in `Workspace::default`.
File: `crates/core_layout/src/lib.rs:16-21`, `crates/core_layout/src/lib.rs:206-216`
Why: `DEFAULT_GAP`, `DEFAULT_OUTER_GAP`, `DEFAULT_COLUMN_WIDTH` exist but `Workspace::default` still hard-codes `10/10/800`. This will drift as defaults evolve.
Fix: Use the constants in `Workspace::default`, or remove the constants if you want literal defaults.

2. [Medium] Invariants are documented but not enforceable with public fields.
File: `crates/core_layout/src/lib.rs:172-205`, public fields in `Workspace`, `Column`, `Rect`
Why: External code can insert duplicate windows, create empty columns, or set invalid indices. This undermines the new duplicate checks and the invariant documentation.
Fix options:
- Make fields private and expose methods/getters.
- Or add a `validate()` method and clarify invariants are only guaranteed if callers use provided APIs (and call `validate()` in debug/tests).

3. [Low] `Rect::new` clamps dimensions, but struct literals bypass clamping.
File: `crates/core_layout/src/lib.rs:40-58`
Why: Public fields allow `Rect { width: -5, .. }` to exist even though `Rect::new` clamps.
Fix: Same as #2 (private fields or validation). If you keep fields public, at least document that `Rect::new` is recommended and that invariants aren’t enforced on literals.

4. [Low] Gap arithmetic can overflow or behave oddly with negative gaps.
File: `crates/core_layout/src/lib.rs:494-506`
Why: `window_gaps = self.gap * (window_count - 1)` is not saturating, and negative gaps are allowed.
Fix: Clamp `gap`/`outer_gap` to >= 0 (either at `with_gaps` or when used) and use saturating arithmetic for `window_gaps` to match `total_width()` behavior.

## Verification Notes
- Claude ran `cargo +stable-x86_64-pc-windows-gnu test -p openniri-core-layout` and reported all 25 tests passing.
- Full workspace build (`cargo build --all`) should be re-verified after the next changes.

## Requested Actions (Claude)
1. Decide whether to actually use `DEFAULT_*` constants or delete them.
2. Decide how invariants should be enforced (encapsulation vs `validate()` + documentation).
3. Clamp gaps or make gap arithmetic saturating for consistency.

## Optional Tests
- Add a test that exercises `with_gaps` with negative values (if allowed) and document expected behavior.
- Add a `validate()` test if you go that route.
