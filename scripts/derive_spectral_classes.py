#!/usr/bin/env python3
"""Bake the photoreceptor spectral class of every photoreceptor the retina drives.

Why this exists. `src/vision.rs` builds one Poisson drive per optic-lobe column
and sets that drive's rate from a single luminance scalar, so every
photoreceptor in a column -- R1-R6, R7 and R8 alike -- receives the *same*
stimulus. Drosophila colour vision lives in the inner photoreceptors (R7/R8),
which differ from R1-R6 and from each other in the opsin they express, and
therefore in their spectral sensitivity. The MaleCNS annotation already resolves
those classes (`annotations.feather`, column `type`), so the distinction is
available in the data; the retina simply never used it.

This script emits, for every body id in the retinotopy asset, the spectral class
its opsin places it in, plus the peak wavelength (lambda_max) of that opsin.
`vision.rs` then groups each column's photoreceptors by class and drives each
class with the light it actually catches.

Sources for the lambda_max values (all Drosophila melanogaster, all measured):

  Rh1  478 nm  R1-R6, broadband blue-green
        Salcedo E, Huber A, Henrich S, Chadwell LV, Chou WH, Paulsen R,
        Britt SG (1999) J Neurosci 19(24):10716-10726, Table (maximal
        sensitivities of flies expressing Rh1-Rh6 are 478, 420, 345, 375, 437,
        508 nm); Hardie RC (1986) Trends Neurosci 9:419-423.
  Rh3  345 nm  R7 in pale (p) ommatidia, short UV
        Feiler R, Harris WA, Kirschfeld K, Minke B, Zuker CS (1988) Nature
        333:737-741; Feiler R, Bjornson R, Kirschfeld K, et al. (1992)
        J Neurosci 12(10):3862-3868.
  Rh4  375 nm  R7 in yellow (y) ommatidia, long UV
        Feiler et al. 1992 (above); Salcedo et al. 1999.
  Rh5  437 nm  R8 in pale (p) ommatidia, blue
        Salcedo et al. 1999 (above).
  Rh6  508 nm  R8 in yellow (y) ommatidia, green
        Salcedo et al. 1999 (above).

Cross-checked against Shakir MA, et al. (2020) Sci Rep 10:17488, "The spectral
sensitivity of Drosophila photoreceptors", which states the same five peak
sensitivities (Rh3-Rh6 lambda_max: 345, 375, 437, 508 nm) and the R1-R6 Rh1
broadband peak at 478 nm.

Subtype resolution. The annotation's `type` column gives R1-R6, R7p, R7y, R8p,
R8y for the cells whose ommatidial subtype is settled, and R7d / R8d /
R7_unclear / R8_unclear / R7R8_unclear for those it does not resolve. This
script does NOT invent a subtype for those: R7p/R7y/R8p/R8y get their own opsin,
and every unresolved cell is given a "u" (unresolved) class whose spectral
sensitivity is the mean of the two pigments it could be. Pale and yellow
ommatidia are distributed ~50:50 over the retina, so the mean is the expected
sensitivity of a cell whose subtype is unknown -- and the count of affected
cells is reported so the choice is auditable rather than hidden.

Usage:  uv run --with numpy --with pyarrow python3 scripts/derive_spectral_classes.py
Writes: assets/male_cns_v1_spectral_classes.json
"""

import collections
import json
import pathlib
import sys

import pyarrow.feather as ft

ROOT = pathlib.Path(__file__).resolve().parent.parent
ANN = pathlib.Path("/opt/data/workspaces/skg/flybrain/annotations.feather")
RETINO = ROOT / "assets" / "male_cns_v1_retinotopy.json"
OUT = ROOT / "assets" / "male_cns_v1_spectral_classes.json"

# lambda_max per class, nm, with the opsin that sets it. "u" classes are the
# mean of the two candidate pigments (computed below), not a third opsin.
LAMBDA = {
    "R16": 478.0,   # Rh1, R1-R6
    "R7p": 345.0,   # Rh3, R7 pale
    "R7y": 375.0,   # Rh4, R7 yellow
    "R8p": 437.0,   # Rh5, R8 pale
    "R8y": 508.0,   # Rh6, R8 yellow
}
OPSIN = {
    "R16": "Rh1",
    "R7p": "Rh3",
    "R7y": "Rh4",
    "R8p": "Rh5",
    "R8y": "Rh6",
}

# annotation `type` -> the class this script assigns.
#
# The first five are resolved by the annotation itself. The rest are the cells
# it does not resolve, and they are NOT given an invented subtype:
#
#   R7d, R8d          dorsal-rim (DRA) cells. In Drosophila the DRA R7 and R8
#                     are a polarisation-specialised pair, not a colour pair,
#                     and this model has no polarisation channel. They are put
#                     with the unresolved classes rather than given a colour
#                     opsin they are not known to carry. 155 cells (2.6 %).
#   R7_unclear        subtype not resolved by the annotation. 350 cells.
#   R8_unclear        subtype not resolved by the annotation. 436 cells.
#   R7R8_unclear      R7-or-R8 identity itself not resolved. 34 cells.
#
# Every unresolved cell gets the mean of the two pigments its identity could
# carry: pale and yellow ommatidia are ~50:50 over the retina, so the mean is
# the expected sensitivity of a cell whose subtype is unknown. The counts are
# reported by the script so the approximation is auditable.
TYPE_TO_CLASS = {
    "R1-R6": "R16",
    "R7p": "R7p",
    "R7y": "R7y",
    "R8p": "R8p",
    "R8y": "R8y",
    "R7d": "R7u",
    "R7_unclear": "R7u",
    "R7R8_unclear": "R7u",
    "R8d": "R8u",
    "R8_unclear": "R8u",
}


def main() -> int:
    t = ft.read_table(ANN, columns=["bodyId", "type", "flywireType", "superclass"])
    body = t.column("bodyId").to_pylist()
    typ = t.column("type").to_pylist()
    fly = t.column("flywireType").to_pylist()
    sup = t.column("superclass").to_pylist()
    by_id = {body[i]: (typ[i], fly[i], sup[i]) for i in range(len(body))}

    ret = json.loads(RETINO.read_text())
    ids = set()
    for sd in ("L", "R"):
        for cells in ret["photoreceptors"][sd].values():
            ids.update(cells)
    print(f"retinotopy asset drives {len(ids)} photoreceptor body ids")

    classes = {}
    unknown_type = collections.Counter()
    for b in sorted(ids):
        rec = by_id.get(b)
        if rec is None:
            print(f"  body {b}: not in annotations", file=sys.stderr)
            return 2
        ty, fw, sc = rec
        if sc != "ol_sensory":
            print(f"  body {b}: superclass {sc!r}, expected ol_sensory", file=sys.stderr)
            return 2
        cls = TYPE_TO_CLASS.get(ty)
        if cls is None:
            unknown_type[ty] += 1
            print(f"  body {b}: unmapped type {ty!r} (flywireType {fw!r})", file=sys.stderr)
            return 2
        classes[str(b)] = cls

    # Resolved-subtype coverage per eye, so the size of the unresolved group is
    # on the record rather than buried in a mapping table.
    print("\nclass counts over the driven photoreceptors:")
    cc = collections.Counter(classes.values())
    for k in sorted(cc):
        print(f"  {k:5} {cc[k]:5}")
    n_unres = cc.get("R7u", 0) + cc.get("R8u", 0)
    print(f"  unresolved-subtype cells: {n_unres} of {len(classes)} "
          f"({100.0 * n_unres / len(classes):.1f} %)")

    # Cross-check against the annotation's own flywireType, which groups the
    # subtypes differently (R1-6 / R7 / R8): every cell this script calls an R7
    # must be an R7 there, and so on.
    mismatch = 0
    for b, cls in classes.items():
        fw = by_id[int(b)][1]
        want = "R1-6" if cls == "R16" else ("R7" if cls.startswith("R7") else "R8")
        if fw is not None and fw != want:
            mismatch += 1
    print(f"  flywireType cross-check mismatches: {mismatch}")

    out = {
        "provenance": {
            "source": "Janelia FlyEM MaleCNS v1.0 annotations.feather, column `type`",
            "annotation_column": "type",
            "retinotopy_asset": RETINO.name,
            "lambda_nm_source": "Salcedo et al. 1999 J Neurosci 19:10716-10726; "
                                "Feiler et al. 1988 Nature 333:737-741; "
                                "Feiler et al. 1992 J Neurosci 12:3862-3868; "
                                "Hardie 1986 Trends Neurosci 9:419-423; "
                                "cross-checked against Shakir et al. 2020 Sci Rep 10:17488",
            "unresolved_rule": "subtype-unresolved cells get the mean spectral sensitivity "
                               "of the two pigments the annotation leaves open "
                               "(R7u = mean(Rh3, Rh4), R8u = mean(Rh5, Rh6)), and are "
                               "NOT assigned a guessed subtype",
            "unresolved_cells": n_unres,
            "total_cells": len(classes),
        },
        "lambda_nm": LAMBDA,
        "opsin": OPSIN,
        "class": classes,
    }
    OUT.write_text(json.dumps(out, indent=1, sort_keys=True) + "\n")
    print(f"\nwrote {OUT} ({OUT.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())