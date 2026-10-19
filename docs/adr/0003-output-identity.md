# ADR-0003: Output identity from the EDID, in Mutter's form

- Status: accepted
- Date: 2026-10-19
- Spike: #6 (notes and data in `docs/spikes/sp-05-output-identity/README.md`)

## Context

A saved profile must follow the monitor. The same output appears in NVKMS (`HDMI-0`), in DRM sysfs (`HDMI-A-1`) and in Mutter (`HDMI-1`), and NVKMS lists 7 connectors where DRM lists 4. nvibrant addresses outputs by their position in the connector list, which breaks as soon as a monitor moves or a second one appears.

Evidence from the spike:

- NVKMS and DRM return byte-identical EDIDs for the same monitor. Mutter keeps only `(vendor, product, serial)`, built from the EDID by a rule that is the same in mutter 46.2 and 50.1. Applying that rule to the NVKMS and DRM EDIDs produces exactly Mutter's tuple.
- On 92,054 real EDIDs, 12 % have no usable serial of any kind, and 6.9 % of triples with a usable numeric serial still show different manufacture dates. Factory-default numeric serials (`0`, `1`, `0x01000e00`, `0x01010101`, `0x5445`) are common.
- The EDID bytes outside the identity fields change between dumps of the same triple (descriptors, dates), so a hash of the whole EDID is not stable.

## Decision

1. **Identity is the monitor spec**: `(vendor, product, serial)`, built from the EDID base block with Mutter's rule.
   - vendor: the 3-letter manufacturer code
   - product: the monitor name descriptor (`0xFC`) when non-empty and valid UTF-8, else `0x%04x` of the product code
   - serial: the serial descriptor (`0xFF`) when non-empty and valid UTF-8, else `0x%08x` of the numeric serial
   - no EDID: `unknown` for all three
   Descriptor text ends at the first newline and has trailing spaces removed. Strings that are not valid UTF-8 count as absent, as in Mutter.
2. **The connector name is not part of the identity.** The three sources name it differently and it changes with the port. It stays as an attribute, used to apply a setting through a backend.
3. **No hash of the full EDID** as identity.
4. **One source of truth for joining**: the three sources are joined on spec equality. NVKMS and DRM give the raw EDID; Mutter gives the spec. For outputs on a GPU NVKMS does not drive, the EDID comes from DRM sysfs.
5. **Collisions get a slot.** Among the currently connected outputs that share a spec, slots `1..n` are assigned in sorted DRM connector-name order. The id is the spec plus the slot (`PHL:PHL 242V8:0x00008fac`, or `DEL:U2720Q:0x01010101#2`). A slot exists only while a collision exists, and an unrelated monitor coming or going does not change anyone's slot.
6. **Serial quality is tracked.** `Usable`, `Sentinel` (numeric serial in the factory-default list) or `Missing` (no EDID). `vividctl doctor` warns when two outputs collide, and says that swapping their cables swaps their profiles.
7. **Outputs without an EDID** get the spec `unknown:unknown:unknown` plus a slot. They cannot be joined across sources, so they are addressed through the connector name of the backend that owns them. This is rare and documented.
8. **Config stores the fields, not a joined string.** A profile entry has `vendor`, `product`, `serial` and an optional `slot`. The `vendor:product:serial#slot` text is only a display form, because product names can contain `:`.

## Consequences

- The id is stable across ports, reboots and GPUs, and it matches the keys GNOME already uses, so the Mutter backend needs no translation.
- Identical monitors without a usable serial (about 1 in 8 models) are told apart only by port. Swapping their cables swaps their settings. This cannot be fixed from the EDID.
- Two different units that share a serial (6.9 % of the usable ones in the sample) are treated as one monitor. Plugging the other unit gets the same profile. Acceptable: they are the same model.
- `vivid-outputs` has to carry, per output, the owning DRM card and driver, the connector name per source, the raw EDID when available, and the serial quality. The id alone does not choose the backend.
- A monitor firmware update that changes the product name changes the id. The profile is not lost silently: it simply stops matching, and `doctor` lists unmatched profiles.

## Not covered by this spike

Only one real monitor was available, so collisions are covered by synthetic EDIDs, not real twins. DisplayPort MST (NVKMS exposes a DP GUID that could help), docks, KVMs that rewrite the EDID, firmware updates and hybrid laptops (#8) were not exercised.

## Alternatives considered

- **Connector name.** Differs per source and changes with the port. It is what makes a profile follow a cable instead of a monitor.
- **Position or index** (nvibrant). Breaks when anything is plugged or unplugged.
- **Hash of the whole EDID.** Not stable across firmware or dumps, and the part that changes is not the part that identifies the monitor.
- **Mutter's full spec including the connector.** Right for GNOME's per-session layout, wrong for a profile that should survive a port change.
- **Adding the manufacture week and year as a discriminator.** Might separate units that share a serial, but the fields also differ for the same triple across dumps, so it could split one monitor into two. Not adopted without data on real units.
