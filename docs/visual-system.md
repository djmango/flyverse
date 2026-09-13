# The visual system: what the connectome has, and what was missing

## Summary

The MaleCNS v1.0 release contains a complete optic lobe. flyverse's pack build
was silently deleting the entire retinal output, so nothing the fly saw could
reach its brain. That is fixed. The connectome now carries a real
retina -> lamina -> medulla -> lobula plate pathway, and the columns of that
pathway have been given real gaze directions.

The drive that reaches the optic lobe is still a surrogate formula. This
document says exactly where the real data ends.

## What is in the dataset

| Structure | Neurons | Notes |
|---|---|---|
| Photoreceptors R1-R6 | 3,377 | classed `visual`, consensus transmitter `histamine` |
| Photoreceptors R7 | 1,329 | as above |
| Photoreceptors R8 | 1,300 | as above |
| Lamina columns | 23,720 | `assignedOlHex1/2` assigns them to 892 hexagonal columns |
| Lobula plate tangential cells | 44 | `superclass == visual_projection`, types H2, HSE, HSN, HSS, HST, VS, VST1, VST2, VSm |

### Retinotopy

`assignedOlHex1` and `assignedOlHex2` together tile the optic lobe into **892
columns**, and each column holds one neuron of each of L1, L2, L3, L5, Mi1,
Mi4, Mi9, Tm1, Tm2, Tm20, T1 and C3. That is the cartridge structure of the
optic lobe, recovered directly from the release's own annotation columns.

879 of the 892 columns contain both left and right members: the hex grid is a
single column index, and the left and right members of a column are the two
eyes' photoreceptors looking at the same point in the visual field.

T4 and T5, the elementary motion detectors, carry **no hex assignment**
(13,581 neurons). They cannot be driven retinotopically; they are reached from
the columns through the connectome instead.

### The body frame

`somaLocation` is a voxel coordinate system with no published orientation, so
the axes are recovered from unambiguous anatomy in the same table rather than
assumed:

- **left** = centroid(left optic lobe) - centroid(right optic lobe)
- **anterior** = centroid(T1 neuromere) - centroid(A9 neuromere)
- **dorsal** = mushroom body and central complex, versus the ventral nerve cord

The checked result, printed by `scripts/derive_retinotopy.py`:

```
frame: forward [-0.014  0.175 -0.985] left [ 0.999 -0.038 -0.02 ] up [-0.041 -0.984 -0.174]
landmark agreement: left.anterior 0.015, up.dorsal-landmark +0.702
handedness forward.(left x up) = +1.000 (want +1)
```

So in voxel coordinates `+x` is left, `-z` is anterior, and `-y` is dorsal.
The script exits rather than write a table if the landmarks are not close to
orthogonal, if the handedness is wrong, or if the two dorsal structures
disagree with the derived up axis.

### Gaze directions

A sphere is fitted by least squares through the per-column soma centroids of
each eye. Each column's visual axis is the outward direction from that sphere's
centre. The fits:

| Side | Columns | Sphere radius | Median residual |
|---|---|---|---|
| L | 879 | 0.143 mm | 6.0% of radius |
| R | 892 | 0.145 mm | 5.8% of radius |

The radius is the local curvature of the lamina sheet over the column patch,
not the radius of the whole eye. The resulting gaze fields span forward
-0.12..+1.00 and left -0.40..+1.00 for the left eye, and the mirror of that for
the right, with the small frontal overlap a fly eye is expected to have.

Output: `assets/male_cns_v1_retinotopy.json`, 1,771 gaze directions.

### Photoreceptor to column

Only 28 of the 6,091 photoreceptors carry a `somaLocation`, so position cannot
attach them to columns. Connectivity can: a photoreceptor synapses onto the
lamina neurons of its own cartridge, and those do carry the hex. Each
photoreceptor is assigned to the column that most of its hexed targets belong
to.

| Check | Result |
|---|---|
| assigned | 5,895 of 6,091 (196 have no hexed target at all) |
| target sets naming exactly one column | 5,293 |
| dominant column at least 80% of a target set | 5,641 |
| photoreceptor `rootSide` equals its column's side | **5,895 / 5,895** |

The script exits rather than write the table if the side check fails.

**Side lives in two different columns.** Photoreceptors have a null `somaSide`
and carry their side in `rootSide`; lamina and medulla neurons are the exact
reverse, `rootSide` null and `somaSide` populated. Reading one column for both
populations returns null for 6,062 of 6,091 photoreceptors and makes the check
vacuous, which is how the first version of this script came to report a
meaningless "0 mismatches" over 29 neurons. Comparing the two different columns
is what produces the 5,895/5,895 above.

Caveat: this is not a complete retina. Photoreceptors per column average 3.35 on
the left and 4.62 on the right, against the six R1-R6 plus one R7 and one R8
that a cartridge holds, and the retinal sampling is uneven between the two
eyes. 5,895 photoreceptors are placed with confidence; the per-column counts
should not be read as a full ommatidial lattice.

## The bug that killed the visual system

`scripts/build_pack.py` assigned each edge a sign from the transmitter of its
presynaptic cell, and dropped edges whose presynaptic cell had no settled sign:

```python
POSITIVE = {"acetylcholine"}
NEGATIVE = {"gaba", "glutamate"}
```

Photoreceptors are **histaminergic**, and histamine was in neither set. Every
photoreceptor therefore had sign 0, so `edge_ok = src_sign != 0` was false and
**all 491,144 retinal output synapses were deleted**. The photoreceptors ended
up in the graph as isolated nodes with in-degree but no out-degree, which made
the fly blind while looking perfectly healthy in every census.

All 6,091 photoreceptors are labelled `histamine` in the release's own
consensus transmitter table. Histamine is the fast inhibitory transmitter of
the lamina: it hyperpolarises the lamina monopolar cells. It belongs in the
inhibitory set, and now it is there.

The fix was verified before adoption:

```
OLD: photoreceptors in pack 6091; out-degree max 0 sum 0
NEW: photoreceptors in pack 6091; out-degree max 66 sum 74557
     targets: [('L3', 5646), ('L1', 4953), ('Dm9', 4564), ('yDm8', 4159),
               ('L2', 4025), ('pDm8', 3758), ('MeTu3c', 2726), ('Dm2', 2629),
               ('Mi15', 2454), ('Tm5c', 2351)]
old edges missing from the new pack: 0
edges added by the new pack: 89723
```

R1-R6 -> L1/L2/L3 and R7/R8 -> Dm8/MeTu is the canonical first visual synapse,
so the restored edges are the right ones. The new pack is a strict superset of
the old one: no edge was lost. Census moves from 24,469,412 to **24,559,135
edges**; the neuron set is unchanged at 166,700.

### The pathway is now intact

Breadth-first from the 6,091 photoreceptor somata through the pack:

| Depth | New neurons | Lobula plate TCs reached |
|---|---|---|
| 1 | 23,707 | 0 |
| 2 | 76,060 | 37 |
| 3 | 48,049 | 7 |
| 4 | 12,322 | 0 |
| 5 | 160 | 0 |
| 6 | 2 | 0 |

All 44 lobula plate tangential cells are reached in 2-3 synapses, and 166,391
of 166,700 neurons are reachable from the retina in total. Retina -> lamina at
one synapse, motion cells at two to three, is the real pathway depth.

## What is still surrogate

The optic-flow *value* handed to the brain is still the engineered
`0.5 * speed/300 + 0.5 * turn`, not a rendered scene. Note where that lands:
the drive targets `visual_motion_left` and `visual_motion_right`, which resolve
to the 21 left and 23 right **real lobula plate tangential cells** (H2, HSE,
HSN, HSS, HST, VS, VST1, VST2, VSm). So the target set is real and the
computation is not.

Two ways forward, in increasing faithfulness:

1. Drive each of the 44 tangential cells from a real optic-flow field: compute
   image motion from the body's velocity and angular velocity against the room
   geometry, sampled along the 1,771 now-known gaze directions, and project it
   onto each cell's preferred direction. This replaces the formula but still
   injects at the tangential cells.
2. Drive the **photoreceptors** themselves, one per column, with the luminance
   of a textured world along that column's gaze direction, and let the
   connectome compute motion. This is the faithful option, and the pack fix is
   what makes it possible: before it, those neurons had nowhere to send a
   spike.

Option 2 is the target. It needs a textured room and a per-column raycast, and
it puts the entire motion pathway back inside the connectome where it belongs.

## Reproducing

```
# rebuild the pack with the corrected transmitter sign table
FLYVERSE_PACK_OUT=/tmp/pack-new python scripts/build_pack.py

# derive the gaze table (verifies the frame before writing)
./scripts/derive_retinotopy.py
```
