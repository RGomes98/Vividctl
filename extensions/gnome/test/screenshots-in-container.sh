#!/usr/bin/env bash
# Runs inside the container as an unprivileged user. Starts a headless GNOME Shell with one virtual
# monitor and a wallpaper, no windows, installs the extension with the real vividctl, and takes the
# desktop twice: neutral and boosted. Writes off.png and on.png to /out.
set -uo pipefail

export HOME="${HOME:-/tmp/home}"
mkdir -p "$HOME"
export XDG_RUNTIME_DIR=/tmp/runtime-user
mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"

inside() {
    set -uo pipefail
    local value="${VALUE:-425}" size="${SIZE:-1920x1200}"
    shot() { gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot \
                 --method org.gnome.Shell.Screenshot.Screenshot false false "/out/$1.png" >/dev/null 2>&1; }
    # A headless shell repaints late: a throwaway capture forces a frame, then give it time.
    # The capture is sometimes blank (a few KB) when the repaint is still pending: take it again.
    settled() {
        local try
        for try in 1 2 3 4; do
            shot "_warm-$1"; sleep 3; shot "$1"; rm -f "/out/_warm-$1.png"
            [ "$(stat -c %s "/out/$1.png" 2>/dev/null || echo 0)" -gt 100000 ] && return 0
            echo "capture $1 was blank, try $try" >&2
        done
        return 1
    }

    gsettings set org.gnome.shell disable-user-extensions false
    gsettings set org.gnome.shell enabled-extensions "[]"
    /vividctl extension install >/dev/null || { echo "extension install failed"; exit 1; }
    gsettings set org.gnome.desktop.background picture-uri 'file:///wallpaper.png'
    gsettings set org.gnome.desktop.background picture-uri-dark 'file:///wallpaper.png'
    gsettings set org.gnome.desktop.background picture-options zoom

    DBUS_SYSTEM_BUS_ADDRESS="$(dbus-daemon --session --fork --print-address)"
    export DBUS_SYSTEM_BUS_ADDRESS
    gnome-shell --headless --wayland --no-x11 --unsafe-mode --virtual-monitor "$size" >/out/shell.log 2>&1 &
    local shell_pid=$!

    for _ in $(seq 1 60); do
        gdbus introspect --session --dest org.vividctl.Shell --object-path /org/vividctl/Shell >/dev/null 2>&1 && break
        sleep 1
    done
    # No windows: leave the overview if the shell started in it.
    gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
        --method org.gnome.Shell.Eval 'Main.overview.hide()' >/dev/null 2>&1 || true
    sleep 2

    /vividctl list
    /vividctl reset >/dev/null && settled off
    /vividctl set "$value" >/dev/null && settled on
    /vividctl get
    /vividctl reset >/dev/null
    kill "$shell_pid" 2>/dev/null
    ls -l /out/off.png /out/on.png
}

export -f inside
exec dbus-run-session -- bash -c inside
