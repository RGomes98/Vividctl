// Vividctl: a saturation shader over the whole GNOME Shell stage, one value per monitor, controlled
// over D-Bus. See README.md for the interface and docs/adr/0002-gnome-shader-backend.md for why.
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
// The shader has one set of uniforms per monitor, so it is generated for a fixed count.
const MAX_MONITORS = 8;

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

// ShaderEffect cannot take vec or array uniforms from JS, so the source is generated, with one set
// of scalar uniforms (x, y, w, h, saturation) per monitor.
function buildShader(count) {
    let declarations = '';
    let body = 'float sat = 1.0;\n';
    for (let i = 0; i < count; i++) {
        declarations += `uniform float x${i}; uniform float y${i}; uniform float w${i}; uniform float h${i}; uniform float s${i};\n`;
        body += `${i === 0 ? '' : 'else '}if (p.x >= x${i} && p.x < x${i} + w${i} && p.y >= y${i} && p.y < y${i} + h${i}) sat = s${i};\n`;
    }
    return `
uniform sampler2D tex;
uniform float stage_w;
uniform float stage_h;
${declarations}
void main() {
    vec4 c = texture2D(tex, cogl_tex_coord_in[0].xy);
    vec2 p = cogl_tex_coord_in[0].xy * vec2(stage_w, stage_h);
    ${body}
    float luma = dot(c.rgb, vec3(0.2126, 0.7152, 0.0722));
    cogl_color_out = vec4(mix(vec3(luma), c.rgb, sat), c.a);
}`;
}

const SaturationEffect = GObject.registerClass(
class VividctlSaturationEffect extends Clutter.ShaderEffect {
    _init(count) {
        super._init({shader_type: FRAGMENT_SHADER});
        this._count = count;
        this.set_shader_source(buildShader(count));
    }

    // `regions` is a list of {x, y, width, height, saturation} in stage pixels.
    update(regions, stageWidth, stageHeight) {
        this.set_uniform_value('tex', 0);
        this.set_uniform_value('stage_w', asFloat(stageWidth));
        this.set_uniform_value('stage_h', asFloat(stageHeight));
        for (let i = 0; i < this._count; i++) {
            const r = regions[i];
            // Widen each rectangle by a hair so pixels on a border are never missed.
            this.set_uniform_value(`x${i}`, asFloat(r.x - 0.01));
            this.set_uniform_value(`y${i}`, asFloat(r.y - 0.01));
            this.set_uniform_value(`w${i}`, asFloat(r.width + 0.02));
            this.set_uniform_value(`h${i}`, asFloat(r.height + 0.02));
            this.set_uniform_value(`s${i}`, asFloat(r.saturation));
        }
        this.queue_repaint();
    }

    // Mutter paints one stage view per monitor, each under its own redraw clip, and an offscreen
    // effect keeps one buffer from paint to paint. Reusing it let a paint show a buffer rendered for
    // another view or another clip: trails when windows moved, on every monitor the effect covers
    // (#59). Render it on every paint instead.
    vfunc_paint(node, paintContext, flags) {
        super.vfunc_paint(node, paintContext, flags | Clutter.EffectPaintFlags.ACTOR_DIRTY);
    }
});

export default class VividctlExtension extends Extension {
    enable() {
        // Saturation by connector. A monitor that is unplugged keeps its value for when it returns.
        this._saturations = new Map();
        this._effect = null;
        this._effectCount = -1;
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
        this._removeEffect();
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
    _apply() {
        const regions = [];
        for (const [connector, saturation] of this._saturations) {
            if (saturation === 1.0)
                continue;
            const region = this._regionOf(connector, saturation);
            if (region && regions.length < MAX_MONITORS)
                regions.push(region);
        }

        if (regions.length === 0) {
            this._removeEffect();
            this._setUnredirect(false);
            return;
        }

        // The effect is built for exactly the number of regions, and rebuilt when that changes.
        if (!this._effect || this._effectCount !== regions.length) {
            this._removeEffect();
            this._effect = new SaturationEffect(regions.length);
            this._effectCount = regions.length;
            Main.uiGroup.add_effect_with_name(EFFECT_NAME, this._effect);
        }
        this._effect.update(regions, Main.uiGroup.width, Main.uiGroup.height);
        // Fullscreen windows are normally unredirected (scanned out directly), which would bypass
        // the shader. Keep them composited while any monitor is not neutral.
        this._setUnredirect(true);
    }

    _removeEffect() {
        if (this._effect) {
            Main.uiGroup.remove_effect_by_name(EFFECT_NAME);
            this._effect = null;
            this._effectCount = -1;
        }
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
