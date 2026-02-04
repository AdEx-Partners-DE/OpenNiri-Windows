# Claude Code Review Feedback (Cycle 1)
Date: 2026-02-03
Reviewer: Codex
Scope: `crates/core_layout/src/lib.rs`, `crates/platform_win32/src/lib.rs`, `crates/daemon/src/main.rs`, `crates/cli/src/main.rs`, `Cargo.toml`, `README.md`, `.github/workflows/ci.yml`
Status: Review complete. No code changes made.

## Summary
The core layout engine is coherent and test-backed, but there are a few correctness gaps around empty columns, negative/insufficient vertical space, and ambiguous invariants. These can lead to invalid placements or inconsistent workspace state once window removal/stacking becomes real.

## Findings
1. [High] Removing the last window leaves an empty column and inconsistent invariants.
File: `crates/core_layout/src/lib.rs:266-287`, `crates/core_layout/src/lib.rs:219-229`
Why: `remove_window` removes empty columns only if there is more than one column. If the final column becomes empty, the workspace keeps an empty column, yet `total_width()` still counts it and focus indices remain non-empty, which likely breaks “workspace empty” semantics.
Fix: Decide invariant: either allow a truly empty workspace (remove the last column and reset focus indices) or return `LayoutError::CannotRemoveLastColumn` when the last column would become empty. Add a test for this case.

2. [High] `compute_placements` can produce negative heights when vertical space is insufficient.
File: `crates/core_layout/src/lib.rs:432-454`
Why: `usable_height = viewport.height - outer_gap*2` and `window_height = (usable_height - window_gaps) / window_count`. If `outer_gap` is large or many stacked windows exist, `usable_height - window_gaps` can go negative, producing negative heights and inverted rects.
Fix: Clamp `usable_height` to >= 0, and clamp per-window height to >= 0. Consider short-circuiting to zero-height placements when space is insufficient. Add tests that cover small viewport heights and many stacked windows.

3. [Medium] `scroll_offset` is `f64` but is truncated to `i32` for layout, causing jitter for smooth scrolling.
File: `crates/core_layout/src/lib.rs:410-421`
Why: Truncation discards fractional offsets, which will make smooth scroll deltas “stick” until a full pixel is accumulated.
Fix: Decide policy: store and apply `scroll_offset` as `f64` and round (or floor/ceil) consistently at render-time, or switch to `i32` and keep it pixel-precise. Add a test that verifies consistent behavior for fractional offsets.

4. [Low] Unused error variants indicate unclear invariants.
File: `crates/core_layout/src/lib.rs:17-30`
Why: `CannotRemoveLastColumn` and `EmptyColumn` are never emitted. This suggests the intended invariants aren’t fully encoded.
Fix: Either wire these into the control flow (see Finding #1) or remove them until needed.

## Test Gaps
1. No test for `CenteringMode::JustInView` behavior in `ensure_focused_visible`.
2. No test for removing the last window in the last column (empty workspace vs. error case).
3. No tests for stacked windows in a tight viewport where `usable_height - window_gaps <= 0`.
4. No tests for invalid column widths (zero/negative width) if such inputs are possible.

## Questions For Owner (to decide before next iteration)
1. Should an empty workspace be allowed (zero columns), or should a placeholder column always exist?
2. Do you want `outer_gap` to apply both horizontally and vertically, or should there be separate `outer_gap_x`/`outer_gap_y`?
3. Should widths/heights be validated and clamped at insertion time (e.g., minimum column width) rather than only during resizing?

## Recommended Next Actions (Claude)
1. Resolve the empty-workspace invariant and update `remove_window` accordingly.
2. Add height clamping logic in `compute_placements` and tests for tight vertical space.
3. Decide and document `scroll_offset` precision policy (f64 vs i32) and add a test.
4. Add the missing tests listed above.

## Notes
I did not run tests locally due to the known MSVC `link.exe` PATH conflict on this machine.
