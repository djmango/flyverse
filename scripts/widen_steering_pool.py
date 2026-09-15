#!/usr/bin/env python3
"""Re-resolve the `motor_flight_steering_*` groups in the flyverse neural-I/O
artifact from the MaleCNS annotation table.

Why this exists
---------------
The shipped `assets/male_cns_v1_neural_io.json` defines the two steering motor
pools from the b1/b2/b3 motor neurons only -- 3 neurons per side. That is not
the spec's steering set: `docs/physical-model-spec.md` §2 names the steering
muscles **tp1, tp2, hg1-hg4 and b1-b3** (9 per side), and the same annotation
table the power pools are resolved from holds all of them as `superclass ==
vnc_motor and subclass == wm` motor neurons. The upstream generator
(`flyBrain/tools/build_male_cns_io.py`) already resolves the power pools this
way; only its `wing_steering_types` set is narrow.

This script re-resolves the two steering groups with the widened type set, using
the same selector form and the same annotation fields as the power pools, and
rewrites them **in place** in the artifact. Every other group, the group
ordering, and the file's `json.dumps(indent=2, sort_keys=True)` formatting are
preserved -- the only diff is the two steering entries and the derived totals
they feed. It is deterministic: re-running it on the same annotation table
produces the same sorted root IDs.

    python3 scripts/widen_steering_pool.py \
        --annotations /path/to/annotations.feather \
        --pack /path/to/official-pack/neuron_ids.npy \
        --io assets/male_cns_v1_neural_io.json

Everything it writes is measured from the annotation table; nothing is assumed.
A selected body ID that is not in the compiled pack is recorded under
`pack_resolution.missing_root_ids` rather than silently dropped.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
import pyarrow.feather as feather

# The spec's steering muscle set, docs/physical-model-spec.md §2:
#   "Steering muscles (tp1, tp2, hg1-hg4, b1-b3) act as cycle-by-cycle
#    modulators of stroke amplitude, stroke-plane tilt, and flip timing"
# These are the `type` values the MaleCNS annotation table uses for them.
STEERING_TYPES = [
    "b1 MN",
    "b2 MN",
    "b3 MN",
    "hg1 MN",
    "hg2 MN",
    "hg3 MN",
    "hg4 MN",
    "tp1 MN",
    "tp2 MN",
]


def selector(side: str) -> str:
    types = ",".join(STEERING_TYPES)
    return (
        "status == Traced and superclass == vnc_motor and subclass == wm "
        f"and type in {{{types}}} and somaSide == {side}"
    )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--annotations", required=True)
    ap.add_argument("--pack", required=True)
    ap.add_argument("--io", required=True)
    ap.add_argument("--out", default=None, help="defaults to --io (in place)")
    args = ap.parse_args()

    pack_ids = set(int(x) for x in np.load(args.pack))
    table = feather.read_table(
        args.annotations,
        columns=["bodyId", "status", "superclass", "subclass", "type", "somaSide"],
    )
    rows = [
        r
        for r in table.to_pylist()
        if r.get("status") == "Traced"
        and r.get("superclass") == "vnc_motor"
        and r.get("subclass") == "wm"
        and r.get("type") in STEERING_TYPES
    ]

    io_path = Path(args.io)
    doc = json.loads(io_path.read_text())
    groups = doc["groups"]

    for side, side_name in (("L", "left"), ("R", "right")):
        name = f"motor_flight_steering_{side_name}"
        selected = sorted(int(r["bodyId"]) for r in rows if r.get("somaSide") == side)
        present = [i for i in selected if i in pack_ids]
        missing = [i for i in selected if i not in pack_ids]
        if not selected:
            raise ValueError(f"{name}: annotation selection produced no rows")
        if len(set(selected)) != len(selected):
            raise ValueError(f"{name}: duplicate body IDs in selection")
        g = groups[name]
        g["root_ids"] = present
        g["selector"] = selector(side)
        g["pack_resolution"] = {
            "selected_count": len(selected),
            "present_count": len(present),
            "missing_root_ids": missing,
        }
        g["biological_scope"] = (
            "spec steering muscle set tp1/tp2/hg1-hg4/b1-b3 wing motor neurons "
            "(9 per side); docs/physical-model-spec.md §2"
        )
        g["evidence_category"] = "published_flight_wing_motor"
        print(f"{name}: {len(present)} present / {len(selected)} selected (missing {missing})")
        print(f"  selector: {g['selector']}")

    # Rebuild the derived totals from the groups map so they stay consistent.
    res = {
        name: {
            "missing_root_ids": g["pack_resolution"]["missing_root_ids"],
            "present_count": g["pack_resolution"]["present_count"],
            "selected_count": g["pack_resolution"]["selected_count"],
        }
        for name, g in groups.items()
    }
    doc["pack_resolution"]["groups"] = res
    cat: dict[str, dict[str, int]] = {}
    for g in groups.values():
        c = cat.setdefault(
            g["evidence_category"],
            {"groups": 0, "missing_root_ids": 0, "present_root_ids": 0, "selected_root_ids": 0},
        )
        c["groups"] += 1
        c["missing_root_ids"] += len(g["pack_resolution"]["missing_root_ids"])
        c["present_root_ids"] += g["pack_resolution"]["present_count"]
        c["selected_root_ids"] += g["pack_resolution"]["selected_count"]
    doc["summary"]["category_counts"] = cat
    doc["summary"]["group_count"] = len(groups)
    doc["summary"]["missing_root_ids"] = sum(
        len(v["missing_root_ids"]) for v in res.values()
    )
    doc["summary"]["present_root_ids"] = sum(v["present_count"] for v in res.values())
    doc["summary"]["selected_root_ids"] = sum(v["selected_count"] for v in res.values())

    out_path = Path(args.out) if args.out else io_path
    out_path.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(
        f"wrote {out_path}: {doc['summary']['selected_root_ids']} selected root ids "
        f"over {doc['summary']['group_count']} groups"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())