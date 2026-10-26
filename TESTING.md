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
cargo tarpaulin --engine llvm --all-features --workspace --timeout 120 --fail-under 90 --out Xml --out Json --output-dir coverage
```

The LLVM backend instruments the `pv` subprocesses launched by `assert_cmd` as
well as the test harnesses. The previous default ptrace run without `--follow-exec`
reported only 11.49% and missed most integration-exercised code. LLVM also avoids
ptrace's substantial timing overhead in child-process and terminal tests.

On Linux x86_64, this branch measured **91.19% line coverage (1418/1555)** with
all 245 tests passing and no external upstream reference configured. The preceding merged baseline was 83.47% (1298/1555); the expanded suite exceeds
the issue's >90% goal. Source/region mapping differs
between backends, so the old 11.49% is not an apples-to-apples improvement measure.
CI enforces a 90% floor and preserves XML/JSON reports as the `coverage-linux`
artifact; Codecov receives the XML report. Inspect uncovered lines before raising
the floor; do not exclude failure paths to reach a percentage.

Coverage is a Linux measurement; native Windows/macOS CI runs verify their own
code paths but do not contribute to this report. The Linux 32-bit reliability job
installs the i686 target and multilib runtime and actually executes the reliability
and resource-limit suites, including the positioned-input test beyond 4 GiB.

`resource_limits` applies limits only in spawned Linux children. It checks a
256 MiB stream under a 64 MiB address-space cap, a transfer buffer that cannot fit,
and an eight-descriptor open-file cap. Output collection runs concurrently and
children have a 30-second watchdog. These are bounded-memory/resource-pressure
contracts, not proof against every allocator failure or an indefinite leak soak.

Additional tests cover explicit-only remote updates, invalid remote defaults,
numeric/terminal formatting, buffer recovery bounds and non-seekable rejection,
direct-I/O flag restoration on failure, nested directory size references, sparse
and synchronized output, monitor errors and descriptor-watch validation. No
production lines are excluded to reach the threshold.

## Sustained memory and performance checks

The `Sustained reliability` workflow runs on pull requests and main pushes, with
longer runs every night and on manual dispatch. Each memory scenario runs for
12 seconds on a PR and **180 seconds** nightly/manually. Use a longer local run:

```sh
cargo build --release --locked
python3 benchmarks/soak.py --binary target/release/pv --duration 60 --output soak.json
```

Two persistent processes exercise ordinary buffered copying and history/rolling
rate formatting. Producers and consumers hash the complete stream independently,
and the final counts and hashes must match. Streaming is paced to 20 MiB/s to
sustain duration without saturating a shared runner. Linux RSS is sampled every
100 ms: after warmup, growth must stay within 8 MiB, peak RSS within 64 MiB, and
the child has a 128 MiB address-space cap. A watchdog bounds stuck I/O; failed or
incomplete runs cannot report success. This detects sustained growth at the tested
volume/duration, not mathematical proof that no tiny or extremely slow leak exists.

Performance checks build candidate and baseline revisions on the **same runner**.
PRs compare the merge result with the PR base; pushes compare the previous main
revision; nightly/manual runs use the v0.6.0 release anchor. Five workloads cover
file copying, pipes, lines, numeric output and formatting. Every variant first
passes exact payload/hash and numeric-count validation, then warmup. Seven paired
samples run in randomized order. A failure requires all of:

- A median paired slowdown greater than 25%.
- A one-sided paired bootstrap lower bound greater than 10% slowdown.
- A median increase greater than 5 ms.
- The same finding in a second independent seven-pair confirmation batch.

This broad gate catches repeatable substantial regressions without turning tiny
startup/scheduling differences into CI failures. JSON artifacts retain every
sample, binary hashes, results and completion status; a failed command or integrity
check fails the job. Policy tests include noisy/outlier cases and injected growth.
Do not interpret a green gate as a claim that performance cannot regress below
these detection limits, or as resolution of the separate optimization issue #23.

```sh
python3 benchmarks/regression.py --candidate target/release/pv --baseline /path/to/baseline/pv --output regression.json
python3 -m unittest discover -s benchmarks/tests
```

The workflow uploads `sustained-memory` and `paired-performance` artifacts even
when a check fails. Unix signal-termination cleanup is covered by the termination
suite; SIGKILL and forced process termination cannot run Rust destructors.
For detailed upstream comparisons, use `benchmarks/run_benchmarks.sh` and the
methodology in `benchmarks/README.md`.
