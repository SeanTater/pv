#!/bin/bash

# Benchmark script to compare Rust pv implementation with original pv
# Usage: ./run_benchmarks.sh [output_file]

set -euo pipefail

# Configuration
RUST_PV="$(pwd)/target/release/pv"
SYSTEM_PV="/usr/bin/pv"
OUTPUT_FILE="${1:-benchmark_results.md}"
TEMP_DIR="$(mktemp -d)"
TEST_SIZES=(
    "1MB:1048576"
    "10MB:10485760" 
    "100MB:104857600"
    "1GB:1073741824"
)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[BENCH]${NC} $*" >&2
}

error() {
    echo -e "${RED}[ERROR]${NC} $*" >&2
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $*" >&2
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $*" >&2
}

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

# Check dependencies
check_dependencies() {
    log "Checking dependencies..."
    
    if ! command -v "$SYSTEM_PV" &> /dev/null; then
        error "System pv not found at $SYSTEM_PV"
        error "Please install pv: sudo apt-get install pv (Ubuntu/Debian) or sudo yum install pv (RHEL/CentOS)"
        exit 1
    fi
    
    if ! command -v hyperfine &> /dev/null; then
        error "hyperfine not found. Please install hyperfine for accurate benchmarking:"
        error "  cargo install hyperfine"
        error "  # OR"
        error "  wget https://github.com/sharkdp/hyperfine/releases/download/v1.18.0/hyperfine_1.18.0_amd64.deb"
        error "  sudo dpkg -i hyperfine_1.18.0_amd64.deb"
        exit 1
    fi
    
    if [[ ! -f "$RUST_PV" ]]; then
        log "Building Rust pv implementation..."
        cargo build --release
        if [[ ! -f "$RUST_PV" ]]; then
            error "Failed to build Rust pv at $RUST_PV"
            exit 1
        fi
    fi
    
    success "All dependencies found"
}

# Get version information
get_versions() {
    log "Getting version information..."
    
    echo "## System Information" > "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo "- **Date**: $(date -u '+%Y-%m-%d %H:%M:%S UTC')" >> "$OUTPUT_FILE"
    echo "- **System**: $(uname -a)" >> "$OUTPUT_FILE"
    echo "- **CPU**: $(grep 'model name' /proc/cpuinfo | head -1 | cut -d: -f2 | xargs)" >> "$OUTPUT_FILE"
    echo "- **Memory**: $(free -h | grep 'Mem:' | awk '{print $2}')" >> "$OUTPUT_FILE"
    echo "- **Rust pv**: $("$RUST_PV" --version 2>/dev/null || echo "Unable to determine version")" >> "$OUTPUT_FILE"
    echo "- **System pv**: $("$SYSTEM_PV" --version 2>&1 | head -1 || echo "Unable to determine version")" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
}

# Generate test data
generate_test_data() {
    local size_name="$1"
    local size_bytes="$2"
    local file_path="$TEMP_DIR/testdata_${size_name}.bin"
    
    log "Generating test data: $size_name ($size_bytes bytes)"
    
    # Use /dev/urandom for realistic data that doesn't compress well
    dd if=/dev/urandom of="$file_path" bs=1M count=$((size_bytes / 1048576)) status=none 2>/dev/null || \
    dd if=/dev/urandom of="$file_path" bs=1024 count=$((size_bytes / 1024)) status=none 2>/dev/null
    
    echo "$file_path"
}

# Run benchmark for a specific test case
run_benchmark() {
    local test_name="$1"
    local rust_cmd="$2"
    local system_cmd="$3"
    local description="$4"
    
    log "Running benchmark: $test_name"
    
    echo "### $test_name" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo "$description" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    
    # Run hyperfine benchmark
    hyperfine \
        --warmup 2 \
        --min-runs 5 \
        --max-runs 10 \
        --export-markdown "$TEMP_DIR/bench_${test_name// /_}.md" \
        --command-name "Rust pv" "$rust_cmd" \
        --command-name "System pv" "$system_cmd" \
        2>/dev/null || {
            warn "Benchmark failed for $test_name, skipping..."
            echo "❌ Benchmark failed" >> "$OUTPUT_FILE"
            echo "" >> "$OUTPUT_FILE"
            return
        }
    
    # Extract and format results
    if [[ -f "$TEMP_DIR/bench_${test_name// /_}.md" ]]; then
        # Add results to output file
        tail -n +3 "$TEMP_DIR/bench_${test_name// /_}.md" >> "$OUTPUT_FILE"
        echo "" >> "$OUTPUT_FILE"
        
        # Extract winner information
        local winner
        winner=$(grep -E "(Rust pv|System pv)" "$TEMP_DIR/bench_${test_name// /_}.md" | head -1 | awk '{print $2 " " $3}')
        if [[ "$winner" == "Rust pv" ]]; then
            echo "🏆 **Winner**: Rust implementation" >> "$OUTPUT_FILE"
        else
            echo "🏆 **Winner**: System pv" >> "$OUTPUT_FILE"
        fi
    fi
    
    echo "" >> "$OUTPUT_FILE"
}

# Main benchmark suite
run_benchmark_suite() {
    log "Starting benchmark suite..."
    
    echo "# Performance Benchmark: Rust pv vs System pv" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    
    get_versions
    
    echo "## Benchmark Results" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    
    # Test 1: Basic throughput with different file sizes
    for size_spec in "${TEST_SIZES[@]}"; do
        IFS=':' read -r size_name size_bytes <<< "$size_spec"
        test_file=$(generate_test_data "$size_name" "$size_bytes")
        
        run_benchmark \
            "Basic Throughput ($size_name)" \
            "cat '$test_file' | '$RUST_PV' > /dev/null" \
            "cat '$test_file' | '$SYSTEM_PV' > /dev/null" \
            "Basic data transfer test measuring raw throughput for $size_name of random data."
    done
    
    # Test 2: Progress display overhead
    local test_file_1gb
    test_file_1gb=$(generate_test_data "1GB" "1073741824")
    
    run_benchmark \
        "Progress Display Overhead" \
        "cat '$test_file_1gb' | '$RUST_PV' --progress --rate --bytes --eta > /dev/null" \
        "cat '$test_file_1gb' | '$SYSTEM_PV' --progress --rate --bytes --eta > /dev/null" \
        "Tests overhead of full progress display (progress bar, rate, bytes, ETA) with 1GB data."
    
    # Test 3: Rate limiting
    local test_file_100mb
    test_file_100mb=$(generate_test_data "100MB_rate" "104857600")
    
    run_benchmark \
        "Rate Limiting (10MB/s)" \
        "cat '$test_file_100mb' | '$RUST_PV' --rate-limit 10m > /dev/null" \
        "cat '$test_file_100mb' | '$SYSTEM_PV' --rate-limit 10m > /dev/null" \
        "Tests rate limiting accuracy and overhead at 10MB/s with 100MB data."
    
    # Test 4: Line counting mode
    log "Generating line-based test data..."
    local line_file="$TEMP_DIR/lines.txt"
    for i in {1..100000}; do
        echo "This is line number $i with some content to make it realistic" >> "$line_file"
    done
    
    run_benchmark \
        "Line Counting Mode" \
        "cat '$line_file' | '$RUST_PV' --lines > /dev/null" \
        "cat '$line_file' | '$SYSTEM_PV' --lines > /dev/null" \
        "Tests line counting mode performance with 100,000 lines of text data."
    
    # Test 5: Custom format strings
    run_benchmark \
        "Custom Format Strings" \
        "cat '$test_file_1gb' | '$RUST_PV' --format '%t %r %b' > /dev/null" \
        "cat '$test_file_1gb' | '$SYSTEM_PV' --format '%t %r %b' > /dev/null" \
        "Tests custom format string parsing and display with 1GB data (time, rate, bytes)."
    
    # Summary
    echo "## Summary" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo "This benchmark compares the Rust implementation of pv against the standard system pv across various use cases:" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo "- **Basic Throughput**: Raw data transfer performance" >> "$OUTPUT_FILE"
    echo "- **Progress Display**: Overhead of visual progress indicators" >> "$OUTPUT_FILE"
    echo "- **Rate Limiting**: Accuracy and performance of bandwidth throttling" >> "$OUTPUT_FILE"
    echo "- **Line Counting**: Text processing performance" >> "$OUTPUT_FILE"
    echo "- **Custom Formats**: Format string parsing efficiency" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo "All benchmarks use [hyperfine](https://github.com/sharkdp/hyperfine) for accurate timing with warmup runs and statistical analysis." >> "$OUTPUT_FILE"
    
    success "Benchmark suite completed. Results written to $OUTPUT_FILE"
}

main() {
    log "Starting pv performance benchmark"
    log "Output file: $OUTPUT_FILE"
    
    check_dependencies
    run_benchmark_suite
    
    success "Benchmarking complete!"
    log "View results: cat $OUTPUT_FILE"
}

main "$@"