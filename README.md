# Pipe viewer reimplementation

[![CI](https://github.com/SeanTater/pv/workflows/CI/badge.svg)](https://github.com/SeanTater/pv/actions)
[![codecov](https://codecov.io/gh/SeanTater/pv/branch/master/graph/badge.svg)](https://codecov.io/gh/SeanTater/pv)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![crates.io](https://img.shields.io/crates/v/pv.svg)](https://crates.io/crates/pv)

`pv` is a Unix pipe monitoring application. (And this is copy of the much older original)

You can use it in places where a progressbar, or at least a flow rate meter,
would be handy. Some handy examples:

```sh
# Is it still transferring or did something freeze?
docker save excelsior | pv | ssh me@devbox.company.com "docker load"
```

```sh
# Why doesn't gzip have a progressbar already?
pv gigantic-file | gunzip | gawk '/size/ { x += $4 } END {print x}'
```

## Installation

### Pre-built Binaries (Recommended)

Download pre-built binaries for your platform from the [releases page](https://github.com/SeanTater/pv/releases):

#### Linux (x86_64, static)
```bash
curl -L -o pv https://github.com/SeanTater/pv/releases/latest/download/pv-linux-x86_64-binary
chmod +x pv
sudo mv pv /usr/local/bin/
```

#### Windows (x86_64)
Download [`pv-windows-x86_64.exe`](https://github.com/SeanTater/pv/releases/latest) and rename to `pv.exe`, then add to your PATH.

#### macOS (Intel)
```bash
curl -L -o pv https://github.com/SeanTater/pv/releases/latest/download/pv-macos-x86_64-binary
chmod +x pv
sudo mv pv /usr/local/bin/
```

#### macOS (Apple Silicon)
```bash
curl -L -o pv https://github.com/SeanTater/pv/releases/latest/download/pv-macos-arm64-binary
chmod +x pv
sudo mv pv /usr/local/bin/
```

### Package Managers

#### Flatpak (Linux)
```bash
curl -L -o pv.flatpak https://github.com/SeanTater/pv/releases/latest/download/pv.flatpak
flatpak install pv.flatpak
```

#### From crates.io
```bash
cargo install pv
```

### From Source
```bash
# Install from GitHub
cargo install --git https://github.com/SeanTater/pv.git

# Or clone and build locally
git clone https://github.com/SeanTater/pv.git
cd pv
cargo build --release
```

### Verify Installation
```bash
pv --version
```

## Feature Comparison with Standard pv

This Rust implementation covers the core functionality of the original `pv` utility but is missing several advanced features. Here's a comparison:

| Feature | Standard pv | Status |
|---------|-------------|--------|
| **Core Display** |
| Progress bar (`-p`) | ✅ | ✅ Implemented |
| Timer (`-t`) | ✅ | ✅ Implemented |
| ETA (`-e`) | ✅ | ✅ Implemented |
| Finish ETA (`-I`) | ✅ | ✅ Implemented |
| Rate counter (`-r`) | ✅ | ✅ Implemented |
| Average rate (`-a`) | ✅ | ✅ Implemented |
| Byte counter (`-b`) | ✅ | ✅ Implemented |
| Line mode (`-l`) | ✅ | ✅ Implemented |
| Null termination (`-0`) | ✅ | ✅ Implemented |
| Size specification (`-s`) | ✅ | ✅ Implemented |
| Width control (`-w`) | ✅ | ✅ Implemented |
| Name prefix (`-N`) | ✅ | ✅ Implemented |
| Update interval (`-i`) | ✅ | ✅ Implemented |
| Skip input errors (`-E`) | ✅ | ✅ Implemented |
| **Additional Core Features** |
| Buffer percentage (`-T`) | ✅ | 🔴 Not Implemented |
| Last written bytes (`-A`) | ✅ | 🔴 Not Implemented |
| Custom format (`-F`) | ✅ | ✅ Implemented |
| Numeric output (`-n`) | ✅ | ✅ Implemented |
| Quiet mode (`-q`) | ✅ | ✅ Implemented |
| **Display Options** |
| Bits instead of bytes (`-8`) | ✅ | ✅ Implemented |
| SI units (`-k`) | ✅ | ✅ Implemented |
| Wait for first byte (`-W`) | ✅ | ✅ Implemented |
| Delay start (`-D`) | ✅ | ✅ Implemented |
| Gauge mode (`-g`) | ✅ | 🔴 Not Implemented |
| Average rate window (`-m`) | ✅ | 🔴 Not Implemented |
| Bar style (`-u`) | ✅ | 🔴 Not Implemented |
| Extra display (`-x`) | ✅ | 🔴 Not Implemented |
| Transfer stats (`-v`) | ✅ | ✅ Implemented |
| Force output (`-f`) | ✅ | ✅ Implemented |
| Cursor positioning (`-c`) | ✅ | 🔴 Not Implemented |
| **Data Transfer Features** |
| Output to file (`-o`) | ✅ | ✅ Implemented |
| Rate limiting (`-L`) | ✅ | ✅ Implemented |
| Buffer size control (`-B`) | ✅ | 🔴 Not Implemented |
| No splice (`-C`) | ✅ | 🔴 Not Implemented |
| Skip output errors (`-O`) | ✅ | ✅ Implemented |
| Error skip blocks (`-Z`) | ✅ | 🔴 Not Implemented |
| Stop at size (`-S`) | ✅ | ✅ Implemented |
| Sync writes (`-Y`) | ✅ | 🔴 Not Implemented |
| Direct I/O (`-K`) | ✅ | 🔴 Not Implemented |
| Discard output (`-X`) | ✅ | 🔴 Not Implemented |
| Store and forward (`-U`) | ✅ | 🔴 Not Implemented |
| **Advanced Features** |
| Watch file descriptor (`-d`) | ✅ | 🔴 Not Implemented |
| Remote control (`-R`) | ✅ | 🔴 Not Implemented |
| PID file (`-P`) | ✅ | 🔴 Not Implemented |

### Implementation Priority

**High Priority (Additional Core Features):**
- [x] Custom format strings (`-F`) - Essential for scripting and integration
- [x] Numeric output (`-n`) - Important for automation
- [x] Rate limiting (`-L`) - Common use case for bandwidth control  
- [x] Output to file (`-o`) - Basic I/O redirection
- [x] Force output (`-f`) - Important for non-terminal usage
- [x] Quiet mode (`-q`) - Essential for silent operation

**Medium Priority (Enhanced Display):**
- [x] SI units (`-k`) - Standards compliance
- [x] Bits display (`-8`) - Network monitoring use case
- [ ] Buffer percentage (`-T`) - Useful debugging feature
- [x] Transfer statistics (`-v`) - Nice summary feature
- [ ] Gauge mode (`-g`) - Alternative progress display

**Lower Priority (Advanced Features):**
- [ ] Watch file descriptor (`-d`) - Advanced monitoring feature
- [ ] Remote control (`-R`) - Advanced process control
- [ ] Store and forward (`-U`) - Specialized use case
- [ ] Direct I/O (`-K`) - Performance optimization
- [ ] Cursor positioning (`-c`) - Terminal control feature

### Summary

The current implementation covers **72%** of the standard `pv` features (33 out of 46 options). It successfully implements the core progress monitoring functionality including custom format strings, numeric output, rate limiting, output to file, force output, SI units, bits display, stop at size, wait for first byte, and delay start, but lacks many advanced features that make the original `pv` versatile for different use cases.

### Out of Scope Features

Some features are currently **out of scope** for this implementation due to limitations with the underlying `indicatif` library or complexity considerations:

#### Not Planned (Significant Technical Challenges)

**Remote Control (`-R`)**
- Requires inter-process communication and signal handling
- `indicatif` is designed for single-process use
- Would require major architectural changes

**Cursor Positioning (`-c`)**
- Requires precise terminal control beyond `indicatif`'s abstractions
- Complex interaction with terminal state management
- Limited practical use cases

**Buffer Percentage (`-T`) & Last Written Bytes (`-A`)**
- Requires access to internal buffer state that `indicatif` doesn't expose
- Would need custom buffering layer implementation

#### Lower Priority (Possible but Complex)

**Gauge Mode (`-g`)**
- Different display paradigm than `indicatif`'s percentage-focused approach
- Would require custom progress bar rendering

**Advanced Terminal Features**
- Bar style customization (`-u`)
- Complex multi-line displays (`-x`)
- May require extending `indicatif` or custom terminal handling

The focus remains on implementing high-value features that provide the most utility while working well within the `indicatif` framework.

## Performance

This Rust implementation provides competitive performance compared to the original `pv`. Based on benchmarks:

### Throughput Performance
- **Small files (1-10MB)**: System pv is ~10-18% faster
- **Medium files (100MB)**: Performance is nearly equivalent (~5% difference)
- **Large files (1GB)**: System pv is ~36% faster
- **Custom format strings**: System pv is ~13% faster

### Performance Summary
The Rust implementation shows good performance characteristics, especially for medium-sized data transfers. While the original `pv` maintains an edge in most scenarios due to decades of optimization, this implementation provides competitive throughput with the benefits of memory safety and modern Rust tooling.

See [benchmarks/](benchmarks/) for detailed performance analysis and instructions to run your own benchmarks.

## Build Requirements

- Rust 1.70+ (stable, beta, or nightly)
- Cargo package manager

## Development

### Setting up Pre-commit Hooks

This project uses pre-commit hooks to automatically format code and run linting checks before commits. This prevents formatting-related CI failures.

**Option 1: Automatic Git Hook (Recommended)**
The repository includes a Git pre-commit hook that will automatically run `cargo fmt` and `cargo clippy`:

```bash
# Hook is already set up - just make sure it's executable
chmod +x .git/hooks/pre-commit
```

**Option 2: Pre-commit Framework**
For more advanced setups, install the `pre-commit` framework:

```bash
# Install pre-commit (requires Python)
uv tool install pre-commit

# Install the git hook scripts
pre-commit install

# Optionally run against all files
pre-commit run --all-files
```

### Manual Commands

```bash
# Format code
cargo fmt

# Check linting
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test

# Check formatting without fixing
cargo fmt --all -- --check
```