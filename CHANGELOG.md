# Changelog

All notable changes to OpenNiri-Windows will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
As of 2026-02-07, this project has not had a tagged public release yet. All work below is pre-release (`v0.1.0-dev`).

## [Unreleased]

### Added

- `tools/sandbox/run-sandbox-e2e.ps1`: end-to-end test in a disposable Windows Sandbox covering tiling, hotkeys, focus, focus-follows-mouse, cursor latency, stop and crash recovery

- CI now provisions MinGW and uses `stable-x86_64-pc-windows-gnu`, with a release metadata gate that enforces tag/version/changelog consistency (iter 48)
- `openniri-cli health` command and `HealthCheck` IPC endpoint returning uptime, window count, and error count (iter 43)
- Structured crash reports via `format_crash_report()` with timestamped artifacts in `%TEMP%` (iter 43)
- `openniri-cli collect-logs` bundles config, logs, doctor output, and system info for bug reports (iter 42)
- `openniri-cli setup` first-run onboarding: checks config, generates defaults, offers autostart (iter 42)
- `openniri-cli config reset/backup/restore` for config management and recovery (iter 42)
- Injectable window enumeration boundary (`lookup_window_info`, `is_known_window`) for deterministic daemon tests (iter 41)
- Deterministic daemon singleton test via isolated pipe name (iter 41)
- Host smoke test procedure in TESTING_GUIDE.md with scripted PowerShell steps (iter 41)
- `docs/COMPATIBILITY.md` — compatibility matrix for Win32/WPF/Electron/UWP, monitor topologies, elevated windows (iter 44)
- Default profile presets: `openniri-cli init --profile developer/laptop/ultrawide` (iter 44)
- `docs/SUPPORT_PLAYBOOK.md` — common failures and exact fixes (iter 46)
- `docs/UPGRADING.md` — version channels, rollback instructions, config compatibility (iter 47)
- `docs/ROADMAP.md` — alpha/beta/stable milestones and acceptance criteria (iter 48)
- `CODE_OF_CONDUCT.md` — Contributor Covenant v2.1 (iter 48)
- Enhanced GitHub issue templates with required diagnostics section and template chooser (iter 46)
- SHA-256 checksums for release artifacts in CI (iter 47)
- Pre-release channel detection in CI (alpha/beta/rc tags) (iter 47)
- Threat model, privacy statement, and log redaction audit in SECURITY.md (iter 45)
- Support window definition for alpha/beta phases in SECURITY.md (iter 45)
- Test: `test_focus_new_windows_false_preserves_focus_in_daemon` — daemon-level config-driven test (iter 40)
- Test: `test_focused_event_updates_previous_focused_hwnd_for_floating` + `_for_tiled` — focus event regression tests (iter 40)
- `insert_window_no_focus()` method on Workspace for correct `focus_new_windows=false` behavior (iter 39)
- True ToggleFloating roundtrip: floating windows can now be returned to tiling via the same hotkey (iter 39)
- `format_startup_banner()` and `format_tooltip_text()` extracted for testability (iter 39)
- Test: `test_insert_window_no_focus_preserves_focus` + 2 related tests in core_layout (iter 39)
- Test: `test_toggle_floating_roundtrip` — daemon-level tile→float→tile roundtrip (iter 39)
- Test: `test_tooltip_format_paused_with_mismatch` — tooltip output assertion (iter 39)
- Scroll offset preservation through startup: `restore_state()` returns restored monitor set, startup skips `ensure_focused_visible()` for restored monitors (iter 38)
- True OS-registered hotkey count via `registered_count` field on `HotkeyState`, replacing `mapping.len()` in banner and tooltip (iter 38)
- Test: `test_restore_state_returns_restored_monitor_ids` — multi-monitor restore set validation (iter 38)
- Test: `test_hotkey_state_registered_count_default` — registered_count vs mapping.len() independence (iter 38)
- Test: `test_applying_layout_flag_cleared_after_layout_with_windows` — error-path flag cleanup (iter 38)
- MovedOrResized feedback loop suppression via `applying_layout` guard flag (iter 37)
- Config error-path tests: invalid TOML, empty config, unknown keys, wrong types (iter 37)
- Fullscreen-minimize daemon-level regression test (iter 37)
- Release profile with LTO, single codegen unit, and symbol stripping (iter 37)
- Antivirus troubleshooting section in TROUBLESHOOTING.md (iter 37)
- SECURITY.md security policy (iter 37)
- GitHub issue templates for bugs and feature requests (iter 37)
- `cargo audit` dependency vulnerability scanning in CI (iter 37)
- Manual testing guide (docs/TESTING_GUIDE.md) with 16 test scenarios (iter 37)
- Restore-state tests for scroll offset preservation and empty workspace safety (iter 36)
- Pipe-busy error code documentation test (iter 36)
- Hex color validation in config with automatic reset to default on invalid values (iter 34)
- Regex size limit (1 MB) for window rule patterns to prevent pathological freezes (iter 34)
- Shutdown thread join timeout (3s per thread, 5s total) for graceful exit (iter 34)
- Config warning display in startup banner so users see when values are clamped (iter 34)
- Hotkey registration count feedback in banner and tray tooltip (e.g., "7/10 registered") (iter 34)
- Workspace mutation tests for window destroy, floating remove, cross-monitor focus (iter 34)
- Config edge case tests: empty config, zero values, unknown keys, empty hotkeys (iter 34)
- `openniri-cli doctor` diagnostic command (iter 33)
- `--safe-mode`, `--no-hotkeys`, `--no-cloak` daemon flags (iter 33)
- `openniri-cli run --safe-mode` (iter 33)
- Startup banner with version, monitors, windows, hotkeys, config, log path (iter 33)
- `docs/GETTING_STARTED.md` walkthrough (iter 33)
- README rewrite with dual install paths (iter 33)
- Minimize/restore window tracking in Workspace with height redistribution among visible windows (iter 32)
- Daemon handlers for system minimize and restore events with automatic focus management (iter 32)
- Dynamic tray menu text that toggles between "Pause Tiling" and "Resume Tiling" (iter 32)
- Ctrl+C signal handler that triggers clean daemon shutdown (iter 30)
- Uncloak-all-managed-windows on daemon shutdown so windows are never left invisible (iter 30)
- Panic hook that emergency-uncloaks all visible windows on crash (iter 30)
- Per-monitor DPI awareness via `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2` (iter 30)
- Unified shutdown path shared by IPC Stop, Ctrl+C, and tray Exit (iter 30)
- SetForegroundWindow integration so focus commands actually move OS keyboard focus (iter 29)
- Owner-window filtering to exclude dialog windows from tiling (iter 29)
- UWP app detection so ApplicationFrameWindow windows are tiled correctly (iter 29)
- CloseWindow command with default hotkey Win+Shift+Q (iter 29)
- ToggleFloating command with default hotkey Win+F (iter 29)
- ToggleFullscreen command with default hotkey Win+Shift+F (iter 29)
- SetColumnWidth presets: Win+1 (1/3), Win+2 (1/2), Win+3 (2/3), Win+0 (equalize) (iter 29)
- Active window border highlighting via DWM DWMWA_BORDER_COLOR (iter 29)
- QueryStatus IPC command and `openniri-cli status` subcommand (iter 29)
- Tray menu items: Pause Tiling, Open Config, View Logs (iter 29)
- Auto-start via Registry with `openniri-cli autostart enable/disable` (iter 29)
- Reconcile-monitors bug fix and strengthened multi-monitor tests (iter 28)
- Unit tests for all handle_command() IPC branches and reconcile_monitors() scenarios (iter 27)
- Config regex validation at load time and pre-compiled window rule patterns (iter 25-26)
- Real touchpad gesture support via WH_MOUSE_LL low-level mouse hook (iter 24)
- Workspace persistence: save/restore state across daemon restarts via JSON snapshot (iter 24)
- DisplayChange event wiring for monitor hotplug handling (iter 23)
- Focus-follows-mouse with configurable delay and debouncing (iter 23)
- Configurable hide strategy (cloak vs move-off-screen) via `use_cloaking` config (iter 23)
- QueryAllWindows IPC command and `openniri-cli query all` subcommand (iter 23)
- Integration test infrastructure with 17 IPC protocol correctness tests (iter 23)
- HWND validation to skip events for destroyed windows (iter 22)
- catch_unwind wrappers in all Win32 callbacks to prevent callback panics (iter 22)
- DeferWindowPos fallback to individual SetWindowPos on batch failure (iter 22)
- WindowInfo and IpcRect types for rich window querying (iter 22)
- System tray icon with context menu (Refresh Windows, Reload Config, Exit) (iter 21)
- Per-window floating rules with regex matching on class, title, and executable (iter 21)
- Touchpad gesture support with configurable swipe-to-command mapping (iter 21)
- Visual snap hints via transparent overlay window during resize operations (iter 21)
- Hotkey reload fix: config reload now re-registers hotkeys without daemon restart (iter 19)
- Configurable log_level and track_focus_changes in behavior config (iter 19)
- Smooth scroll animations with easing functions (EaseOut, EaseInOut, Linear) (iter 14)
- Animated workspace scrolling with configurable duration (iter 14)
- Global hotkey registration via RegisterHotKey API with config-driven bindings (iter 13)
- Multi-monitor support with per-monitor workspaces (iter 11)
- Cross-monitor commands: FocusMonitorLeft/Right, MoveWindowToMonitorLeft/Right (iter 11)
- TOML configuration file with hot-reload via `openniri-cli reload` (iter 10)
- Default config generation via `openniri-cli init` (iter 10)
- WinEvent hooks for window create, destroy, focus, minimize, restore, and move events (iter 9)
- Cloaked window filtering in window enumeration (iter 9)
- IPC protocol timeout (5 seconds) in CLI (iter 9)
- IPC protocol crate with 15 command variants and 5 response variants (iter 8.1)
- Monitor detection with `enumerate_monitors()` and `get_primary_monitor()` (iter 8.2)
- Async daemon with tokio event loop and named pipe IPC server (iter 8.3)
- Core scrollable tiling layout engine with column-based window management (iter 1-7)
- Win32 window enumeration and positioning via DeferWindowPos (iter 1-7)
- DWM cloaking for hiding off-screen windows (iter 1-7)

### Changed

- Windows scrolled out of view are now **cloaked in place through the Windows shell** (`IApplicationView::SetCloak`, as in GlazeWM) instead of being moved off-screen. DWM's own cloak call is denied for other processes' windows, so cloaking never worked before. The shell connection is re-established after Explorer restarts.

- **Default hotkeys now use Alt instead of Win** (`Alt+H/J/K/L` focus, `Alt+Shift+H/L` move column, `Alt+Ctrl+H/L` resize, `Alt+F` / `Alt+Shift+F` floating/fullscreen, `Alt+1/2/3/0` widths, `Alt+Shift+Q` close, `Alt+Shift+R` refresh). Most `Win+letter` combinations are reserved by Windows 11 and `Win+L` can never be registered. Monitor bindings (`Win+Alt+...`) and the `Win+Ctrl+Escape` panic key are unchanged. Existing config files keep their bindings.
- Hotkeys that register are kept when others fail; each failed hotkey is logged by name (previously one failure disabled all hotkeys, including the panic key)
- Touchpad gestures are off by default and marked experimental: swipes are detected from scroll-wheel deltas, so ordinary scrolling triggered them
- Five consecutive failed layout applications pause tiling, stop animations and restore window visibility
- Windows of processes running with higher privileges than OpenNiri (for example an administrator terminal) are no longer tiled

- Focused event handler now updates `previous_focused_hwnd` for all managed windows including floating, making ToggleFloating roundtrip reliable (iter 40)
- Event validation skips `is_valid_window` for already-managed windows, preventing racy event drops during focus transitions (iter 40)
- ToggleFloating roundtrip test rewritten to use `handle_window_event(Focused(...))` instead of direct internal-field mutation (iter 40)
- ToggleFloating now unfloats the OS-foreground window if it is floating, enabling true tile↔float toggle (iter 39)
- Daemon window-create path uses `insert_window_no_focus()` when `focus_new_windows=false` to preserve keyboard position (iter 39)
- All 7 startup banner tests now assert output content via `format_startup_banner()` instead of only checking no-panic (iter 39)
- Tray tooltip tests now call extracted `format_tooltip_text()` instead of duplicating format logic (iter 39)
- Overlay doc-test changed from `ignore` to `no_run` with proper imports — now compile-checked (iter 38)
- Snap hints and gestures enabled by default for better out-of-box experience (iter 29)
- FocusUp/FocusDown now call apply_layout and sync_foreground_window (iter 29)
- HOTKEY_SENDER changed from OnceLock to Mutex to support hotkey re-registration (iter 19)
- AppState refactored from single workspace to per-monitor HashMap of workspaces (iter 11)

### Security

- The IPC pipe is created with an access list that admits only the user running the daemon and SYSTEM (previously the default ACL)
- The CLI no longer falls back to the global `\\.\pipe\openniri` name, which any local user could create, and refuses a pipe server running in another logon session or owned by another user

### Fixed

- Configuration reload validates and clamps values just like startup, preserves `--safe-mode` / `--no-hotkeys` / `--no-cloak`, and refreshes hotkeys for every reload source (including hotkeys and gestures), even if applying the new layout fails. Pending mouse focus is cancelled on reload.

- Focus-follows-mouse no longer applies stale debounce requests: pending focus is replaced directly in the event loop and the window under the cursor is checked again before focusing (daemon portion of #63).

- Gaps between tiled windows now match the configured `gap`/`outer_gap`: placement compensates for the invisible resize borders Windows 10/11 adds around windows (`DWMWA_EXTENDED_FRAME_BOUNDS`). Previously visible gaps were about 14 px larger and differed between applications.

- Windows cloaked by a daemon that crashed or was killed are recorded in `%LOCALAPPDATA%\openniri\cloaked-windows.txt` and restored by `emergency-uncloak`, the panic path and the next daemon start
- Windows at the edge of the strip are moved fully into view on stop and recovery instead of staying partly off-screen
- `openniri-cli query all` shows titles for cloaked windows instead of "Unknown"

- **Windows hidden by moving them off-screen were never restored.** Windows clamps positions to -32768, so windows parked at -100000 were not found again by stop, `emergency-uncloak` or the panic key. The parking position is now -30000, and windows left at -32768 by older versions are restored too. Minimized windows are never repositioned.
- Windows found at startup are tiled immediately instead of on the first command
- Focus-follows-mouse no longer scrolls the strip; scrolling put another window under the cursor, which then took focus
- Setting the foreground window now attaches to the current foreground thread, so focus changes that do not come from a hotkey (focus-follows-mouse) are no longer reverted by Windows
- Config reload no longer disables monitor-change detection
- Shutdown recovery no longer logs an uncloak failure for every window that was never cloaked

- WinEvent hooks, mouse hooks and the tray icon now run on threads with a message loop. They used to sit on the async main thread, which caused cursor lag after start, late or missing window events, gestures that never fired and an unresponsive tray menu (#12)
- Uncloak is only attempted for windows that are cloaked, and a denied cloak falls back to moving the window off-screen; this ends the repeated `DwmSetWindowAttribute(CLOAK=0) failed` errors and failed layout passes, also with `use_cloaking = false` (#12)
- A failed layout pass no longer forces the foreground window during rollback, which kept stealing focus during animations (INC-49)
- Closing a child window (tab, browser pane) no longer removes its still-open top-level window from management
- Tiled windows snap back when the user finishes dragging or resizing them (`EVENT_SYSTEM_MOVESIZEEND`) instead of on every position change, which fought the drag. Maximizing, Win+Arrow snapping or an app moving itself no longer triggers a snap-back; the next layout pass corrects the position
- New windows are adopted when first shown or uncloaked (`EVENT_OBJECT_SHOW`, `EVENT_OBJECT_UNCLOAKED`), not only when created; most apps create their window hidden
- Cloak/uncloak errors now include the HRESULT

- Restored scroll offsets no longer overwritten by `ensure_focused_visible()` during startup (iter 38)
- Banner and tray tooltip now show true OS-registered hotkey count instead of parsed-config count (iter 38)
- Duplicate-daemon detection now treats pipe-busy (ERROR_PIPE_BUSY/231) as "running" — prevents second daemon when first is serving a client (iter 36)
- Workspace persistence restore order: scroll offsets are no longer clamped to 0 on empty workspaces because restore now runs after window enumeration (iter 36)
- Tray tooltip now shows actual hotkey registration mismatch counts instead of always None (iter 36)
- Critical unwrap on floating_rect replaced with safe fallback to centered 800x600 window (iter 22)
- Hotkey changes in config now take effect on reload without restarting daemon (iter 19)
- Build failure from missing Config::generate_default method (iter 19, fixing iter 17-18 regressions)

### Removed

- Agent-process ballast: the tracking files under `docs/1_Progress and review/` (blocker plan JSON, open items, iteration log, review transcripts and templates), the `reviews/` templates, empty handoff/session/task-board files, `GEMINI.md`, old research exports in `docs/0_Research/`, the pre-stable and public-readiness checklists, the superseded manual `sandbox/` setup and `tools/inc49/` script, and the CI script that tested the workflow file. Everything remains in git history. The incident report moved to `docs/incidents/`.
- The CI release gate that read INC-49 status from the tracking JSON. Before tagging a release, run the sandbox end-to-end test (`tools/sandbox/`).

- Dead HideStrategy::Minimize and HideStrategy::MoveOffScreen variants (iter 9, restored MoveOffScreen in iter 23)
- Unused buffer_zone field from PlatformConfig (iter 9)
- Fake WM_POINTER-based gesture implementation replaced with real mouse hook (iter 24)
