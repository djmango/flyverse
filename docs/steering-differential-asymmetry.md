# The one-signed steering differential: real connectome asymmetry, not a read-out artefact

`docs/steering-pool-gradedness.md` §4 closed with a finding it did not explain:

> The steering differential is still **one-signed** in every airborne sample
> (seed 7: range `[-0.106, -0.026]`), so the yaw torque is persistently in one
> direction — the "yaw attribution" question from §7 of `force-rate-map.md` is
> unchanged: the circling is a standing asymmetry in the connectome's own
> steering output, not a noise-driven random walk.

The differential is `steer_r - steer_l`, the motor read-out of the two
`motor_flight_steering_{left,right}` pools (`sim.rs` `raw_motors`, `Out::norm`),
i.e. the tilt-differential command the body turns into yaw. This note asks the
three questions that finding left open: is the one-signedness a read-out bug, is
it real specimen anatomy, and is it upstream or downstream of the wall-hugging.

**Answer in one paragraph.** The one-signedness **replicates and is real** — it
is negative in 100 % of airborne samples in all three seeds, not a seed-7
artefact — and it is **not a read-out artefact**: the two pools are built by the
same code path, with the same 200 Hz anchor, the same 2 ms window, the same
3/80 ms smoothing, the same 9 members, the correct per-side somata and no shared
neurons. It is **genuine anatomy of this single specimen**: the left steering
motor neurons receive **+17 331 vs +13 483 net signed contacts** (a 1.29x net /
1.13x absolute-|contact| excess), and a 1.13 L/R ratio is exactly ordinary for
the MaleCNS motor system — across the **25 bilateral `vnc_motor`/`wm` cell
types** the |contact| L/R ratio has median **1.117**, range 0.67-1.30, with 20 of
25 types net-left-hotter, and the steering pool's own 1.134 sits mid-distribution.
The asymmetry is present **in the raw Janelia connectivity table**, not introduced
by the compiler (0 mismatched edge weights, 0 pack-only pairs). Its origin is the
**last hop**: VNC intrinsic premotor interneurons project onto the two pools with
large per-partner laterality (`sum |L-R|` = 136 206 contacts over 3 810 partners)
that almost entirely cancels to a **+3 848 net residual** — i.e. the standing turn
is a small residual of an almost bilaterally balanced premotor convergence. And it
is **independent of the wall-hugging, and upstream of it**: with both visual
channels off and the fly grounded 70 mm from the nearest wall with `loom = 0`, the
differential is already negative 10 ms into the run and stays 100 % negative — so
it is not caused by wall proximity or by the saturated loom, while the standing
one-signed yaw moment it commands is what repeatedly puts the fly at the walls.
**No fix is proposed: this is not a defect.**

---

## 1. VERIFY — the one-signedness replicates; the read-out path is symmetric

`analyze --seconds 12 --every 1` (every 2 ms control window), airborne
`= mode in {TAKEOFF, CRUISE, LANDING}`, differential `= steer_r - steer_l`:

| seed | airborne n | positive | negative | zero | mean | sd | min | max |
|---|---|---|---|---|---|---|---|---|
| 7 | 781 | 0 (0.0 %) | **781 (100.0 %)** | 0 | -0.0683 | 0.0249 | -0.1079 | -0.0245 |
| 11 | 3493 | 0 (0.0 %) | **3493 (100.0 %)** | 0 | -0.0791 | 0.0159 | -0.1155 | -0.0322 |
| 23 | 2413 | 0 (0.0 %) | **2413 (100.0 %)** | 0 | -0.0696 | 0.0123 | -0.0995 | -0.0275 |

Over **all** samples (ground included) it is 5 995/6 000, 5 996/6 000 and
5 997/6 000 negative — the handful of non-negative samples are the first
5-6 windows of the run, when both pools are still exactly 0. The seed-7 range
`[-0.108, -0.025]` reproduces `steering-pool-gradedness.md`'s `[-0.106, -0.026]`
bit-for-bit (that doc used `--every 10`). **The finding is not a seed-7 draw.**

Read-out path, checked for exactly the failure mode an off-by-one or a swap would
produce:

- `sim.rs` builds `o_steer_l` from `"motor_flight_steering_left"` and
  `o_steer_r` from `"motor_flight_steering_right"` (lines 405-406), and
  `Out::new` only stores `groups.idx(name)`.
- The nine members of each pool were re-resolved from
  `annotations.feather`: every member of the "left" group has `somaSide == 'L'`
  and every member of the "right" group `'R'`, all nine types present on each
  side (hg1-hg4, b1-b3, tp1, tp2), **no neuron in both pools**. There is no swap
  and no side collapse.
- Both pools carry the same anchor: `phys_full_scale_hz` matches the two exact
  names to `STEER_MN_FULL_SCALE_HZ` (200 Hz). `group_size` is 9/9.
- Both go through one code path — `GroupRates::commit` then `norm` — with the
  same `window_steps` (2 ms), the same `MOTOR_RATE_TAU_S` (80 ms) tonic estimate
  and the same `clamp(0,1)`. There is no per-group branch between them.
- The per-cell motor calibration (`apply_mn_input_gain`) is applied to the two
  **power** pools only (`MN_CALIBRATED_GROUPS`), never to steering, so it cannot
  bias one steering pool against the other.
- Neither pool is at the rail: `pct_steering_readout_saturated = 0.0 %` in all
  three seeds, and the read-out levels are 0.34-0.44 of the anchor — the same
  place the full-scale anchor puts them, so a ceiling is not clipping one side.

The read-out reports a left-minus-right difference because the left pool really
does fire more: `steer_l_hz` mean **82.2** vs `steer_r_hz` **67.9** (seed 7,
airborne). Nothing in the pipeline manufactures it.

---

## 2. ANATOMY — a real, specimen-wide left-heavier motor innervation

Incoming connectivity of the two pools from the pack (`official-pack` CSR,
signed contact counts):

| | left pool | right pool |
|---|---|---|
| members | 9 | 9 |
| incoming edges (partners) | 5 716 | 5 971 |
| **total signed contacts** | **+17 331** | **+13 483** |
| total \|contacts\| | 110 495 | 97 433 |
| excitatory / inhibitory edges | 3 273 / 2 443 | 3 393 / 2 578 |
| per neuron, mean \|incoming\| | 12 277 | 10 826 |
| per neuron, mean net | +1 926 | +1 498 |

Per member (type: net signed contacts, left vs right): hg1 **+5338**/+5123,
hg3 **+3241**/+2651, hg4 **+2683**/+1232, b2 **+2579**/+1865, b3
**+2204**/+1692, tp2 **+1799**/+1431, b1 **+3**/-176, tp1 **+8**/-12, hg2
**-524**/-323. Seven of nine types are net-hotter on the left; none is
dramatically inverted. **Net L-R = +3 848 contacts (1.29x net, 1.13x absolute).**

Is 1.13 ordinary for this specimen, or specific to steering? Type-matched
bilateral comparison over every `vnc_motor`/`wm` type present on both sides:

| type | nL | nR | \|cnt\| L | \|cnt\| R | ratio | net L | net R |
|---|---|---|---|---|---|---|---|
| DLMn a, b (power) | 1 | 1 | 9 945 | 10 637 | 0.935 | +3 225 | +3 159 |
| DLMn c-f (power) | 4 | 4 | 8 711 | 8 341 | 1.044 | +2 283 | +2 214 |
| DVMn 1a-c (power) | 3 | 3 | 4 423 | 4 292 | 1.031 | +2 279 | +2 133 |
| DVMn 2a, b (power) | 2 | 2 | 2 852 | 2 553 | 1.117 | +1 350 | +1 210 |
| DVMn 3a, b (power) | 2 | 2 | 3 937 | 3 333 | 1.181 | +1 193 | +999 |
| MNwm35 | 1 | 1 | 16 468 | 14 126 | 1.166 | +4 980 | +4 158 |
| MNwm36 | 1 | 1 | 19 678 | 17 655 | 1.115 | +3 342 | +3 271 |
| STTMm | 2 | 2 | 7 140 | 7 158 | 0.997 | +1 602 | +1 532 |
| TTMn | 1 | 1 | 2 409 | 3 594 | 0.670 | -773 | -1232 |
| **b1 MN** | 1 | 1 | 12 363 | 10 800 | **1.145** | +3 | -176 |
| **b2 MN** | 1 | 1 | 14 855 | 12 761 | **1.164** | +2 579 | +1 865 |
| **b3 MN** | 1 | 1 | 11 574 | 9 578 | **1.208** | +2 204 | +1 692 |
| **hg1 MN** | 1 | 1 | 16 710 | 16 095 | **1.038** | +5 338 | +5 123 |
| **hg2 MN** | 1 | 1 | 5 286 | 5 587 | **0.946** | -524 | -323 |
| **hg3 MN** | 1 | 1 | 15 351 | 13 589 | **1.130** | +3 241 | +2 651 |
| **hg4 MN** | 1 | 1 | 16 139 | 12 450 | **1.296** | +2 683 | +1 232 |
| **tp1 MN** | 1 | 1 | 8 628 | 7 828 | **1.102** | +8 | -12 |
| **tp2 MN** | 1 | 1 | 9 589 | 8 745 | **1.097** | +1 799 | +1 431 |
| i1 MN | 1 | 1 | 13 954 | 12 090 | 1.154 | +4 588 | +4 070 |
| i2 MN | 1 | 1 | 17 357 | 15 233 | 1.139 | +4 519 | +3 457 |
| iii1 MN | 1 | 1 | 7 655 | 6 947 | 1.102 | +2 517 | +1 865 |
| iii3 MN | 1 | 1 | 4 908 | 4 329 | 1.134 | +424 | +313 |
| ps1 MN | 1 | 1 | 16 220 | 16 376 | 0.990 | +5 062 | +4 926 |
| ps2 MN | 1 | 1 | 8 564 | 7 272 | 1.178 | +1 506 | +1 576 |
| tpn MN | 1 | 1 | 9 939 | 7 810 | 1.273 | +1 409 | +660 |

**25 bilateral types: median ratio 1.117, range 0.670-1.296; 20 of 25 are net
left-hotter.** The nine steering types span 0.946-1.296 (median 1.13) and the
pool aggregate is 1.134 — the middle of the distribution. The same left bias is
visible in a completely independent read-out: the **flight power** pools, whose
own wiring ratio is only 1.044, still read `flight_power_l 0.512` vs
`flight_power_r 0.448` in the closed loop (all three seeds). So this is not a
steering-specific defect; it is the **MaleCNS specimen's motor system being
left-heavier**, which the steering pools inherit at the ordinary magnitude.

### It is in the source data, not in the compiler

`scripts/build_pack.py` compiles the official Janelia release into the pack.
Re-aggregated the raw `official-connectivity.feather` (151.8 M rows) for the 18
steering body IDs and compared to the pack:

- 18 213 raw `(pre, post)` pairs onto the 18 MNs; **11 687 in the pack; 6 526
  dropped** — all of the dropped ones are presynaptic neurons with no annotated
  soma or no settled transmitter sign, which the compiler documents dropping.
- **pairs in the pack that are not in the source: 0.**
- **Weight mismatches over the 11 687 shared pairs: 0.**

And the asymmetry is in the source itself: the raw-table \|contact\| totals are
**L 115 358 / R 103 420 = 1.115**, essentially the pack's 1.134 (the small
difference is exactly the dropped unannotated presynaptic cells). A construction
error would have to be present in Janelia's own table.

---

## 3. ORIGIN — induced by the drive, located at the last hop, and not sensory

### Not intrinsic: with no drive the network is silent, so the differential is 0

`FLYVERSE_NO_SENSE=1` zeroes every sensory channel and stops the retina emitting.
Seeds 7 and 11, 4 s, `--every 1`: **0 spikes in 2 000 control windows**
(`win_spikes = 0` throughout), and the differential is **exactly 0 in 2 000/2 000
samples** on both seeds. So the network has no autonomous asymmetry, and there is
nothing in the read-out to bias: the one-signedness is a **driven response** of an
asymmetric network to the stimulus.

### The drive's own asymmetry is a minor contributor

Attribution of net signed contacts onto the pools, by named I/O group:

| group | n | onto L | onto R |
|---|---|---|---|
| `flight_state_sapp_left` | 75 | +348 | +23 |
| `flight_state_sapp_right` | 73 | +7 | +348 |
| `flight_dng02_left` | 15 | +185 | +159 |
| `flight_dng02_right` | 14 | +174 | +185 |
| `flight_dng07_left` | 8 | +0 | +20 |
| `flight_dng07_right` | 8 | +26 | +5 |
| `flight_steering_dn_left/right` | 6/6 | +16/+10 | +8/+3 |
| `visual_loom_left/right` | 1/1 | 0 | 0 |
| `visual_motion_left/right` | 21/23 | 0 | 0 |
| `olfaction_left/right` | 798/1247 | 0 | 0 |
| `vnc_sensory` target set | 6 370 | +613 | +462 |

The two descending flight-state (sapp) groups are symmetric by construction
(`sapp_left` +348 onto the **left** pool, `sapp_right` +348 onto the **right**
one), the descending pairs are near-symmetric pair for pair, the
visual/olfactory groups project **nothing** directly onto the pools, and the
whole fixed `vnc_sensory` reference set contributes only **+151** net — **4 % of
the +3 848 imbalance**. There is no single sensory or descending channel carrying
it.

### It is not the visual channels and not the walls: three controls

| control | seeds | differential airborne | first non-zero | fly state at that moment |
|---|---|---|---|---|
| normal (`retina` on) | 7, 11, 23 | **100 % negative**, mean -0.068/-0.079/-0.070 | t = 0.014 s | grounded, `wall_dist` 70 mm, `loom = 0` |
| `FLYVERSE_NO_RETINA=1` | 7, 11, 23 | **100 % negative** (2995/2996/2997 of 3000) | t = 0.012 / 0.010 / 0.008 s | grounded, 70 mm, `loom = 0` |
| `NO_RETINA=1 NO_FLOW=1` (**vnc-only**) | 7, 11, 23 | **100 % negative**, mean -0.073/-0.078/-0.074 | t = 0.012 / 0.010 / 0.008 s | grounded, 70 mm, `loom = 0`, flow 0 |

With **both** visual channels off, the only meaningful drive on the ground is the
pose-independent `vnc_sensory` reference set (150 Hz) plus ground-load tactile —
and the differential is still negative from the **8-12 ms** mark and 100 %
negative for the whole run, at the same magnitude as the full-sense runs. So it
is not the retina, not the optic-flow proxy, not the loom channel, and not the
wall.

Its sign is also invariant to wall proximity and to loom across the whole run
(seed 7, all 6 000 samples):

| `loom` bucket | 0.0 | 0.1 | 0.3 | 0.5 | 0.7 | 0.9 | 1.0 |
|---|---|---|---|---|---|---|---|
| negative fraction | 99.9 % (4637/4642) | 100 % | 100 % | 100 % | 100 % | 100 % | 100 % |

| `wall_dist` (mm) | 0 | 20 | 60 | 80 | 120 | 160 |
|---|---|---|---|---|---|---|
| negative fraction | 100 % | 100 % | 99.9 % | 100 % | 100 % | 100 % |

### Where it enters: the last hop, as a residual of near-balanced convergence

For each of the **3 810** distinct presynaptic partners of either pool, the
difference between its projection onto the left pool and onto the right pool:

```
sum |L-R| over all partners      = 136 206 contacts
net imbalance (L total - R total) =   3 848 contacts   (2.8 % of it)
```

Individual premotor neurons are strongly lateralized — e.g. `IN03A011`
body 802 916 +1 282 onto L and +6 onto R, while `IN03A011` body 803 760 is
+17 onto L and +889 onto R; `IN19B008` 800 084 is +1 380/+349. But these
per-partner differences **almost entirely cancel**, leaving a +3 848 residual.
The imbalance by presynaptic type is dominated by VNC intrinsic interneurons —
`IN17A049` +558, `IN19B008` +529, `IN03A011` +404, `IN03B072` -382, `dMS2`
+288, `IN11A006` +279 — i.e. it is distributed premotor wiring, not one driver.
Restricted to the 1 567 partners both pools share, the left pool still receives
**+15 063 vs +10 835**: the same premotor cells are wired more strongly to the
left steering MNs. **The asymmetry enters at the premotor-to-steering-MN
synapses**, as a small residual of an otherwise bilaterally balanced convergence.

**Where I could not establish it:** which single upstream layer first breaks
symmetry. The residual is a sum over ~3 800 partners whose individual asymmetries
cancel, so there is no one partner to trace; separating "the premotor weights are
asymmetric" from "the layer feeding them is asymmetric" needs per-layer activity
instrumentation this repo does not have. The one thing that is established is
that it is *not* in the sensory drive set (4 %) and *not* in the read-out (0 %).

---

## 4. SEPARATE THE TWO PROBLEMS — the asymmetry is (c) independent, and upstream

The task's chain was: saturated loom (85 % of airborne samples `loom > 0.9`
because the fly is always near a wall) → no loom differential → absent loom
response. The measured answer for the steering differential:

- **(b) consequence of the wall-hugging via saturated loom — RULED OUT.** The
  differential is one-signed at `loom = 0.0` (4637/4642 negative) and at
  `wall_dist = 160 mm` (100 %), it is already negative 10 ms into the run while
  the fly is on the ground 70 mm from the nearest wall, and it is unchanged when
  both visual channels are removed entirely. It does not depend on loom, on wall
  distance, or on the visual system at all.
- **(c) independent of it — the direct answer.** Its cause is the premotor wiring
  (§3); the room cannot supply it, since it is negative before the fly has moved.
- **(a) upstream of it** — in the causal sense. The read-out commands a
  **one-signed yaw moment**: `frac_tau_yaw_positive` is 0.9987 / 0.9946 / 0.9996
  and `mean_tilt_r > mean_tilt_l` (+0.2135 vs +0.1780 rad) at every seed, so the
  fly holds a standing turn of 1.1-1.2 rad/s and repeats its wall encounters. The
  wall-hugging is therefore **downstream** of the asymmetry, and the saturated
  loom is downstream of the wall-hugging. Fixing the standing turn would move the
  fly off the walls and un-saturate the loom; fixing the loom cannot touch the
  turn.

One honest caveat on the "upstream" claim: at this fly's speeds (166-511 mm/s
mean) in a 600 x 440 x 220 mm room, wall encounters are also partly geometric — a
perfectly straight flier would still reach a wall within a second or two. The
standing turn is upstream of the *persistence* of wall contact, not the sole
reason a wall is ever reached.

---

## 5. VERDICT — no fix, and why

**The one-signed steering differential is genuine connectome anatomy of the
MaleCNS specimen, not a construction or read-out artefact.** It is the network's
response to a near-symmetric drive, entering at the premotor-to-steering-MN
synapses as a +3 848-contact net residual of an almost bilaterally balanced
convergence, at the ordinary L/R magnitude of this specimen's motor system
(steering 1.13 vs 25-type median 1.12). It is present in the raw Janelia table,
reproduces across all three seeds, is unaffected by the visual system and the
room, and cannot be produced by the pipeline (which was checked for the swap,
the index error, the anchor and window mismatch that would mimic it).

**No code change is made here, and no balance term is added.** The options, in
order of defensibility:

1. **Accept and document it** (done). A single-specimen connectome has no
   symmetric-average alternative in this release; the asymmetry is a property of
   the animal whose wiring is being simulated.
2. **Bilateral-average the premotor-to-MN weights** if the project ever needs a
   straight-flying demo. That is a *construction choice about which animal is
   simulated*, not the repair of a defect, and it would have to be labelled as
   such — it is not justified by any measurement in this repo.
3. **Do not chase the loom response in this loop.** §4 and
   `steering-pool-gradedness.md` §5 together say the loom stimulus cannot vary
   while the fly is pinned to the walls, and the pinning is downstream of a
   specimen asymmetry that is not removable. A loom-response question should be
   asked with an open-loop probe (the `haltere-probe` pattern), where the
   stimulus is imposed rather than produced by the fly.
4. Adding a gain, offset or balance term to make the turn go away is the
   surrogate this project forbids, and it would hide the finding rather than
   answer it.

---

## 6. Multi-seed behaviour block (current tree, `analyze --seconds 12 --every 1`)

| | seed 7 | seed 11 | seed 23 | mean |
|---|---|---|---|---|
| takeoffs | 1 | 2 | 3 | 2.0 |
| mode % ground / takeoff / cruise | 87.0 / 2.2 / 10.8 | 41.8 / 7.3 / 50.9 | 59.8 / 23.3 / 17.0 | 62.9 / 10.9 / **26.2** |
| altitude mean / p95 / max, mm | 85.4 / 213.7 / 220.0 | 17.2 / 160.3 / 220.0 | 28.2 / 187.2 / 220.0 | 43.6 / 187.1 / 220.0 |
| path (horizontal), mm | 801.5 | 1160.6 | 1649.0 | 1204 |
| mean speed, mm/s | 510.6 | 165.7 | 340.6 | 339 |
| wall hits | 10 | 62 | 15 | 29 |
| **tortuosity** | 1.264 | 1.819 | 3.148 | **2.077** |
| % airborne turning > 0.5 rad/s | 77.0 | 82.0 | 92.6 | **83.9** |
| mean \|body yaw rate\|, rad/s | 2.02 | 1.78 | 1.33 | 1.71 |
| airborne samples within 20 mm of a wall | 62.1 % | 96.5 % | 94.6 % | 84.4 % |
| **steer differential, % negative (airborne)** | **100** | **100** | **100** | **100** |
| mean steer differential | -0.0683 | -0.0791 | -0.0696 | -0.0723 |
| mean \|steer differential\| | 0.0683 | 0.0791 | 0.0696 | 0.0723 |
| `flight_steer_l` / `flight_steer_r` (mean) | 0.4146 / 0.3404 | 0.4121 / 0.3332 | 0.4140 / 0.3395 | 0.4136 / 0.3377 |
| `flight_power_l` / `flight_power_r` (mean) | 0.5117 / 0.4484 | 0.5141 / 0.4407 | 0.5203 / 0.4516 | 0.5154 / 0.4469 |
| `frac_tau_yaw_positive` | 0.9987 | 0.9946 | 0.9996 | 0.9976 |
| steering read-out at rail | 0.0 % | 0.0 % | 0.0 % | 0.0 % |
| `corr(loom, steer differential)` | -0.417 | +0.098 | -0.019 | -0.113 |

Every value reproduces `steering-pool-gradedness.md` §4 (that note sampled
`--every 10`; this one samples every window, hence the small tortuosity
difference 2.077 vs 2.065). The `-0.417` at seed 7 against `+0.098`/`-0.019` at
11/23 confirms that note's conclusion that the seed-7 loom correlation was a
favourable draw.

`./build.sh test --release`: **39 passed, 0 failed**. No code was changed by this
investigation.

## 7. Reproduce

```sh
# per-seed sign distribution (full loop)
for s in 7 11 23; do ./build.sh run --release -- analyze --seconds 12 --seed $s --every 1 --out /tmp/fv/a$s; done
# awk the airborne fraction of steer_r-steer_l from /tmp/fv/a$s/trace.csv (mode 1/2/3)

# controls
FLYVERSE_NO_SENSE=1 ./build.sh run --release -- analyze --seconds 4 --seed 7 --every 1 --out /tmp/fv/nosense7
FLYVERSE_NO_RETINA=1 ./build.sh run --release -- analyze --seconds 6 --seed 7 --every 1 --out /tmp/fv/noretina7
FLYVERSE_NO_RETINA=1 FLYVERSE_NO_FLOW=1 ./build.sh run --release -- analyze --seconds 6 --seed 7 --every 1 --out /tmp/fv/vnconly7

./build.sh test --release     # 39
```

Anatomy above was measured directly from `official-pack/*.npy` (CSR) and, for the
source check, from `official-connectivity.feather` restricted to the 18 steering
body IDs.