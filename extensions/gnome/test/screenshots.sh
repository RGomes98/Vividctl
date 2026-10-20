#!/usr/bin/env bash
# Takes a neutral and a boosted screenshot of an empty desktop with a wallpaper, in a headless GNOME
# Shell in a container, using the real extension and the real vividctl. For documentation images.
#   VIVIDCTL=path/to/vividctl extensions/gnome/test/screenshots.sh WALLPAPER OUT_DIR [ubuntu-tag]
# Environment: VALUE (default 400), SIZE of the virtual monitor (default 1920x1200).
# Then join the halves with scripts/split-screenshot.py. Software rendering, no GPU, no display.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
wallpaper="${1:?usage: screenshots.sh WALLPAPER OUT_DIR [ubuntu-tag]}"
out="${2:?usage: screenshots.sh WALLPAPER OUT_DIR [ubuntu-tag]}"
tag="${3:-26.04}"
: "${VIVIDCTL:?set VIVIDCTL to a vividctl binary built for Ubuntu 24.04 or newer}"
mkdir -p "$out"
docker build --build-arg "BASE=ubuntu:$tag" -t "vividctl-gnome:$tag" "$here" >/dev/null
# dbus-daemon needs a passwd entry for the uid it runs as.
passwd="$(mktemp)"
trap 'rm -f "$passwd"' EXIT
docker run --rm "vividctl-gnome:$tag" cat /etc/passwd > "$passwd"
echo "tester:x:$(id -u):$(id -g)::/tmp/home:/bin/bash" >> "$passwd"
chmod 644 "$passwd"
docker run --rm \
    --user "$(id -u):$(id -g)" -e HOME=/tmp/home -e VALUE="${VALUE:-400}" -e SIZE="${SIZE:-1920x1200}" \
    -v "$passwd:/etc/passwd:ro" \
    -v "$(realpath "$VIVIDCTL"):/vividctl:ro" \
    -v "$(realpath "$wallpaper"):/wallpaper.png:ro" \
    -v "$here/screenshots-in-container.sh:/test/screenshots-in-container.sh:ro" \
    -v "$(realpath "$out"):/out" \
    "vividctl-gnome:$tag" bash /test/screenshots-in-container.sh
