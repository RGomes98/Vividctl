#!/usr/bin/env bash
# Installs the extension for the current user. On Wayland, GNOME Shell only discovers a new
# extension at login: log out and back in once, then enable it.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
uuid="$(sed -n 's/.*"uuid": *"\([^"]*\)".*/\1/p' "$here/metadata.json")"
dest="${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/$uuid"
mkdir -p "$dest"
cp "$here/extension.js" "$here/metadata.json" "$dest/"
echo "installed to $dest"

# `gnome-extensions enable` exits 0 for an extension the running shell has not discovered yet and
# does nothing, so ask the shell what it knows instead of trusting the exit status.
if gnome-extensions list 2>/dev/null | grep -qx "$uuid"; then
    gnome-extensions enable "$uuid"
    if gnome-extensions info "$uuid" 2>/dev/null | grep -qE 'Enabled: Yes|Habilitada: Sim'; then
        echo "enabled"
        exit 0
    fi
fi
echo "the running shell does not know the extension yet: log out and back in, then run:"
echo "  gnome-extensions enable $uuid"
