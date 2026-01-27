#!/bin/sh
set -e

# Create dummy lib sources
for dir in libs/aether-*; do
    [ -d "$dir" ] || continue
    mkdir -p "$dir/src"
    echo "fn main() {}" > "$dir/src/lib.rs"
done

# Create dummy binary sources
for dir in apps/*; do
    [ -d "$dir" ] || continue
    mkdir -p "$dir/src"
    echo "fn main() {}" > "$dir/src/main.rs"
done
