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

This is event-level diagnostic output, not a per-buffer performance tracer.
For profiling, use the reproducible benchmark harness and system profiling tools.
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
These cases cannot guarantee cleanup. Native Windows console cancellation remains
separate work; ordinary Windows I/O error handling is unchanged.

Memory-allocation failures for transfer, line-estimation and kernel-fallback
buffers return an error instead of invoking an infallible allocation. Reduce `-B`
when the transfer buffer exceeds available address space. Descriptor exhaustion
suggests closing descriptors or raising the open-file limit; overlong paths
suggest shortening the path or filename component.
