# Colour vision, a fruit in the room, and the retinotopic default

This note records what the retina can and cannot see, what was added to it and
why, and what happened to the fly's behaviour when a fruit was put in the room.
The measurement is reported as it came out, including the part that is a null.

Everything here is measured by the shipped binary. The reproduction commands are
at the end.

---

## 0. Reconnaissance: what the retina actually did

Before this change, `src/vision.rs` was 562 lines and was luminance-only.
Confirmed by reading the file at `HEAD` and grepping it for every colour term:

```
git show HEAD:src/vision.rs | grep -nE 'colou|color|spectr|wavelength|R7|R8|opsin'
```

The only hits are the word "luminance" in a doc comment and the luminance
function itself. There is **no colour, spectral, RGB or wavelength code
anywhere** in the previous retina.

### How one photoreceptor's input was computed

`Retina::update`, former `src/vision.rs:186-207`:

1. `src/vision.rs:191` — the eye's origin is the body position plus `EYE_OFF`
   rotated by the body quaternion. (`EYE_OFF` is `[[1.0, 0.32, 0.12],
   [1.0, -0.32, 0.12]]` mm: just behind the head, 0.32 mm either side.)
2. `src/vision.rs:192` — the ray direction is the column's fixed gaze vector,
   also rotated by the body quaternion.
3. `src/vision.rs:193-198` — `scene_hit(eye, dir, room)` is a plain raycast:
   six walls, then the table box. It returns the hit point `p` and the face
   normal `n`. A ray that escapes returns luminance 0.
4. `src/vision.rs:194` — the sampled scalar is `luminance(p, n)`.
5. `src/vision.rs:205` — that scalar becomes the ONE drive rate for the whole
   column: `d.rate_hz = BASE_HZ + GAIN_HZ * lum`, with `BASE_HZ = 30.0` and
   `GAIN_HZ = 150.0` (`vision.rs:61-62`).

The key point is what `luminance` was, former `src/vision.rs:448-453`:

```rust
pub fn luminance(p: V3, n: V3) -> f32 {
    let seed = face_seed(n);
    let a = vnoise(p[0] / 3.0, p[1] / 3.0, p[2] / 3.0, seed);
    let b = vnoise(p[0] / 1.1, p[1] / 1.1, p[2] / 1.1, seed ^ 0x9E37_79B9);
    (0.65 * a + 0.35 * b).clamp(0.0, 1.0)
}
```

It is a **fixed value-noise texture keyed by world position and by which face
was hit**. The surface has no albedo and no spectrum; the scene's appearance was
a procedural pattern, not a property of any object. One `Drive` (a Poisson
process) is built per optic-lobe column and every photoreceptor in that column
receives it identically, so the retina was a luminance camera with no lateral
colour and no per-cell identity.

### What the MaleCNS pack has for photoreceptors

**It distinguishes them, and the annotation is the type, not the receptor.**

The 5895 photoreceptor cells the retinotopy asset drives were matched against
`/opt/data/workspaces/skg/flybrain/annotations.feather`. All 5895 match, and
their `type` column reads:

| annotation `type` | cells | share |
|---|---:|---:|
| `R1-R6` | 3344 | 56.7 % |
| `R8y` | 481 | 8.2 % |
| `R7y` | 469 | 8.0 % |
| `R8_unclear` | 436 | 7.4 % |
| `R7_unclear` | 350 | 5.9 % |
| `R8p` | 330 | 5.6 % |
| `R7p` | 296 | 5.0 % |
| `R7d` | 80 | 1.4 % |
| `R8d` | 75 | 1.3 % |
| `R7R8_unclear` | 34 | 0.6 % |

So the pack does carry R7/R8 and the pale/yellow split: **1576 cells (26.7 %)
have a resolved colour opsin**. A further 820 are subtype-unresolved and 155
are dorsal-rim (`d`) cells. Note that `annotations.feather`'s `receptorType`
column is *not* the source — it holds only olfactory and gustatory receptors
(`putative_IR52b`, `putative_ppk23`, `putative_ppk25`). The photoreceptor
classes come from `type`.

### The scene, before and after

`src/room.rs` still has everything it had:

- the **table** (`ROOM.table`, `x [40, 280]`, `y [-160, 120]`, top 40 mm, 12 mm
  thick) — unchanged;
- the **sugar cube** and hence `food_home()`, at `(160, 40, 44)` — unchanged;
- the **odour field**, `Room::odor`, with its upwind shift (`wind`, `odor_amp`,
  `odor_lambda`) — unchanged.

**The odour field is live, not dead code.** `World::sense`
(`src/sim.rs:659-676`) samples it at the two antennae positions and writes the
result into the `olfaction_left` / `olfaction_right` drives at 120 Hz times
concentration. `FLYVERSE_NO_ODOR=1` silences it. See §5.

The fruit is new (§2).

---

## 1. Colour vision, grounded

The retina now has a real spectrum. Two things were added, both derived from
data rather than invented.

### The pigments

Each spectral class is a rhodopsin alpha-band built from the Govardovskii et al.
(2000) template (*Vis Neurosci* 17(4):509-528), evaluated on 41 bands from 300
to 700 nm at 10 nm spacing, with the measured peak:

| class | opsin | λmax | annotation `type` |
|---|---|---:|---|
| `R16` | Rh1 | 478 nm | `R1-R6` |
| `R7p` | Rh3 | 345 nm | `R7p` |
| `R7y` | Rh4 | 375 nm | `R7y` |
| `R8p` | Rh5 | 437 nm | `R8p` |
| `R8y` | Rh6 | 508 nm | `R8y` |
| `R7u` | mean(Rh3, Rh4) | — | `R7_unclear`, `R7R8_unclear`, `R7d` |
| `R8u` | mean(Rh5, Rh6) | — | `R8_unclear`, `R8d` |

λmax sources: Salcedo et al. (1999) *J Neurosci* 19:10716-10726, cross-checked
against Shakir et al. (2020) *Sci Rep* 10:17488; with Feiler et al. (1988)
*Nature* 333:737-741, Feiler et al. (1992) *J Neurosci* 12:3862-3868 and Hardie
(1986) *Trends Neurosci* 9:419-423.

The mapping from annotation `type` to class is in
`scripts/derive_spectral_classes.py`, and the per-cell result is baked into
`assets/male_cns_v1_spectral_classes.json` (5895 entries, 0 cross-check
mismatches). The retina loads that asset and partitions the drives by it, so
which cells are chromatic is the annotation's statement, not this model's.

**Nothing was invented.** The two `u` classes are explicitly *not* given a
guessed subtype: they get the mean of the two pigments their identity could
carry, which is the expected sensitivity of a cell whose subtype is unknown
because pale and yellow ommatidia are ~50:50 over the retina. `R7d`/`R8d` are
dorsal-rim cells — a polarisation-specialised pair, not a colour pair — and this
model has no polarisation channel, so they are left with the unresolved classes
rather than given a colour opsin they are not known to carry. 155 cells, 2.6 %.

### The illuminant

The room has no lamp, so the illuminant is stated explicitly rather than
smuggled in: the CIE equal-energy reference stimulus E, radiance 1 at every
wavelength. All spectral shaping then comes from the opsins and the surfaces'
reflectance. Every number below is stated against it.

### The check: does a coloured fruit differ from an equi-luminant grey one?

This is the test that colour information exists in the retina at all.
`vision::tests::a_coloured_fruit_differs_from_an_equi_luminant_grey_one` takes
the fruit's measured spectrum and a spectrally flat grey chosen to have the
*same R1-R6 (Rh1) catch*, then compares the per-class catches:

```
  R16  catch  0.06296  grey  0.06296  ratio 1.000
  R7p  catch  0.05878  grey  0.06296  ratio 0.934
  R7y  catch  0.06263  grey  0.06296  ratio 0.995
  R8p  catch  0.05905  grey  0.06296  ratio 0.938
  R8y  catch  0.08081  grey  0.06296  ratio 1.284
  chromatic ratio spread: 0.934 .. 1.284
```

The luminance channel is identical by construction (ratio 1.000), and the
chromatic channels are not: the Rh6/508 nm channel sees the fruit **28.4 %
brighter** than the grey, while Rh3/345 nm and Rh5/437 nm see it **6-7 %
dimmer**. The tests assert exactly this signature, that the green channel is the
one that gains, and that the red edge is invisible to this eye (no class peaks
beyond 508 nm).

**Colour information therefore exists in the retina, and it is modest — for a
reason worth stating plainly.** Every fly opsin peaks at or below 508 nm. A ripe
tomato's most conspicuous feature is its red edge, rising from ~0.09 at 550 nm
to ~0.6 above 650 nm — and that entire rise is outside this animal's spectral
range. What survives is the small green/blue difference above. To a fly, a ripe
tomato is a slightly greenish dark object, not a red one. That is a property of
*Drosophila* vision, not of the model.

---

## 2. The fruit

`room::Fruit` is a sphere, `FRUIT = { c: [240, -130, 65], r: 25 }` — 50 mm
across, resting on the table in the corner diagonally opposite the sugar cube,
so the visual object and the odour source are two different places in the room.

It is a **scene object and nothing else**:

- the optics ray-cast against it exactly as they do against the walls and the
  table (`scene_hit` tests the sphere, and the sphere occludes what is behind
  it);
- it has a reflectance spectrum, and that spectrum is what the renderer
  returns for a hit;
- there is **no collision, no taste channel, no reward, no food value, and no
  signal injected into the network** for it. The banner says so, and the code
  does nothing else with it. `food_home()` still points at the sugar cube.

The reflectance is the measured shape of a ripe red tomato: under 0.1 from 400
to 575 nm with an absorption minimum near 450-475 nm, a sharp rise through the
red edge above 560-590 nm, and a peak between 650 and 705 nm. Sources are
recorded at `FRUIT_REFLECTANCE_ANCHORS`: ElMasry & Sun and Ciaccheri et al.
(2018) as reported in PMC9274195 ("reflectance value is below 0.1 from 400 to
575 nm ... above 560 nm, reflectance values rose sharply because of the red
coloration of ripened fruits") and the reported 652 nm peak.

Two environment flags, used only by the measurement:

- `FLYVERSE_NO_FRUIT=1` removes it. Same room, same table, same sugar cube, same
  odour field — the clear-room control.
- `FLYVERSE_FRUIT_GREY=1` renders it with the equi-luminant flat reflectance from
  §1 instead of its colour. The luminance channel cannot tell this apart from
  the coloured fruit, so any behavioural difference between the two is a *colour*
  difference. (Measured: they differ, but by ~0.0005 in the retinal signal and
  by 1-3 spikes in the network — see §4.)

---

## 3. The faithful per-eye retinotopic path is now the default

It was backwards. The twice-a-window shared scalar `loom()` casts one ray along
the body's forward axis and delivers the *same number* to both `visual_loom`
pools, so the bilateral visual difference is **identically 0.0000** — the
lateral visual information is not merely small, it is exactly zero by
construction. Per-eye retinotopic loom input is the faithful one, and it sat
behind `FLYVERSE_LOOM_RETINOTOPIC`, off.

**Both defaults, explicitly:**

| | before | after |
|---|---|---|
| retinotopic per-eye loom | off (flag had to be set) | **on** |
| shared scalar loom | **on by default** | off, via `FLYVERSE_LOOM_SCALAR=1` |

So the default run *does* change, and the flag has been inverted rather than
merely added. There was no concrete reason to keep the old default: the scalar
path is the less faithful of the two in exactly the terms the project cares
about (it destroys laterality), and it is still reachable for A/B work.

Confirmed in the runs: the summary's `fruit.visual_drive.loom_retinotopic` is
`true` for all three default runs and `false` for all three
`FLYVERSE_LOOM_SCALAR=1` runs.

**And the two paths measured side by side** (`loom-diag`, 12 s, same seed both
ways, with the new spectral retina in both):

| seed | scalar `mean |L−R|` | scalar samples >0.05 | per-eye `mean |L−R|` | per-eye samples >0.05 |
|---:|---:|---:|---:|---:|
| 7 | **0.0000** | 0.0 % | 0.0186 | 2.4 % |
| 11 | **0.0000** | 0.0 % | 0.3448 | 40.5 % |
| 23 | **0.0000** | 0.0 % | 0.6287 | 68.5 % |

The scalar path's bilateral difference is *exactly* zero in every seed — not
approximately, identically, because it is one number written twice. The per-eye
path carries a real bilateral difference, whose size varies a lot by seed
(0.019 to 0.63) because how much laterality a run happens to contain depends on
where that fly flew. The old default therefore destroyed the lateral visual
signal by construction, and the new default does not.

---

## 4. Measurement: does the fly get to the fruit?

Twelve-second runs, seeds 7 / 11 / 23, five configurations: the new default with
the colour fruit, the same with the fruit grey, the same with no fruit at all,
and the old scalar loom with and without the fruit.

The metric is the horizontal distance from the body to the fruit's **place**,
`FRUIT.c`, not to `room.fruit` — it is measured whether or not the object is
rendered, which is what makes the control comparable at all. The bearing is
signed against the body's horizontal heading, positive to the left.

### The result: the fruit moves nothing

Not "little". Nothing.

| comparison | seeds | columns that differ | body/motor columns |
|---|---|---|---|
| colour fruit vs **no fruit** | 7, 11, 23 | 6 of 68 | **none** |
| colour fruit vs **grey fruit** | 7, 11, 23 | 2-5 of 68 | **none** |
| grey fruit vs no fruit | 7, 11, 23 | 6 of 68 | **none** |
| colour fruit vs no fruit, old scalar loom | 7, 11, 23 | 6 of 68 | **none** |

Every body and motor column — `x, y, z, speed, yaw, yaw_rate, roll, pitch,
wing_amp, mode, pow_l, pow_r, steer_l, steer_r, walk_*, land_*` — is
**bit-identical** across all of those pairs, in all three seeds. The fly's
trajectory does not change when a fruit is put in the room, when it is coloured,
or when it is removed.

What *does* change is upstream of the body:

| seed | columns on fruit (max) | max Δ luminance | max Δ UV catch | max Δ green catch | Δ total spikes |
|---:|---:|---:|---:|---:|---:|
| 7 | 44 | 0.0288 | 0.0205 | 0.0226 | 466 |
| 11 | 11 | 0.0059 | 0.0020 | 0.0030 | 27 |
| 23 | 42 | 0.0265 | 0.0198 | 0.0239 | 472 |

So the fruit **is** seen and **does** reach the network — hundreds of extra
spikes fire — and it still moves no motor neuron and no millimetre of the body.

### Distance to the fruit, and the control

| config | seed | first | last | min | change | % samples closer than start |
|---|---:|---:|---:|---:|---:|---:|
| colour fruit | 7 | 460.4 | 76.4 | 60.0 | **+384.0** | 99.9 |
| colour fruit | 11 | 460.4 | 349.7 | 132.6 | **+110.8** | 99.9 |
| colour fruit | 23 | 460.4 | 355.0 | 60.0 | **+105.5** | 99.9 |
| no fruit | 7 | 460.4 | 76.4 | 60.0 | **+384.0** | 99.9 |
| no fruit | 11 | 460.4 | 349.7 | 132.6 | **+110.8** | 99.9 |
| no fruit | 23 | 460.4 | 355.0 | 60.0 | **+105.5** | 99.9 |
| grey fruit | 7 | 460.4 | 76.4 | 60.0 | **+384.0** | 99.9 |

The fly does get closer to the fruit's place over 12 s — by 384 mm in seed 7 —
and it gets closer by **exactly the same amount with no fruit in the room**.
The difference attributable to the fruit is **+0.0 mm in every seed**, to the
millimetre and to the bit. The % column is a trap worth naming: "99.9 % of
samples are closer than the start" is true of the no-fruit control too, because
any run that drifts in one direction produces it. It measures the fly wandering,
not the fly approaching.

### Was the fruit actually visible? Yes, while airborne

| seed | % of all samples with the fruit in view | % of **airborne** samples | mean columns on it when seen |
|---:|---:|---:|---:|
| 7 | 3.23 % | **34.34 %** | 8.4 |
| 11 | 0.25 % | 0.45 % | 5.7 |
| 23 | 3.42 % | 5.32 % | 5.5 |

The test is not vacuous: in seed 7 the fly had the fruit on its retina while
airborne for a third of the time it spent in the air. Seed 11 barely sees it
because that fly spends most of its time grounded at the far end of the room.
The occlusion is real and geometric — a fly on the floor at the spawn corner
looks at the fruit across the table's 40 mm rim, and the ray passes ~38 mm up,
under the top.

### The sign test, and why it does not carry the weight it looks like it does

"Turn toward" is defined as the steering command reducing the bearing error:
`fruit_az > 0` (fruit to the left) with `steer_l > steer_r` (a left steer).

| config | seed | airborne | % steering toward | p | % yawing toward | mean cos(bearing error) |
|---|---:|---:|---:|---:|---:|---:|
| colour | 7 | 565 | 100.0 | <1e-4 | 99.8 | +0.361 |
| colour | 11 | 3358 | 45.7 | <1e-4 | 43.0 | −0.871 |
| colour | 23 | 3851 | 62.6 | <1e-4 | 59.5 | −0.550 |
| no fruit | 7 | 565 | 100.0 | <1e-4 | 99.8 | +0.361 |
| no fruit | 11 | 3358 | 45.7 | <1e-4 | 43.0 | −0.871 |
| no fruit | 23 | 3851 | 62.6 | <1e-4 | 59.5 | −0.550 |

The numbers are **identical to the no-fruit control in every seed**, which
follows from the bit-identical trajectories and is the real content of this
table: the sign test measures where the fruit happens to sit relative to a
fly that is turning anyway, not a response to it. The tiny p values are an
artefact of n — they say the fly's steering is not a fair coin, which was
already known.

Two further cautions, both worth recording:

1. **The steering differential is one-signed, and this is robust.** 100 % of
   airborne samples have `steer_r - steer_l < 0` in all three seeds (means
   −0.087 / −0.082 / −0.076). That is the specimen anatomy the steering note
   established, and it survives the new visual front end unchanged.
2. **The yaw direction is *not* one-signed.** Mean airborne yaw rate is +1.74 /
   −0.98 / −0.11 rad/s as seeds 7 / 11 / 23. That was already true before this
   change (−0.13 / −0.51 / +0.80) and in the scalar configuration (−0.81 / +0.41
   / −0.41). The one-signed claim is about the steering command, not about the
   resulting heading, and the sign test should not be read as though the fly
   were turning consistently one way. Consistently,
   `corr(steer R-L, yaw_rate)` is −0.47 / −0.09 / +0.13: the differential is
   almost constant, so its correlation with yaw is dominated by small
   fluctuations, and it does not replicate in sign either.

### The visual drive against the steering differential, per seed

Lateral luminance `lum_l - lum_r` and lateral chromatic contrast
`(uv_l - uv_r) - (gr_l - gr_r)`, against `steer_r - steer_l`, on airborne
samples:

| config | seed | corr(lum, steer) | corr(chroma, steer) | sd lum L−R | sd chroma L−R |
|---|---:|---:|---:|---:|---:|
| colour | 7 | −0.191 | +0.151 | 0.1374 | 0.0070 |
| colour | 11 | +0.014 | +0.033 | 0.1636 | 0.0043 |
| colour | 23 | −0.234 | −0.012 | 0.1638 | 0.0070 |

**Spread, not a pooled number:**

- luminance: mean −0.137, spread **−0.234 .. +0.014** across 3 seeds
- chromatic: mean +0.057, spread **−0.012 .. +0.151** across 3 seeds

The luminance correlation changes sign between seeds and the chromatic one is
within noise of zero throughout. Neither replicates. This is the same
non-replication the earlier work found (a seed-7 value of −0.42 that did not
reappear), and it is the second piece of evidence that the visual drive does not
reach the steering pool as a usable signal.

Note also how small the chromatic drive is: its standard deviation is 0.004-0.007
against 0.14-0.16 for luminance, i.e. **~4 % of the lateral luminance signal**.
That is the quantitative reason a colour-specific behavioural effect was never
likely: the colour channel carries a real but tiny fraction of what the eye
already fails to use.

### Behaviour block: before vs after

The visual front end changed, so flight changed. `before` is the shipped
defaults (luminance retina, scalar loom); `after` is this change; `scalar` is
the same retina with the old loom.

| seed | config | takeoffs | cruise % | ground % | alt p95 mm | speed mm/s | path mm | tortuosity | wall hits | turning > 0.5 % | \|steer\| | corr(loom,steer) |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 7 | before | 1 | 10.8 | 87.0 | 213.7 | 510.6 | 1060.5 | 1.26 | 10 | 77.0 | 0.0683 | −0.417 |
| 7 | **after** | 1 | 7.0 | 90.6 | 213.4 | 593.6 | 848.2 | 1.28 | 4 | 99.6 | 0.0870 | +0.299 |
| 7 | scalar | 2 | 9.7 | 60.6 | 192.6 | 177.8 | 1120.4 | 1.33 | 41 | 85.7 | 0.0736 | −0.169 |
| 11 | before | 2 | 50.9 | 41.8 | 160.3 | 165.7 | 1438.5 | 1.82 | 62 | 82.0 | 0.0791 | +0.098 |
| 11 | **after** | 2 | 23.1 | 44.0 | 105.3 | 137.8 | 1099.5 | 1.46 | 71 | 86.5 | 0.0815 | −0.169 |
| 11 | scalar | 2 | 43.5 | 50.6 | 186.4 | 339.0 | 2298.6 | 3.15 | 28 | 92.8 | 0.0779 | −0.116 |
| 23 | before | 3 | 17.0 | 59.8 | 187.2 | 340.6 | 1958.4 | 3.15 | 15 | 92.6 | 0.0696 | −0.019 |
| 23 | **after** | 3 | 15.3 | 35.8 | 82.9 | 260.8 | 2100.6 | 3.16 | 38 | 85.4 | 0.0764 | +0.143 |
| 23 | scalar | 2 | 14.2 | 42.3 | 169.1 | 161.8 | 1314.5 | 1.77 | 49 | 85.9 | 0.0741 | −0.102 |

Takeoffs are stable (1 / 2 / 3). Cruise fraction, altitude and wall hits all
move, in both directions, and the direction differs by seed: seed 7 cruises less
and hits fewer walls, seed 11 cruises less and flies lower, seed 23 flies lower
and hits more walls. The one consistent reading is that the retina now delivers
a real signal — mean luminance over the run rises from 0.045 / 0.000 / 0.007
(dark, and mostly zero) to 0.266 / 0.159 / 0.000 — and the fly's flight is
correspondingly different. That is expected; it is why the before/after block is
reported rather than assumed.

The `corr(loom, steer)` column is the closed-loop loom test. It is not a stable
number in any configuration (before: −0.417 / +0.098 / −0.019; after: +0.299 /
−0.169 / +0.143) and it has the same non-replication problem as the visual-drive
correlations above.

---

## 5. Does the odour field reach the steering pathway?

The odour field already existed and reaches the network at 120 Hz times
concentration on the two `olfaction_left` / `olfaction_right` pools (1247 and
798 cells respectively, from `assets/male_cns_v1_neural_io.json`). So this is
the natural second route to a fruit. Two measurements, neither of which builds
anything.

### Anatomy: how far is it from the olfactory pools to the steering pool?

Breadth-first over the shipped pack exactly as the sim uses it (166 700 neurons,
24 559 135 edges; `official-pack/row_ptr.npy` etc.). Targets are the flight
steering motor pool (`motor_flight_steering_left/right`, 9 cells each) and the
pre-motor steering descending pool (`flight_steering_dn_left/right`, 6 each).

| source | target | hop 1 | hop 2 | hop 3 |
|---|---|---:|---:|---:|
| olfaction_left (798 cells) | motor_flight_steering_left | 0 | **5 of 9** | +4 of 9 |
| olfaction_left | flight_steering_dn_left | 0 | 2 of 6 | +4 of 6 |
| visual_loom_left (1 cell) | motor_flight_steering_left | 0 | **8 of 9** | +1 of 9 |
| visual_loom_left | flight_steering_dn_left | 1 of 6 | +5 of 6 | — |

There are **no direct (hop-1) contacts** from either olfactory pool to either
steering pool: 0 excitatory and 0 inhibitory contacts onto all four target
pools from all four source pools.

Read this carefully, because the hop-2 numbers look more impressive than they
are. At hop 2 the olfactory frontier contains 29 868 cells; if the 9 target
cells were scattered at random among the 166 700, a frontier that size would
expect to meet 9 × 29868/166700 ≈ 1.6 of them. Meeting 5 is about 3× chance.
The visual loom's hop-2 frontier is 51 591 cells, expecting ≈ 2.8 of the 9 and
meeting 8, also ~3× chance. With 24.5 M edges and a mean out-degree of ~147,
almost anything reaches almost everything in 2-3 hops. So the honest reading is:

- the odour channel is **not excluded** from the steering pathway — it is
  polysynaptically connected to it within 2 hops, structurally at least as close
  as the visual loom channel that was already shown to have no measurable effect;
- but there is **no dedicated or enriched olfactory→steering projection** in
  this connectome: 5/9 at 3× chance is a diffuse route through ~30 000
  intermediate cells, not a labelled line. Which is what fly neuroanatomy
  predicts — odour information reaches steering behaviour through the antennal
  lobe, mushroom body / lateral horn and central complex, not by a direct
  projection to wing-steering motor neurons.

### Open loop: does an odour gradient move the steering at all?

`loom-probe --odour` imposes an odour concentration at each antenna and drives
the two `olfaction` pools at 120 Hz times concentration — the same mapping
`World::sense` uses — and measures the steering differential and yaw response
against a null control by Welch t, with the measured null-vs-null floor. Four
concentrations (0.25 / 0.5 / 1.0 / 2.0, i.e. 30 / 60 / 120 / 240 Hz per pool;
the top one is deliberately above the 120 Hz physiological ceiling for a
dose-response), both signs, 8 trials per condition, 200 ms response window,
seeds 7 / 11 / 23.

**Positive control — the stimulus does arrive.** The imposed drive is delivered
exactly as specified, to the right pool, in every seed and every condition:

```
left antenna  +0.25 / +0.50 / +1.00 / +2.00   ->  driveL 30 / 60 / 120 / 240 Hz,  driveR 0
right antenna +0.25 / +0.50 / +1.00 / +2.00   ->  driveL  0,                     driveR 30 / 60 / 120 / 240 Hz
```

**The lateral gradient, which is the real test.** Stimulating one antenna
against the other and comparing the two conditions directly:

| seed | level (odor Hz) | L: steerR−L | R: steerR−L | difference | t |
|---:|---:|---:|---:|---:|---:|
| 7 | 0.25 (30) | −0.0855 | −0.0728 | −0.0127 | 2.54 |
| 7 | 0.50 (60) | −0.0754 | −0.0815 | +0.0061 | 0.81 |
| 7 | 1.00 (120) | −0.0817 | −0.0840 | +0.0023 | 0.38 |
| 7 | 2.00 (240) | −0.0750 | −0.0875 | +0.0125 | 1.74 |
| 11 | 0.25 (30) | −0.0716 | −0.0835 | +0.0119 | 1.81 |
| 11 | 0.50 (60) | −0.0802 | −0.0846 | +0.0045 | 0.65 |
| 11 | 1.00 (120) | −0.0770 | −0.0858 | +0.0088 | 1.29 |
| 11 | 2.00 (240) | −0.0747 | −0.0825 | +0.0078 | 1.35 |
| 23 | 0.25 (30) | −0.0854 | −0.0840 | −0.0014 | 0.19 |
| 23 | 0.50 (60) | −0.0764 | −0.0773 | +0.0009 | 0.14 |
| 23 | 1.00 (120) | −0.0835 | −0.0776 | −0.0059 | 1.08 |
| 23 | 2.00 (240) | −0.0827 | −0.0765 | −0.0062 | 1.03 |

Max |t| of the lateral gradient on the steering differential: **2.54 / 1.81 /
1.08** for seeds 7 / 11 / 23, against the repo's |t| ~ 2.3 bar. One of the
twelve comparisons clears it, at the *lowest* concentration, and its sign then
flips at every higher concentration — no dose-response, and it does not
replicate. The established lateral luminance result looks the same in shape
(max |t| 1.67 / 1.47 / 2.51 against floors 2.43 / 1.74 / 3.00).

**And the largest |t| anywhere is not a lateral response.** The probe's maximum
t against the null was 2.85 / 4.26 / 4.80, and every one of them is on a
**symmetric** condition (both antennae stimulated equally) — on `lift/weight` or
on `steer right`. A symmetric stimulus carries no left/right difference at all,
so it cannot be a directional odour response; and 12 conditions × 6 motor
channels = 72 comparisons were made, against measured null-vs-null floors of
1.18-1.34, which is what puts a maximum in that range within reach of chance.

So: **the odour field reaches the steering pools only through the same diffuse
polysynaptic route as everything else, and a lateral odour gradient produces no
measurable steering response at the established floor.** This is a measurement
of the existing channel, not a new construction — nothing was built for it
beyond the open-loop hook in `World::set_imposed_odor`, which replaces the odour
at the antennae and touches no wiring.

One labelling wrinkle worth recording: in odour mode the probe's two
positive-control channels are still written to JSON under the fixed channel
names `visual_loom_left Hz` / `visual_loom_right Hz`, because the JSON writer
uses `CHANNEL_NAMES`. The console table labels them `driveL Hz` / `driveR Hz`
correctly. The values in the JSON are the odour drive rates, not loom rates.

---

## Verdict

**What the retina can and cannot see.** It now has real colour, and the fly's
own pigments set its limits rather than a modelling choice. Before, all 5895
photoreceptors were driven by one luminance scalar derived from a procedural
noise texture keyed by world position — the scene had no albedo and no spectrum,
and there was no colour code anywhere in `vision.rs`. Now each cell carries the
opsin its annotation says it carries (Rh1 478, Rh3 345, Rh4 375, Rh5 437, Rh6
508 nm), and the surface it is looking at contributes a measured reflectance
spectrum. The check that colour information exists passes: an equi-luminant grey
fruit and the coloured one are indistinguishable to the luminance channel (ratio
1.000) and clearly different to the chromatic ones (Rh6 sees the fruit 28.4 %
brighter, Rh3/Rh5 6-7 % dimmer). What it **cannot** see is red: every fly opsin
peaks at or below 508 nm, so the red edge that makes a ripe tomato conspicuous
to a human is outside this animal's range entirely. To a fly a ripe tomato is a
slightly greenish dark object. That is a fact about *Drosophila*, not about the
model, and it is the main reason colour was never going to be a strong cue here.

**What was added, and why it is grounded.** Spectral sensitivity, driven by the
MaleCNS annotation's own `type` column per cell, with a Govardovskii template
and measured λmax values; a fruit with a measured ripe-tomato reflectance, cast
through the existing optics as an ordinary scene object with no collision, no
taste, no reward and no injected signal; and the per-eye retinotopic loom made
the default, with the shared-scalar path moved behind `FLYVERSE_LOOM_SCALAR=1`.
The unresolved cell types were deliberately *not* given invented subtypes — they
get the mean of the pigments their identity could carry, and the dorsal-rim
cells are left unresolved rather than assigned a colour opsin. Nothing in the
model encodes an outcome. No homing term, attraction gradient, food-seeking
rule, steering coefficient, balance term or stabiliser was added; the only new
behavioural-adjacent code is measurement telemetry and the two open-loop
stimulus hooks (`set_imposed_odor`, alongside the existing `set_imposed_loom`),
both of which replace a stimulus at the sense organ and touch no wiring.

**Does the fly get to the fruit? No — and not by a small margin.** The answer is
an exact null, not a weak effect. Across three seeds and four configurations,
every body and motor column of the trace is **bit-identical** between the run
with the coloured fruit, the run with the fruit rendered grey, and the run with
no fruit in the room at all. The fruit demonstrably reaches the eye (up to 44
columns at a time, 0.029 in luminance, and in seed 7 it was on the retina for
34 % of airborne samples) and demonstrably reaches the net (up to 475 extra
spikes), and it moves no motor neuron and no millimetre of the body. The
distance to the fruit's place closes by 384 / 111 / 105 mm over 12 s — and by
*exactly the same amount with nothing there*.

**By what route, then?** None that this experiment can resolve, and the
arithmetic says why. The lateral chromatic drive has a standard deviation of
0.004-0.007 against 0.14-0.16 for lateral luminance — the colour channel carries
about 4 % of a luminance signal that the fly already fails to act on (that
failure being the established result this work was asked not to repeat). Its
correlation with the steering differential is +0.151 / +0.033 / −0.012, mean
+0.057, which does not replicate in sign. The earlier flat result did not cover
this case in the sense that nobody had put a *coloured* object in the room
before; it turns out to cover it anyway. The odour route, measured but not
built, is the same story: the olfactory pools reach the steering pool within 2
hops but only through a diffuse ~30 000-cell frontier, with no direct contacts
at all, and a lateral odour gradient produces no steering response above the
established floor (max |t| 2.54 / 1.81 / 1.08).

**The one thing that did change is flight itself**, and honestly so: making the
retinotopic path the default and giving the retina a real spectrum changed how
the fly flies — cruise fraction, altitude and wall contacts all move, and the
before/after block in §4 records it. That is a change in the *sensory apparatus*,
which is the legitimate kind, and it is reported rather than buried.

**The finding, stated plainly.** A faithful colour visual front end, built from
the animal's own annotated photoreceptor types and its own measured pigment
sensitivities, with a real fruit in the room, produces no behavioural response
to that fruit in this connectome — because the projection from the visual
steering pathway onto the steering motor pool is bilaterally balanced, and
colour contributes a few percent of a signal that is already too small. The
honest verdict the task anticipated is the one that came out. Nothing was tuned
to produce approach behaviour, and had approach appeared it would have needed
the arithmetic above to explain it; it did not appear.

---

## Reproducing

```bash
./build.sh test --release                      # 45 tests: the original 39, + 6 new

# the measurement grid, 3 seeds x 5 configurations, 12 s each
./build.sh run --release -- analyze --seconds 12 --seed 7  --every 1 --out /tmp/fv/after_s7
FLYVERSE_NO_FRUIT=1   ./build.sh run --release -- analyze --seconds 12 --seed 7 --every 1 --out /tmp/fv/grain_s7
FLYVERSE_FRUIT_GREY=1 ./build.sh run --release -- analyze --seconds 12 --seed 7 --every 1 --out /tmp/fv/grey_s7
FLYVERSE_LOOM_SCALAR=1 ./build.sh run --release -- analyze --seconds 12 --seed 7 --every 1 --out /tmp/fv/scalar_s7

# the odour probe, open loop
./build.sh run --release -- loom-probe --odour --levels 0.25,0.5,1 --trials 8 --warmup 8 \
    --seed 7 --out /tmp/fv/odorprobe_7

# the spectral asset (already committed under assets/)
uv run --with numpy --with pyarrow python3 scripts/derive_spectral_classes.py
```