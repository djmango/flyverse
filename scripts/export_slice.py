"""Export the leg motor pool slice as a browser-loadable binary asset.

Slice definition (no sampling, no pruning, no synthetic neurons):

    seed   = the 23 walking motor neurons from the dataset's own neural_io.json
    slice  = every neuron with a direct edge onto a seed, plus the seeds

Everything else follows mechanically: the CSR rows are the real rows of the
MaleCNS v1.0 pack, filtered to edges whose both ends are inside the slice, with
the real signed contact counts. Dropped edges are exactly the ones leaving the
slice (the motor neurons' own output) and edges among unselected neurons.

Output: build/fly-lif-slice.bin plus build/fly-lif-slice.json.
"""

from __future__ import annotations

import json
import struct
from collections import Counter
from hashlib import sha256
from pathlib import Path

import numpy as np
import pyarrow.feather as feather

# Locate the checkout from this file: <repo>/scripts/export_slice.py
REPO = Path(__file__).resolve().parent.parent
ROOT = REPO.parent
PACK = ROOT / "official-pack"
OUT = REPO / "build"
OUT.mkdir(parents=True, exist_ok=True)

row_ptr = np.load(PACK / "row_ptr.npy")
dest = np.load(PACK / "destinations.npy")
cnt = np.load(PACK / "signed_counts.npy")
ids = np.load(PACK / "neuron_ids.npy")
n = len(ids)
index_of = {int(b): i for i, b in enumerate(ids)}

t = feather.read_table(ROOT / "annotations.feather")
sup_col = t.column("superclass").cast("string").to_pylist()
type_col = t.column("type").cast("string").to_pylist()
class_col = t.column("class").cast("string").to_pylist()
sub_col = t.column("subclass").cast("string").to_pylist()
side_col = t.column("rootSide").cast("string").to_pylist()
body_col = t.column("bodyId").cast("int64").to_pylist()
row_of_body = {b: i for i, b in enumerate(body_col)}
positions = np.fromfile(REPO / "data/soma_positions.f32", dtype=np.float32).reshape(-1, 3)

io = json.loads((REPO / "assets/male_cns_v1_neural_io.json").read_text())


def idx_of(body_ids):
    return np.array(sorted(index_of[b] for b in body_ids if b in index_of), dtype=np.int64)


walk_motor = idx_of(
    io["groups"]["motor_walking_left"]["root_ids"] + io["groups"]["motor_walking_right"]["root_ids"]
)

order = np.argsort(dest, kind="stable")
rev_src = np.repeat(np.arange(n, dtype=np.int64), np.diff(row_ptr))[order]
bounds = np.searchsorted(dest[order], np.arange(n + 1))
pre = np.unique(np.concatenate([rev_src[bounds[s] : bounds[s + 1]] for s in walk_motor]))

keep = np.zeros(n, dtype=bool)
keep[pre] = True
keep[walk_motor] = True
sel = np.flatnonzero(keep)
n_sel = len(sel)
local = np.full(n, -1, dtype=np.int64)
local[sel] = np.arange(n_sel)

# Edges with both ends inside the slice.
src_in = np.repeat(keep, np.diff(row_ptr))
mask = src_in & keep[dest]
edge_src = np.repeat(np.arange(n, dtype=np.int64), np.diff(row_ptr))[mask]
edge_dst = dest[mask]
edge_cnt = cnt[mask].astype(np.int32)

# Sort by source so the CSR build is a single pass.
ordr = np.argsort(edge_src, kind="stable")
edge_src = edge_src[ordr]
edge_dst_l = local[edge_dst[ordr]].astype(np.uint32)
edge_cnt = edge_cnt[ordr]

m = len(edge_src)
new_row = np.zeros(n_sel + 1, dtype=np.uint32)
np.add.at(new_row, np.searchsorted(sel, edge_src, side="right"), 1)
new_row = np.cumsum(new_row).astype(np.uint32)

print(f"slice: {n_sel:,} neurons, {m:,} edges")
assert new_row[-1] == m, (new_row[-1], m)

# Per-neuron superclass, as a small integer id plus a legend.
legend = sorted({sup_col[row_of_body[int(ids[g])]] or "(none)" for g in sel})
legend_index = {name: i for i, name in enumerate(legend)}
superclass_id = np.array(
    [legend_index[sup_col[row_of_body[int(ids[g])]] or "(none)"] for g in sel], dtype=np.uint8
)

# Named subsets, all as local indices.
def local_group(body_ids):
    g = local[idx_of(body_ids)]
    return np.sort(g[g >= 0]).astype(np.uint32)


PROPRIOCEPTIVE_CLASSES = {"mechanosensory_proprioceptive"}
PROPRIOCEPTIVE_SUBCLASSES = {"chordotonal organ", "hair plate", "campaniform sensilla"}


def by_body(pred):
    return local_group([int(b) for b, *rest in zip(body_col, sup_col, class_col, sub_col, side_col)
                        if pred(*rest)])


def side_rows(want: str) -> list[int]:
    return [int(b) for b, sd in zip(body_col, side_col) if sd == want]


groups: dict[str, np.ndarray] = {
    "motor_walking": local_group(
        io["groups"]["motor_walking_left"]["root_ids"] + io["groups"]["motor_walking_right"]["root_ids"]
    ),
    "long_leg_motor": local_group(
        [b for b, x in zip(body_col, type_col) if x in {"LLPC1", "LLPC2", "LLPC3", "LLPC4"}]
    ),
    "vnc_sensory": local_group([int(b) for b, s in zip(body_col, sup_col) if s == "vnc_sensory"]),
    "descending": local_group([int(b) for b, s in zip(body_col, sup_col) if s == "descending_neuron"]),
    "ascending": local_group([int(b) for b, s in zip(body_col, sup_col) if s == "ascending_neuron"]),
    # Legs for the closed loop: the two walking motor pools are already labelled
    # left and right by the dataset.
    "motor_walking_left": local_group(io["groups"]["motor_walking_left"]["root_ids"]),
    "motor_walking_right": local_group(io["groups"]["motor_walking_right"]["root_ids"]),
    # Proprioception: chordotonal organs, hair plates and campaniform sensilla
    # report joint angle and its rate. Split by side so the loop can feed a leg
    # its own movement.
    "proprioceptive": by_body(
        lambda sup, cls, sub, sd: cls in PROPRIOCEPTIVE_CLASSES or sub in PROPRIOCEPTIVE_SUBCLASSES
    ),
    "proprioceptive_left": local_group(
        [int(b) for b, cls, sub, sd in zip(body_col, class_col, sub_col, side_col)
         if (cls in PROPRIOCEPTIVE_CLASSES or sub in PROPRIOCEPTIVE_SUBCLASSES) and sd == "L"]
    ),
    "proprioceptive_right": local_group(
        [int(b) for b, cls, sub, sd in zip(body_col, class_col, sub_col, side_col)
         if (cls in PROPRIOCEPTIVE_CLASSES or sub in PROPRIOCEPTIVE_SUBCLASSES) and sd == "R"]
    ),
    # Every afferent that reaches a walking motor neuron, by side.
    "sensory_left": local_group(
        [int(b) for b, sup, sd in zip(body_col, sup_col, side_col) if sup == "vnc_sensory" and sd == "L"]
    ),
    "sensory_right": local_group(
        [int(b) for b, sup, sd in zip(body_col, sup_col, side_col) if sup == "vnc_sensory" and sd == "R"]
    ),
    # Contact sense (bristles, campaniform hairs, taste hairs): the afferents of
    # each side that are not proprioceptive, so the closed loop can send a foot
    # strike back without also sending the joint's own report twice.
    "sensory_touch_left": local_group(
        [int(b) for b, sup, cls, sub, sd in zip(body_col, sup_col, class_col, sub_col, side_col)
         if sup == "vnc_sensory" and sd == "L"
         and not (cls in PROPRIOCEPTIVE_CLASSES or sub in PROPRIOCEPTIVE_SUBCLASSES)]
    ),
    "sensory_touch_right": local_group(
        [int(b) for b, sup, cls, sub, sd in zip(body_col, sup_col, class_col, sub_col, side_col)
         if sup == "vnc_sensory" and sd == "R"
         and not (cls in PROPRIOCEPTIVE_CLASSES or sub in PROPRIOCEPTIVE_SUBCLASSES)]
    ),
}
groups = {k: v for k, v in groups.items() if len(v)}

# Binary asset.
buf = bytearray()
buf += b"FLYLIF01"
buf += struct.pack("<IIIII", n_sel, m, len(groups), len(legend), 0)

# The legend travels with the asset, so a reader never has to guess which
# colour or label belongs to which cell class.
for name in legend:
    nb = name.encode()
    buf += struct.pack("<H", len(nb)) + nb
    buf += b"\x00" * (-len(buf) % 4)
buf += positions[sel].astype("<f4").tobytes()
buf += superclass_id.tobytes()
# Pad to a 4-byte boundary so every numbered section can be viewed as a typed
# array in JavaScript without copying.
buf += b"\x00" * (-len(buf) % 4)
buf += new_row.astype("<u4").tobytes()
buf += edge_dst_l.astype("<u4").tobytes()
buf += edge_cnt.astype("<i2").tobytes()
for name, idxs in groups.items():
    nb = name.encode()
    buf += struct.pack("<H", len(nb)) + nb
    # Align each group table so the index array can be viewed in place.
    buf += b"\x00" * (-len(buf) % 4)
    buf += struct.pack("<I", len(idxs))
    buf += idxs.astype("<u4").tobytes()

asset = OUT / "fly-lif-slice.bin"
asset.write_bytes(bytes(buf))

# Composition, for the lesson text.
comp = Counter(
    sup_col[row_of_body[int(ids[g])]] or "(none)" for g in sel
)
types = Counter(type_col[row_of_body[int(ids[g])]] for g in sel)

manifest = {
    "asset": asset.name,
    "asset_bytes": len(buf),
    "asset_sha256": sha256(bytes(buf)).hexdigest(),
    "source": "FlyEM MaleCNS v1.0 (CC BY 4.0), our own compiled pack",
    "pack": "official-pack",
    "slice_rule": (
        "seed is the 23 walking motor neurons in the dataset's own "
        "male_cns_v1_neural_io.json; the slice is every neuron with a direct "
        "edge onto a seed, plus the seeds. No sampling, no pruning, no "
        "synthetic neurons."
    ),
    "counts": {
        "neurons": int(n_sel),
        "edges": int(m),
        "excitatory_edges": int(np.count_nonzero(edge_cnt > 0)),
        "inhibitory_edges": int(np.count_nonzero(edge_cnt < 0)),
        "contact_sum": int(edge_cnt.sum()),
        "full_pack_neurons": int(n),
        "full_pack_edges": int(len(dest)),
    },
    "composition_superclass": dict(comp.most_common()),
    "composition_type_top20": dict(types.most_common(20)),
    "afferent_subclass": dict(
        Counter(
            sub_col[row_of_body[int(ids[g])]] or "(none)"
            for g in sel
            if sup_col[row_of_body[int(ids[g])]] == "vnc_sensory"
        ).most_common()
    ),
    "groups": {k: int(len(v)) for k, v in groups.items()},
    "superclass_legend": legend,
    "bytes_per_edge": 6,
    "stim_default": "vnc_sensory",
}
(OUT / "fly-lif-slice.json").write_text(json.dumps(manifest, indent=1))
np.asarray(groups["vnc_sensory"], dtype="<u4").tofile(OUT / "fly-lif-stim.u32")

print(f"asset: {asset} {len(buf):,} B  sha256 {manifest['asset_sha256'][:16]}")
print("groups:", manifest["groups"])
print("composition:", json.dumps(dict(comp.most_common(8))))

# The same slice as a FlyVerse pack directory, so the native Rust engine can be
# run on identical bytes for the parity test.
npy_dir = OUT / "slice-pack"
npy_dir.mkdir(exist_ok=True)
np.save(npy_dir / "neuron_ids.npy", ids[sel])
np.save(npy_dir / "row_ptr.npy", new_row)
np.save(npy_dir / "destinations.npy", edge_dst_l)
np.save(npy_dir / "signed_counts.npy", edge_cnt.astype(np.int16))
(npy_dir / "manifest.json").write_text(
    json.dumps({"dataset": "MaleCNS", "dataset_version": "v1.0", "slice_of": "official-pack",
                "counts": manifest["counts"], "slice_rule": manifest["slice_rule"]}, indent=1)
)
print(f"pack dir: {npy_dir}")
