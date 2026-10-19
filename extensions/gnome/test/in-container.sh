#!/usr/bin/env bash
# Runs inside the container as an unprivileged user. Starts a headless GNOME Shell with two virtual
# monitors and the extension, then checks the D-Bus interface, the per-monitor effect (on the pixels
# of screenshots), unredirect handling and a clean disable. Exit status is the number of failures.
set -uo pipefail

export UUID="vividctl@rgomes98.github.io"
export HOME="${HOME:-/tmp/home}"
mkdir -p "$HOME"
export XDG_RUNTIME_DIR=/tmp/runtime-user
mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"
mkdir -p "$HOME/.local/share/gnome-shell/extensions"
cp -r /extension "$HOME/.local/share/gnome-shell/extensions/$UUID"
rm -rf "$HOME/.local/share/gnome-shell/extensions/$UUID/test"

inside() {
    set -uo pipefail
    local fails=0
    ok()   { echo "PASS  $*"; }
    fail() { echo "FAIL  $*"; fails=$((fails + 1)); }

    local iface=org.vividctl.Shell1
    call()  { gdbus call --session --dest org.vividctl.Shell --object-path /org/vividctl/Shell --method "$iface.$1" "${@:2}" 2>&1; }
    prop()  { gdbus call --session --dest org.vividctl.Shell --object-path /org/vividctl/Shell \
                  --method org.freedesktop.DBus.Properties.Get "$iface" "$1" 2>&1; }
    shot()  { gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot \
                  --method org.gnome.Shell.Screenshot.Screenshot false false "/out/$1.png" >/dev/null 2>&1; }
    # A headless shell repaints late (Shell 46 needs seconds): a throwaway capture forces a frame.
    settled() { shot "_warm-$1"; sleep 3; shot "$1"; }
    getsat() { call GetSaturation "$1" | grep -o '[0-9][0-9.]*' | head -1 | awk '{ printf "%g", $1 }'; }
    # gdbus exits 1 on a D-Bus error, which pipefail would turn into a failed grep: capture first.
    says() { local pattern="$1" out; shift; out="$(call "$@")"; grep -q -- "$pattern" <<<"$out"; }

    echo "gnome-shell: $(gnome-shell --version)"
    gsettings set org.gnome.shell disable-user-extensions false
    gsettings set org.gnome.shell enabled-extensions "['$UUID']"
    # A flat, coloured background so a saturation change is measurable.
    gsettings set org.gnome.desktop.background picture-options none
    gsettings set org.gnome.desktop.background primary-color '#4a78b5'
    gsettings set org.gnome.desktop.background color-shading-type solid

    # GNOME Shell 50 connects to the system bus (logind) at startup. There is no systemd here, so a
    # private bus stands in for it: the connection works, nothing answers on it.
    DBUS_SYSTEM_BUS_ADDRESS="$(dbus-daemon --session --fork --print-address)"
    export DBUS_SYSTEM_BUS_ADDRESS

    gnome-shell --headless --wayland --no-x11 --unsafe-mode \
        --virtual-monitor 800x600 --virtual-monitor 640x480 >/out/shell.log 2>&1 &
    shell_pid=$!

    for _ in $(seq 1 60); do
        gdbus introspect --session --dest org.vividctl.Shell --object-path /org/vividctl/Shell >/dev/null 2>&1 && break
        sleep 1
    done

    # --- interface
    prop Version | grep -q "uint32 1" && ok "the extension loads and answers on D-Bus (interface version 1)" || fail "no answer, or the wrong interface version"
    prop ShellVersion | grep -q "'$(gnome-shell --version | grep -o '[0-9][0-9.]*' | head -1)" && ok "ShellVersion matches the running shell" || fail "ShellVersion does not match"
    prop UnredirectDisabled | grep -q "<false>" && ok "starts neutral with unredirect enabled" || fail "does not start neutral"

    # Mutter's connector names for the two virtual monitors, as a client would learn them.
    mapfile -t monitors < <(gdbus call --session --dest org.gnome.Mutter.DisplayConfig --object-path /org/gnome/Mutter/DisplayConfig \
        --method org.gnome.Mutter.DisplayConfig.GetCurrentState | grep -o "(('[^']*'" | sed "s/((//; s/'//g")
    if [ "${#monitors[@]}" -eq 2 ]; then ok "Mutter reports two monitors: ${monitors[*]}"; else fail "expected two monitors, got: ${monitors[*]:-none}"; fi
    left="${monitors[0]:-Meta-0}"; right="${monitors[1]:-Meta-1}"

    # --- set and get
    call SetSaturation "$right" 1.8 | grep -q '^()' && ok "SetSaturation answers" || fail "SetSaturation failed"
    [ "$(getsat "$right")" = "1.8" ] && ok "GetSaturation returns what was set" || fail "GetSaturation returned '$(getsat "$right")'"
    [ "$(getsat "$left")" = "1" ] && ok "another monitor stays neutral" || fail "the other monitor changed to '$(getsat "$left")'"
    prop UnredirectDisabled | grep -q "<true>" && ok "unredirect is disabled while a monitor is boosted" || fail "unredirect not disabled"

    # --- errors
    says "unknown monitor" SetSaturation "no-such-monitor" 1.5 && ok "an unknown monitor is refused" || fail "an unknown monitor was accepted"
    says "unknown monitor" GetSaturation "no-such-monitor" && ok "an unknown monitor cannot be read" || fail "reading an unknown monitor did not fail"
    says "outside" SetSaturation "$left" 9.5 && ok "a saturation out of range is refused" || fail "an out-of-range saturation was accepted"
    says "outside" SetSaturation "$left" -- -0.5 && ok "a negative saturation is refused" || fail "a negative saturation was accepted"
    [ "$(getsat "$left")" = "1" ] && ok "a refused call changes nothing" || fail "a refused call changed the value"

    # --- pixels: only the boosted monitor changes
    call SetSaturation "$right" 1.0 >/dev/null
    settled neutral
    call SetSaturation "$right" 2.0 >/dev/null
    settled right-boosted
    call SetSaturation "$left" 2.0 >/dev/null
    settled both-boosted
    call SetSaturation "$left" 1.0 >/dev/null; call SetSaturation "$right" 1.0 >/dev/null
    settled reset

    python3 - <<'PY' || fails=$((fails + 1))
import os
from PIL import Image

def sat(path, box):
    im = Image.open(path).convert("RGB").crop(box)
    px = list(im.convert("HSV").getdata())
    return sum(p[1] for p in px) / len(px) / 255.0

try:
    w, h = Image.open("/out/neutral.png").size
    # Monitor 0 is 800 px wide at x=0, monitor 1 starts at x=800. Stay 20 px away from the seam.
    L, R = (0, 0, 780, 600), (820, 0, w, 480)
    n_l, n_r = sat("/out/neutral.png", L), sat("/out/neutral.png", R)
    rb_l, rb_r = sat("/out/right-boosted.png", L), sat("/out/right-boosted.png", R)
    bb_l, bb_r = sat("/out/both-boosted.png", L), sat("/out/both-boosted.png", R)
    re_l, re_r = sat("/out/reset.png", L), sat("/out/reset.png", R)
    print(f"saturation left/right  neutral {n_l:.3f}/{n_r:.3f}  right-boosted {rb_l:.3f}/{rb_r:.3f}  "
          f"both {bb_l:.3f}/{bb_r:.3f}  reset {re_l:.3f}/{re_r:.3f}")
    xfail = os.environ.get("REGION_XFAIL") == "1"
    checks = [
        ("only the boosted monitor changes (right boosted, left untouched)", rb_r > n_r * 1.3 and abs(rb_l - n_l) < 0.03),
        ("boosting the other monitor too changes the left as well", bb_l > n_l * 1.3 and bb_r > n_r * 1.3),
        ("resetting both returns to the neutral image", abs(re_l - n_l) < 0.03 and abs(re_r - n_r) < 0.03),
    ]
    bad = 0
    for label, passed in checks:
        if passed:
            print("PASS ", label)
        elif xfail:
            print("XFAIL", label, "(known on headless Shell 46, see README)")
        else:
            print("FAIL ", label)
            bad += 1
    raise SystemExit(1 if bad else 0)
except FileNotFoundError as e:
    print("FAIL  screenshot missing:", e)
    raise SystemExit(1)
PY

    # --- reset is neutral and gives fullscreen windows back
    prop UnredirectDisabled | grep -q "<false>" && ok "unredirect is restored at neutral" || fail "unredirect not restored"

    # --- the real vividctl against the real extension (only when a binary was handed in)
    if [ -x /vividctl ]; then
        vc() { /vividctl "$@" 2>&1; }
        says_vc() { local pattern="$1" out; shift; out="$(vc "$@")"; grep -q -- "$pattern" <<<"$out"; }
        says_vc "gnome: GNOME Shell .* extension, interface 1" list && ok "vividctl list finds the extension" || fail "vividctl list does not show the gnome backend: $(vc list | head -3)"
        for conn in "$left" "$right"; do
            vc list | grep -q "^gnome  *$conn " && ok "vividctl list shows $conn" || fail "vividctl list misses $conn"
        done
        vc set 512 --output "$right" >/dev/null && ok "vividctl set 512 succeeds on $right" || fail "vividctl set failed on $right"
        awk -v v="$(getsat "$right")" 'BEGIN { exit !(v > 1.49 && v < 1.51) }' \
            && ok "the extension holds the mapped saturation (about 1.5)" || fail "saturation after set 512 is '$(getsat "$right")'"
        [ "$(vc get --output "$right" | grep -o -- '-\?[0-9][0-9]*$' | tail -1)" = "512" ] && ok "vividctl get reads 512 back" || fail "vividctl get: $(vc get --output "$right")"
        [ "$(getsat "$left")" = "1" ] && ok "vividctl set --output leaves the other monitor alone" || fail "the other monitor changed"
        vc set -300 --output "$left" >/dev/null && vc get --output "$left" | grep -q -- '-300' && ok "a negative vibrance round-trips" || fail "negative vibrance did not round-trip"
        vc reset >/dev/null && [ "$(getsat "$left")" = "1" ] && [ "$(getsat "$right")" = "1" ] && ok "vividctl reset returns every monitor to neutral" || fail "reset left a monitor boosted"
        vc set 100 --output no-such-monitor >/dev/null; rc=$?
        { [ "$rc" -eq 1 ] || [ "$rc" -eq 2 ]; } && ok "an unknown monitor is a failure (exit $rc)" || fail "unknown monitor gave exit $rc"
    else
        echo "SKIP  no /vividctl binary: the end-to-end check needs VIVIDCTL=path/to/vividctl"
    fi

    # --- disabling the extension releases the bus name and the effect
    if command -v gnome-extensions >/dev/null; then
        call SetSaturation "$right" 1.5 >/dev/null
        gnome-extensions disable "$UUID" >/dev/null 2>&1; sleep 3
        gdbus introspect --session --dest org.vividctl.Shell --object-path /org/vividctl/Shell >/dev/null 2>&1 \
            && fail "the bus name is still owned after disable" || ok "disable releases the bus name"
        gnome-extensions enable "$UUID" >/dev/null 2>&1
        for _ in $(seq 1 20); do
            gdbus introspect --session --dest org.vividctl.Shell --object-path /org/vividctl/Shell >/dev/null 2>&1 && break
            sleep 1
        done
        [ "$(getsat "$right")" = "1" ] && ok "enabling again starts neutral (no state leaks across a disable)" || fail "state leaked across disable"
    else
        echo "SKIP  gnome-extensions is not installed in this image"
    fi

    echo "--- shell log: JS errors"
    grep -i -E "JS ERROR|SyntaxError|ImportError|TypeError|vividctl" /out/shell.log | head -10 || true
    kill "$shell_pid" 2>/dev/null
    echo "failures: $fails"
    exit "$fails"
}

export -f inside
major="$(gnome-shell --version | grep -o '[0-9][0-9]*' | head -1)"
export REGION_XFAIL=$([ "$major" -lt 48 ] && echo 1 || echo 0)
exec dbus-run-session -- bash -c inside
