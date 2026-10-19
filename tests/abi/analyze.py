"""Groups driver tags into ABI generations from the output of run.sh.

  python3 -I tests/abi/analyze.py <dir-with-<tag>.txt-files> [--json | --rust]

Two tags are in the same generation when the probe printed exactly the same keys and values for
both. Tags are ordered by version, so a generation is a run of consecutive tags. For every key it
also lists the distinct values and where they change.

`--json` prints the generations as data (tests/abi/generations.json). `--rust` prints the Rust
source of the layout table used by vivid-nvkms (crates/vivid-nvkms/src/generations.rs); pipe it
through `cargo +nightly fmt` after writing it.
"""

import collections
import glob
import json
import os
import sys


def version_key(tag):
    return tuple(int(part) for part in tag.split("."))


def load(directory):
    layouts = {}
    for path in glob.glob(os.path.join(directory, "*.txt")):
        tag = os.path.basename(path)[:-4]
        values = {}
        with open(path) as f:
            for line in f:
                key, _, value = line.rstrip("\n").partition("=")
                values[key] = value
        layouts[tag] = values
    return layouts


# (Rust field, rust type, probe key, what it is). The Rust struct and the table are generated from
# this list, so a field cannot exist in one and not in the other.
LAYOUT_FIELDS = [
    ("cmd_alloc_device", "u32", "const.NVKMS_IOCTL_ALLOC_DEVICE", "Command number of `ALLOC_DEVICE`."),
    ("cmd_free_device", "u32", "const.NVKMS_IOCTL_FREE_DEVICE", "Command number of `FREE_DEVICE`."),
    ("cmd_query_disp", "u32", "const.NVKMS_IOCTL_QUERY_DISP", "Command number of `QUERY_DISP`."),
    ("cmd_query_connector_static_data", "u32", "const.NVKMS_IOCTL_QUERY_CONNECTOR_STATIC_DATA", "Command number of `QUERY_CONNECTOR_STATIC_DATA`."),
    ("cmd_query_dpy_dynamic_data", "u32", "const.NVKMS_IOCTL_QUERY_DPY_DYNAMIC_DATA", "Command number of `QUERY_DPY_DYNAMIC_DATA`."),
    ("cmd_set_dpy_attribute", "u32", "const.NVKMS_IOCTL_SET_DPY_ATTRIBUTE", "Command number of `SET_DPY_ATTRIBUTE`."),
    ("cmd_get_dpy_attribute", "u32", "const.NVKMS_IOCTL_GET_DPY_ATTRIBUTE", "Command number of `GET_DPY_ATTRIBUTE`."),
    ("attribute_digital_vibrance", "u32", "const.NV_KMS_DPY_ATTRIBUTE_DIGITAL_VIBRANCE", "Attribute number of Digital Vibrance."),
    ("version_string_len", "usize", "const.NVKMS_NVIDIA_DRIVER_VERSION_STRING_LENGTH", "Size of the version string field, including the terminator."),
    ("max_subdevices", "usize", "const.NVKMS_MAX_SUBDEVICES", "Capacity of the disp handle array."),
    ("max_connectors_per_disp", "usize", "const.NVKMS_MAX_CONNECTORS_PER_DISP", "Capacity of the connector handle array."),
    ("dpy_name_size", "usize", "const.NVKMS_DPY_NAME_SIZE", "Size of the display name field."),
    ("edid_buffer_size", "usize", "const.NVKMS_EDID_BUFFER_SIZE", "Size of the EDID buffer."),
    ("status_success", "u32", "const.NVKMS_ALLOC_DEVICE_STATUS_SUCCESS", "`ALLOC_DEVICE` reply status: success."),
    ("status_version_mismatch", "u32", "const.NVKMS_ALLOC_DEVICE_STATUS_VERSION_MISMATCH", "`ALLOC_DEVICE` reply status: the version string did not match the driver."),
    ("status_no_hardware_available", "u32", "const.NVKMS_ALLOC_DEVICE_STATUS_NO_HARDWARE_AVAILABLE", "`ALLOC_DEVICE` reply status: no such GPU."),
    ("connector_type_dp", "u32", "const.NVKMS_CONNECTOR_TYPE_DP", "Connector type: `DisplayPort`."),
    ("connector_type_vga", "u32", "const.NVKMS_CONNECTOR_TYPE_VGA", "Connector type: VGA."),
    ("connector_type_dvi_i", "u32", "const.NVKMS_CONNECTOR_TYPE_DVI_I", "Connector type: DVI-I."),
    ("connector_type_dvi_d", "u32", "const.NVKMS_CONNECTOR_TYPE_DVI_D", "Connector type: DVI-D."),
    ("connector_type_adc", "u32", "const.NVKMS_CONNECTOR_TYPE_ADC", "Connector type: ADC."),
    ("connector_type_lvds", "u32", "const.NVKMS_CONNECTOR_TYPE_LVDS", "Connector type: LVDS."),
    ("connector_type_hdmi", "u32", "const.NVKMS_CONNECTOR_TYPE_HDMI", "Connector type: HDMI."),
    ("alloc_size", "usize", "sizeof.struct NvKmsAllocDeviceParams", "Size of the `ALLOC_DEVICE` block."),
    ("alloc_reply_offset", "usize", "offsetof.struct NvKmsAllocDeviceParams.reply", "Where the reply starts in the `ALLOC_DEVICE` block."),
    ("alloc_version_string_offset", "usize", "offsetof.ALLOC_REQ.versionString", "Offset of the version string in the request."),
    ("alloc_device_id_offset", "usize", "offsetof.ALLOC_REQ.deviceId", "Offset of the device id (the GPU index) in the request."),
    ("alloc_no3d_offset", "usize", "offsetof.ALLOC_REQ.no3d", "Offset of the `no3d` flag in the request."),
    ("alloc_status_offset", "usize", "offsetof.ALLOC_REP.status", "Offset of the status in the reply."),
    ("alloc_device_handle_offset", "usize", "offsetof.ALLOC_REP.deviceHandle", "Offset of the device handle in the reply."),
    ("alloc_num_disps_offset", "usize", "offsetof.ALLOC_REP.numDisps", "Offset of the disp count in the reply."),
    ("alloc_disp_handles_offset", "usize", "offsetof.ALLOC_REP.dispHandles", "Offset of the disp handle array in the reply."),
    ("free_size", "usize", "sizeof.struct NvKmsFreeDeviceParams", "Size of the `FREE_DEVICE` block."),
    ("free_device_handle_offset", "usize", "offsetof.struct NvKmsFreeDeviceRequest.deviceHandle", "Offset of the device handle in the `FREE_DEVICE` request."),
    ("query_disp_size", "usize", "sizeof.struct NvKmsQueryDispParams", "Size of the `QUERY_DISP` block."),
    ("query_disp_device_handle_offset", "usize", "offsetof.struct NvKmsQueryDispRequest.deviceHandle", "Offset of the device handle in the `QUERY_DISP` request."),
    ("query_disp_disp_handle_offset", "usize", "offsetof.struct NvKmsQueryDispRequest.dispHandle", "Offset of the disp handle in the `QUERY_DISP` request."),
    ("query_disp_reply_offset", "usize", "offsetof.struct NvKmsQueryDispParams.reply", "Where the reply starts in the `QUERY_DISP` block."),
    ("disp_num_connectors_offset", "usize", "offsetof.DISP_REP.numConnectors", "Offset of the connector count in the `QUERY_DISP` reply."),
    ("disp_connector_handles_offset", "usize", "offsetof.DISP_REP.connectorHandles", "Offset of the connector handle array in the `QUERY_DISP` reply."),
    ("connector_size", "usize", "sizeof.struct NvKmsQueryConnectorStaticDataParams", "Size of the `QUERY_CONNECTOR_STATIC_DATA` block."),
    ("connector_device_handle_offset", "usize", "offsetof.struct NvKmsQueryConnectorStaticDataRequest.deviceHandle", "Offset of the device handle in the request."),
    ("connector_disp_handle_offset", "usize", "offsetof.struct NvKmsQueryConnectorStaticDataRequest.dispHandle", "Offset of the disp handle in the request."),
    ("connector_connector_handle_offset", "usize", "offsetof.struct NvKmsQueryConnectorStaticDataRequest.connectorHandle", "Offset of the connector handle in the request."),
    ("connector_reply_offset", "usize", "offsetof.struct NvKmsQueryConnectorStaticDataParams.reply", "Where the reply starts in the block."),
    ("connector_dpy_id_offset", "usize", "offsetof.CONN_REP.dpyId", "Offset of the display id in the reply."),
    ("connector_type_offset", "usize", "offsetof.CONN_REP.type", "Offset of the connector type in the reply."),
    ("connector_type_index_offset", "usize", "offsetof.CONN_REP.typeIndex", "Offset of the connector type index in the reply."),
    ("dynamic_size", "usize", "sizeof.struct NvKmsQueryDpyDynamicDataParams", "Size of the `QUERY_DPY_DYNAMIC_DATA` block."),
    ("dynamic_device_handle_offset", "usize", "offsetof.DYN_REQ.deviceHandle", "Offset of the device handle in the request."),
    ("dynamic_disp_handle_offset", "usize", "offsetof.DYN_REQ.dispHandle", "Offset of the disp handle in the request."),
    ("dynamic_dpy_id_offset", "usize", "offsetof.DYN_REQ.dpyId", "Offset of the display id in the request."),
    ("dynamic_reply_offset", "usize", "offsetof.struct NvKmsQueryDpyDynamicDataParams.reply", "Where the reply starts in the block."),
    ("dynamic_connected_offset", "usize", "offsetof.DYN_REP.connected", "Offset of the connected flag in the reply."),
    ("dynamic_edid_size_offset", "usize", "offsetof.DYN_REP.edid.bufferSize", "Offset of the EDID size in the reply."),
    ("dynamic_edid_valid_offset", "usize", "offsetof.DYN_REP.edid.valid", "Offset of the EDID validity flag in the reply."),
    ("dynamic_edid_buffer_offset", "usize", "offsetof.DYN_REP.edid.buffer", "Offset of the EDID bytes in the reply."),
    ("set_size", "usize", "sizeof.struct NvKmsSetDpyAttributeParams", "Size of the `SET_DPY_ATTRIBUTE` block."),
    ("set_device_handle_offset", "usize", "offsetof.struct NvKmsSetDpyAttributeRequest.deviceHandle", "Offset of the device handle in the request."),
    ("set_disp_handle_offset", "usize", "offsetof.struct NvKmsSetDpyAttributeRequest.dispHandle", "Offset of the disp handle in the request."),
    ("set_dpy_id_offset", "usize", "offsetof.struct NvKmsSetDpyAttributeRequest.dpyId", "Offset of the display id in the request."),
    ("set_attribute_offset", "usize", "offsetof.struct NvKmsSetDpyAttributeRequest.attribute", "Offset of the attribute in the request."),
    ("set_value_offset", "usize", "offsetof.struct NvKmsSetDpyAttributeRequest.value", "Offset of the value in the request."),
    ("get_size", "usize", "sizeof.struct NvKmsGetDpyAttributeParams", "Size of the `GET_DPY_ATTRIBUTE` block."),
    ("get_device_handle_offset", "usize", "offsetof.struct NvKmsGetDpyAttributeRequest.deviceHandle", "Offset of the device handle in the request."),
    ("get_disp_handle_offset", "usize", "offsetof.struct NvKmsGetDpyAttributeRequest.dispHandle", "Offset of the disp handle in the request."),
    ("get_dpy_id_offset", "usize", "offsetof.struct NvKmsGetDpyAttributeRequest.dpyId", "Offset of the display id in the request."),
    ("get_attribute_offset", "usize", "offsetof.struct NvKmsGetDpyAttributeRequest.attribute", "Offset of the attribute in the request."),
    ("get_reply_offset", "usize", "offsetof.struct NvKmsGetDpyAttributeParams.reply", "Where the reply starts in the block."),
    ("get_value_offset", "usize", "offsetof.struct NvKmsGetDpyAttributeReply.value", "Offset of the value in the reply."),
]


def rust_version(tag):
    parts = [int(p) for p in tag.split(".")]
    parts += [0] * (3 - len(parts))
    return "Version::new({}, {}, {})".format(*parts)


def emit_rust(generations):
    out = [
        "//! The NVKMS layouts of every known driver generation.",
        "//!",
        "//! GENERATED by `tests/abi/analyze.py --rust` from `tests/abi/generations.json`. Do not edit by",
        "//! hand: change the probe or the data and regenerate (see `tests/abi/README.md`).",
        "",
        "use crate::Version;",
        "",
        "/// The sizes, offsets and constants one driver generation uses for the NVKMS ioctls.",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        "pub struct Layout {",
    ]
    for name, rust_type, _, doc in LAYOUT_FIELDS:
        out += [f"    /// {doc}", f"    pub {name}: {rust_type},"]
    out += [
        "}",
        "",
        "/// A run of driver versions whose NVKMS layout is identical.",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        "pub struct Generation {",
        "    /// Position in the table, starting at 1.",
        "    pub id: u32,",
        "    /// The first driver version known to have this layout.",
        "    pub first: Version,",
        "    /// The last driver version known to have this layout.",
        "    pub last: Version,",
        "    /// The layout itself.",
        "    pub layout: Layout,",
        "}",
        "",
        "/// Every known generation, oldest first.",
        "pub const GENERATIONS: &[Generation] = &[",
    ]
    for gen in generations:
        out += ["    Generation {", f"        id: {gen['id']},", f"        first: {rust_version(gen['first'])},", f"        last: {rust_version(gen['last'])},", "        layout: Layout {"]
        for name, _, key, _ in LAYOUT_FIELDS:
            if key not in gen["values"]:
                sys.exit(f"probe output has no key {key!r}")
            out.append(f"            {name}: {gen['values'][key]},")
        out += ["        },", "    },"]
    out += ["];", ""]
    return "\n".join(out)


def main(directory, as_json, as_rust=False):
    layouts = load(directory)
    tags = sorted(layouts, key=version_key)
    if not tags:
        sys.exit("no probe output found")

    generations = []
    for tag in tags:
        if generations and layouts[generations[-1]["tags"][-1]] == layouts[tag]:
            generations[-1]["tags"].append(tag)
        else:
            generations.append({"tags": [tag]})

    keys = sorted({k for layout in layouts.values() for k in layout})
    for number, gen in enumerate(generations, start=1):
        gen["id"] = number
        gen["first"], gen["last"] = gen["tags"][0], gen["tags"][-1]
        gen["values"] = layouts[gen["first"]]
        if number > 1:
            before = generations[number - 2]["values"]
            gen["changed"] = {k: [before.get(k), gen["values"].get(k)] for k in keys if before.get(k) != gen["values"].get(k)}
        else:
            gen["changed"] = {}

    if as_rust:
        print(emit_rust(generations), end="")
        return

    if as_json:
        document = [
            {"id": g["id"], "first": g["first"], "last": g["last"], "tags": len(g["tags"]), "values": g["values"]}
            for g in generations
        ]
        print(json.dumps(document, indent=2, sort_keys=True))
        return

    print(f"{len(tags)} tags, {len(generations)} distinct layouts (consecutive runs)\n")
    print(f"{'#':>2}  {'first':<11} {'last':<11} {'tags':>4}  what changed from the previous run")
    for gen in generations:
        changed = ", ".join(f"{k}: {a}->{b}" for k, (a, b) in gen["changed"].items())
        print(f"{gen['id']:>2}  {gen['first']:<11} {gen['last']:<11} {len(gen['tags']):>4}  {changed[:200] if gen['id'] > 1 else '(first)'}")

    print("\nKeys that never change:")
    constant = [k for k in keys if len({layouts[t].get(k) for t in tags}) == 1]
    print("  " + ", ".join(constant))

    print("\nKeys that change, with their distinct values:")
    for k in keys:
        values = collections.OrderedDict()
        for t in tags:
            values.setdefault(layouts[t].get(k), []).append(t)
        if len(values) > 1:
            print(f"  {k}")
            for value, group in values.items():
                print(f"      {value:>8}  {group[0]} .. {group[-1]}  ({len(group)} tags)")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    main(args[0] if args else "out", "--json" in sys.argv, "--rust" in sys.argv)
