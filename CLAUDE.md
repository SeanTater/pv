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

## Testing

The project includes comprehensive integration tests in the `tests/` directory:

- `integration_tests.rs` - Basic functionality tests (17 tests)
- `format_tests.rs` - Custom format string tests (19 tests) 
- `edge_cases.rs` - Edge cases and error handling (22 tests)

Tests use `assert_cmd` for CLI testing and `tempfile` for file-based tests. All tests verify that data passes through correctly while testing the progress monitoring functionality.

## Development Workflow

When adding new features:

1. Create a feature branch: `git checkout -b feature/feature-name`
2. Implement the feature with tests
3. Run tests: `cargo test`
4. Run linting: `cargo clippy`
5. Commit changes with descriptive messages
6. Push branch: `git push -u origin feature/feature-name`
7. Create PR: `gh pr create --title "Add feature" --body "Description"`

Always ensure tests pass before creating pull requests.