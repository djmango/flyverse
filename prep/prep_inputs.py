#!/usr/bin/env python3
"""Dump the flyverse input files from the official Janelia MaleCNS tables.

Writes, into flyverse/data/:
  targets_vnc_sensory.u64   sorted body IDs of the vnc_sensory superclass
  soma_positions.f32        n*3 little-endian float32 soma xyz in mm, model order
  superclass_ids.json       body IDs grouped by superclass, for the region HUD

Soma coordinates come from the official neuPrint neuron table
(somaLocation:point{srid:9157}, 8 nm voxels -> mm via 8e-6).
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.feather as feather

# Locate the checkout from this file: <repo>/prep/prep_inputs.py
REPO = Path(__file__).resolve().parent.parent
# The public MaleCNS release tables sit beside the checkout, not inside it.
ROOT = REPO.parent
PACK = ROOT / "official-pack"
OUT = REPO / "data"
OUT.mkdir(parents=True, exist_ok=True)

ids = np.load(PACK / "neuron_ids.npy")
n = int(ids.size)
print(f"pack: {n:,} neurons", flush=True)


BODY_COL = "bodyId:long"
SOMA_COL = "somaLocation:point{srid:9157}"

# neuPrint serialises its point extension type as the literal string
# "{x:37124, y:22258, z:36274}" in 8 nm voxels for srid 9157.
_POINT_RE = __import__("re").compile(
    r"x:\s*(-?\d+(?:\.\d+)?)\s*,\s*y:\s*(-?\d+(?:\.\d+)?)\s*,\s*z:\s*(-?\d+(?:\.\d+)?)"
)


def parse_point(s):
    if not isinstance(s, str):
        return None
    m = _POINT_RE.search(s)
    if not m:
        return None
    return (float(m.group(1)), float(m.group(2)), float(m.group(3)))


def stream_columns(path: Path, wanted: list[str], value_set=None, key: str | None = None):
    """Stream a feather file's record batches, keeping only `wanted` columns and,
    when given, only rows whose `key` column is in `value_set`."""
    with pa.memory_map(str(path), "r") as source:
        reader = pa.ipc.open_file(source)
        for b in range(reader.num_record_batches):
            batch = reader.get_batch(b)
            if value_set is not None:
                mask = pc.is_in(batch.column(key), value_set=value_set)
                n_hit = pc.sum(mask).as_py() or 0
                if n_hit == 0:
                    continue
                batch = batch.filter(mask)
            yield batch.select(wanted)


# ---- annotations: superclass per bodyId -------------------------------------
ann = feather.read_table(
    ROOT / "annotations.feather", columns=["bodyId", "superclass", "type"]
)
body = ann.column("bodyId").to_numpy().astype(np.uint64)
sc = ann.column("superclass").cast(pa.string()).to_pylist()
by_sc: dict[str, list[int]] = {}
for b, s in zip(body, sc):
    if s:
        by_sc.setdefault(s, []).append(int(b))
for k in by_sc:
    by_sc[k].sort()
tot = sum(len(v) for v in by_sc.values())
print(f"annotations: {tot:,} labelled rows, {len(by_sc)} superclasses", flush=True)
json.dump(by_sc, open(OUT / "superclass_ids.json", "w"))

vnc = np.array(by_sc.get("vnc_sensory", []), dtype=np.uint64)
vnc.tofile(OUT / "targets_vnc_sensory.u64")
print(f"vnc_sensory targets: {vnc.size:,} -> targets_vnc_sensory.u64", flush=True)

# ---- soma coordinates from the official neuPrint neuron table ---------------
pos = np.full((n, 3), np.nan, dtype=np.float32)
# index the pack ids for fast lookup
order = np.argsort(ids)
sorted_ids = ids[order]
found = 0
rows = 0
value_set = pa.array(ids.astype(np.int64))
for tbl in stream_columns(
    ROOT / "Neuprint_Neurons.feather",
    [BODY_COL, SOMA_COL],
    value_set=value_set,
    key=BODY_COL,
):
    b = tbl.column(BODY_COL).to_numpy(zero_copy_only=False)
    soma = tbl.column(SOMA_COL).to_pylist()
    for bi, s in zip(b, soma):
        rows += 1
        if not s:
            continue
        pt = parse_point(s)
        if pt is None:
            continue
        p = np.searchsorted(sorted_ids, np.uint64(bi))
        if p >= n or sorted_ids[p] != np.uint64(bi):
            continue
        mi = order[p]
        pos[mi] = (pt[0] * 8e-6, pt[1] * 8e-6, pt[2] * 8e-6)
        found += 1
    print(f"  scanned {rows:,} matched neuron rows, {found:,} with soma", flush=True)

nan = int(np.isnan(pos[:, 0]).sum())
print(f"soma: {found:,} placed, {nan:,} missing", flush=True)
# Fill missing somas with the centroid so the point cloud stays complete.
if nan:
    good = ~np.isnan(pos[:, 0])
    pos[~good] = pos[good].mean(axis=0)
pos.astype("<f4").tofile(OUT / "soma_positions.f32")
print(f"wrote soma_positions.f32 ({(n*3*4)//1024//1024} MB)", flush=True)
