// SP-04 throwaway proof of concept: a saturation shader over the whole GNOME Shell stage,
// with one saturation value per monitor, controlled over D-Bus.
//
// This file is a small fixed loader. The real code is in impl.js. gjs caches ES modules by URI
// and ignores query strings, so each (re)load copies impl.js to a uniquely named file in
// $XDG_RUNTIME_DIR and imports that copy. `Reload` then picks up edits without logging out.
//
//   gdbus call --session --dest org.vividctl.Spike --object-path /org/vividctl/Spike \
//     --method org.vividctl.Spike.SetSaturation 0 1.8
//
// Saturation 1.0 is neutral. Values above 1 boost, below 1 wash out, 0 is greyscale.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const DBUS_XML = `
<node>
  <interface name="org.vividctl.Spike">
    <method name="SetSaturation">
      <arg type="u" direction="in" name="monitor"/>
      <arg type="d" direction="in" name="saturation"/>
    </method>
    <method name="Describe">
      <arg type="s" direction="out" name="json"/>
    </method>
    <method name="Reload"/>
  </interface>
</node>`;

export default class VividSpikeLoader extends Extension {
    enable() {
        this._impl = null;
        this._generation = 0;
        this._copies = [];
        this._dbus = Gio.DBusExportedObject.wrapJSObject(DBUS_XML, this);
        this._dbus.export(Gio.DBus.session, '/org/vividctl/Spike');
        this._ownerId = Gio.bus_own_name_on_connection(
            Gio.DBus.session, 'org.vividctl.Spike', Gio.BusNameOwnerFlags.NONE, null, null);
        this._load(null).catch(e => logError(e, 'vivid-spike: load failed'));
    }

    disable() {
        Gio.bus_unown_name(this._ownerId);
        this._dbus.unexport();
        this._dbus = null;
        this._unload();
        for (const copy of this._copies) {
            try {
                copy.delete(null);
            } catch (e) {
                // already gone
            }
        }
        this._copies = [];
    }

    async _load(state) {
        const dir = GLib.build_filenamev([GLib.get_user_runtime_dir(), 'vivid-spike']);
        GLib.mkdir_with_parents(dir, 0o700);
        const copy = Gio.File.new_for_path(
            GLib.build_filenamev([dir, `impl-${++this._generation}-${GLib.get_monotonic_time()}.js`]));
        this.dir.get_child('impl.js').copy(copy, Gio.FileCopyFlags.OVERWRITE, null, null);
        this._copies.push(copy);
        const module = await import(copy.get_uri());
        this._impl = new module.Impl();
        this._impl.enable();
        if (state)
            this._impl.restoreState(state);
    }

    _unload() {
        if (!this._impl)
            return null;
        const state = this._impl.keepState();
        this._impl.disable();
        this._impl = null;
        return state;
    }

    // D-Bus methods, forwarded to the current implementation.
    SetSaturation(monitor, saturation) {
        this._impl.SetSaturation(monitor, saturation);
    }

    Describe() {
        return this._impl.Describe();
    }

    Reload() {
        const state = this._unload();
        this._load(state).catch(e => logError(e, 'vivid-spike: reload failed'));
    }
}
