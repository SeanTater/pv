# Pipe viewer in Rust

`pv` monitors and controls data flowing through files and Unix pipelines:

```sh
pv archive.tar | gzip > archive.tar.gz
pv --rate-limit 10M image.iso > /dev/null
producer | pv --numeric --bytes > output.bin
```

Build with Rust **1.85 or newer**:

```sh
cargo build --release --locked
./target/release/pv --help
```

The original Pipe Viewer is the behavioral reference, currently pinned to
**pv 1.12.0**. This implementation targets drop-in compatibility, but does not
claim complete parity. The checklist below distinguishes tested contracts from
capabilities with remaining differences.

## Compatibility checklist

| Area | Available behavior | Verification / limits |
|---|---|---|
| Transfer and CLI | stdin, ordered files, `-`, long options, `-V`, `-s SIZE -S`, `-0` implying line mode | Differential payload/numeric tests against 1.12.0; input/output aliases rejected before truncation |
| Numeric output | `-n`, raw byte/line/bit counts, timer/rate fields, custom formats | Pinned differential fixtures; no unit suffixes in numeric counters |
| Display lifecycle | `-q`, `-f`, `-W`, `-D`, `-i`, names, widths | Real pseudo-terminal tests cover stalled input and wait; delay never sleeps the transfer |
| Buffer and copy paths | `-B`, `-C`, Linux `splice` / `copy_file_range`, best-effort `-J` | Binary boundary tests, kernel fallback tests, benchmark integrity checks |
| Rates and reporting | `-r`, `-a`, `-m`, `-e`, `-I`, `-v` / `--stats` | Rolling-window unit tests; byte statistics remain byte rates in line mode |
| Buffer inspection | `-T`, `-A`, `%T`, `%nA`, `%L` | Last-written content tests; inspected data uses buffered copying. `%T` can report the kernel transfer method |
| Additional display | `-8`, `-k`, gauge, four bar styles, cursor, extra display, ConEmu state | Cursor coordination tested with two processes on a pseudo-terminal; visual styling is not byte-for-byte upstream output |
| Transfer modifiers | discard, sparse files, error recovery / skip blocks, sync, named or temporary store-and-forward | Binary/sparse-tail tests; staging honors stop limits and displays both phases |
| Direct I/O | `-K` for regular files on Linux | Aligned allocations; unaligned tails use cached I/O so no data is lost. Unsupported filesystems report errors |
| Process monitoring | `-M in\|out\|both -- COMMAND ARGS...` | Unix input/output monitoring tests preserve payload and command exit status |
| Descriptor watching | `-d PID[:FD]`, multiple targets, `=NAME`, `@LISTFILE` | Linux `/proc` only; aggregates watched descriptors into one byte counter; line-mode watching is rejected |
| Process control | `-P`, `-R`, `-Q` | Live rate-change/query tests. Private IPC between normal-transfer or watch-mode **Rust pv** instances; cannot control/query upstream pv |

Transfer errors produce nonzero exits, including final flush failures. Interrupted
I/O retries; partial writes count only bytes actually written. Non-seekable read
errors fail instead of spinning under `-E`. Seekable input recovery advances past
bad ranges and writes zero padding. Error recovery is bounded by EOF for regular
files; real faulty-media/device recovery has not been validated here.

Numeric counts and payload compatibility are more tightly tested than terminal
appearance. Remaining parity work includes upstream IPC interoperability,
per-descriptor watch displays, advanced color/format sequences, exact terminal
styles and statistics formatting, native Windows process/terminal features, and
broader flag-interaction coverage. Cursor rows are coordinated between Rust pv
instances, not upstream instances. Process titles currently use Linux's short
process name (15 bytes), rather than replacing the full command line. Remote/query require a writable private directory under the
user's temporary directory; ordinary copying still works when IPC is unavailable.

Memory is bounded: transfer buffers accept 1 byte through 64 MiB (0 chooses the
default), last-written history is capped at 1 MiB, and rolling-rate samples use
bounded time buckets. The update interval and display delay must be finite,
nonnegative values of at most 86400 seconds; interval 0 selects 0.1 seconds.

## Changes to this project's earlier CLI

These intentional changes make existing upstream scripts work:

- Replace old `-S SIZE` with **`-s SIZE -S`**.
- **`-O` means sparse output**, not ignoring write errors. Write errors always fail.
- `-n -b` prints raw numbers, not `KiB` or other formatted units.
- `-D` delays only reporting; transfers continue immediately.
- `-v` / `--stats` reports rate statistics. `--verbose` remains an alias.
- `-a` uses a rolling average and `-I` displays a completion clock time.

## Tests and development

```sh
cargo test --locked
cargo test --locked --release
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
PV_REFERENCE=/usr/bin/pv cargo test --test compatibility_tests
```

`PV_REFERENCE` must identify original pv **1.12.0**. Without it, reference-dependent
fixtures are skipped; all self-contained regression tests still run. CI builds
upstream from a checksum-pinned source archive and runs those comparisons, along
with native Linux/macOS/Windows tests and an MSRV job.

The engine, CLI, rendering, IPC, process modes and terminal coordination live in
separate modules under `src/`. Add behavioral regressions before implementation;
use injected I/O faults and supplied time samples for deterministic checks, and
subprocess/pseudo-terminal tests for OS behavior.

## Performance

The copy engine uses Linux kernel-assisted transfers where possible, falling back
from the current descriptor offsets when a syscall is unsupported. Line counting
uses `memchr`. Accounting and progress formatting are independent, and rendering
runs at the requested interval rather than on every block.

See [the benchmark guide](benchmarks/README.md) and its versioned raw results.
Comparisons cover quiet copying, forced display, numeric/custom formats, line
counting, throttling and slow consumers. Performance wins are workload-specific;
this project does not claim it universally beats upstream.

## Installation and packaging

```sh
cargo install --path . --locked
# Or install the published version (which may predate this checkout):
cargo install pv
```

Prebuilt packages are available through the project's
[releases](https://github.com/SeanTater/pv/releases). Linux kernel acceleration,
direct I/O and `/proc` watching are platform-specific; ordinary buffered transfer
remains portable.

After changing dependencies, regenerate Flatpak's offline crate source list:

```sh
python3 benchmarks/generate_flatpak_sources.py
```
