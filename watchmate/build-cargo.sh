#!/bin/sh
set -e
manifest="$1"
target_dir="$2"
output="$3"

CARGO_HOME="$4" cargo build --manifest-path "$manifest" --target-dir "$target_dir" -p watchmate --release --locked
cp "$target_dir/release/watchmate" "$output"
