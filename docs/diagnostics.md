# Errors and diagnostics

`pv` writes errors to stderr and keeps payload on stdout (or the file selected
with `-o`). Error messages identify failed input, output, PID and staging paths.
Invalid quantity options identify the option, value, and accepted units.
Transfers that fail report how many bytes were successfully written.

Common failures include a recovery hint: check paths or permissions, investigate
the downstream reader for a broken pipe, or free destination space for a full
filesystem. Error skipping (`-E`) is for seekable input recovery and can replace
unreadable data with zeros; it is not a way to validate a failed copy. No resume
command is suggested because this implementation does not support safe resumption.

Exit status categories remain unchanged: 2 for open/access failures, 8 for read
failures, 16 for write failures, and 64 for invalid options. Other failures may
return 1; child monitoring can propagate a command's exit code. Read stderr for
the actual diagnosis. A missing input can be reported while readable inputs are
still transferred; the final status remains nonzero.

## Opt-in diagnostics

Set `PV_DEBUG=1` to report the pv version, OS/architecture, normalized buffer and
rate settings, copy method and transferred count. Errors include the cause chain
and raw OS error code when available. Diagnostics go to stderr even in quiet
mode; `-v` / `--stats` continues to mean final transfer statistics.

```sh
PV_DEBUG=1 pv -q -C input.bin -o output.bin 2> pv-debug.log
```

PowerShell:

```powershell
$env:PV_DEBUG = '1'
.\pv.exe -q -C input.bin -o output.bin 2> pv-debug.log
Remove-Item Env:PV_DEBUG
```

Logs may contain file paths from errors. Review them before sharing. Debugging
is disabled unless the value is exactly `1`; it does not change transfer options.
Ordinary unredirected progress, numeric records, and debugging all share stderr,
so leave debugging off when another program parses numeric output.

For per-operation profiling, use `PV_TRACE=1` as described below; for comparable
throughput measurements use the benchmark harness with diagnostics disabled.
The copier transports arbitrary bytes; it does not diagnose content encoding or
application-level corruption. Interrupted system I/O is retried, and kernel copy
paths fall back when supported; unsupported platform features report errors.

## Interrupting a transfer (Unix)

SIGINT (Ctrl-C), SIGTERM, and SIGHUP request cooperative cancellation. Copy loops,
backpressure polling and throttling stop, ordinary Rust unwinding removes IPC
sockets and cursor registrations, and anonymous staging files close. Cancellation
returns `128 + signal` (130 for SIGINT, 143 for SIGTERM, 129 for SIGHUP). A partial
destination is not a complete copy. Named staging files and requested PID files
are retained, just as after ordinary exit.

Monitored commands run in their own process group. Cancellation forwards the
signal to that group and reaps the command; after a one-second grace period,
remaining group members are killed. Descendants that deliberately leave the group
are outside that scope. A parent's ignored signal disposition is respected.

A repeated handled signal forces immediate exit. SIGKILL cannot be caught, and
uninterruptible kernel/device operations may delay cooperative cancellation.
These cases cannot guarantee cleanup. Windows console cancellation is described
below; ordinary Windows I/O error handling keeps the same exit categories.

Memory-allocation failures for transfer, line-estimation and kernel-fallback
buffers return an error instead of invoking an infallible allocation. Reduce `-B`
when the transfer buffer exceeds available address space. Descriptor exhaustion
suggests closing descriptors or raising the open-file limit; overlong paths
suggest shortening the path or filename component.

## Operation profiling

`PV_TRACE=1` emits versioned `pv-trace: {JSON}` records on stderr. It is opt-in
and independent of `PV_DEBUG`. Read/write/kernel calls, kernel fallback, byte
accounting, throttling, rendering, and stream start/stop carry monotonic elapsed
and operation-duration nanoseconds, byte counts, method, error kind, process ID and a stream
ID. Staging and command-monitor directions use separate IDs. Records contain no
payload, argv, or file paths; requested normal progress fields can still display
content independently, so use `-q` for a clean diagnostic capture.

```sh
PV_TRACE=1 pv -q -C input.bin -o output.bin 2> trace.log
python3 scripts/summarize_trace.py trace.log > profile.json
```

The summary aggregates calls, bytes, total/max operation time and errors, and
marks streams without a stop record as incomplete. It
rejects malformed, unsupported or empty traces rather than inventing a profile.
Concurrent stream timings overlap; their sum is not wall-clock runtime. Per-buffer
logging changes timing significantly: compare performance with tracing disabled,
then use the trace to locate costly operations. Trace output is best effort if
stderr cannot be written. Disabled tracing performs no per-buffer clock reads or
record allocation. Numeric output shares stderr; do not enable tracing for an
unmodified numeric-output parser.

## Windows console cancellation

Ctrl-C and Ctrl-Break now request cooperative cancellation with exit status 130.
A console callback wakes a worker that cancels synchronous I/O on the copy thread,
including redirected reads/writes. Repeating the request forces immediate exit.
The handler ignores console-close/logoff/shutdown events so Windows retains its
normal forced-termination behavior. Programs without an attached console still
copy normally; no console keyboard event can be delivered to such a process.
Some device drivers cannot cancel I/O immediately; forced termination remains an
OS operation and cannot guarantee Rust-level cleanup.

The Windows CI suite creates a private console and exercises real Ctrl-C and
Ctrl-Break during stalled input, blocked output, throttling and temporary staging.
Anonymous staging files close/delete before successful cooperative exit. The
callback does not use Rust stdio or attempt cleanup itself.

## Error-reporting scope

All reported failures include an operation and a recovery hint. File/option
failures name paths or rejected values; copy failures identify source, destination
and successfully written bytes. Descriptor watching identifies list paths, line
numbers, malformed targets and descriptor metadata paths; disappearing descriptors
are treated as normal process-exit races. Causes and exit categories are retained.

A pipe viewer copies arbitrary bytes: it does not parse file formats, validate
UTF-8, or infer corruption from payload content. EOF is normal, and `--size` is an
estimate rather than an integrity contract. Hash/checksum validation belongs to
the producer/consumer or an explicit verification step. Simulated invalid encoding
is therefore a byte-preservation test, not an error scenario. Kernel scheduling
starvation, uncatchable termination and universal resilience after allocator
failure cannot be solved by an application error message. Bounded buffers, resource
fault tests, watchdogs and system-error context cover the actionable contracts.
