#!/usr/bin/env bash
# Builds an Ubuntu image with GNOME Shell and runs the extension test in it.
#   extensions/gnome/test/run.sh [ubuntu-tag]    24.04 (Shell 46), 25.04 (48), 25.10 (49), 26.04 (50, default)
# Needs Docker. No GPU and no display: the shell renders in software on two virtual monitors.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
tag="${1:-26.04}"
out="$here/out/$tag"
mkdir -p "$out"
docker build --build-arg "BASE=ubuntu:$tag" -t "vividctl-gnome:$tag" "$here"
# Run as the calling user so the files written to out/ belong to them.
docker run --rm \
    --user "$(id -u):$(id -g)" -e HOME=/tmp/home \
    -v "$here/..:/extension:ro" \
    -v "$here/in-container.sh:/test/in-container.sh:ro" \
    -v "$out:/out" \
    "vividctl-gnome:$tag" bash /test/in-container.sh
