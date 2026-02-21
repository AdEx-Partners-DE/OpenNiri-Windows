# Repository Assessment: OpenNiri-Windows

**Date:** 2026-02-18
**Assessor:** Gemini CLI Agent
**Type:** Deep Dive

## Executive Summary
`OpenNiri-Windows` is a scrollable tiling window manager for Windows, inspired by the Niri WM on Linux. It features a robust, modular architecture built in Rust, separating the core layout logic from platform-specific win32 API calls. The project demonstrates high engineering standards with comprehensive testing, "safe mode" recovery mechanisms, and clear separation of concerns via a Cargo workspace.

## 1. Architecture & Structure
*   **Pattern:** Rust Workspace (Monorepo).
*   **Crates:**
    *   `core_layout`: Pure logic for the infinite horizontal strip layout. Platform-agnostic.
    *   `platform_win32`: Safe wrappers around Win32 APIs (hooks, window enumeration, dwm).
    *   `daemon`: The main event loop, state management, and orchestration.
    *   `ipc`: Inter-process communication protocol.
    *   `cli`: Command-line interface for controlling the daemon.

## 2. Code Quality
*   **Safety:** Strong use of Rust's safety guarantees.
    *   **Concurrency:** `AtomicBool` and `Arc` used correctly for signal handling and thread coordination.
    *   **Error Handling:** `anyhow` and `thiserror` used for structured error propagation.
*   **Resilience:**
    *   **Panic Revert:** The daemon is designed to uncloak/restore windows on exit, ensuring the user isn't left with a broken desktop.
    *   **Timeouts:** Layout application has hard timeouts to prevent the daemon from hanging if a Win32 call blocks.
*   **Testing:** `core_layout` has excellent unit test coverage, validating complex layout logic independently of the OS.

## 3. Configuration & Dependencies
*   **Build:** `cargo` (standard).
*   **Dependencies:** `windows-rs` (official MS bindings), `tokio` (async runtime), `clap` (CLI), `serde` (serialization).
*   **Platform:** Windows-only (by definition).

## 4. Security & Hygiene
*   **Root:** ✅ **Pass**. Clean.
*   **Secrets:** None.
*   **Hygiene:** Consistent Rustfmt style.

## 5. Recommendations
### Minor Improvements
*   **CI:** Ensure GitHub Actions are set up to run `cargo test` and `cargo clippy`.
*   **Docs:** Add a developer guide explaining the architecture of the `core_layout` engine for contributors.

## Forensic Audit Findings (2026-02-18)
1.  **Panic Safety:** The daemon implements a robust "Panic Revert" mechanism using `std::panic::catch_unwind` and a best-effort `restore_all_windows_moved_offscreen` function. This ensures users aren't left with a broken desktop if the WM crashes.
2.  **Concurrency:** Layout application is offloaded to a worker thread with a strict timeout (`APPLY_LAYOUT_TIMEOUT`), preventing the daemon from hanging on blocked Win32 calls.
3.  **Hook Races:** The use of `WINEVENT_OUTOFCONTEXT` introduces an inherent race condition where windows may be destroyed before their creation event is processed. The code handles this gracefully with validity checks.
