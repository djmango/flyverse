#!/usr/bin/env python3
"""Derive the retinotopic gaze table for the flyverse visual front end.

Everything here comes from `annotations.feather` of the MaleCNS v1.0 release.
No external atlas or literature constant is used except the 8 nm voxel pitch
(the release ships `somaLocation` as point{srid:9157}, an 8 nm isotropic grid).

Two things are derived:

1. The body frame. `somaLocation` is a voxel coordinate system with an unknown
   orientation, so the axes are recovered from unambiguous anatomy in the same
   table rather than assumed:
       left    = centroid(left optic lobe)  - centroid(right optic lobe)
       anterior= centroid(T1 neuromere)     - centroid(A9 neuromere)
       dorsal  = mushroom body / central complex  vs  the ventral nerve cord
   The third axis only needs its *sign*, which is taken from the fact that the
   mushroom body and central complex sit dorsally in the brain while every
   VNC neuromere lies ventral to them.

2. One gaze direction per optic lobe column. The `assignedOlHex1/2` columns
   tile the optic lobe into 892 hexagonal columns, each holding one neuron of
   each of L1-L5, Mi1/4/9, Tm1/2/20, T1, C3 -- i.e. the cartridge structure.
   A sphere is fitted through the per-column soma centroids; the outward
   direction from its centre is that column's visual axis.

Usage: derive_retinotopy.py [--out assets/male_cns_v1_retinotopy.json]
"""
import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np
import pyarrow.feather as f

AP = Path("/opt/data/workspaces/skg/flybrain/annotations.feather")
COLS = ("bodyId", "flywireType", "class", "assignedOlHex1", "assignedOlHex2",
        "somaSide", "somaLocation", "somaNeuromere", "exitNerve")
# the cell types that make up one optic lobe cartridge
COLUMN_TYPES = {"L1", "L2", "L3", "L4", "L5", "Mi1", "Mi4", "Mi9",
                "Tm1", "Tm2", "Tm20", "T1", "C3"}


def load():
    t = f.read_table(AP, columns=list(COLS))
    return {k: t.column(k).to_pylist() for k in COLS}


def derive_frame(c):
    """Recover the body frame from anatomy. Returns a 3x3 voxel->body matrix."""
    n = len(c["bodyId"])

    def centroid(pred):
        v = [c["somaLocation"][i] for i in range(n)
             if pred(i) and c["somaLocation"][i] is not None]
        return np.array(v, dtype=float).mean(axis=0), len(v)

    def lobe(side):
        return centroid(lambda i: c["somaSide"][i] == side
                        and c["flywireType"][i] in COLUMN_TYPES)

    eye_l, nl = lobe("L")
    eye_r, nr = lobe("R")
    t1, n1 = centroid(lambda i: c["somaNeuromere"][i] == "T1")
    a9, n9 = centroid(lambda i: c["somaNeuromere"][i] == "A9")
    mb, nmb = centroid(lambda i: c["class"][i] == "Kenyon_Cell")
    cx, ncx = centroid(lambda i: c["class"][i] == "CX")
    vnc, nvnc = centroid(lambda i: c["somaNeuromere"][i] in ("T1", "T2", "T3"))

    for label, k in (("left lobe", nl), ("right lobe", nr), ("T1", n1),
                     ("A9", n9), ("Kenyon cells", nmb),
                     ("central complex", ncx), ("VNC", nvnc)):
        if k < 20:
            sys.exit(f"landmark {label} has only {k} neurons with a soma location")

    left = eye_l - eye_r
    anterior = t1 - a9
    # dorsal: the two dorsal brain structures must both sit dorsal of the VNC
    dorsal = (mb + cx) / 2.0 - vnc

    left /= np.linalg.norm(left)
    anterior /= np.linalg.norm(anterior)

    # The left-right axis is the most reliable landmark (the two optic lobes are
    # mirror images with no offset along any other axis), so it is taken as
    # exact and the other two are orthogonalised against it.
    if abs(left @ anterior) > 0.15:
        sys.exit(f"left and anterior landmarks are not near orthogonal "
                 f"({abs(left @ anterior):.3f})")
    up = (mb + cx) / 2.0 - vnc
    up /= np.linalg.norm(up)

    L = left
    A = anterior - (anterior @ L) * L
    A /= np.linalg.norm(A)
    U = np.cross(A, L)          # forward x left = up
    U /= np.linalg.norm(U)
    if U @ up < 0:
        sys.exit("derived up vector disagrees with the mushroom body landmark")

    # rows of M are the body axes expressed in voxel coordinates, so that
    # M @ v gives v as (forward, left, up)
    M = np.vstack([A, L, U])
    print(f"  frame: forward {np.round(A,3)} left {np.round(L,3)} up {np.round(U,3)}")
    print(f"  landmark agreement: left.anterior {abs(L@anterior):.3f}, "
          f"up.dorsal-landmark {U@up:+.3f}")
    print(f"  handedness forward.(left x up) = {A @ np.cross(L, U):+.3f} (want +1)")
    return M


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="assets/male_cns_v1_retinotopy.json")
    args = ap.parse_args()

    c = load()
    n = len(c["bodyId"])
    print("=== body frame")
    M = derive_frame(c)

    def to_body(v):
        return M @ np.asarray(v, dtype=float)

    cols = defaultdict(list)
    for i in range(n):
        h1, h2 = c["assignedOlHex1"][i], c["assignedOlHex2"][i]
        if h1 is not None and h2 is not None and c["somaLocation"][i] is not None \
                and c["somaSide"][i] in ("L", "R"):
            cols[(c["somaSide"][i], (h1, h2))].append(c["somaLocation"][i])

    out = {"frame_voxel_to_body": [[round(float(x), 6) for x in row] for row in M],
           "voxel_pitch_nm": 8, "columns": {}}
    for side in ("L", "R"):
        cen = {k[1]: to_body(np.mean(v, axis=0))
               for k, v in cols.items() if k[0] == side}
        P = np.array(list(cen.values()))
        # least-squares sphere through the column centroids
        A = np.hstack([2 * P, np.ones((len(P), 1))])
        sol, *_ = np.linalg.lstsq(A, (P ** 2).sum(axis=1), rcond=None)
        ctr = sol[:3]
        rad = float(np.sqrt(sol[3] + (ctr ** 2).sum()))
        resid = np.abs(np.linalg.norm(P - ctr, axis=1) - rad)
        print(f"=== side {side}: {len(P)} columns, sphere radius {rad:,.0f} voxels "
              f"({rad*8/1e6:.3f} mm), median residual {np.median(resid):,.0f} "
              f"({np.median(resid)/rad*100:.1f}%)")
        if np.median(resid) / rad > 0.15:
            sys.exit(f"side {side}: the column centroids are not sphere-like")
        dirs = {}
        for tile, p in cen.items():
            d = p - ctr
            k = np.linalg.norm(d)
            if k < 1e-6:
                continue
            dirs[f"{int(tile[0])},{int(tile[1])}"] = [round(float(x), 5) for x in d / k]
        V = np.array(list(dirs.values()))
        print(f"    gaze ranges: forward {V[:,0].min():+.2f}..{V[:,0].max():+.2f}  "
              f"left {V[:,1].min():+.2f}..{V[:,1].max():+.2f}  "
              f"up {V[:,2].min():+.2f}..{V[:,2].max():+.2f}")
        out["columns"][side] = {"sphere_centre": [round(float(x), 1) for x in ctr],
                                "sphere_radius": round(rad, 1), "gaze": dirs}

    p = Path(args.out)
    p.parent.mkdir(parents=True, exist_ok=True)
    json.dump(out, open(p, "w"))
    total = sum(len(v["gaze"]) for v in out["columns"].values())
    print(f"\nwrote {total} column gaze directions to {p}")


if __name__ == "__main__":
    main()
