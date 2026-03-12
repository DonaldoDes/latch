# latch

[![CI](https://github.com/donmusic/latch/actions/workflows/ci.yml/badge.svg)](https://github.com/donmusic/latch/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**Transparent terminal session manager** -- attach, detach, persist.

latch lets you run programs that keep going when you walk away. Attach and detach from running processes without ever interfering with the terminal stream. No hotkeys intercepted, no terminal emulation, no magic -- just raw PTY passthrough.

## Installation

### From source

```bash
git clone https://github.com/donmusic/latch.git
cd latch
cargo install --path .
```

### Cargo

```bash
cargo install latch
```

> Homebrew formula coming soon.

## Quick Start

```bash
# Start a new session
latch new mysession bash

# Detach (from another terminal)
latch detach

# List all sessions
latch list

# Re-attach
latch attach mysession

# Interactive TUI picker
latch
```

## Commands

| Command | Description |
|---------|-------------|
| `latch new <name> [cmd]` | Create a new session running `cmd` (default: `$SHELL`) |
| `latch attach <session>` | Attach to an existing session |
| `latch detach` | Detach from the current session (reads `$LATCH_SESSION`) |
| `latch list` | List all sessions with liveness status |
| `latch kill <session>` | Kill a session and its child process |
| `latch history <session>` | Show the scrollback history of a session |
| `latch rename <session> <new>` | Rename a session |
| `latch` | Launch the interactive TUI session picker |

## Philosophy

latch is **not** a terminal multiplexer. It does not do split panes, tabs, or window management. It does one thing: manage terminal sessions that survive detachment.

**Key principles:**

- **Transparency** -- raw PTY passthrough. The terminal sees exactly what the child process produces. No interception, no escape sequences consumed.
- **Session history survives everything** -- ring buffer persisted to disk. Crash, disconnect, reboot -- the history remains.
- **One server per session** -- no global daemon. Each session is an isolated process. A crash affects only that session.
- **Multiple clients** -- several terminals can attach to the same session simultaneously.
- **Tiny and auditable** -- small codebase, no feature creep. Every line justified.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup instructions, coding conventions, and PR process.

## License

[MIT](LICENSE)
