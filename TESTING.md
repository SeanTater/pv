# Testing and coverage

Run the cross-platform suite with Rust 1.85+:

```sh
cargo test --locked
cargo test --locked --release
cargo clippy --all-targets --all-features -- -D warnings
```

CI runs debug tests on Linux, macOS and Windows, plus release tests on Linux.
The minimum Rust version, stable, beta and nightly are tested. Reference fixtures
compare with original pv **1.12.0**: set `PV_REFERENCE` to that executable. CI
builds it from a checksum-pinned source; local reference fixtures skip without it.

## Regression contracts

The integration suites cover ordinary copying, lines/NUL records, numeric output,
formats, throttling, output files and wait/delay/stop behavior. The additional
`reliability_tests` suite covers:

- Unicode and spaced paths with binary payloads on every platform.
- Conflicting modes and excessive memory-related options rejected before output truncation.
- High-frequency numeric updates with monotonic counts and exact final totals.
- Buffering/staging/inspection/rate option combinations and binary boundaries.
- Concurrent copies with independent payloads and destinations.
- Unix regular input positioned beyond 4 GiB using a sparse file, reading only its small tail.
- Temporary staging and IPC cleanup on success and ordinary output failure.
- Unix file permissions (skipped for root, which can bypass mode bits).

Fault-injection unit tests cover interrupted reads/writes, short writes, zero
writes and final flush failure. Other integration suites exercise Linux kernel
fallback, direct I/O, sparse output, Unix IPC, process monitoring and real PTYs.
Short transfer tests use generous watchdogs rather than asserting startup latency.

## Coverage

Install cargo-tarpaulin and run the same LLVM-based collection used in CI:

```sh
cargo tarpaulin --engine llvm --all-features --workspace --timeout 120 --fail-under 80 --out Xml --out Json --output-dir coverage
```

The LLVM backend instruments the `pv` subprocesses launched by `assert_cmd` as
well as the test harnesses. The previous default ptrace run without `--follow-exec`
reported only 11.49% and missed most integration-exercised code. LLVM also avoids
ptrace's substantial timing overhead in child-process and terminal tests.

On Linux x86_64, this branch measured **82.64% line coverage (1223/1480)** with
all 239 tests passing and no external upstream reference configured. This is a
baseline, not a claim of the issue's >90% target. Source/region mapping differs
between backends, so the old 11.49% is not an apples-to-apples improvement measure.
CI enforces an 80% floor and preserves XML/JSON reports as the `coverage-linux`
artifact; Codecov receives the XML report. Inspect uncovered lines before raising
the floor; do not exclude failure paths to reach a percentage.

Coverage is a Linux measurement; native Windows/macOS CI runs verify their own
code paths but do not contribute to this report. This suite does not establish
32-bit correctness merely by testing a 64-bit process beyond a 4 GiB file offset.
Remaining work for #24 includes >90% measured coverage, long-transfer memory/leak
measurements, controlled allocation/resource exhaustion, a 32-bit execution job,
and signal-termination cleanup tests. Normal error unwinding and EOF cleanup are
covered; uncatchable termination does not run Rust destructors.

## Performance

Use `benchmarks/run_benchmarks.sh` and the methodology in `benchmarks/README.md`
for repeated performance comparisons with integrity checks. The functional suite
is not a continuous performance regression monitor. Do not add tight shared-runner
throughput thresholds as substitutes for reproducible measurements.
