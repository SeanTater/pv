## System Information

- **Date**: 2025-07-19 14:36:08 UTC
- **System**: Linux charlie 6.14.0-23-generic #23-Ubuntu SMP PREEMPT_DYNAMIC Fri Jun 13 23:02:20 UTC 2025 x86_64 x86_64 x86_64 GNU/Linux
- **CPU**: Intel(R) Core(TM) i7-1065G7 CPU @ 1.30GHz
- **Memory**: 14Gi
- **Rust pv**: Unable to determine version
- **System pv**: pv 1.9.31

## Benchmark Results

### Basic Throughput (1MB)

Basic data transfer test measuring raw throughput for 1MB of random data.

| `Rust pv` | 1.8 ± 0.1 | 1.7 | 2.0 | 1.10 ± 0.25 |
| `System pv` | 1.6 ± 0.4 | 1.4 | 2.4 | 1.00 |

🏆 **Winner**: System pv

### Basic Throughput (10MB)

Basic data transfer test measuring raw throughput for 10MB of random data.

| `Rust pv` | 6.3 ± 0.7 | 5.6 | 7.6 | 1.18 ± 0.15 |
| `System pv` | 5.3 ± 0.4 | 4.8 | 5.8 | 1.00 |

🏆 **Winner**: System pv

### Basic Throughput (100MB)

Basic data transfer test measuring raw throughput for 100MB of random data.

| `Rust pv` | 46.7 ± 4.5 | 41.5 | 53.5 | 1.05 ± 0.19 |
| `System pv` | 44.6 ± 6.9 | 36.5 | 54.7 | 1.00 |

🏆 **Winner**: System pv

### Basic Throughput (1GB)

Basic data transfer test measuring raw throughput for 1GB of random data.

| `Rust pv` | 508.0 ± 108.5 | 417.1 | 635.9 | 1.36 ± 0.29 |
| `System pv` | 372.5 ± 9.6 | 359.2 | 382.9 | 1.00 |

🏆 **Winner**: System pv

### Progress Display Overhead

Tests overhead of full progress display (progress bar, rate, bytes, ETA) with 1GB data.

❌ Benchmark failed

### Rate Limiting (10MB/s)

Tests rate limiting accuracy and overhead at 10MB/s with 100MB data.

❌ Benchmark failed

### Line Counting Mode

Tests line counting mode performance with 100,000 lines of text data.

❌ Benchmark failed

### Custom Format Strings

Tests custom format string parsing and display with 1GB data (time, rate, bytes).

| `Rust pv` | 427.5 ± 11.7 | 412.3 | 444.4 | 1.13 ± 0.05 |
| `System pv` | 379.2 ± 11.7 | 363.2 | 399.3 | 1.00 |

🏆 **Winner**: System pv

## Summary

This benchmark compares the Rust implementation of pv against the standard system pv across various use cases:

- **Basic Throughput**: Raw data transfer performance
- **Progress Display**: Overhead of visual progress indicators
- **Rate Limiting**: Accuracy and performance of bandwidth throttling
- **Line Counting**: Text processing performance
- **Custom Formats**: Format string parsing efficiency

All benchmarks use [hyperfine](https://github.com/sharkdp/hyperfine) for accurate timing with warmup runs and statistical analysis.
