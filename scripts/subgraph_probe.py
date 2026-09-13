"""Measure candidate WASM subgraphs on the official pack.

We want a small, honest slice of the real connectome: real neurons, real signed
edges among them, no synthetic cells. Measure a few candidate sets before
choosing which one ships a lesson asset.
"""
from __future__ import annotations

import json
from pathlib import Path

import numpy as np

PACK = Path("/opt/data/workspaces/skg/flybrain/official-pack")
IO = Path("/opt/data/workspaces/skg/flybrain/flyverse/assets/male_cns_v1_neural_io.json")
TARGETS = Path("/opt/data/workspaces/skg/flybrain/flyverse/data/targets_vnc_sensory.u64")

row_ptr = np.load(PACK / "row_ptr.npy")
dest = np.load(PACK / "destinations.npy")
cnt = np.load(PACK / "signed_counts.npy")
ids = np.load(PACK / "neuron_ids.npy")
n = len(ids)
print(f"pack: {n:,} neurons, {len(dest):,} edges")

index_of = {int(b): i for i, b in enumerate(ids)}


def to_idx(body_ids) -> np.ndarray:
    return np.array(
        sorted(index_of[b] for b in body_ids if b in index_of), dtype=np.int64
    )


def downstream(seed: np.ndarray, hops: int) -> np.ndarray:
    cur = np.zeros(n, dtype=bool)
    cur[seed] = True
    seen = cur.copy()
    for h in range(hops):
        src = np.flatnonzero(cur)
        outs = np.concatenate(
            [dest[row_ptr[s] : row_ptr[s + 1]] for s in src]
        ) if len(src) else np.array([], dtype=dest.dtype)
        nxt = np.zeros(n, dtype=bool)
        nxt[outs] = True
        nxt &= ~seen
        if not nxt.any():
            break
        print(f"  hop {h + 1}: +{int(nxt.sum()):,} new")
        seen |= nxt
        cur = nxt
    return seen


def upstream(seed: np.ndarray, hops: int) -> np.ndarray:
    """Reverse CSR: build once, then BFS backwards."""
    order = np.argsort(dest, kind="stable")
    rev_dst = np.repeat(np.arange(n, dtype=np.int64), np.diff(row_ptr))
    rev_src = rev_dst[order]
    boundaries = np.searchsorted(dest[order], np.arange(n + 1))
    cur = np.zeros(n, dtype=bool)
    cur[seed] = True
    seen = cur.copy()
    for h in range(hops):
        src = np.flatnonzero(cur)
        outs = (
            np.concatenate(
                [rev_src[boundaries[s] : boundaries[s + 1]] for s in src]
            )
            if len(src)
            else np.array([], dtype=np.int64)
        )
        nxt = np.zeros(n, dtype=bool)
        nxt[outs] = True
        nxt &= ~seen
        if not nxt.any():
            break
        print(f"  up hop {h + 1}: +{int(nxt.sum()):,} new")
        seen |= nxt
        cur = nxt
    return seen


def edges_within(mask: np.ndarray) -> int:
    src_in = np.repeat(mask, np.diff(row_ptr))
    return int(np.count_nonzero(src_in & mask[dest]))


targets = to_idx(np.fromfile(TARGETS, dtype=np.uint64))
print(f"sensory targets present: {len(targets):,}")

io = json.loads(IO.read_text())
groups = io["groups"]


def group(name: str) -> np.ndarray:
    return to_idx(groups[name]["root_ids"])


walk_motor = np.concatenate(
    [group("motor_walking_left"), group("motor_walking_right"),
     group("motor_landing_left"), group("motor_landing_right")]
)
walk_dn = np.concatenate(
    [group("walking_dn_left"), group("walking_dn_right"),
     group("landing_dn_left"), group("landing_dn_right")]
)
print(f"leg motor neurons: {len(walk_motor)}, descending walk: {len(walk_dn)}")

print("\n(A) downstream of leg sensory, 2 hops")
a2 = downstream(targets, 2)
print("\n(B) upstream of leg motor (+DN), 3 hops")
b3 = upstream(np.concatenate([walk_motor, walk_dn]), 3)

for name, mask in (("A2", a2), ("B3", b3), ("A2∩B3", a2 & b3), ("A2∪B3", a2 | b3)):
    cells = int(mask.sum())
    e = edges_within(mask)
    print(f"{name:8s} neurons {cells:7,d}  edges {e:9,d}  asset {cells * 6 + e * 6:,} B")

np.save("/opt/data/workspaces/skg/flybrain/out/probe_A2.npy", a2)
np.save("/opt/data/workspaces/skg/flybrain/out/probe_B3.npy", b3)
