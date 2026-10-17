# AGENTS.md

## Build and test
- GNU/MinGW toolchain is configured in `.cargo/config.toml`; no MSVC Developer Prompt is needed.
- In this workstation's PowerShell, use `cargo.exe` if the `cargo` wrapper intercepts short flags such as `-p`.
- Unit tests: `cargo test --workspace`. CI also runs `cargo clippy -- -D warnings` and `cargo fmt --check`.
- Desktop behaviour (tiling, hiding, focus, recovery) is only verified by the Windows Sandbox end-to-end test: `cargo build --release --workspace`, then `tools/sandbox/run-sandbox-e2e.ps1`. Run it for changes to `platform_win32` or the daemon's window handling, and before tagging a release.

## Rules
- Never run the daemon on the operator's own desktop to test something; use the sandbox test. A broken build can hide windows or trap focus (see `docs/incidents/2026-02-07-desktop-lockout.md`).
- Write tests that protect behaviour or a real regression. No tests for constants, message wording, one-per-enum-variant mappings or "does not panic"; prefer one table-driven test.
- Record user-visible changes in `CHANGELOG.md`. Open work belongs in GitHub issues, not in tracking files.

The role of this file is to describe common mistakes and confusion points. If you encounter something surprising or get stuck, tell the developer and note it here so future agents avoid the same issue.
