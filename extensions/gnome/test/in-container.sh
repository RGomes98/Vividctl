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
# Without a vividctl binary the extension is copied by hand. With one, `vividctl extension install`
# puts it in place before the shell starts, which is what a login looks like to it.
if [ ! -x /vividctl ]; then
    mkdir -p "$HOME/.local/share/gnome-shell/extensions"
    cp -r /extension "$HOME/.local/share/gnome-shell/extensions/$UUID"
    rm -rf "$HOME/.local/share/gnome-shell/extensions/$UUID/test"
fi

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
    if [ -x /vividctl ]; then
        # Another extension is already enabled: installing ours must keep it.
        gsettings set org.gnome.shell enabled-extensions "['other@example.org']"
        install_out="$(/vividctl extension install 2>&1)"; install_rc=$?
        [ "$install_rc" -eq 0 ] && grep -q "installed the extension" <<<"$install_out" && ok "vividctl extension install puts the extension in place" || fail "extension install: rc=$install_rc $install_out"
        grep -q "log out and back in once" <<<"$install_out" && ok "the first install says to log in again" || fail "no log-in hint: $install_out"
        enabled_now="$(gsettings get org.gnome.shell enabled-extensions)"
        { grep -q "other@example.org" <<<"$enabled_now" && grep -q "$UUID" <<<"$enabled_now"; } && ok "it enabled the extension and kept the other one" || fail "enabled-extensions is: $enabled_now"
        [ "$(/vividctl extension status >/dev/null 2>&1; echo $?)" = "1" ] && ok "status exits 1 while the extension is not running yet" || fail "status should fail before the shell loads it"
    else
        gsettings set org.gnome.shell enabled-extensions "['$UUID']"
    fi
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

    if [ -x /vividctl ]; then
        # The shell started after the install, so it loaded the extension without any enable call.
        status_out="$(/vividctl extension status 2>&1)"; status_rc=$?
        { [ "$status_rc" -eq 0 ] && grep -q "running: yes" <<<"$status_out"; } && ok "after a fresh start, status says installed, enabled and running" || fail "status: rc=$status_rc $status_out"
        /vividctl extension install 2>&1 | grep -q "nothing more to do" && ok "installing again on a running extension changes nothing" || fail "second install was not a no-op"
    fi

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
    # The boosted monitor is covered by an overlay that clones the shell UI: a click there must still
    # reach the shell underneath, not the overlay.
    pick="$(gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval \
        "const C = imports.gi.Clutter; const a = global.stage.get_actor_at_pos(C.PickMode.REACTIVE, 1120, 240); String(a instanceof C.Clone) + ' ' + a.constructor.name" 2>&1)"
    if ! grep -q "^(true," <<<"$pick"; then fail "could not ask what a click on the boosted monitor reaches: $pick"
    elif grep -q "'true " <<<"$pick"; then fail "a click on the boosted monitor reaches the overlay: $pick"
    else ok "a click on the boosted monitor reaches the shell, not the overlay"; fi

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

    # --- locking the screen keeps the values. The shell disables, on a session mode change, every
    # extension that does not support the new mode; the lock screen is the unlock-dialog mode.
    shell_eval() { gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
                       --method org.gnome.Shell.Eval "$1" 2>&1; }
    owned() { gdbus introspect --session --dest org.vividctl.Shell --object-path /org/vividctl/Shell >/dev/null 2>&1; }
    call SetSaturation "$right" 1.8 >/dev/null
    shell_eval "Main.sessionMode.pushMode('unlock-dialog')" >/dev/null; sleep 2
    shell_eval "Main.sessionMode.currentMode" | grep -q "unlock-dialog" && ok "the shell is in the lock screen mode" || fail "could not enter unlock-dialog: $(shell_eval 'Main.sessionMode.currentMode')"
    owned && ok "the extension keeps running while the screen is locked" || fail "the extension was disabled by the lock screen"
    [ "$(getsat "$right")" = "1.8" ] && ok "the value survives the lock" || fail "the value after the lock is '$(getsat "$right")'"
    shell_eval "Main.sessionMode.popMode('unlock-dialog')" >/dev/null; sleep 2
    [ "$(getsat "$right")" = "1.8" ] && ok "the value is still there after unlocking" || fail "the value after unlocking is '$(getsat "$right")'"
    call SetSaturation "$right" 1.0 >/dev/null

    # --- moving something leaves no trail, on either monitor (#59). There are two stage views;
    # a square crosses from the left monitor to the boosted right one, and after each step the
    # place it left has to look exactly like a capture taken without it.
    call SetSaturation "$right" 1.5 >/dev/null
    shell_eval "const St = imports.gi.St; global.vividSquare = new St.Widget({style: 'background-color: #d93b3b;', width: 160, height: 160}); Main.uiGroup.add_child(global.vividSquare); global.vividSquare.hide(); 'ok'" >/dev/null
    settled trail-ref
    shell_eval "global.vividSquare.show(); 'ok'" >/dev/null
    step=0
    for x in 120 340 560 780 1000 1220; do
        shell_eval "global.vividSquare.set_position($x, 200); 'ok'" >/dev/null
        settled "trail-$step"; step=$((step + 1))
    done
    # --- something partly off the screen must not move the boosted rectangle (#59): the effect's
    # buffer follows the paint box of uiGroup, which then reaches past the stage.
    shell_eval "global.vividSquare.hide(); 'ok'" >/dev/null
    settled paintbox-ref
    shell_eval "const St = imports.gi.St; global.vividOff = new St.Widget({style: 'background-color: #3bd96b;', width: 400, height: 40}); Main.uiGroup.add_child(global.vividOff); global.vividOff.set_position(-300, 540); 'ok'" >/dev/null
    settled paintbox-off
    shell_eval "global.vividSquare.destroy(); global.vividOff.destroy(); delete global.vividSquare; delete global.vividOff; 'ok'" >/dev/null
    call SetSaturation "$right" 1.0 >/dev/null
    python3 - <<'PY' || fails=$((fails + 1))
from PIL import Image, ImageChops
def load(name): return Image.open(f"/out/{name}.png").convert("RGB")
def differing(a, b, box):
    """Share of pixels inside box that differ visibly between two captures."""
    d = ImageChops.difference(a.crop(box), b.crop(box)).convert("L")
    px = list(d.getdata())
    return sum(1 for v in px if v > 12) / max(1, len(px))
bad = 0
def saturation(im, box):
    px = [q for q in im.crop(box).convert("HSV").getdata()]
    return sum(q[1] for q in px) / len(px) / 255.0
# Without this a broken effect would pass: no effect, no trail. The right monitor must be boosted.
right_box = (820, 20, 1420, 460)
neutral_sat = saturation(load("neutral"), right_box)
for name in ("trail-ref", "paintbox-ref", "paintbox-off"):
    s = saturation(load(name), right_box)
    if s < neutral_sat * 1.3:
        print(f"FAIL  the effect is not on the right monitor in {name} ({s:.3f}, neutral {neutral_sat:.3f})")
        bad += 1
ref = load("trail-ref")
xs = [120, 340, 560, 780, 1000, 1220]
worst = 0.0
for i in range(1, len(xs)):
    now = load(f"trail-{i}")
    # The part of the previous square that the current one does not cover, inset by 4 px.
    x0, x1 = xs[i - 1] + 4, min(xs[i - 1] + 160, xs[i]) - 4
    if x1 > x0:
        share = differing(now, ref, (x0, 204, x1, 356))
        worst = max(worst, share)
        if share > 0.01:
            print(f"      step {i}: {share:.1%} of the place the square left still differs (x {x0}..{x1})")
            bad += 1
print(("PASS " if bad == 0 else "FAIL ") + f" moving a square across both monitors leaves no trail (worst {worst:.1%})")
pref, poff = load("paintbox-ref"), load("paintbox-off")
# Everything but the off-screen bar itself: the right monitor and the left one above the bar.
share = max(differing(pref, poff, (820, 20, 1420, 460)), differing(pref, poff, (20, 20, 780, 520)))
print(("PASS " if share <= 0.01 else "FAIL ") + f" an actor partly off the screen does not shift the effect ({share:.1%} differs)")
bad += share > 0.01
raise SystemExit(1 if bad else 0)
PY

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

    if [ -x /vividctl ]; then
        remove_out="$(/vividctl extension remove 2>&1)"
        grep -q "deleted the extension" <<<"$remove_out" && [ ! -e "$HOME/.local/share/gnome-shell/extensions/$UUID" ] && ok "vividctl extension remove deletes the files" || fail "remove: $remove_out"
        after="$(gsettings get org.gnome.shell enabled-extensions)"
        { grep -q "other@example.org" <<<"$after" && ! grep -q "$UUID" <<<"$after"; } && ok "remove disabled it and kept the other extension" || fail "enabled-extensions after remove: $after"
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
