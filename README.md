# OpenNiri-Windows

An open-source scrollable-tiling window manager for Windows, inspired by [Niri](https://github.com/YaLTeR/niri).

## What is Scrollable Tiling?

Traditional tiling window managers divide your screen into fixed regions. When you open a new window, existing windows shrink to make room.

**Scrollable tiling** takes a different approach: windows are arranged on an **infinite horizontal strip**, and your monitor acts as a **viewport** (camera) that slides over this strip. When you open a new window, it simply appends to the strip without affecting existing windows.

This paradigm offers significant ergonomic advantages:
- Maintain spatial memory of your applications ("browser is to the right of terminal")
- No window resizing when opening new apps
- Navigate with smooth horizontal scrolling
- Perfect for ultrawide monitors and multi-tasking workflows

## Project Status

**Early Development** - This project is in its initial stages. Core layout algorithms are being implemented.

## Architecture

OpenNiri-Windows is structured as a Rust workspace with four crates:

| Crate | Purpose |
|-------|---------|
| `openniri-core-layout` | Platform-agnostic scrollable strip layout engine |
| `openniri-platform-win32` | Windows-specific HWND manipulation, DWM cloaking |
| `openniri-daemon` | Main event loop and state machine |
| `openniri-cli` | Command-line interface for control |

## Why Not Port Niri Directly?

Niri is a Wayland compositor that owns the entire rendering pipeline. On Windows, the Desktop Window Manager (DWM) is the exclusive compositor - no user-space application can replace it. OpenNiri-Windows operates as a "window controller" that manipulates window positions while DWM handles compositing.

## Comparison

| Feature | Niri (Linux) | Komorebi | GlazeWM | OpenNiri-Windows |
|---------|--------------|----------|---------|------------------|
| Scrollable tiling | Yes | Partial | No | **Goal** |
| Open source | GPL-3.0 | Source-available | GPL-3.0 | GPL-3.0 |
| Redistributable | Yes | No | Yes | **Yes** |
| Language | Rust | Rust | Rust | Rust |

## Building

### Prerequisites

- Rust (stable toolchain)
- Visual Studio Build Tools with "C++ build tools" workload

### Build Commands

```bash
# Clone the repository
git clone https://github.com/AdEx-Partners-DE/OpenNiri-Windows.git
cd OpenNiri-Windows

# Build all crates
cargo build --release

# Run tests
cargo test --all
```

### Troubleshooting

**Linker error: "link: extra operand"**

This occurs when Git Bash's `link` command shadows MSVC's `link.exe`. Solutions:
1. Use the "Developer Command Prompt for VS" instead of Git Bash
2. Or add MSVC bin directory to PATH before Git: `C:\Program Files\Microsoft Visual Studio\...\VC\Tools\MSVC\...\bin\Hostx64\x64`

## Usage

*Coming soon* - The project is in early development.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [Niri](https://github.com/YaLTeR/niri) - The original scrollable-tiling Wayland compositor
- [PaperWM](https://github.com/paperwm/PaperWM) - GNOME extension that pioneered the scrollable tiling concept
- [Komorebi](https://github.com/LGUG2Z/komorebi) - Windows tiling WM that informed Windows platform constraints
- [GlazeWM](https://github.com/glzr-io/glazewm) - Open-source Windows tiling WM

## Research

The `0_Research/` directory contains feasibility studies and research documents that informed the project's technical decisions.
