#!/usr/bin/env bash
# Installs the extension for the current user. On Wayland, GNOME Shell only discovers a new
# extension at login: log out and back in once, then it is enabled.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
uuid="$(sed -n 's/.*"uuid": *"\([^"]*\)".*/\1/p' "$here/metadata.json")"
dest="${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/$uuid"
mkdir -p "$dest"
cp "$here/extension.js" "$here/metadata.json" "$dest/"
echo "installed to $dest"
if gnome-extensions enable "$uuid" 2>/dev/null; then
    echo "enabled"
else
    echo "not known to the running shell yet: log out and back in, then run: gnome-extensions enable $uuid"
fi
