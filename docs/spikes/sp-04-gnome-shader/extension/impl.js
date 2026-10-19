// SP-04 spike: the reloadable part. extension.js imports this file with a cache-busting query
// string so it can be re-imported (see Reload in extension.js) without logging out.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const MAX_MONITORS = 4;
const EFFECT_NAME = 'vivid-saturation';

// One float uniform set per monitor (x, y, w, h in stage pixels, plus saturation).
// ShaderEffect cannot take vec4 or array uniforms from JS, so the source is generated.
function buildShader(count) {
    let decls = '';
    let body = 'float sat = 1.0;\n';
    for (let i = 0; i < count; i++) {
        decls += `uniform float x${i}; uniform float y${i}; uniform float w${i}; uniform float h${i}; uniform float s${i};\n`;
        body += `${i === 0 ? '' : 'else '}if (p.x >= x${i} && p.x < x${i} + w${i} && p.y >= y${i} && p.y < y${i} + h${i}) sat = s${i};\n`;
    }
    return `
uniform sampler2D tex;
uniform float stage_w;
uniform float stage_h;
${decls}
void main() {
    vec4 c = texture2D(tex, cogl_tex_coord_in[0].xy);
    vec2 p = cogl_tex_coord_in[0].xy * vec2(stage_w, stage_h);
    ${body}
    float luma = dot(c.rgb, vec3(0.2126, 0.7152, 0.0722));
    cogl_color_out = vec4(mix(vec3(luma), c.rgb, sat), c.a);
}`;
}

const LOAD_ID = Math.floor(Math.random() * 1e9);
const SaturationEffect = GObject.registerClass({GTypeName: `VividSaturationEffect_${LOAD_ID}`},
class SaturationEffect extends Clutter.ShaderEffect {
    _init(count) {
        super._init({shader_type: Clutter.ShaderType.FRAGMENT_SHADER});
        this._count = count;
        this.set_shader_source(buildShader(count));
    }

    update(monitors, saturations, stageWidth, stageHeight) {
        this.set_uniform_value('tex', 0);
        this.set_uniform_value('stage_w', stageWidth);
        this.set_uniform_value('stage_h', stageHeight);
        for (let i = 0; i < this._count; i++) {
            const m = monitors[i];
            this.set_uniform_value(`x${i}`, m.x);
            this.set_uniform_value(`y${i}`, m.y);
            this.set_uniform_value(`w${i}`, m.width);
            this.set_uniform_value(`h${i}`, m.height);
            this.set_uniform_value(`s${i}`, saturations[i]);
        }
        this.queue_repaint();
    }
});

export class Impl {
    enable() {
        this._saturations = [];
        this._effect = null;
        this._unredirectDisabled = false;
        this._monitorsChangedId = Main.layoutManager.connect('monitors-changed', () => this._rebuild());
        this._rebuild();
    }

    disable() {
        Main.layoutManager.disconnect(this._monitorsChangedId);
        this._removeEffect();
        this._setUnredirect(false);
    }

    keepState() {
        return this._saturations;
    }

    restoreState(saturations) {
        this._saturations = saturations.slice();
        this._rebuild();
    }
    _monitors() {
        return Main.layoutManager.monitors.slice(0, MAX_MONITORS);
    }

    _rebuild() {
        this._removeEffect();
        const monitors = this._monitors();
        while (this._saturations.length < monitors.length)
            this._saturations.push(1.0);
        this._effect = new SaturationEffect(monitors.length);
        Main.uiGroup.add_effect_with_name(EFFECT_NAME, this._effect);
        this._apply();
    }

    _removeEffect() {
        if (this._effect) {
            Main.uiGroup.remove_effect_by_name(EFFECT_NAME);
            this._effect = null;
        }
    }

    _apply() {
        const monitors = this._monitors();
        this._effect.update(monitors, this._saturations, Main.uiGroup.width, Main.uiGroup.height);
        // Fullscreen windows are normally unredirected (scanned out directly), which would
        // bypass the shader. Keep them composited while any monitor is not neutral.
        this._setUnredirect(this._saturations.some(s => s !== 1.0));
    }

    _setUnredirect(disable) {
        if (disable === this._unredirectDisabled)
            return;
        if (disable)
            Meta.disable_unredirect_for_display(global.display);
        else
            Meta.enable_unredirect_for_display(global.display);
        this._unredirectDisabled = disable;
    }

    SetSaturation(monitor, saturation) {
        if (monitor >= this._saturations.length)
            throw new Error(`no monitor ${monitor}`);
        this._saturations[monitor] = saturation;
        this._apply();
    }

    Describe() {
        return JSON.stringify({
            monitors: this._monitors().map((m, i) => ({
                index: i, x: m.x, y: m.y, width: m.width, height: m.height,
                saturation: this._saturations[i],
            })),
            stage: {width: Main.uiGroup.width, height: Main.uiGroup.height},
            unredirectDisabled: this._unredirectDisabled,
        });
    }
}
