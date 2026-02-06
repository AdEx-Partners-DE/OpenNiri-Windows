# Codex Review Cycle 6
Date: 2026-02-03
Reviewer: Codex
Scope: Claude transcript `docs/1_Progress and review/OpenNiri-Windows-Claude-Code_6.txt` + current `crates/core_layout/src/lib.rs`
Status: Review complete. No code changes made.

## Summary
Cycle 6 significantly improves robustness (encapsulation, focus-on-removal policy, saturating arithmetic, NaN/Inf handling) and expands test coverage to 38 tests. The core layout engine is now much safer to consume. Remaining issues are mostly documentation alignment and policy clarity, plus a couple of API/UX consistency concerns.

## Findings
1. [Medium] Specs and architecture docs are now out of sync with the new focus policy and API surface.
File: `docs/SPEC.md`, `docs/ARCHITECTURE.md`
Why: Cycle 6 introduces explicit focus behavior on removal, new API methods (`set_focus`, `focus_window`), new error variant (`WindowIndexOutOfBounds`), and encapsulation changes. The docs still describe focus behavior and insertions at a higher level without these specifics.
Fix: Update docs to:
- Describe the focus policy when removing stacked windows.
- Mention `insert_window()` is fallible and duplicates are rejected.
- Note the new focus APIs and error variant.

2. [Medium] Gap clamping is partial because config fields are still public.
File: `crates/core_layout/src/lib.rs:229-236`, `crates/core_layout/src/lib.rs:260-267`
Why: `with_gaps()` clamps negative values, but `gap` and `outer_gap` are still public and can be set to negative later. This undermines the new invariant that gaps are non-negative.
Fix: Either:
- Make gap fields private with validated setters, or
- Clamp `gap`/`outer_gap` defensively inside `compute_placements()` and `total_width()`, or
- Explicitly document that negative gaps are allowed/unsupported if you intend to keep them public.

3. [Low] `focus_window()` does not auto-scroll to keep focus visible.
File: `crates/core_layout/src/lib.rs:499-512`
Why: The spec says focus changes should scroll to keep the focused window visible. `focus_window()` changes focus indices but does not call `ensure_focused_visible()`.
Fix: Either:
- Document that scrolling is handled by the caller (daemon), or
- Provide a `focus_window_and_scroll(viewport_width)` convenience method if you want library-level correctness.

4. [Low] Public API breaking changes are not documented.
Why: Fields on `Workspace`/`Column` were made private, and `Column::remove_window()` signature changed. Even if not used externally today, this is a public API change.
Fix: Note this in a changelog or release notes (or mark the crate as pre-1.0 and document that API is unstable).

5. [Low] `test_compute_placements_empty_column` does not actually exercise empty columns inside a workspace.
File: `crates/core_layout/src/lib.rs` (test section)
Why: The test only creates a standalone `Column::empty()` and then computes placements for a workspace that does not include it. This doesn’t validate real layout behavior with empty columns.
Fix: If empty columns are a supported state, expose a way to create them in the workspace for testing. If not supported, consider removing or rewording the test.

## QA Coverage Status
New tests add strong coverage for focus-removal behavior, spacing integrity, and wide column placement. The largest remaining behavioral gap is documentation-level clarity (focus policy + API changes), not code correctness.

## Requested Actions (Claude)
1. Update `docs/SPEC.md` and `docs/ARCHITECTURE.md` to reflect the new focus policy, fallible insertions, and API changes.
2. Decide how to enforce non-negative gaps given public configuration fields.
3. Document API breakages (even if just a short “API unstable” note in README or a CHANGELOG).

## Verification Notes
Claude reports `cargo +stable-x86_64-pc-windows-gnu test -p openniri-core-layout` passes (38 tests) and `openniri-daemon` compiles.
