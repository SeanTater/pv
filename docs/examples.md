# Usage recipes

Examples use a POSIX shell. `pv` sends payload to stdout and progress to stderr;
redirections keep them separate. See [the man page](pv.1) for every option.
The local examples are exercised by `python3 scripts/check_examples.py --binary PATH`.

| Task | Command |
| --- | --- |
| Copy a file | `pv input.bin > output.bin` |
| Copy stdin quietly | `cat input.bin \| pv -q > output.bin` |
| Write a named destination | `pv -q -o output.bin input.bin` |
| Concatenate files | `pv -q first.bin second.bin > combined.bin` |
| Limit bandwidth | `pv -L 10M input.bin > output.bin` |
| Copy only a prefix | `pv -s 100 -S input.bin > prefix.bin` |
| Count lines | `pv -l -n -b records.txt > output.txt 2> counts.txt` |
| Count NUL-delimited records | `pv -0 -n -b records.bin > output.bin 2> counts.txt` |
| Record numeric byte counts | `pv -n -b input.bin > output.bin 2> counts.txt` |
| Customize the progress display | `pv -f -N backup -F '%N %b %r %e' input.bin > output.bin` |
| Inspect buffer and recent bytes | `pv -f -T -A 16 input.bin > output.bin` |
| Print final rate statistics | `pv -q -v input.bin > output.bin` |
| Count without writing data | `pv -X -n -b input.bin 2> counts.txt` |
| Stage before forwarding | `pv -q -U spool.bin input.bin > output.bin` |
| Compress a stream | `pv input.bin \| gzip > input.bin.gz` |

For an unknown-length pipe, add `-s SIZE` if you know the total, or `-g -p -r`
for a gauge relative to the highest observed rate. `-D 2` hides short-lived
progress without delaying the payload. Use `-W` to begin timing at the first byte.

## External tool integrations

These examples require the named tools and services and are not run by local CI:

```sh
# Export a local Docker image to a remote Docker daemon.
docker save myimage:latest | pv | ssh user@host 'docker load'
# Copy an archive over SSH, throttled to 10 MiB/s.
pv -L 10M archive.tar | ssh user@host 'cat > archive.tar'
# Compress a PostgreSQL database dump.
pg_dump mydb | pv | gzip > mydb.sql.gz
```

Check every pipeline stage for failure. In Bash, `set -o pipefail` makes a failed
producer or `pv` visible in the pipeline's exit status.

## Troubleshooting and migration

- No progress: check stderr redirection; use `-f` to force display or `-n` for scripts.
- No percentage/ETA: stdin often has no known size; supply `-s` or use byte/rate fields.
- Broken pipe: the receiving command exited or stopped reading. Inspect its errors.
- Permission denied or missing file: check input paths and destination directory access.
- Disk full: choose a destination with space; a failed copy is not a verified backup.
- Performance: compare `-C` and default copying using the [benchmark guide](../benchmarks/README.md).
  `-B`, `-T`, `-A` and line mode select buffering; `-Y` trades throughput for synchronized writes.
- Migrating from older Rust pv: use `-s SIZE -S`, and remember that `-O` means sparse output.
  See the [compatibility checklist](../README.md#compatibility-checklist) for remaining upstream differences.
- Windows: use `-o FILE` for direct file output when shell pipeline encoding could change bytes.
