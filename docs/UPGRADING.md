# Upgrading & Rollback Guide

## Version Channels

OpenNiri-Windows follows semantic versioning with pre-release channels:

| Tag Pattern | Channel | GitHub Release Type |
|-------------|---------|---------------------|
| `v0.1.0-alpha.1` | Alpha | Pre-release |
| `v0.1.0-beta.1` | Beta | Pre-release |
| `v0.1.0-rc.1` | Release Candidate | Pre-release |
| `v0.1.0` | Stable | Full release |

Pre-releases are intended for testing and feedback. They may contain breaking changes between versions.

## Upgrading

### From a Release

1. Stop the running daemon:
   ```
   openniri-cli stop
   ```

2. Download the new release from [GitHub Releases](https://github.com/AdEx-Partners-DE/OpenNiri-Windows/releases)

3. Replace `openniri.exe` and `openniri-cli.exe` with the new versions

4. Start the daemon:
   ```
   openniri-cli run
   ```

### From Source

```bash
git pull
cargo build --release
openniri-cli stop
# Binaries are in target/release/
openniri-cli run
```

### Verifying the Download

Each release includes a `checksums.txt` file with SHA-256 hashes. To verify:

```powershell
# Download checksums.txt from the release
# Then verify each file:
(Get-FileHash openniri.exe -Algorithm SHA256).Hash
# Compare with the hash in checksums.txt
```

## Config Compatibility

- **Within the same minor version** (e.g., 0.1.0 to 0.1.1): Config files are fully compatible. No changes needed.
- **Across minor versions** (e.g., 0.1.x to 0.2.x): New config keys may be added with sensible defaults. Existing configs continue to work. Deprecated keys are logged as warnings.
- **Breaking changes**: If a config key is renamed or removed, the release notes will document the migration steps.

The daemon always starts successfully even with an outdated config — unrecognized keys are ignored with a warning, and missing keys use defaults.

## Rolling Back

If a new version causes problems:

1. Stop the daemon:
   ```
   openniri-cli stop
   ```

2. Replace the binaries with the previous version (keep old releases or build from a previous tag):
   ```bash
   # From source:
   git checkout v0.1.0
   cargo build --release
   ```

3. Start the daemon:
   ```
   openniri-cli run
   ```

Your config file and saved workspace state are forward-compatible. Rolling back does not require config changes.

## Unsigned Binaries

OpenNiri binaries are currently **not code-signed**. Windows SmartScreen may show a warning when you first run downloaded binaries:

> "Windows protected your PC — Microsoft Defender SmartScreen prevented an unrecognized app from starting."

To proceed:
1. Click "More info"
2. Click "Run anyway"

This is a one-time prompt per binary. You can verify binary integrity using the SHA-256 checksums in each release.

Binaries are built by GitHub Actions CI in a clean environment. Each release links to the CI workflow run that produced it.
