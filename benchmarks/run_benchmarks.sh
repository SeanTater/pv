#!/usr/bin/env bash
# Binary paths are explicit; never accidentally benchmark this pv against itself.
set -euo pipefail
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cargo build --locked --release --manifest-path "$repo_dir/Cargo.toml"
exec python3 "$repo_dir/benchmarks/compare.py" --rust "$repo_dir/target/release/pv" "$@"
