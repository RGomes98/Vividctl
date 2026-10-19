#!/usr/bin/env bash
# Compiles and runs layout.c against the headers of one or more open-gpu-kernel-modules tags.
#
#   tests/abi/run.sh <path-to-open-gpu-kernel-modules-clone> <tag> [<tag> ...]
#
# For each tag it checks the tag out (sparse), compiles the probe and writes `out/<tag>.txt`
# (the probe output) or `out/<tag>.error` (the compiler errors). Exit status is non-zero if any
# tag failed to compile. Needs git and gcc.
set -uo pipefail

repo="${1:?usage: run.sh <clone> <tag> [<tag> ...]}"
shift
here="$(cd "$(dirname "$0")" && pwd)"
out="${ABI_OUT:-$here/out}"
mkdir -p "$out"

dirs=(
    src/nvidia-modeset/interface
    src/common/sdk/nvidia/inc
    src/common/inc
    kernel-open/common/inc
    src/common/unix/common/inc
    src/common/unix/common/utils/interface
    src/common/unix/nvidia-push/interface
    src/common/shared/nvstatus
)
git -C "$repo" sparse-checkout set "${dirs[@]}" >/dev/null 2>&1 || true

failed=0
for tag in "$@"; do
    rm -f "$out/$tag.txt" "$out/$tag.error"
    if ! git -C "$repo" checkout -q "$tag" 2>"$out/$tag.error"; then
        echo "checkout failed: $tag"; failed=1; continue
    fi
    includes=()
    for d in "${dirs[@]}"; do [ -d "$repo/$d" ] && includes+=("-I$repo/$d"); done
    bin="$(mktemp)"
    if gcc -std=gnu11 -w -o "$bin" "${includes[@]}" "$here/layout.c" 2>"$out/$tag.error"; then
        "$bin" >"$out/$tag.txt"
        rm -f "$out/$tag.error"
    else
        failed=1
    fi
    rm -f "$bin"
done
exit "$failed"
