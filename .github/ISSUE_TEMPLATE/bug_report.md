---
name: Bug Report
about: Report a problem with OpenNiri-Windows
title: ''
labels: bug, needs-logs
assignees: ''
---

**Describe the bug**
A clear description of what happened.

**To reproduce**
Steps to reproduce the behavior:
1. ...
2. ...
3. ...

**Expected behavior**
What you expected to happen.

**Diagnostics (required)**
Please run `openniri-cli doctor` and paste the output:
```
# Paste openniri-cli doctor output here
```

If the daemon is running, also paste `openniri-cli status`:
```
# Paste openniri-cli status output here
```

**Environment**
- Windows version (`winver`):
- OpenNiri version (`openniri-cli status`):
- Running as admin: yes / no
- Number of monitors:
- Antivirus/EDR:

**Config (if relevant)**
```toml
# Paste relevant sections of your config.toml
```

**Logs**
```
# Set log_level = "debug" in config.toml, reproduce, then paste relevant lines
# Log location: %TEMP%\openniri-daemon.log
# Or use: openniri-cli collect-logs
```

**Screenshots**
If applicable, add screenshots to help explain your problem.
