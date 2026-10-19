"""SP-05: how reliable are EDID identity fields on real monitors?

Reads the public EDID collection https://github.com/linuxhw/EDID (a `Digital/<vendor>/<model>/<dump>`
tree of text files, each starting with an `edid-decode (hex)` block) and prints:

  - how often the numeric serial is zero or a factory default,
  - how often an ASCII serial descriptor (tag 0xFF) is present,
  - the share of monitors with no usable serial of any kind (identical twins would collide),
  - how often one (vendor, product, serial) triple shows different EDID base blocks.

Usage:  python3 -I edid-stats.py /path/to/EDID/Digital

The input is untrusted text; the script only parses hex lines and never executes anything from it.
The collection may have scrubbed ASCII serial descriptors, so the 0xFF figures are a lower or
upper bound, not exact.
"""

import collections
import os
import re
import sys

HEX_LINE = re.compile(r"(?:[0-9a-f]{2} ?){16}")
HEADER = bytes.fromhex("00ffffffffffff00")


def read_base_block(path):
    try:
        with open(path, errors="replace") as f:
            text = f.read(4000)
    except OSError:
        return None
    lines = []
    for line in text.split("\n")[2:]:
        if HEX_LINE.fullmatch(line.strip()):
            lines.append(line.strip())
        elif lines:
            break
    if len(lines) < 8:
        return None
    block = bytes.fromhex(" ".join(lines[:8]))
    return block if block[:8] == HEADER else None


def fields(block):
    mfg = (block[8] << 8) | block[9]
    vendor = "".join(chr(((mfg >> shift) & 0x1F) + 64) for shift in (10, 5, 0))
    product = block[10] | (block[11] << 8)
    serial = block[12] | (block[13] << 8) | (block[14] << 16) | (block[15] << 24)
    ascii_serial = None
    for offset in (54, 72, 90, 108):
        if block[offset:offset + 3] == b"\x00\x00\x00" and block[offset + 3] == 0xFF:
            ascii_serial = block[offset + 5:offset + 18].split(b"\n")[0].decode("ascii", "replace").strip()
    return vendor, product, serial, ascii_serial, block[:127]


def main(root):
    rows = []
    for directory, _, names in os.walk(root):
        for name in names:
            block = read_base_block(os.path.join(directory, name))
            if block:
                rows.append(fields(block))
    n = len(rows)
    if not n:
        sys.exit("no EDIDs found")

    models_per_serial = collections.defaultdict(set)
    for vendor, product, serial, _, _ in rows:
        models_per_serial[serial].add((vendor, product))
    # A numeric serial shared by many unrelated models is a factory default.
    sentinels = {s for s, models in models_per_serial.items() if len(models) >= 50} | {0}

    def ascii_ok(value):
        return bool(value) and len(set(value)) > 1

    numeric_ok = sum(1 for r in rows if r[2] not in sentinels)
    ascii_count = sum(1 for r in rows if ascii_ok(r[3]))
    neither = sum(1 for r in rows if not ascii_ok(r[3]) and r[2] in sentinels)

    print(f"EDIDs parsed: {n}")
    print("factory-default numeric serials (data-driven):", sorted(hex(s) for s in sentinels))
    print(f"usable numeric serial       : {100 * numeric_ok / n:.1f}%")
    print(f"usable ASCII serial (0xFF)  : {100 * ascii_count / n:.1f}%")
    print(f"no usable serial of any kind: {100 * neither / n:.1f}%")

    bodies = collections.defaultdict(set)
    for vendor, product, serial, _, body in rows:
        if serial not in sentinels:
            bodies[(vendor, product, serial)].add(body)
    varying = sum(1 for v in bodies.values() if len(v) > 1)
    print(f"triples with a usable numeric serial: {len(bodies)}")
    print(f"  of those, with more than one different base block: {varying} ({100 * varying / len(bodies):.1f}%)")
    differing = collections.Counter()
    for v in bodies.values():
        if len(v) > 1:
            blocks = list(v)
            for i in range(127):
                if len({b[i] for b in blocks}) > 1:
                    differing[i] += 1
    print("  most common differing byte offsets:", differing.most_common(6))


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "Digital")
