#!/usr/bin/env bash
set -euo pipefail
blockly_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd -- "$blockly_dir/../../../.." && pwd)"
build_dir="${MTGISH_BLOCKLY_BUILD:-$repo_dir/__build/blockly}"
output_file="${1:-$repo_dir/__build/card-blocks.html}"
mkdir -p "$build_dir"
export RUST_MIN_STACK=33554432
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-8}"
# Keep wasm-bindgen's CLI in sync with the exact dependency version in blockly-wasm.
cargo build --locked --manifest-path "$repo_dir/rust_syntax/Cargo.toml" -p mtgish-blockly-wasm --release --target wasm32-unknown-unknown
cargo run --locked --manifest-path "$repo_dir/rust_syntax/Cargo.toml" -p mtgish-ast --bin blockly_catalog -- "$repo_dir/data/mtgish.lines.ron" "$build_dir"
target_dir="$(cargo metadata --no-deps --format-version 1 --manifest-path "$repo_dir/rust_syntax/Cargo.toml" | python3 -c 'import json,sys;print(json.load(sys.stdin)["target_directory"])')"
wasm-bindgen "$target_dir/wasm32-unknown-unknown/release/mtgish_blockly_wasm.wasm" --target no-modules --out-dir "$build_dir/wasm" --out-name validator
python3 "$blockly_dir/pack.py" "$build_dir" "$output_file"
