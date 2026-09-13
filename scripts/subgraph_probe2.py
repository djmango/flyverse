"""Second pass: how small can an honest slice be?

The 2-hop neighbourhood of the leg sensory set is 50k neurons and 9.0M edges,
which is too big to ship. Print the annotation schema, the superclass sizes, and
hop sizes for smaller seed sets, so we can pick a slice that is both real and
loadable in a browser.
"""
from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import pyarrow.feather as feather

# Locate the checkout from this file: <repo>/scripts/subgraph_probe2.py
REPO = Path(__file__).resolve().parent.parent
ROOT = REPO.parent
PACK = ROOT / "official-pack"

row_ptr = np.load(PACK / "row_ptr.npy")
dest = np.load(PACK / "destinations.npy")
ids = np.load(PACK / "neuron_ids.npy")
n = len(ids)
index_of = {int(b): i for i, b in enumerate(ids)}

t = feather.read_table(ROOT / "annotations.feather")
print("annotation columns:", t.column_names)
print("rows:", t.num_rows)
t0 = t.slice(0, 1).to_pydict()
for k, v in t0.items():
    print(f"  {k}: {str(v)[:80]}")

sup = t.column("superclass").cast("string").to_pylist()
print("\nsuperclass sizes:")
vals, counts = np.unique(np.array([s or "" for s in sup]), return_counts=True)
for v, c in sorted(zip(vals, counts), key=lambda x: -x[1])[:16]:
    print(f"  {v or '(none)':28s} {c:7,d}")


def idx_of_body_ids(body_ids):
    return np.array(sorted(index_of[b] for b in body_ids if b in index_of), dtype=np.int64)


def edges_within(mask: np.ndarray) -> int:
    src_in = np.repeat(mask, np.diff(row_ptr))
    return int(np.count_nonzero(src_in & mask[dest]))


def downstream(seed, hops):
    cur = np.zeros(n, dtype=bool)
    cur[seed] = True
    seen = cur.copy()
    for _ in range(hops):
        src = np.flatnonzero(cur)
        outs = np.concatenate([dest[row_ptr[s] : row_ptr[s + 1]] for s in src]) if len(src) else np.array([], dtype=dest.dtype)
        nxt = np.zeros(n, dtype=bool)
        nxt[outs] = True
        nxt &= ~seen
        seen |= nxt
        cur = nxt
        if not nxt.any():
            break
    return seen


targets_all = np.fromfile(REPO / "data/targets_vnc_sensory.u64", dtype=np.uint64).astype(np.int64)
rng = np.random.default_rng(7)

print("\nseed size -> 2 hops downstream (neurons, induced edges, MB at 6 B/entry):")
for k in (25, 100, 400, 1600, 6370):
    seed = idx_of_body_ids(rng.choice(targets_all, size=min(k, len(targets_all)), replace=False))
    mask = downstream(seed, 2)
    cells, e = int(mask.sum()), edges_within(mask)
    print(f"  {k:6d} seeds -> {cells:7,d} neurons {e:10,d} edges  {cells * 6 + e * 6:12,d} B")

io = json.loads((REPO / "assets/male_cns_v1_neural_io.json").read_text())
walk_motor = idx_of_body_ids(
    io["groups"]["motor_walking_left"]["root_ids"] + io["groups"]["motor_walking_right"]["root_ids"]
)
print("\nsingle-hop upstream of the 23 walking motor neurons:")
up1 = downstream(walk_motor, 0)  # placeholder, replaced below
# reverse adjacency
order = np.argsort(dest, kind="stable")
rev_src = np.repeat(np.arange(n, dtype=np.int64), np.diff(row_ptr))[order]
bounds = np.searchsorted(dest[order], np.arange(n + 1))


def upstream(seed, hops):
    cur = np.zeros(n, dtype=bool)
    cur[seed] = True
    seen = cur.copy()
    for _ in range(hops):
        src = np.flatnonzero(cur)
        outs = np.concatenate([rev_src[bounds[s] : bounds[s + 1]] for s in src]) if len(src) else np.array([], dtype=np.int64)
        nxt = np.zeros(n, dtype=bool)
        nxt[outs] = True
        nxt &= ~seen
        seen |= nxt
        cur = nxt
        if not nxt.any():
            break
    return seen


for h in (1, 2):
    m = upstream(walk_motor, h)
    print(f"  up {h}: {int(m.sum()):7,d} neurons {edges_within(m):12,d} edges")

print("\nboth directions, 1 hop, around walking motors:")
both1 = upstream(walk_motor, 1) | downstream(walk_motor, 1)
print(f"  {int(both1.sum()):7,d} neurons {edges_within(both1):10,d} edges")
