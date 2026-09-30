# Performance comparison

Run on Unix with Python 3.11+, Rust, and an explicit upstream pv 1.12.0 binary:

```bash
./benchmarks/run_benchmarks.sh --reference /usr/bin/pv --runs 7 --size-mib 256 --output benchmarks/results.json
```

The wrapper builds a locked release binary. The harness rejects identical binaries and other upstream versions. It compares default transfers and forced buffered transfers (`-C`) for both implementations.

Each case verifies payload hashes outside timed runs, warms up each variant, and randomizes variant order across repetitions. Numeric output also checks the final byte count. JSON records individual wall times, medians, ranges, child CPU time, hardware, binary hashes, and checkout identity. Only a report with `complete: true` represents a finished suite.

The nine cases cover file to null, pipe to pipe, file to file, line counting with an explicit total, forced progress display, numeric output, custom formatting, rate limiting, and a slow consumer. Most use 256 MiB; throttling uses 64 MiB at 64 MiB/s and the slow consumer uses 4 MiB. A 120-second watchdog bounds each invocation. Rate checks allow upstream's initial burst; finishing faster under a rate cap is not a throughput win.

## Local results, 2026-09-30

Linux x86_64, Intel i7-12700H, warm cache, seven measured runs. Default-mode medians from [the complete raw report](results-2026-09-30.json):

| Case | Rust milliseconds | Upstream milliseconds | Upstream time / Rust time |
| --- | ---: | ---: | ---: |
| File to null | 8.63 | 6.17 | 0.715 |
| Pipe to pipe | 9.67 | 6.66 | 0.689 |
| File to file | 61.77 | 62.83 | 1.017 |
| Line counting | 34.33 | 169.51 | 4.938 |
| Forced display | 8.49 | 6.32 | 0.745 |
| Numeric | 7.87 | 7.20 | 0.915 |
| Custom format | 8.46 | 7.35 | 0.869 |
| Rate limited | 1003.45 | 1009.92 | 1.006 |
| Slow consumer | 614.28 | 637.85 | 1.038 |

A ratio above one favors Rust. Line counting is substantially faster here; upstream remains faster for pipes and several short transfer/display cases. File-copy and consumer-limited differences are small and should not be treated as established wins. These are one-machine warm-cache measurements, not disk durability or cold-cache results. Small timing differences include process startup and scheduling noise. Inspect individual samples and buffered variants before drawing conclusions.

`sample_results.md` contains historical results from the previous harness and is not evidence for the current implementation. Differential correctness tests use `PV_REFERENCE=/path/to/pv-1.12.0 cargo test --locked --test compatibility_tests`; CI builds a checksum-pinned upstream source archive.
