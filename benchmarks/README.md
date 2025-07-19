# Performance Benchmarks

This directory contains scripts and results for benchmarking the Rust `pv` implementation against the original system `pv`.

## Running Benchmarks

### Prerequisites

1. **System pv**: Install the original pv utility
   ```bash
   # Ubuntu/Debian
   sudo apt-get install pv
   
   # RHEL/CentOS/Fedora
   sudo yum install pv
   # or
   sudo dnf install pv
   
   # macOS
   brew install pv
   ```

2. **hyperfine**: Install the benchmarking tool
   ```bash
   # Via cargo
   cargo install hyperfine
   
   # Via package manager (Ubuntu/Debian)
   wget https://github.com/sharkdp/hyperfine/releases/download/v1.18.0/hyperfine_1.18.0_amd64.deb
   sudo dpkg -i hyperfine_1.18.0_amd64.deb
   ```

3. **Build Rust pv**: Ensure the release build exists
   ```bash
   cargo build --release
   ```

### Running the Benchmark Suite

```bash
# Run all benchmarks and save results
./benchmarks/run_benchmarks.sh benchmarks/results.md

# View results
cat benchmarks/results.md
```

## Benchmark Categories

The benchmark suite tests the following scenarios:

1. **Basic Throughput**: Raw data transfer performance with various file sizes (1MB, 10MB, 100MB, 1GB)
2. **Progress Display Overhead**: Performance impact of visual progress indicators
3. **Rate Limiting**: Accuracy and overhead of bandwidth throttling
4. **Line Counting Mode**: Text processing performance
5. **Custom Format Strings**: Format parsing efficiency

## Interpreting Results

- **Mean Time**: Average execution time across multiple runs
- **Relative Performance**: How much faster/slower compared to the other implementation
- **Standard Deviation**: Consistency of performance across runs

## Contributing Benchmark Results

If you run benchmarks on different systems, please consider contributing results:

1. Run the benchmark suite: `./benchmarks/run_benchmarks.sh`
2. Include system information (CPU, memory, OS)
3. Submit results via issue or pull request

This helps build a comprehensive performance picture across different hardware configurations.