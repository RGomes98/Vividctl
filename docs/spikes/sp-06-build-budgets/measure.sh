#!/usr/bin/env bash
# Measures one candidate crate: clean and incremental build time, binary size, dependency count.
#
#   ./measure.sh <crate-dir> [bin-name] [runs]      (runs defaults to 3; the median is reported)
#
# Dependencies are fetched first, so the numbers are compile time only, not download time.
# Every run starts from an empty target directory. Release uses the profile in the crate's
# Cargo.toml (lto, codegen-units=1, panic=abort, strip). Prints one markdown table row.
set -euo pipefail
export LC_ALL=C

dir="${1:?usage: measure.sh <crate-dir> [bin-name] [runs]}"
bin="${2:-$(basename "$dir")}"
runs="${3:-3}"
target="$(mktemp -d)"
trap 'rm -rf "$target"' EXIT
cd "$dir"
export CARGO_TARGET_DIR="$target"
cargo fetch -q

median() { sort -n | awk '{a[NR]=$1} END {print a[int((NR+1)/2)]}'; }
timed() { /usr/bin/time -f "%e" "$@" 2>&1 >/dev/null | tail -1; }

clean_release=(); clean_dev=(); incr_dev=()
for _ in $(seq "$runs"); do
    rm -rf "$target"/*
    clean_release+=("$(timed cargo build -q --release)")
    size="$(stat -c %s "$target/release/$bin")"
done
for _ in $(seq "$runs"); do
    rm -rf "$target"/*
    clean_dev+=("$(timed cargo build -q)")
    touch src/main.rs
    incr_dev+=("$(timed cargo build -q)")
done

crates="$(cargo tree -e normal --prefix none 2>/dev/null | sort -u | wc -l)"
loc="$(wc -l < src/main.rs)"
printf '| %s | %s | %s | %s | %s KiB | %s | %s |\n' \
    "$(basename "$dir")" \
    "$(printf '%s\n' "${clean_release[@]}" | median) s" \
    "$(printf '%s\n' "${clean_dev[@]}" | median) s" \
    "$(printf '%s\n' "${incr_dev[@]}" | median) s" \
    "$((size / 1024))" "$crates" "$loc"
