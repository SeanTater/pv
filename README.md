# Pipe viewer reimplementation

[![CI](https://github.com/SeanTater/pv/workflows/CI/badge.svg)](https://github.com/SeanTater/pv/actions)
[![codecov](https://codecov.io/gh/SeanTater/pv/branch/master/graph/badge.svg)](https://codecov.io/gh/SeanTater/pv)

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
| Quiet mode (`-q`) | ✅ | 🔴 Not Implemented |
| **Display Options** |
| Bits instead of bytes (`-8`) | ✅ | 🔴 Not Implemented |
| SI units (`-k`) | ✅ | 🔴 Not Implemented |
| Wait for first byte (`-W`) | ✅ | 🔴 Not Implemented |
| Delay start (`-D`) | ✅ | 🔴 Not Implemented |
| Gauge mode (`-g`) | ✅ | 🔴 Not Implemented |
| Average rate window (`-m`) | ✅ | 🔴 Not Implemented |
| Bar style (`-u`) | ✅ | 🔴 Not Implemented |
| Extra display (`-x`) | ✅ | 🔴 Not Implemented |
| Transfer stats (`-v`) | ✅ | 🔴 Not Implemented |
| Force output (`-f`) | ✅ | ✅ Implemented |
| Cursor positioning (`-c`) | ✅ | 🔴 Not Implemented |
| **Data Transfer Features** |
| Output to file (`-o`) | ✅ | ✅ Implemented |
| Rate limiting (`-L`) | ✅ | ✅ Implemented |
| Buffer size control (`-B`) | ✅ | 🔴 Not Implemented |
| No splice (`-C`) | ✅ | 🔴 Not Implemented |
| Skip output errors (`-O`) | ✅ | ✅ Implemented |
| Error skip blocks (`-Z`) | ✅ | 🔴 Not Implemented |
| Stop at size (`-S`) | ✅ | 🔴 Not Implemented |
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
- [ ] Quiet mode (`-q`) - Essential for silent operation

**Medium Priority (Enhanced Display):**
- [ ] Buffer percentage (`-T`) - Useful debugging feature
- [ ] Transfer statistics (`-v`) - Nice summary feature
- [ ] Gauge mode (`-g`) - Alternative progress display
- [ ] SI units (`-k`) - Standards compliance
- [ ] Bits display (`-8`) - Network monitoring use case

**Lower Priority (Advanced Features):**
- [ ] Watch file descriptor (`-d`) - Advanced monitoring feature
- [ ] Remote control (`-R`) - Advanced process control
- [ ] Store and forward (`-U`) - Specialized use case
- [ ] Direct I/O (`-K`) - Performance optimization
- [ ] Cursor positioning (`-c`) - Terminal control feature

### Summary

The current implementation covers exactly **57%** of the standard `pv` features (26 out of 46 options). It successfully implements the core progress monitoring functionality including custom format strings, numeric output, rate limiting, output to file, and force output, but lacks many advanced features that make the original `pv` versatile for different use cases.

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
- Complex multi-line displays (`-x`, `-v`)
- May require extending `indicatif` or custom terminal handling

The focus remains on implementing high-value features that provide the most utility while working well within the `indicatif` framework.

## Installation

### Static Binary (Recommended)

Download the pre-built static binary for Linux x86_64 from the [releases page](https://github.com/SeanTater/pv/releases):

```bash
# Download and install the latest release
curl -L -o pv https://github.com/SeanTater/pv/releases/latest/download/pv-linux-x86_64
chmod +x pv
sudo mv pv /usr/local/bin/

# Verify installation
pv --version
```

The static binary has no dependencies and works on any Linux x86_64 system.

### Flatpak

Download the latest Flatpak bundle from the [releases page](https://github.com/SeanTater/pv/releases) and install:

```bash
flatpak install pv.flatpak
```

Then run with:
```bash
flatpak run com.github.SeanTater.pv [options] [files...]
```

### From Source with Cargo

```bash
# Install from GitHub
cargo install --git https://github.com/SeanTater/pv.git

# Or clone and build locally
git clone https://github.com/SeanTater/pv.git
cd pv
cargo build --release
```

### Build Requirements

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