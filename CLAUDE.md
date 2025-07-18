# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust reimplementation of the Unix `pv` (pipe viewer) utility. It monitors data flowing through pipes and provides progress information including transfer rates, elapsed time, estimated completion time, and progress bars.

## Common Commands

### Build and Run
```bash
cargo build          # Build the project
cargo run             # Run with default settings
cargo run -- --help  # Show available options
```

### Testing
```bash
cargo test           # Run all tests (unit + integration)
cargo test --quiet   # Run tests with minimal output
cargo check          # Quick syntax check
```

### Development
```bash
cargo clippy         # Run linter
cargo fmt            # Format code
```

## Architecture

The application is structured as a single-file Rust program (`src/main.rs`) with these key components:

### Core Types
- `PipeViewConfig`: CLI configuration struct using `clap::Parser` with extensive command-line options
- `PipeView`: Main processing struct that handles data transfer with progress tracking
- `LineMode`: Enum for byte vs line counting modes

### Key Dependencies
- `clap`: Command-line argument parsing with derive features
- `indicatif`: Progress bar and spinner functionality
- `chrono`: Time handling for progress calculations

### Data Flow
1. Parse CLI arguments into `PipeViewConfig`
2. Determine input sources (stdin or files specified with `-f`)
3. Create chained readers for multiple input files
4. Configure progress bar based on CLI options and estimated size
5. Copy data from source to stdout with progress updates
6. Handle read/write errors based on skip flags

### Progress Bar Configuration
The progress bar template is dynamically built based on CLI flags:
- Name prefix (`-N`)
- Elapsed time (`-t`)
- Progress bar with configurable width (`-w`)
- Byte/line counts (`-b`, `-l`)
- Transfer rates (`-r`, `-a`)
- ETA calculations (`-e`, `-I`)

Default template when no specific options are provided shows elapsed time, progress bar, percentage, transferred/total, rate, and ETA.

## Key Implementation Details

- Uses 64KB default buffer size for I/O operations
- Supports both byte and line counting modes (line mode counts newlines or null terminators)
- Automatically estimates total size from input file metadata when possible
- Implements error skipping for both input and output operations
- Uses `indicatif::ProgressBar` for cross-platform progress display
- Rate limiting (`-L` flag) with k/m/g/t suffix support for bytes or lines per second
- Cumulative timing-based rate limiting that tracks total transfer progress

## Testing

The project includes comprehensive integration tests in the `tests/` directory:

- `integration_tests.rs` - Basic functionality tests (17 tests)
- `format_tests.rs` - Custom format string tests (19 tests) 
- `edge_cases.rs` - Edge cases and error handling (22 tests)
- `numeric_tests.rs` - Numeric output mode tests (18 tests)
- `rate_limiting_tests.rs` - Rate limiting functionality tests (11 tests)

Tests use `assert_cmd` for CLI testing and `tempfile` for file-based tests. All tests verify that data passes through correctly while testing the progress monitoring functionality. Rate limiting tests include timing-based assertions to verify actual rate limiting behavior.

## Development Workflow

When adding new features:

1. Create a feature branch: `git checkout -b feature/feature-name`
2. Set up pre-commit hooks (first time only): `chmod +x .git/hooks/pre-commit`
3. Implement the feature with tests
4. Run tests: `cargo test`
5. Run linting: `cargo clippy`
6. Format code: `cargo fmt` (or rely on pre-commit hook)
7. Commit changes with descriptive messages
8. Push branch: `git push -u origin feature/feature-name`
9. Create PR: `gh pr create --title "Add feature" --body "Description"`

Always ensure tests pass before creating pull requests.

## Pre-commit Hooks

The repository includes a Git pre-commit hook that automatically:
- Runs `cargo fmt` to format code and stages formatted files
- Runs `cargo clippy` to check for linting issues
- Prevents commits that don't pass formatting or linting checks

The hook is located at `.git/hooks/pre-commit` and should be automatically executable. If not, run:
```bash
chmod +x .git/hooks/pre-commit
```

For advanced setups, a `.pre-commit-config.yaml` is also provided for use with the `pre-commit` framework:
```bash
uv tool install pre-commit
pre-commit install
```