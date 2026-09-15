# The loom-to-steering projection: source vs pack, and the per-eye retinotopic loom

Status: measurement note for the `FLYVERSE_LOOM_RETINOTOPIC` change. Default runs
are unchanged (see [Byte identity](#byte-identity)).

Three questions, in order:

1. Is the bilaterally balanced loom-to-steering projection real anatomy, or an
   artefact of the packed CSR? **It is real - the raw source gives the same
   numbers, synapse for synapse, at the hops that matter.**
2. Is the shared scalar loom the binding constraint on visual steering? **No.**
3. Can the visual steering loop be closed in this model at all? **Not by
   changing the input. See [Verdict](#verdict).**

---

## 1. The projection, checked against the raw source

`docs/room-size-and-loom-pathway.md` measured the loom-to-steering projection on
the shipped pack's CSR and found it bilaterally balanced to within a fraction of
a percent. A perfectly balanced projection is a priori suspicious - real
looming-sensitive neurons project to descending neurons with lateral specificity -
so the same walk was run on the **raw** Janelia table
(`official-connectivity.feather`, 151.8 M rows, all segments) and against the
pack, on identical hop semantics.

Hop semantics follow that doc: the seed is hop 0, and "hop `k`" counts synapses
landing on a steering pool *from nodes at hop `k`*, so hop-1 synapses are 2-hop
paths from the seed (which is what "reaches the steering MNs in 2 hops" means).
Seeds: the two `visual_loom` VP neurons, body ids `10142` (left) and `10723`
(right). Pools: the 9 + 9 flight-steering motor neurons.

### 1.1 Per-hop synapse counts

| source | seed | hop | shell | onto steer-left | onto steer-right | \|contacts\| onto pool |
|---|---|---|---|---|---|---|
| **raw** | L `10142` | 1 | 5 948 | 0 | 0 | 0 / 0 |
| | | 2 | 1 246 163 | **30** | **25** | **810 / 610** |
| | | 3 | 54 880 003 | 3 152 | 3 332 | 64 118 / 54 827 |
| **raw** | R `10723` | 1 | 5 581 | 0 | 0 | 0 / 0 |
| | | 2 | 1 289 926 | **25** | **25** | **661 / 693** |
| | | 3 | 55 058 133 | 3 324 | 3 469 | 63 026 / 58 067 |
| **pack** | L `10142` | 1 | 360 | 0 | 0 | 0 / 0 |
| | | 2 | 51 952 | **30** | **25** | **810 / 610** |
| | | 3 | 161 353 | 2 992 | 3 174 | 63 340 / 54 071 |
| **pack** | R `10723` | 1 | 346 | 0 | 0 | 0 / 0 |
| | | 2 | 61 833 | **25** | **25** | **661 / 693** |
| | | 3 | 162 458 | 3 158 | 3 309 | 62 067 / 57 175 |

The decisive row is hop 2: **raw and pack agree exactly** - 30/25 and 25/25
synapses, contact totals 810/610 and 661/693, i.e. the identical edge set with
the identical anatomical weights. The shell sizes differ (raw 5 948 vs pack 360
at hop 1) purely because the pack keeps only neurons with an annotated soma and a
settled transmitter sign - a filter on *who is in the graph*, not on *who projects
where*. At hop 3 the two agree to within ~1 % (raw 3 152/3 332 vs pack
2 992/3 174 from the left seed), the residual being that same filter.

### 1.2 Ipsilateral excess, per hop

`ip - co`, where `ip = L->steerL + R->steerR` and `co = L->steerR + R->steerL`:

| source | hop | edges ip / co / total | excess | contacts ip / co / total | excess |
|---|---|---|---|---|---|
| raw | 1 | 0 / 0 / 0 | - | 0 / 0 / 0 | - |
| raw | 2 | 55 / 50 / 105 | **+5 (+4.76 %)** | 1 503 / 1 271 / 2 774 | **+232 (+8.36 %)** |
| raw | 3 | 6 621 / 6 656 / 13 277 | -35 (-0.26 %) | 122 185 / 117 853 / 240 038 | +4 332 (+1.80 %) |
| pack | 2 | 55 / 50 / 105 | **+5 (+4.76 %)** | 1 503 / 1 271 / 2 774 | **+232 (+8.36 %)** |
| pack | 3 | 6 301 / 6 332 / 12 633 | -31 (-0.25 %) | 120 515 / 116 138 / 236 653 | +4 377 (+1.85 %) |

**Answer to (1): the near-balance is real, not a packing artefact.** The 2-hop
projection is byte-for-byte the same in the source and in the pack, and it is
genuinely almost - but not exactly - symmetric. The small residual is a modest
*ipsilateral* excess (+4.8 % by partner count, +8.4 % by contacts at 2 hops),
which is the wrong sign for a corrective looming reflex: a wall on the left drives
the left steering pool slightly more, which pushes the fly further into it. It is
also only a few percent, and it decays below 1 % one hop further out.

Sanity check at the pool level, same as the steering-differential investigation:
the raw source's total in-degree onto the pools is 8 610 edges / 115 358
|contacts| (left) and 9 603 / 103 420 (right) - ratio 1.115, exactly the figure
`docs/steering-differential-asymmetry.md` reports from the raw table, versus
5 716 / 5 971 edges in the pack. The pack drops edges uniformly; it does not
redistribute laterality.

---

## 2. What replacing the scalar loom is - and is not

The scalar loom is computed in `sim::World::loom` from the time-to-collision to
the nearest wall along the **body's forward axis** - one number - and `sense()`
set `d_loom_l.rate_hz = d_loom_r.rate_hz = loom * 60`. Both eyes' `visual_loom`
pools therefore received an identical stimulus in every window, at every value of
the room scale, so no lateral visual information existed anywhere in the loop.

That is not a physiological model of a looming-sensitive pathway; it is a
shortcut that discards the one quantity (left/right difference) the pathway
exists to compute. The fly already has a genuine lateral visual sense:
`src/vision.rs` builds 1462 optic-lobe columns and 5895 photoreceptors with
per-eye, per-direction geometry, and it is already wired into the network
(ablating it drops the mean network rate from ~20 Hz to ~17.2 Hz). What the model
lacks is any *use* of that signal for steering, because steering reads the scalar.

So:

- **Driving each eye's loom pool from that eye's own retinotopic depth is
  removing a shortcut.** The columns, their gaze directions and the raycast are
  the ones the retina already uses to drive the photoreceptors; `loom_by_eye`
  reads back the same per-column distances and applies the *same* TTC mapping
  (`1 - clamp(ttc/0.6, 0, 1)`, `ttc = dist / max(speed, 20 mm/s)`) that the scalar
  path already used. Nothing is added: no gain, no balance term, no stabiliser,
  no steering coefficient. The change *reduces* the information the loop invents
  (one shared number) to what the sense organ actually reports.
- **Adding any term whose purpose is to make the fly turn would be a surrogate.**
  None was added. The probe below is the test of whether the connectome's own
  projection can do the job once it is given a lateral input, and the answer is
  that it cannot - which is a result about the connectome, not something to be
  patched around.

## 3. Implementation

- `src/vision.rs`: `Retina` records `side` and `dist` per column (already
  raycast); new `loom_by_eye(speed)` returns the per-eye nearest-surface TTC
  loom, plus `set_imposed_lum` / `clear_imposed_lum` and per-eye readouts.
- `src/sim.rs`: `World::loom_retinotopic`, from `FLYVERSE_LOOM_RETINOTOPIC`
  (set, not read as a value). In `sense()`: imposed loom wins; else per-eye
  retinotopic when the flag is on and the retina is on; else the shipped scalar
  for both eyes. `set_loom_retinotopic()` for the probe.
- `src/analyze/loom.rs`: the probe gains `--retinal 1` (stimulate one eye's 678 /
  784 columns with additive luminance at the photoreceptor, with the closed loop
  on the per-eye loom - `mode = retinal-lateral`), `--levels`, and reports the
  lateral contrast by level; JSON records `loom_retinotopic_closed_loop`.
- `src/main.rs`: `loom-probe` options, `Args::has()`, new `loom-diag` subcommand
  (runs the shipped scalar and the per-eye loom from the same seed and reports
  the per-eye loom statistics).

### Byte identity

With the flag off the run is unchanged. `git archive HEAD` (the pre-change tree)
was built in `/tmp` and run head-to-head against the working tree:

```
analyze --seconds 4 --seed 7 --every 1
  pre-change tree (git archive HEAD)  md5 trace.csv = aba0efbdb6bdb5b29e365634a2290337
  working tree, flag off              md5 trace.csv = aba0efbdb6bdb5b29e365634a2290337
```

Identical - the shipped default is the pre-change default, byte for byte. Nothing
in the default path reads the new code.

### Does the per-eye signal actually carry laterality?

`loom-diag`, 12 s per seed, same seed, shipped vs per-eye:

| seed | shipped mean \|L-R\| | shipped samples \|L-R\|>0.05 | per-eye mean \|L-R\| | per-eye samples \|L-R\|>0.05 |
|---|---|---|---|---|
| 7 | 0.0000 | 0.0 % | 0.2924 | 37.2 % |
| 11 | 0.0000 | 0.0 % | 0.4307 | 51.6 % |
| 23 | 0.0000 | 0.0 % | 0.2832 | 34.6 % |

The shipped loop's differential is *exactly* zero by construction; the per-eye
loom gives the two eyes different values in a third to a half of all samples.
The lateral signal now exists. The question is whether it steers.

---

## 4. The lateral probe

`loom-probe --retinal 1 --levels 0.25,0.5,1 --trials 10 --warmup 10
--interval 0.25 --response 200`, with `FLYVERSE_LOOM_RETINOTROPIC=1` so the
closed loop is per-eye throughout. One eye's photoreceptors get an additive
luminance step; the other eye is untouched. 7 conditions x 10 trials = 70 trials
per seed, 54 stimulated-motor comparisons.

### 4.1 The stimulus lands (positive control)

Left-eye stimulation, drive in Hz at the photoreceptor (mean over trials):

| seed | level | driveL (t) | driveR (t) |
|---|---|---|---|
| 7 | 0.25 | 81.6 (+9.1) | 32.9 (+0.1) |
| 7 | 0.50 | 118.8 (+14.3) | 30.6 (+1.4) |
| 7 | 1.00 | 180.0 (+51.2) | 30.9 (+1.1) |
| 11 | 0.25 | 79.8 (+8.3) | 31.9 (+0.3) |
| 11 | 0.50 | 116.2 (+12.6) | 31.0 (+0.8) |
| 11 | 1.00 | 180.0 (+39.3) | 31.4 (+0.6) |
| 23 | 0.25 | 80.0 (+5.7) | 36.1 (+0.3) |
| 23 | 0.50 | 119.5 (+12.9) | 31.1 (+1.8) |
| 23 | 1.00 | 180.0 (+29.1) | 31.7 (+1.5) |

The stimulated eye's drive rises monotonically with the imposed level, from a
null of ~38-43 Hz to the 180 Hz ceiling, at |t| up to 51; the unstimulated eye
stays flat (|t| <= 1.8). Mirror-symmetric for right-eye stimulation. The lateral
stimulus is delivered and it is genuinely lateral.

### 4.2 The motor output does not answer

Steering differential (R-L, the pool asymmetry) and its t against the null, per
seed, over the whole condition table:

| seed | null steer diff | null-vs-null floor | max \|t\| on any motor channel | reached its own floor? |
|---|---|---|---|---|
| 7 | -0.0837 | 2.43 | 1.67 (left eye 0.50 / steer left) | **no** |
| 11 | -0.0737 | 1.74 | 1.47 (symmetric 0.25 / steer right) | **no** |
| 23 | -0.0791 | 3.00 | 2.51 (symmetric 1.00 / lift-weight) | **no** |

Every stimulated condition leaves the steering differential at the null value
(-0.068 ... -0.083, i.e. 100 % negative as in the closed loop); the largest |t| on
the differential is 1.52 / 0.80 / 1.60. In all three seeds the biggest excursion
of *any* of the six motor channels is below the floor the null reaches against
itself, so nothing here is distinguishable from chance - and none of it is
direction-selective, since the largest effects sit on `lift / weight` and
`|yaw rate|`, not on the differential.

**Correct-sign check.** The corrective prediction is that a stimulus on the left
eye steers the other way: `diff = steer_diff(L stimulated) - steer_diff(R
stimulated)` should have one reproducible sign and grow with level.

| seed | level 0.25 | level 0.50 | level 1.00 | monotone? |
|---|---|---|---|---|
| 7 | -0.00699 (t 1.44) | +0.00172 (t 0.31) | +0.00334 (t 0.58) | no, sign flips |
| 11 | -0.00019 (t 0.04) | +0.00501 (t 0.81) | -0.00294 (t 0.54) | no, sign flips |
| 23 | +0.00773 (t 1.64) | -0.00280 (t 0.64) | -0.00637 (t 1.19) | no, sign flips |

The sign is not reproducible across seeds at any single level (seed 7 and seed 23
disagree at level 0.25, the only level where either is anywhere near its floor),
it flips between levels within every seed, and |t| never exceeds 1.64 - below the
*smallest* measured floor (1.74). **There is no dose-response and no correct
sign.**

### 4.3 Window bound

The 200 ms window bounds only what happens within 200 ms. The same probe with a
400 ms response window (seed 7, levels 0.5 and 1.0, 8 trials, 56 trials):

- stimulus still lands: driveL t = 23.6 (0.50) and 51.8 (1.00); driveR t = 108.9
  and 243.3 when the right eye is stimulated;
- steering differential: |t| <= 1.58, largest |t| of any stimulated condition
  1.92 against a null-vs-null floor of 2.01;
- lateral contrast: +0.0062 (t 1.16) at 0.50 and +0.0080 (t 1.41) at 1.00 - both
  the same sign as the null's own -0.08, i.e. not corrective, and below floor.

Doubling the window does not produce a response. The flatness is not a latency
artefact of the window.

---

## 5. Closed-loop behaviour, before and after

`analyze --seconds 12`, seeds 7 / 11 / 23. "before" = shipped scalar loom,
"after" = `FLYVERSE_LOOM_RETINOTOPIC=1`; the "before" column reproduces
`docs/steering-differential-asymmetry.md` section 6 exactly, confirming the
baseline is the shipped tree.

| seed | | before | after |
|---|---|---|---|
| 7 | cruise % / air samples | 10.8 / 781 | 84.8 / 5213 |
| | path mm / speed mm/s | 802 / 511 | 1289 / 124 |
| | tortuosity | 1.264 | 2.026 |
| | wall hits | 10 | 74 |
| | mean steer differential | -0.0683 | -0.0751 |
| | steer differential negative | 100 % | **100 %** |
| | corr(loom, steer diff) | -0.417 | +0.013 |
| 11 | cruise % / air samples | 50.9 / 3493 | 9.4 / 4451 |
| | path mm / speed mm/s | 1161 / 166 | 1206 / 135 |
| | tortuosity | 1.819 | 1.889 |
| | wall hits | 62 | 75 |
| | mean steer differential | -0.0791 | -0.0780 |
| | steer differential negative | 100 % | **100 %** |
| | corr(loom, steer diff) | +0.098 | +0.092 |
| 23 | cruise % / air samples | 17.0 / 2413 | 17.4 / 5667 |
| | path mm / speed mm/s | 1649 / 341 | 1212 / 107 |
| | tortuosity | 3.148 | 1.925 |
| | wall hits | 15 | 108 |
| | mean steer differential | -0.0696 | -0.0758 |
| | steer differential negative | 100 % | **100 %** |
| | corr(loom, steer diff) | -0.019 | +0.098 |

What changes: the fly is airborne far more (the per-eye loom drives the loom
pools harder on average, so it flies instead of sitting), and it therefore hits
walls more.

What does **not** change: the steering differential. It is still negative in 100 %
of samples in all three seeds, its mean is still -0.075 ... -0.078 (before:
-0.068 ... -0.079), and the correlation between the loom and the steering
differential is still ~0 (+0.013 / +0.092 / +0.098 against -0.417 / +0.098 /
-0.019). The one seed-7 correlation that prompted the original hypothesis does not
come back - it was chance, as the earlier failed replication already suggested.

**The circling is not reduced.** Giving the loop a genuine lateral visual signal
leaves the direction of the steering command exactly as it was.

---

## Verdict

- **(b) is real anatomy, not a packing artefact.** The raw source and the pack
  give the same loom-to-steering edges with the same weights at the hops that
  matter (30/25 and 25/25 synapses, |contacts| 810/610 and 661/693), and the same
  near-balance: +4.8 % ipsilateral by partner count, +8.4 % by contacts at 2 hops,
  decaying to ~0 within three. The pack filters neurons out of the graph; it does
  not flatten laterality. (Pool-level check: raw L/R |contact| ratio 1.115, the
  same as the steering-differential investigation found.)
- **The scalar loom was a real defect, and it is now gone** - behind a flag, with
  the default byte-identical - but **it was not the binding constraint.** With a
  genuinely lateral stimulus delivered at the photoreceptor (t up to 51 on the
  driven eye), with the closed loop on the per-eye loom, the steering
  differential does not move: no condition in any of three seeds reaches its own
  null floor, the lateral contrast flips sign between levels and between seeds,
  and there is no dose-response at 200 ms or at 400 ms.
- **The binding constraint is the projection, not the input.** A channel that is
  bilaterally balanced to within a few percent - and, where it is not, biased
  slightly *ipsilaterally*, i.e. the wrong way for a corrective reflex - cannot
  turn a lateral sensory difference into a lateral motor command no matter how
  good the input is. That is the structural prediction of section 1, and section 4
  is it happening.
- **The visual steering loop cannot be closed in this model.** Not by fixing the
  input: the input is now correct and the output is unchanged. The fly's turning
  is the specimen's own one-signed steering command (left pool 1.13x the right's
  contacts), and the only sense that could correct a lateral error reaches the
  steering output symmetrically. Circling is not fixable by the visual pathway in
  this connectome; anything that did fix it - a steering gain, a balance term, a
  corrective projection - would be a surrogate, not this animal.