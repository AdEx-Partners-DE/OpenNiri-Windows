# Codex QA + Architecture Review (Cycle 5)
Date: 2026-02-03
Reviewer: Codex
Scope: Current core_layout implementation + docs (`docs/ARCHITECTURE.md`, `docs/SPEC.md`, `docs/WINDOWS_CONSTRAINTS.md`)
Status: Review complete. No code changes made.

## Executive Summary
Core layout logic is significantly stronger after Cycle 2, but there are still correctness risks around focus behavior on removals, unchecked arithmetic in placement calculations, and mismatch between documented invariants and public-field mutability. Architecture is on track for Phase 1 (layout engine + docs), but the platform layer, daemon event loop, IPC, and multi-monitor strategy remain stubs. The roadmap should now pivot from layout correctness to platform integration and observability.

## Cycle 4 Carryover (Not Yet Addressed)
- DEFAULT_* constants exist but aren’t used in `Workspace::default`, which risks drift between documented defaults and code.
- Invariants are documented but still not enforceable due to public fields (covered again below for emphasis).

---

## QA Findings (Code-Level)
1. [High] Focus can silently jump when removing a non-focused window earlier in the same column.
File: `crates/core_layout/src/lib.rs:319-347`
Why: `Column::remove_window` returns only a bool; `Workspace::remove_window` cannot detect removal index. If the focused index is after the removed window, the focused window shifts left but `focused_window_in_column` doesn’t decrement. Example:
- Column windows = [A, B, C], focused_window_in_column = 1 (B)
- Remove A
- Remaining windows = [B, C], but focus index remains 1 → focus changes to C unintentionally
Fix: Make `Column::remove_window` return `Option<usize>` (removed index). If removed index < focused index, decrement focus index. If removed index == focused index, define focus policy (e.g., next window, else previous).

2. [Medium] `column_x` and `compute_placements` use non-saturating arithmetic, inconsistent with `total_width()`.
File: `crates/core_layout/src/lib.rs:401-533`
Why: `total_width()` now saturates to prevent overflow, but `current_x += column.width + self.gap` can overflow for extreme widths or many columns. That produces negative coordinates and undefined placement ordering.
Fix: Use saturating arithmetic in `column_x` and `current_x` accumulation, or assert maximum column count/width in debug builds.

3. [Medium] `compute_placements` does not clamp or validate `scroll_offset`.
File: `crates/core_layout/src/lib.rs:460-535`
Why: `scroll_offset` is clamped in `scroll_by()` and `ensure_focused_visible()`, but callers can set `scroll_offset` directly (public field) or skip those methods. This can lead to placements far outside the intended range.
Fix: Either clamp inside `compute_placements()` (defensive), or make `scroll_offset` private and expose setters that clamp.

4. [Medium] Public fields undermine the newly documented invariants.
File: `crates/core_layout/src/lib.rs:172-205`
Why: Even with duplicate checks, any external code can mutate `columns`, `focused_column`, or `scroll_offset` directly, violating invariants without detection.
Fix: Encapsulate fields or add `validate()` + debug assertions, and clarify invariants are only guaranteed if callers use APIs.

5. [Low] `Rect::new` clamps negative dimensions but is bypassable with literals.
File: `crates/core_layout/src/lib.rs:40-58`
Why: `Rect` fields are public. Any direct literal can create negative width/height.
Fix: Either make fields private or add debug validation before consuming `Rect`.

6. [Low] Gap arithmetic allows negative gaps.
File: `crates/core_layout/src/lib.rs:494-506`
Why: `gap` and `outer_gap` are not clamped. Negative gaps invert spacing and can create overlap or odd placement.
Fix: Clamp to >= 0 at `with_gaps` or before use, or explicitly document negative gaps as supported behavior.

---

## Missing / Incomplete Tests
### Core Layout Unit Tests
1. Focus behavior when removing a window **before** the focused index in a column.
- Expected: focus remains on the same window ID, so index should decrement.

2. Focus behavior when removing the **focused** window in a stacked column.
- Expected: define policy (next if exists, else previous). Add test for both middle and last window cases.

3. `compute_placements` with column widths larger than viewport.
- Confirm placement and visibility behavior when a column is wider than viewport (center vs just-in-view).

4. `compute_placements` spacing integrity.
- For stacked windows, verify: sum(heights) + sum(gaps) == viewport.height - outer_gap*2.

5. `scroll_offset` clamping behavior.
- If `scroll_offset` is set above max or below 0 before `compute_placements`, decide and test whether it is clamped or left as-is.

6. Negative or extreme gap values.
- If negative gaps are invalid, test that they’re clamped/normalized. If valid, test expected overlap behavior.

7. Empty column handling in layout.
- `Column::empty()` exists and has tests, but `compute_placements` behavior for empty columns is not validated (should create no placements but still occupy horizontal space).

### Property-Based / Fuzz Tests (Recommended)
1. Operation sequence invariants.
- Random sequence of insert/remove/move/resize/focus should preserve invariants (no dup windows, valid focus indices, non-negative sizes).

2. Placement monotonicity.
- `column_x` positions should be non-decreasing and separated by at least `gap`.

3. Bounding tests.
- All placements should have non-negative width/height and remain within i32 bounds (no overflow).

### Integration / System Tests (Later Phases)
1. Daemon → layout → platform flow.
- Mock platform layer to verify placements are produced and applied on window events.

2. IPC command tests.
- Once named pipe server is implemented, test that each CLI command results in the correct workspace mutation.

---

## Architecture & Plan Check (Are We On Track?)
### What’s Complete (Phase 1)
- Repo structure + CI.
- Core layout engine with unit tests.
- High-level architecture docs and constraints.

### What’s Still Missing (Phase 2+)
1. **Win32 platform layer implementation**
- `enumerate_windows`, `apply_placements`, `cloak_window`, `install_event_hooks` are all stubs.

2. **Daemon event loop**
- No real event ingestion, IPC, or animation tick implemented.

3. **IPC protocol**
- `openniri-cli` prints text but doesn’t send commands.

4. **Multi-monitor abstraction**
- Not implemented in daemon or core. No monitor-aware workspace state.

5. **Gesture/input layer**
- No actual integration strategy (AHK, Raw Input, etc.).

6. **Configuration system**
- Spec mentions per-workspace config, but no config file format or reload mechanism exists.

### Alignment With Original Plan
- On-track for initial scaffolding and layout engine.
- Not yet on-track for “usable WM” because platform + daemon integration is still entirely stubbed.
- The next milestone should focus on a minimal vertical slice: enumerate windows → compute placements → apply placements for a single monitor.

---

## Document Alignment (ARCHITECTURE.md / SPEC.md)
### Still Accurate
- High-level crate roles and data flow match the current structure.
- Constraints in `WINDOWS_CONSTRAINTS.md` align with the planned cloaking + DeferWindowPos approach.

### Drift / Missing Updates
- `insert_window()` now returns `Result`, but `SPEC.md` and `ARCHITECTURE.md` still describe it as infallible.
- Duplicate window rejection is now enforced, but the spec does not mention uniqueness as an invariant.
- Focus-removal policy is undefined in `SPEC.md` (needs a clear rule for stacked columns).
- IPC is described as JSON over a named pipe, but the CLI/daemon do not implement IPC yet.
- Configuration is mentioned in the spec but no format or loader exists; needs either removal or a “planned” label.

### Recommended Doc Updates
- Update `SPEC.md` to define focus behavior when removing windows and clarify uniqueness/invariants.
- Update `ARCHITECTURE.md` to reflect current API (`insert_window -> Result`) and mark IPC/config as “planned”.

---

## Recommended Next Milestones (QA-Driven)
1. **Core Layout Hardening**
- Fix focus-on-removal bug (see Finding #1).
- Add tests for removal/focus rules, wide columns, and gap behavior.

2. **Platform Smoke Test (Single Monitor)**
- Implement minimal `enumerate_windows` and `apply_placements`.
- Create a demo CLI command that tiles current windows and verify manual behavior.

3. **Minimal Daemon Loop**
- Add a basic event loop with periodic tick to recompute layout.
- Wire IPC for at least `focus left/right` and `scroll left/right`.

4. **Observability**
- Add a debug dump of workspace state + placements to verify correctness before adding more features.

---

## Open Questions (Need Decisions)
1. What is the focus policy when removing the currently focused window in a stacked column?
2. Are negative gaps allowed or should they be clamped to >= 0?
3. Should `compute_placements` defensively clamp `scroll_offset` or should that remain caller responsibility?
4. Are you willing to make `Workspace` fields private (stronger invariants) or keep them public for flexibility?

---

## Notes
No changes were made. This is a QA + architecture assessment only.
