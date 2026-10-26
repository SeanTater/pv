# Contributing

Use Rust 1.85+ and Python 3.11+. Start with a focused issue and branch; preserve
payload bytes and upstream-compatible exit behavior when changing display code.

```sh
cargo test --locked
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --locked
python3 scripts/generate_man.py --binary target/debug/pv
python3 scripts/check_examples.py --binary target/debug/pv
```

Commit `docs/pv.1` after changing CLI help or `docs/man-sections.roff`. CI checks
that the generated option reference matches the binary and renders with groff.
Install locally with `install -Dm644 docs/pv.1 ~/.local/share/man/man1/pv.1`;
ensure your man implementation searches that directory. Preview with
`man -l docs/pv.1`. Windows users can use `pv --help` and the Markdown recipes.
Cargo installs only the executable; install the man page separately.

## Architecture and tests

- `src/cli.rs`: option parsing and validation; quantities and normalized settings.
- `src/transfer.rs`: buffered and Linux kernel copying, throttling, direct/sparse I/O.
- `src/display.rs`: format parsing, counters, rates and rendering.
- `src/control.rs`: private Unix IPC for remote changes and queries.
- `src/process.rs`: child command monitoring and Linux descriptor watching.
- `src/terminal.rs`: terminal detection and cursor row coordination.
- `src/error.rs`: failure categories and exit status extraction.

`cargo doc --no-deps --document-private-items` builds the internal Rust reference.
Keep module docs and safety comments current when modifying a subsystem.

Prefer deterministic payload/error contracts and injected I/O failures to tight
wall-clock assertions. Tests under `tests/` cover CLI behavior, transfer modes,
terminal behavior, and processes. OS-specific tests use `cfg` guards. CI covers
Linux, macOS, Windows and the minimum Rust version. Reference comparisons use
upstream **pv 1.12.0**; see `.github/workflows/ci.yml` for its checksum-pinned build.
Set `PV_REFERENCE=/path/to/pv-1.12.0` to run those comparisons locally. Without it,
reference-dependent fixtures skip. See `benchmarks/README.md` for performance
methodology; timing comparisons need repeated measurements and integrity checks.

## Documentation review

Check that new options appear in the generated man page, examples preserve data,
platform restrictions are accurate, and unsupported behavior is not advertised.
External service examples are reviewed but not executed by CI. Keep recipes in
`docs/examples.md` consistent with `scripts/check_examples.py`.

## Packaging and releases

The GitHub release workflow ships a standalone man page alongside binaries and
checksums; Flatpak installs it under `/app/share/man/man1`. Future distribution
packages should install it in their standard man directory. Follow the README's
release procedure; regenerate Flatpak sources after dependency changes. Native
package-manager integrations remain tracked separately in #26.
