# The activation-to-force map: force versus motor firing rate

This note grounds the last link in the chain that decides whether the fly can fly:

```
motor_flight_power_*  --(read-out)-->  a(t)  --(map)-->  stroke amplitude  -->  force
                        SCALE           |    SHAPE
```

`docs/activation-scale.md` corrected the SCALE (500 Hz window ceiling -> a
physiological anchor) and handed the SHAPE off unresolved;
`docs/motor-mn-calibration.md` §6 then proved the pool is now **graded** (8 Hz
mean, `a` mean 0.17, 0 % pinned at the rail) and closed with an explicit
hand-off:

> "This body model can only be lifted by a rail-level motor command ... Fixing
> this is a body/read-out task (the 2 ms window's 20 Hz full scale, the
> `0.12 + 0.88 a` amplitude map, or the lift gain), not a motor-neuron task."

This note does that body-side task. It answers one question: **does the model's
net force versus the pool's firing rate match the relation the animal was
measured to have, and if so, at what firing rate does the fly hover?**

**Answer: the hover rate is 6.69 Hz/neuron** — inside the measured 3-12 Hz
in-flight band — and the force curve's exponent is the measured 0.483. Before
the change the same body did not hover until **73 Hz/neuron**, 6.1x the top of
the band. The scrutiny test the task demanded is §4.

---

## 1. The measured relation

Gordon & Dickinson (2006), *PNAS* 103(11):4311-4315,
`doi:10.1073/pnas.0510109103`, tethered *Drosophila* in front of a vertically
drifting grating, recording A-IFM motor-neuron spikes alongside wing kinematics.
Two statements of the same measurement:

- Explicit measured pair (Fig. 1e): "a 3-fold increase in spike frequency
  (from 3 to 9 Hz) results in a 1.7-fold increase in power output".
- The paper's rounded summary of the same data (Fig. 1c): "An ≈2-fold change in
  power accompanied a 3-fold change in steady-state spike frequency".

These give two candidate exponents, and the explicit *measured pair* is the one
used here:

```
ln(1.7) / ln(3) = 0.483     <- used
ln(2.0) / ln(3) = 0.631     <- the rounded 3-fold-to-2-fold statement
```

Either way the relation is **compressive**, and either way it is far from the
exponent of 2 the model produced. The rounded 0.63 figure is the one the task
quotes; the explicit 1.7x pair is the tighter measurement and is used, with the
sensitivity to that choice reported in §6.

Aerodynamic force goes as stroke amplitude SQUARED (physical-model-spec.md
§1.3; `wing::stroke_force_is_quadratic_in_amplitude`). So a measured power
exponent `e` requires the amplitude map to carry exponent `e/2`:

```
lift ∝ amp^2 ∝ (a^0.2415)^2 = a^0.483
```

**Band the curve is anchored over: 3-12 Hz/neuron**, the measured in-flight
working range — Huerkey et al. 2023, *Nature* 618:118-124, Fig. 1c
(`doi:10.1038/s41586-023-06099-0`): "approximately 3-12 Hz", with the
citation's own 3-9 Hz measurement point inside it and Gordon & Dickinson's
sustained value at "approximately 5 Hz".

**What the curve does outside that band:**

- *Below 3 Hz* it is the same continuous power law, not a new claim. It is
  steep there because a compressive law is steep at the origin: at 1 Hz it
  gives `a = 0.083` and 0.30 `W` of lift. It reaches 0 lift at 0 Hz, which is
  only true because the old uncited `0.12` amplitude floor is **removed** (§2).
- *Above 12 Hz* the command **saturates**: `a` is clamped at 1, the full-stroke
  endpoint, so lift is flat at `1.326 W`. The animal's relation keeps rising to
  its ~20 Hz manoeuvre rate; the model's actuator cannot, because the spec pins
  the full-stroke endpoint (§6.5) and the endpoint is not a free parameter. That
  is a stated limitation, not a fit: the model has 1.33x of lift surplus at
  full stroke and no more.

---

## 2. What changed

Three constants and one read-out, in the files this task owns:

| file | change |
|---|---|
| `src/body.rs` | `stroke_amp_for_activation(a) = STROKE_AMP_MAX * a^0.2415` replaces `MAX * (0.12 + 0.88 a)`. New `POWER_RATE_EXPONENT = 0.483` (G&D 2006) and `STROKE_AMP_EXPONENT = 0.2415 = 0.483/2`. New pure functions `lift_ratio_at_activation` / `lift_ratio_at_rate` so the curve is inspectable without running the sim. |
| `src/wing.rs` | `STROKE_AMP_MIN` (the `0.12` floor) **deleted**. `STROKE_AMP_MAX = 2.757` rad (158°) untouched. |
| `src/groups.rs` | `POWER_MN_MAX_HZ = 20.0` -> `POWER_MN_FULL_STROKE_HZ = 12.0`; added `POWER_MN_BAND_LO_HZ`/`HI_HZ` (3/12) and `POWER_MN_MANOEUVRE_MAX_HZ` (20, kept for reporting only). Anchored groups now read a **tonic** rate estimate (`MOTOR_RATE_TAU_S = 80 ms`, the measured myoplasmic Ca2+ decay, Huerkey et al. 2023 Fig. 4b) instead of one 2 ms window's spike count, so `a` is linear in the pool's firing rate. |
| `src/analyze/mn_audit.rs` | prints the force-versus-rate curve, the before curve, the measured law, the hover rate, and the produced exponent. |

**The floor.** The old map was `0.12 + 0.88 a`; the `0.12` was an uncited [E]
holdover ("a wing with zero motor drive still sweeps a little") that only added
force — 1.4 % of full-stroke lift at `a = 0`, and it pushed the map's *shape*
away from any measured law. It is removed. Nothing needs it: a compressive law
rises steeply from the origin on its own (`a = 0.05` gives 47 % of full stroke),
and at `a = 0` — a pool that is not firing — the amplitude is genuinely 0.

**The endpoint is preserved.** `stroke_amp_for_activation(1.0) ==
STROKE_AMP_MAX` exactly, so `a = 1` still means full stroke and the spec's
`F/W ≈ 1.2 at a = 1` rule (physical-model-spec.md §6.5) still holds. Pinned by
the new test `full_stroke_endpoint_survives_the_compressive_shape`.

---

## 3. The curve

Printed by `mn-audit` (`./build.sh run --release -- mn-audit --seconds 4`).
`lift/W` is total lift from both wings over body weight at rest, so 1.0 is
hover; `before` is the superseded chain, reconstructed in the instrument as
`1.326 * (0.12 + 0.88*(1-(1-0.002 f)^12))^2`; `meas@5Hz` is the measured law
with weight support at the animal's ~5 Hz sustained rate (the only absolute
anchor the measurement supplies — renormalising the two curves to each other
would make the comparison a tautology, so the measured law is anchored
independently).

| Hz | a | amp/MAX | lift/W | before a | before | meas@5Hz |
|---|---|---|---|---|---|---|
| 0.0 | 0.000 | 0.000 | 0.000 | 0.000 | 0.019 | 0.000 |
| 1.0 | 0.083 | 0.549 | 0.399 | 0.024 | 0.026 | 0.460 |
| 2.0 | 0.167 | 0.649 | 0.558 | 0.047 | 0.035 | 0.642 |
| 3.0 | 0.250 | 0.715 | 0.679 | 0.070 | 0.044 | 0.781 |
| 4.0 | 0.333 | 0.767 | 0.780 | 0.092 | 0.054 | 0.898 |
| 5.0 | 0.417 | 0.809 | 0.869 | 0.114 | 0.064 | 1.000 |
| 6.0 | 0.500 | 0.846 | 0.949 | 0.135 | 0.076 | 1.092 |
| **6.69** | **0.557** | **0.868** | **1.000** | 0.152 | 0.086 | 1.151 |
| 7.0 | 0.583 | 0.878 | 1.022 | 0.156 | 0.088 | 1.176 |
| 8.0 | 0.667 | 0.907 | 1.090 | 0.176 | 0.100 | 1.255 |
| 9.0 | 0.750 | 0.933 | 1.154 | 0.196 | 0.113 | 1.328 |
| 10.0 | 0.833 | 0.957 | 1.215 | 0.215 | 0.127 | 1.398 |
| 12.0 | 1.000 | 1.000 | 1.326 | 0.253 | 0.156 | 1.526 |
| 15.0 | 1.000 | 1.000 | 1.326 | 0.306 | 0.201 | 1.700 |
| 20.0 | 1.000 | 1.000 | 1.326 | 0.387 | 0.282 | 1.953 |
| 30.0 | 1.000 | 1.000 | 1.326 | 0.524 | 0.448 | 2.376 |
| 40.0 | 1.000 | 1.000 | 1.326 | 0.632 | 0.607 | 2.730 |
| 60.0 | 1.000 | 1.000 | 1.326 | 0.784 | 0.871 | 3.321 |
| 80.0 | 1.000 | 1.000 | 1.326 | 0.877 | 1.054 | 3.816 |
| 110.0 | 1.000 | 1.000 | 1.326 | 0.949 | 1.211 | 4.450 |

Shape checks, all from the same run:

| quantity | measured | model after | model before |
|---|---|---|---|
| force exponent over 3-12 Hz | 0.483 | **0.483** | 0.918 |
| lift ratio 3 -> 9 Hz | **1.70x** | **1.70x** | 2.60x |
| hover rate (lift/W = 1) | ~5 Hz sustained | **6.69 Hz** | **73 Hz** |
| hover rate vs the 3-12 Hz band | inside | inside (0.56x the band top) | 6.1x the band top |

The old curve's force exponent over 3-12 Hz is 0.918 — and 2.60x over 3-9 Hz:
super-linear, while the measurement is compressive. The map was not merely
mis-scaled, its **direction** was wrong: the model demanded more force per
additional spike than the animal produces.

---

## 4. THE scrutiny test: at what rate does it hover?

A change like this makes the fly fly, so "it flies" is not evidence. What
matters is whether it flies on a **physiological** command.

| | before | after |
|---|---|---|
| hover rate, from the force law (`lift/W = 1`) | **73 Hz/neuron** | **6.69 Hz/neuron** |
| as a multiple of the 3-12 Hz band top | 6.1x | 0.56x |
| inside the measured 3-12 Hz band? | **no** | **yes** |
| pool's actual mean rate in the closed loop (4 s, `mn-audit`) | 330 Hz (railed) | **6.1 Hz** |
| pool's actual mean rate while airborne (12 s trace, mode=cruise) | n/a (never airborne) | **5.9 Hz** |
| actuator `a` in the closed loop | 0.995 (rail, 84.7 % pinned) | mean **0.498**, sd 0.076, max 0.605, **0 %** at the rail |

**The fly hovers at 6.69 Hz/neuron and cruises with its power pool at ~5.9
Hz/neuron — both inside the measured 3-12 Hz in-flight band.** This is not a
re-tuned scale: the demand moved from a rate the animal never reaches (73 Hz,
6.1x the band) to one it sustains (5-9 Hz).

Residual, stated plainly: hover lands at 6.69 Hz against the animal's ~5 Hz
sustained rate, i.e. the model asks **1.34x** more motor drive than the animal
for the same force. That residual is not free to remove: the exponent (0.483)
and the full-stroke endpoint (1.326 `W`, pinned by spec §6.5) together fix the
hover rate, and 6.69 Hz is inside the band that the measurement covers. It is
also robust to the two judgement calls in §1 and §5:

- with the paper's rounded `3-fold -> 2-fold` exponent (0.63) instead of the
  explicit 1.7x pair, hover is 7.67 Hz — still inside 3-12 Hz;
- with the anchor at the manoeuvre maximum (20 Hz) instead of the band top
  (12 Hz), hover is 11.2 Hz — still inside 3-12 Hz;
- to place hover exactly on the animal's 5 Hz the full-stroke anchor would have
  to be ~9.0 Hz, i.e. 0.75x the band top.

---

## 5. Which denominator: 12 Hz or 20 Hz?

`POWER_MN_MAX_HZ = 20.0` was the previous anchor. `a = 1` is the **full-stroke**
endpoint, fixed by the spec at `F_lift/(W/2) ~ 1.2` (§6.5), so the anchor is the
rate at which the muscle is maximally activated and the wing at full stroke.

**Chosen: 12 Hz/neuron, the top of the measured in-flight band.** It is the
citation's own working range (Huerkey et al. 2023 Fig. 1c: "approximately 3-12
Hz"; Gordon & Dickinson 2006 sustain ~5 Hz inside it). The 20 Hz figure is the
"up to approximately 20 Hz" seen during *visually elicited manoeuvres* — an
excursion, not the activation ceiling — and the measured f-I curve stays linear
well past it (to 30 Hz), so 20 Hz is not a saturation point either. Anchoring at
the band top keeps the animal's whole sustained envelope inside the graded range
of the command; anchoring at 20 Hz makes sustained flight a fraction of full
scale and any manoeuvre saturate. `POWER_MN_MANOEUVRE_MAX_HZ = 20.0` is
retained for reporting, deliberately not as the actuator's full scale.

The choice is not load-bearing for the verdict: because `a = f / anchor` and the
map is a pure power law, the hover rate *in Hz* scales linearly with the anchor,
and both candidates put hover (6.69 / 11.2 Hz) inside 3-12 Hz.

---

## 6. The spec's mandatory self-check still passes

`docs/physical-model-spec.md` §6.3 is a no-tuning consistency check the
implementation is required to pass: at `alpha = 45 deg`, `Phi = 160 deg`,
`f = 200 Hz` the blade-element integral must give `F_lift/(W/2) ~ 1.22` for the
1.8 mm^2 wing the spec assumes. It caught a real error once (a `C_F` substituted
for `C_L`, overstating lift 35 %), so it is not decorative.

It is unchanged and still passing. Recomputed from the geometry constants:

```
cl(45 deg)              = 1.8046
F                       = 6687.6 mg*mm/s^2
W/2                     = 4821.6 mg*mm/s^2
F_lift / (W/2)          = 1.387
```

1.387 for the measured 2.036 mm^2 planform, i.e. 1.22 x (2.036/1.8) = 1.380 —
exactly the spec's band scaled to the real wing. The map change touches only the
`a -> amplitude` stage, downstream of the force coefficient, so the check could
only fail if the full-stroke endpoint had moved. It has not:
`stroke_amp_for_activation(1.0) == STROKE_AMP_MAX == 2.757 rad`, pinned by test.
Test `wing::tests::normal_force_reproduces_the_specs_mandatory_self_check`
passes, and the new
`body::tests::full_stroke_endpoint_survives_the_compressive_shape` asserts the
full-stroke lift ratio still lands in the spec's band (1.0-1.5).

---

## 7. Before / after behaviour

`./build.sh run --release -- analyze --seconds 12 --seed 7`, same tree, same
seed; the only difference is the amplitude map, the anchor and the tonic
read-out. The engine is bit-reproducible per seed.

| | before | after |
|---|---|---|
| mode % ground / takeoff / cruise | 100.0 / 0.0 / 0.0 | 59.2 / 2.3 / **38.4** |
| takeoffs | 0 | 1 |
| wall hits | 0 | 35 (175/min) |
| path / net, mm | 5 / 5 | 1108 / 638 |
| tortuosity | 1.00 | 1.74 |
| mean speed, mm/s | 0 | 228 |
| altitude mean / p95 / max, mm | 0.0 / 0.0 / 0.0 | 31.2 / 188.6 / 220.0 |
| airborne samples within 20 mm of a wall | 0.0 % | 95.1 % |
| circles airborne | 0.00 | 1.52 |
| network mean rate | 22.21 Hz | 20.29 Hz |

The fly now leaves the ground and cruises for 38 % of a 12 s run, at a mean
altitude of 31 mm and 228 mm/s. Two honest caveats, both visible in the table:

- **It is marginal, not a strong flier.** The pool cruises at ~5.9 Hz
  (lift/W ~ 0.94) against a hover threshold of 6.69 Hz, so lift is near — not
  clearly above — weight, and takeoff depends on thrust/tilt geometry rather
  than raw lift. That is the expected consequence of a compressive map: the
  force band the fly has to work in is narrow by construction.
- **95 % of airborne samples are within 20 mm of a wall**, with 35 wall hits in
  12 s. In a 220 mm box that is contact-dominated flight; it is the room, not
  the map, that limits it. The `analyze` LOOM TEST correlations
  (`corr(loom, yaw rate) = -0.127`, |yaw rate| 2.35 looming vs 1.16 calm) are
  weak and are not claimed as evidence of anything here.

---

## 8. Verdict

- **The force-versus-rate relation is now the measured one.** Exponent 0.483
  over 3-12 Hz, 1.70x per 3-fold rate change (measured 1.70x), against 0.918 /
  2.60x before. The exponent and its direction both match the citation, and the
  change is a shape change grounded in a measurement, not a scale factor.
- **The fly hovers at 6.69 Hz/neuron and cruises at ~5.9 Hz/neuron.** The
  measured in-flight band is 3-12 Hz. **Inside the band.** The fly flies on a
  **physiological** command, not a re-tuned one: the previous body needed
  73 Hz/neuron to hover — 6.1x the top of the band — and 110 Hz for sustained
  cruise under the gain sweep in `docs/motor-mn-calibration.md` §4.1.
- **The floor is gone and the endpoint is pinned.** The uncited `0.12`
  amplitude floor is deleted; `a = 1` is still full stroke, so spec §6.5's
  `F/W ~ 1.2 at a = 1` rule is intact.
- **The §6.3 mandatory self-check still passes** (1.387 vs the spec's 1.22
  scaled to the measured planform), and **38 tests pass** (33 pre-existing + 5
  new). No pre-existing test's expected value was weakened to make this work;
  see §9.
- **The residual is 1.34x, not a scale factor.** Hover lands at 6.69 Hz against
  the animal's ~5 Hz sustained rate, and it stays inside 3-12 Hz whichever way
  the two judgement calls (explicit 1.7x vs rounded 2.0x exponent; 12 Hz vs
  20 Hz anchor) are made.
- **Limitation, stated not buried:** above 12 Hz the actuator saturates at
  1.326 `W`, so the model cannot represent the animal's rising power between 12
  and ~20 Hz. Lifting that cap means moving the full-stroke endpoint, which spec
  §6.5 forbids without re-doing §6.3.

## 9. Tests

`./build.sh test --release`: **38 passed, 0 failed** (33 pre-existing, 5 new).

New (in `src/body.rs` and `src/groups.rs`):

- `body::tests::full_stroke_endpoint_survives_the_compressive_shape` — `a = 1`
  is still `STROKE_AMP_MAX`, `a = 0` is still 0, no additive floor, and the
  full-stroke lift ratio is still inside spec §6.3's band.
- `body::tests::force_follows_the_measured_frequency_to_power_relation` — the
  3-9 Hz ratio is 1.7x and the exponent over 3-12 Hz is 0.483.
- `body::tests::hover_sits_inside_the_measured_inflight_band` — the hover rate
  (1.0 < `lift/W` crossing) falls inside `POWER_MN_BAND_LO_HZ..HI_HZ`.
- `groups::tests::anchored_readout_is_linear_in_the_pool_rate` — the anchored
  read-out reads `f/12` at 3, 6 and 9 Hz (the old per-window read-out read
  0.070 at 3 Hz, a factor 3.6 low), and saturates at the anchor.
- `groups::tests::anchored_readout_is_not_the_per_window_occupancy` — pins the
  two forms as different quantities, so a regression to the per-window form
  cannot pass silently.

One pre-existing test's expectations changed:
`groups::tests::flight_power_readout_is_scaled_by_the_motoneuron_max_rate`
(renamed `..._by_the_full_stroke_rate`). Three edits, each justified:

1. `Some(POWER_MN_MAX_HZ)` (20 Hz) -> `Some(POWER_MN_FULL_STROKE_HZ)` (12 Hz).
   The **test** was asserting a superseded anchor value, not a property of the
   read-out — §5 is the argument for 12 Hz, and the test now also asserts the
   structural relation `FULL_STROKE == BAND_HI < MANOEUVRE_MAX`.
2. The `rate_hz` assertions now drive `smooth_hz`/`anchored`, because the
   anchored branch reads the tonic estimate. Necessarily so: the read-out
   contract changed, so the test that exercises it has to set the new inputs.
3. The trailing `assert_eq!(r.norm(0), 1.0)` (a pool at 333 Hz/neuron saturates)
   was **removed**, and the replacement asserts the pool no longer needs to
   saturate. That assertion was a statement about the *network's* rate scale, not
   about the read-out: it was true only while the power pools fired at 330
   Hz/neuron. The per-cell motor calibration (`docs/motor-mn-calibration.md`,
   gain 0.0216) took the pool to 6-8 Hz, so the same assertion is now false for
   the pool it was written about. Its survival would have pinned the very defect
   that doc was written to remove.

No new test is weaker than what it replaced, and the spec's §6.3 check is
untouched.