# Repository guidance

This Rust implementation targets drop-in compatibility with upstream pv 1.12.0. Read README.md for supported behavior and remaining differences; do not claim complete parity. Rust 1.85 is the minimum supported version.

## Architecture

- `src/main.rs`: dispatch, sources, output setup, and store-and-forward orchestration.
- `src/cli.rs`: clap configuration, checked quantity parsing, and validation.
- `src/transfer.rs`: buffered IO, Linux kernel copying, throttling, sparse output, direct IO, and recoverable read errors.
- `src/display.rs`: format parsing, counters, rolling rates, timing, statistics, and rendering.
- `src/error.rs`: typed failure categories and exit statuses.
- `src/control.rs`: private Unix socket updates and queries.
- `src/process.rs`: command monitoring and Linux descriptor watching.
- `src/terminal.rs`: terminal dimensions and coordinated cursor rows.

The default buffer is 128 KiB, bounded to 64 MiB. Display sampling and last-written history are bounded. Preserve payload bytes and account only successfully written bytes. Do not truncate an output alias of an input. A byte or line stop must not consume subsequent input. Flush failures must propagate. Kernel fallback must preserve partial progress and staged bytes.

## Validation

```bash
cargo test --locked
cargo test --locked --release
cargo +1.85.0 test --locked
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
PV_REFERENCE=/path/to/pv-1.12.0 cargo test --locked --test compatibility_tests
```

Use regression tests for observed failures and differential fixtures for upstream behavior. Inject IO failures for partial writes, interruptions, zero writes, and final flush errors. Use real pseudo-terminals for terminal behavior; regular captured stderr does not exercise those paths. Cross-compilation does not establish native runtime behavior.

See benchmarks/README.md for the reproducible performance harness. Regenerate Flatpak dependency sources with `python3 benchmarks/generate_flatpak_sources.py` after dependency changes. Do not commit, push, or publish unless requested.
