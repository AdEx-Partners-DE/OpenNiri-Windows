# Codex Review Cycle 3
Date: 2026-02-03
Reviewer: Codex
Scope: Claude outputs `docs/1_Progress and review/OpenNiri-Windows-Claude-Code_2.txt` and `_3.txt`
Status: Review complete. No code changes made.

## Summary
Claude addressed the prior findings and added robustness, but the new `insert_window()` API change was not propagated to non-test crates. CI will fail until those call sites are updated. There are also maintainability/invariant issues due to public fields and unused default constants.

## Findings
1. [High] `insert_window()` signature change not updated in non-test code.
File: `crates/daemon/src/main.rs:74-78`
Why: `insert_window()` now returns `Result<(), LayoutError>` but the daemon still calls it as `()`. This will break `cargo build --all` and CI.
Fix: Update all call sites to handle the `Result` (e.g., `.unwrap()` for demo code, or proper error handling).

2. [Medium] Default constants are defined but not used.
File: `crates/core_layout/src/lib.rs:13-21` vs `crates/core_layout/src/lib.rs:206-216`
Why: `DEFAULT_GAP`, `DEFAULT_OUTER_GAP`, `DEFAULT_COLUMN_WIDTH` exist, but `Workspace::default` still uses hard-coded literals (10/10/800). This invites drift.
Fix: Use the constants in `Workspace::default`, or remove the constants if you don’t want them.

3. [Medium] Invariants documented but not enforceable due to public fields.
File: `crates/core_layout/src/lib.rs:172-205`
Why: `Workspace.columns`, `Column.windows`, and other fields are public. External code can insert duplicates or empty columns, violating the stated invariants even after the new checks.
Fix options:
- Make fields private and expose methods/getters.
- Or add a `validate()` method and clarify that invariants apply only when using provided methods.

4. [Low] `Rect::new` clamps width/height, but `Rect` fields are public.
File: `crates/core_layout/src/lib.rs:40-58`
Why: Callers can still create invalid rectangles via struct literals, bypassing the clamp. If other code assumes non-negative sizes, this is a latent footgun.
Fix: Make fields private and require construction via `Rect::new`, or add debug assertions where `Rect` is consumed.

5. [Low] Inconsistent overflow handling in `compute_placements` vs `total_width`.
File: `crates/core_layout/src/lib.rs:494-506`
Why: `total_width` now uses saturating arithmetic, but `window_gaps = self.gap * (window_count - 1)` can still overflow for extreme values.
Fix: Use saturating arithmetic or clamp `gap` to >= 0 with reasonable bounds.

## Verification Notes
- Claude ran `cargo +stable-x86_64-pc-windows-gnu test -p openniri-core-layout`.
- Please run `cargo build --all` or `cargo check --all` after fixing call sites to confirm the workspace builds.

## Requested Actions (Claude)
1. Update all `insert_window()` call sites outside tests (start with `crates/daemon/src/main.rs`).
2. Decide whether to use the new `DEFAULT_*` constants or remove them.
3. Decide how to enforce invariants with public fields (encapsulation vs `validate()` + documentation).
4. Optionally add overflow-safety in `compute_placements` to match `total_width`.
