// Vividctl: a saturation shader over each boosted monitor of GNOME Shell, one value per monitor,
// controlled over D-Bus. See README.md for the interface and docs/adr/0002-gnome-shader-backend.md
// for why.
//
// Saturation 1.0 is neutral, above 1 boosts, below 1 washes out, 0 is greyscale. Monitors are
// named by their Mutter connector (`eDP-1`, `HDMI-1`); the client learns them from Mutter.

import Clutter from 'gi://Clutter';
import Cogl from 'gi://Cogl';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Config from 'resource:///org/gnome/shell/misc/config.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const BUS_NAME = 'org.vividctl.Shell';
const OBJECT_PATH = '/org/vividctl/Shell';
const INTERFACE_VERSION = 1;
const EFFECT_NAME = 'vividctl-saturation';

// Saturation values the interface accepts. The command line maps vibrance (-1024..1023) into 0..2.
const MIN_SATURATION = 0.0;
const MAX_SATURATION = 4.0;

const DBUS_XML = `
<node>
  <interface name="org.vividctl.Shell1">
    <method name="SetSaturation">
      <arg type="s" direction="in" name="connector"/>
      <arg type="d" direction="in" name="saturation"/>
    </method>
    <method name="GetSaturation">
      <arg type="s" direction="in" name="connector"/>
      <arg type="d" direction="out" name="saturation"/>
    </method>
    <property name="Version" type="u" access="read"/>
    <property name="ShellVersion" type="s" access="read"/>
    <property name="UnredirectDisabled" type="b" access="read"/>
  </interface>
</node>`;

// Two public mutter APIs change in GNOME 48 (checked in the mutter sources, run on 46, 48, 49 and
// 50 in CI):
//  - ClutterShaderEffect:shader-type takes Cogl.ShaderType instead of Clutter.ShaderType
//  - Meta.disable_unredirect_for_display() became Meta.Compositor.disable_unredirect()
const SHELL_MAJOR = parseInt(Config.PACKAGE_VERSION.split('.')[0], 10);
const FRAGMENT_SHADER = SHELL_MAJOR >= 48 ? Cogl.ShaderType.FRAGMENT : Clutter.ShaderType.FRAGMENT_SHADER;

function setUnredirect(disable) {
    if (typeof Meta.disable_unredirect_for_display === 'function') {
        if (disable)
            Meta.disable_unredirect_for_display(global.display);
        else
            Meta.enable_unredirect_for_display(global.display);
    } else if (disable) {
        global.compositor.disable_unredirect();
    } else {
        global.compositor.enable_unredirect();
    }
}

// gjs turns whole JS numbers into G_TYPE_INT, and ShaderEffect silently ignores an int for a
// float uniform. Nudge integers so they stay doubles.
const asFloat = v => (Number.isInteger(v) ? v + 1e-6 : v);

const SHADER = `
uniform sampler2D tex;
uniform float saturation;
void main() {
    vec4 c = texture2D(tex, cogl_tex_coord_in[0].xy);
    float luma = dot(c.rgb, vec3(0.2126, 0.7152, 0.0722));
    cogl_color_out = vec4(mix(vec3(luma), c.rgb, saturation), c.a);
}`;

const SaturationEffect = GObject.registerClass(
class VividctlSaturationEffect extends Clutter.ShaderEffect {
    _init() {
        super._init({shader_type: FRAGMENT_SHADER});
        this.set_shader_source(SHADER);
        this.set_uniform_value('tex', 0);
    }

    setSaturation(saturation) {
        this.set_uniform_value('saturation', asFloat(saturation));
        this.queue_repaint();
    }

    // An offscreen effect keeps its buffer from paint to paint. Render it on every paint, so a paint
    // never shows a buffer rendered under another redraw clip (#59).
    vfunc_paint(node, paintContext, flags) {
        super.vfunc_paint(node, paintContext, flags | Clutter.EffectPaintFlags.ACTOR_DIRTY);
    }
});

export default class VividctlExtension extends Extension {
    enable() {
        // Saturation by connector. A monitor that is unplugged keeps its value for when it returns.
        this._saturations = new Map();
        // One overlay per boosted monitor, by connector (see _apply).
        this._overlays = new Map();
        this._unredirectDisabled = false;

        this._monitorsChangedId = Main.layoutManager.connect('monitors-changed', () => this._apply());

        this._dbus = Gio.DBusExportedObject.wrapJSObject(DBUS_XML, this);
        this._dbus.export(Gio.DBus.session, OBJECT_PATH);
        this._ownerId = Gio.bus_own_name_on_connection(
            Gio.DBus.session, BUS_NAME, Gio.BusNameOwnerFlags.NONE, null, null);
    }

    disable() {
        Gio.bus_unown_name(this._ownerId);
        this._dbus.unexport();
        this._dbus = null;
        Main.layoutManager.disconnect(this._monitorsChangedId);
        this._removeOverlays();
        this._setUnredirect(false);
        this._saturations = null;
    }

    // The stage rectangle of a connector, or null when Mutter does not know it (right now).
    _regionOf(connector, saturation) {
        const index = global.backend.get_monitor_manager().get_monitor_for_connector(connector);
        const monitor = index >= 0 ? Main.layoutManager.monitors[index] : null;
        if (!monitor)
            return null;
        return {x: monitor.x, y: monitor.y, width: monitor.width, height: monitor.height, saturation};
    }

    // Applies the stored saturations to the monitors that exist now.
    //
    // Each boosted monitor gets an overlay: a clone of the whole shell UI on top of it, clipped to
    // that monitor, with the effect on the clone. One effect on Main.uiGroup covered every monitor at
    // once, and with more than one monitor (one stage view each, on two GPUs on a hybrid laptop) it
    // left trails and dark flicker (#59). An overlay stays inside one view and a neutral monitor gets
    // nothing. The GNOME magnifier applies its colour effects to a clone of uiGroup the same way.
    // The clone is not reactive, so input still reaches the shell underneath.
    _apply() {
        const regions = new Map();
        for (const [connector, saturation] of this._saturations) {
            if (saturation === 1.0)
                continue;
            const region = this._regionOf(connector, saturation);
            if (region)
                regions.set(connector, region);
        }

        for (const [connector, overlay] of this._overlays) {
            if (!regions.has(connector)) {
                overlay.destroy();
                this._overlays.delete(connector);
            }
        }
        for (const [connector, r] of regions) {
            let overlay = this._overlays.get(connector);
            if (!overlay) {
                overlay = new Clutter.Clone({source: Main.uiGroup, reactive: false});
                overlay.add_effect_with_name(EFFECT_NAME, new SaturationEffect());
                global.stage.add_child(overlay);
                this._overlays.set(connector, overlay);
            }
            overlay.set_position(0, 0);
            overlay.set_clip(r.x, r.y, r.width, r.height);
            overlay.get_effect(EFFECT_NAME).setSaturation(r.saturation);
        }

        // Fullscreen windows are normally unredirected (scanned out directly), which would bypass
        // the overlay. Keep them composited while any monitor is not neutral.
        this._setUnredirect(regions.size > 0);
    }

    _removeOverlays() {
        for (const overlay of this._overlays.values())
            overlay.destroy();
        this._overlays.clear();
    }

    _setUnredirect(disable) {
        if (disable === this._unredirectDisabled)
            return;
        setUnredirect(disable);
        this._unredirectDisabled = disable;
    }

    _checkConnector(connector) {
        if (global.backend.get_monitor_manager().get_monitor_for_connector(connector) < 0)
            throw new GLib.Error(Gio.dbus_error_quark(), Gio.DBusError.INVALID_ARGS, `unknown monitor '${connector}'`);
    }

    // D-Bus: SetSaturation(connector: s, saturation: d)
    SetSaturation(connector, saturation) {
        if (!Number.isFinite(saturation) || saturation < MIN_SATURATION || saturation > MAX_SATURATION) {
            throw new GLib.Error(Gio.dbus_error_quark(), Gio.DBusError.INVALID_ARGS,
                `saturation ${saturation} is outside ${MIN_SATURATION}..${MAX_SATURATION}`);
        }
        this._checkConnector(connector);
        this._saturations.set(connector, saturation);
        this._apply();
    }

    // D-Bus: GetSaturation(connector: s) -> d
    GetSaturation(connector) {
        this._checkConnector(connector);
        return this._saturations.get(connector) ?? 1.0;
    }

    // D-Bus properties
    get Version() {
        return INTERFACE_VERSION;
    }

    get ShellVersion() {
        return Config.PACKAGE_VERSION;
    }

    get UnredirectDisabled() {
        return this._unredirectDisabled;
    }
}
