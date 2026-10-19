#!/usr/bin/env bash
# Builds an Ubuntu image with GNOME Shell and runs the headless extension test in it.
#   ./run.sh [ubuntu-tag]     24.04 (Shell 46), 25.04 (48), 25.10 (49), 26.04 (50, default)
#   EXT_DIR=path ./run.sh ...  tests another copy of the extension (used for negative controls)
# Needs Docker. No GPU, no display: the shell renders in software on a virtual monitor.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
tag="${1:-26.04}"
out="$here/out/$tag"
mkdir -p "$out"
docker build --build-arg "BASE=ubuntu:$tag" -t "vivid-gnome:$tag" "$here"
# Run as the calling user so the files written to out/ belong to them.
docker run --rm \
    --user "$(id -u):$(id -g)" -e HOME=/tmp/home \
    -v "${EXT_DIR:-$here/../extension}:/spike/extension:ro" \
    -v "$here/in-container.sh:/spike/in-container.sh:ro" \
    -v "$out:/out" \
    "vivid-gnome:$tag" bash /spike/in-container.sh
