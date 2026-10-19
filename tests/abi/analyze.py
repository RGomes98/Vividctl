"""Groups driver tags into ABI generations from the output of run.sh.

  python3 -I tests/abi/analyze.py <dir-with-<tag>.txt-files> [--json]

Two tags are in the same generation when the probe printed exactly the same keys and values for
both. Tags are ordered by version, so a generation is a run of consecutive tags. For every key it
also lists the distinct values and where they change.
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


def main(directory, as_json):
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
    main(args[0] if args else "out", "--json" in sys.argv)
