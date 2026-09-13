"""Third pass: describe the candidate slice before exporting it.

Candidate: every neuron that projects directly onto a leg motor neuron or a
walking/landing descending neuron, plus those targets themselves. Report its
composition so the lesson can state exactly what is in it.
"""
from __future__ import annotations

import json
from collections import Counter
from pathlib import Path

import numpy as np
import pyarrow.feather as feather

ROOT = Path("/opt/data/workspaces/skg/flybrain")
PACK = ROOT / "official-pack"

row_ptr = np.load(PACK / "row_ptr.npy")
dest = np.load(PACK / "destinations.npy")
ids = np.load(PACK / "neuron_ids.npy")
n = len(ids)
index_of = {int(b): i for i, b in enumerate(ids)}

t = feather.read_table(ROOT / "annotations.feather")
sup_col = t.column("superclass").cast("string").to_pylist()
type_col = t.column("type").cast("string").to_pylist()
body_col = t.column("bodyId").cast("int64").to_pylist()
row_of_body = {b: i for i, b in enumerate(body_col)}

io = json.loads((ROOT / "flyverse/assets/male_cns_v1_neural_io.json").read_text())


def idx_of(body_ids):
    return np.array(sorted(index_of[b] for b in body_ids if b in index_of), dtype=np.int64)


seeds = idx_of(
    io["groups"]["motor_walking_left"]["root_ids"] + io["groups"]["motor_walking_right"]["root_ids"]
)
print(f"seed (leg motor + walking/landing descending): {len(seeds)}")

order = np.argsort(dest, kind="stable")
rev_src = np.repeat(np.arange(n, dtype=np.int64), np.diff(row_ptr))[order]
bounds = np.searchsorted(dest[order], np.arange(n + 1))
pre = np.unique(np.concatenate([rev_src[bounds[s] : bounds[s + 1]] for s in seeds]))
print(f"direct presynaptic partners: {len(pre):,}")

keep = np.zeros(n, dtype=bool)
keep[pre] = True
keep[seeds] = True
print(f"slice: {int(keep.sum()):,} neurons")

src_in = np.repeat(keep, np.diff(row_ptr))
in_slice = src_in & keep[dest]
m = int(in_slice.sum())
print(
    f"edges inside the slice: {m:,}  "
    f"(excitatory {int(np.count_nonzero(in_slice & (np.load(PACK / 'signed_counts.npy') > 0))):,})"
)
print(f"asset bytes (u32 dst + i16 count): {int(keep.sum()) * 6 + m * 6:,}")

local = {int(g): i for i, g in enumerate(np.flatnonzero(keep))}
comp = Counter()
for g in np.flatnonzero(keep):
    body = int(ids[g])
    r = row_of_body.get(body)
    comp[sup_col[r] or "(none)" if r is not None else "(no annotation)"] += 1
print("\ncomposition of the slice:")
for k, v in comp.most_common(12):
    print(f"  {k:22s} {v:6,d}")

named = Counter()
for g in np.flatnonzero(keep):
    body = int(ids[g])
    r = row_of_body.get(body)
    named[type_col[r] if r is not None else None] += 1
print("\nmost common cell types:")
for k, v in named.most_common(12):
    print(f"  {str(k)[:40]:42s} {v:5,d}")

# do the real sensory targets reach the slice directly?
targets = set(np.fromfile(ROOT / "flyverse/data/targets_vnc_sensory.u64", dtype=np.uint64).astype(int).tolist())
in_slice_sensory = [b for b in targets if int(b) in local]
print(f"\nvnc_sensory body IDs inside the slice: {len(in_slice_sensory)}")
