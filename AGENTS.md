# Repository Guidelines

## Project Structure & Module Organization

Five stdio MCP tools: `read_file`, `write_file`, `edit_file`, `glob`, and `grep`.

- `src/server.rs` and `src/types.rs`: MCP handlers, schemas, and request types.
- `src/encoding.rs`: strict UTF-8/GBK decoding and byte boundaries.
- `src/hooks.rs`: Claude Hook decisions; `plugins/gbk-mcp/`: offline plugin templates.
- `src/files.rs` and `src/workspace.rs`: file operations, revision checks, atomic commits, and workspace confinement.
- `src/search.rs` and `src/control.rs`: parallel traversal, matching, limits, and cancellation.
- `tests/`: MCP and Hook tests; `scripts/`: smoke tests, benchmarks, and packaging.
- `examples/`: Claude/Codex configuration and consumer instruction snippets; `docs/`: validation records.
- `third-party-licenses/` and `THIRD_PARTY_NOTICES.md`: dependency license records.

## Build, Test, and Development Commands

Requires Rust 1.88+ and Python 3. Run `python3 scripts/test_package_plugin.py` for both package layouts and executable-mode checks.

```sh
cargo build --locked                         # Build debug binary
cargo run --locked -- --root /absolute/project # Start stdio server
cargo fmt --check                            # Check formatting
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked                          # Run Rust tests
python3 scripts/smoke_test.py --binary target/debug/gbk-mcp
cargo build --release --locked               # Build optimized binary
```

On Windows, append `.exe` to binary paths. CI runs formatting, linting, tests, protocol smoke tests, and packaging on Linux, Windows, and macOS. Use `python3 scripts/benchmark.py --generate --files 10000` after a release build to benchmark.

## Coding Style & Naming Conventions

Follow Rust 2024 conventions and default rustfmt formatting, including four-space indentation. Use `snake_case` for modules, functions, and tests; `PascalCase` for types; and `SCREAMING_SNAKE_CASE` for constants. Prefer existing dependencies for traversal, matching, encoding, and protocol handling. Keep blocking filesystem work off async executor threads.

## Testing Guidelines

Use `#[test]` and `#[tokio::test]` with temporary fixtures and behavior-based names. Cover encoding ambiguity, invalid bytes, BOM/CRLF preservation, conflicts, path boundaries, cancellation, limits, and Hook decisions. Assert original bytes where preservation matters. `scripts/plugin_smoke.py --binary target/debug/gbk-mcp` tests the real client with a local scripted model; use `--archive PATH` to test either packaged format; `--archive-dir dist` requires exactly one plugin archive; `--live` additionally requires authentication and consumes quota.

## Commit & Pull Request Guidelines

Use concise imperative commit messages, such as `fix: preserve untouched GBK bytes`. PRs should explain the problem, behavior changes, validation, and platform limitations; link relevant issues. Update schemas, examples, and documentation when tool contracts change. Release tags follow `vMAJOR.MINOR.PATCH` and must match the Cargo package version.

## Encoding & Configuration Safeguards

Never silently replace invalid characters or create transcoded UTF-8 copies. Preserve existing encoding and untouched bytes during edits. Reserve stdout for MCP messages; send diagnostics to stderr. Treat incomplete searches explicitly. Keep generated fixtures, build outputs, and credentials out of contributions. Update `Cargo.lock` and license records when dependencies change.
