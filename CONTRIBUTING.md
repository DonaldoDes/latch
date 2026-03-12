# Contributing to latch

Thank you for your interest in contributing to latch.

## Prerequisites

- **Rust stable** (install via [rustup](https://rustup.rs/))
- **Cargo** (comes with Rust)

## Setup

```bash
git clone https://github.com/donmusic/latch.git
cd latch
cargo build
cargo test
```

## Code Conventions

### Formatting

`cargo fmt` is mandatory before every commit. CI enforces this.

```bash
cargo fmt
```

### Linting

Zero warnings policy. CI runs clippy with `-D warnings`.

```bash
cargo clippy -- -D warnings
```

### Tests

All code must be tested. Run the full suite:

```bash
cargo test
```

## Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): short description

Optional body with more detail.
```

**Types:** `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `perf`

**Scopes:** `pty`, `ipc`, `tui`, `cli`, `history`, `session`

## Pull Request Process

1. Create a branch: `type/short-description` (e.g., `feat/pty-server`, `fix/socket-cleanup`)
2. Write tests first (TDD)
3. Ensure CI passes: `make lint && make test`
4. Open a PR with:
   - Clear description of the change
   - Link to the related issue (`Closes #XXX`)
   - All checklist items checked
5. Wait for review

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). By participating, you agree to abide by its terms.
