# Contributing to microvm

## Your First Pull Request

### 1. Fork and clone

```bash
git clone https://github.com/YOUR-USERNAME/microvm.git
cd microvm
```

### 2. Set up

```bash
rustup toolchain install stable
cargo build
```

Codesigning is required to run:

```bash
codesign --sign - --entitlements entitlements.plist --force target/debug/microvm
```

### 3. Create a branch

```bash
git switch -c fix/my-first-contribution
```

### 4. Make your changes

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

All must pass.

### 5. Commit with a conventional message

```bash
git commit -m "fix: reject memory above host physical limit"
```

| Prefix     | Meaning              |
| ---------- | -------------------- |
| `feat`     | New feature          |
| `fix`      | Bug fix              |
| `docs`     | Documentation only   |
| `refactor` | No behavior change   |
| `test`     | Tests only           |
| `chore`    | Build, CI, tooling   |

### 6. Push and open a PR

```bash
git push origin fix/my-first-contribution
```

Open a PR against `main`. CI runs automatically. A maintainer will review.

## Code Standards

- `clippy::pedantic` at warn level, `-D warnings` in CI
- All lint suppressions are line-level with justification comments
- Functions under 70 lines, files under 500 (excluding tests)
- No `unwrap` on runtime data (allowed in tests and compile-time provables)

## Vouch System

External contributions require being listed in `VOUCHED.td`. Open a
"Vouch request" issue to get started.

## AI Disclosure

If you use AI tools (Copilot, Claude, Cursor), mention it in your PR.
You must understand code you submit.

## Links

| Resource                                 | Description               |
| ---------------------------------------- | ------------------------- |
| [SECURITY.md](SECURITY.md)               | Reporting vulnerabilities |
| [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) | Community standards       |
| [SUPPORT.md](SUPPORT.md)                 | Getting help              |
