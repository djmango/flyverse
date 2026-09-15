# The scale of `a(t)`: what the flight motor neuron actually fires at

This note grounds the two remaining links in the activation chain that decides
whether the fly can fly, after the motor read-out and the wing force coefficient
were fixed:

```
motor_flight_power_*  --(read-out)-->  a(t)  --(map)-->  stroke amplitude  -->  force
                        SCALE           |    SHAPE
```

It answers one question: of the scale (the 500 Hz normalisation) and the shape
(the linear `amp = MAX * (0.12 + 0.88 a)` map), which is the defect?

**Answer: the SCALE.** The 500 Hz full scale is the *measurement's* arithmetic
ceiling, not a rate any flight muscle can be asked for; the physiological full
scale of a flight power motor neuron is **20 Hz per neuron**. The SHAPE is not
the defect — the in-vivo activation data are linear over the flight working
range — so it is left alone. The consequences of correcting the scale are
reported in full below, including the one that matters: the model's power pool
already fires at ~334 Hz/neuron, ~17x the physiological maximum, so the
corrected read-out saturates at 1.0 and the fly flies. That coincidence is
diagnostic, not a result, and it is stated plainly in §4.

---

## 1. What a Drosophila flight POWER motor neuron fires at

The flight power muscles are the dorsolongitudinal (DLM) and dorsoventral (DVM)
muscles, driven by motor neurons MN1-5 (DLM) and the DVM motor units. They are
**asynchronous** indirect flight muscles: the wingbeat is produced by the
thorax's own stretch-activated oscillator and is decoupled from the motor neuron
firing rate, which sets only the activation level. This is exactly the mechanism
`physical-model-spec.md` §5.1 cites (Pringle 1949; Machin & Pringle 1959), and
it is what makes the motor neuron rate *low* rather than one spike per beat:

| Quantity | Value | Source |
|---|---|---|
| DLM/DVM motor neuron rate during sustained flight | **~5 Hz** | Gordon & Dickinson 2006 (PNAS 103:4311, `doi:10.1073/pnas.0510109103`): "The motor neurons of the A-IFMs fire at a rate (≈5 Hz in *Drosophila*) well below contraction frequency (≈200 Hz)". Lee, Iyengar & Wu (J. Neurogenet., PMC6602807) measure the same: overall DLM firing rate 4.90 Hz in flight. |
| Spikes per wingbeat | **1 per ~20th-40th wingbeat** | Nature 618:118-124 (2023), `doi:10.1038/s41586-023-06099-0`: each of MN1-5 "fires only every approximately 20th to 40th wingbeat". At 200 Hz that is 5-10 Hz. |
| Tethered-flight working range | **~3-12 Hz** | same paper; the f-I curve is "approximately linearly related for 2-30 Hz ... covering and even exceeding the working range observed during tethered flight". |
| Maximum observed (manoeuvring flight) | **~20 Hz** | Gordon & Dickinson 2006 recorded spike frequency "up to ~20 Hz"; Lehmann & Bartussek 2016 (J. Comp. Physiol. A, `doi:10.1007/s00359-016-1133-9`): "the A-IFM's low spike frequency between 5 and 20 Hz maintains rather constant intramuscular calcium levels during flight". |
| Rate at which maximum power is produced | top of that range | Namiki et al. 2022 (Curr. Biol. 32:1189, `doi:10.1016/j.cub.2022.01.008`) drive the DNg02 population and elicit "maximum power output from the flight motor" bounded by a ~200 W/kg mechanical-power isoline, with wingbeat amplitude saturating at ~160°. |

So a flight power motor neuron tops out near **R ≈ 20 Hz**, with ~5 Hz the
sustained hovering/cruise rate and 3-12 Hz the working range. The decoupling is
the whole point: at a 200 Hz wingbeat the neurons fire 10-60x slower than the
muscle oscillates, which is only possible because the beat is myogenic. A model
that demanded one motor-neuron spike per wingbeat would be modelling a
*synchronous* muscle, not this one.

## 2. The arithmetic, tied to the code

Before this change, `GroupRates::norm` (src/groups.rs) read

```
norm = rate_hz / full_scale_hz(),   full_scale_hz() = 1000 / window_ms = 1000 / 2 = 500 Hz
```

and 500 Hz is exactly one spike per member per 2 ms window — the largest rate a
spike count over that window can express, since the 2.2 ms LIF refractory
(`lif::REFRACTORY_MS`) guarantees a member contributes at most one spike per
window. It is a property of the **measurement**, and it was being used as the
**actuator's** full scale, i.e. as the rate that means "full stroke amplitude".

Those are different quantities, and the ratio is the size of the error:

```
measurement ceiling  500 Hz
physiological R       20 Hz
ratio                  25x
```

- `full_scale_hz()` was **25x too large** as an actuator full scale.
- Level flight needs `a ≈ 0.85-0.87` (§4). In physiological units that is
  `0.865 * 20 = 17.3 Hz/neuron` — about **87 % of the maximum rate a DLM motor
  neuron is ever observed to fire** (up to ~20 Hz during manoeuvres, working
  range 3-12 Hz). That is plausible for a hovering fly, which is the most
  power-demanding sustained behaviour it has. Contrast the pre-change framing:
  the same demand read as "95 % of the LIF refractory ceiling, held for
  seconds", which is not a thing a muscle does.
- The model's own power pool fires at **~334 Hz/neuron** at the observed
  `norm` of 0.6677 (`0.6677 * 500`, and `probe` prints 250-333 Hz/neuron
  directly). That is **~17x the physiological maximum** and near the 455 Hz
  refractory limit. So the connectome in this model does not issue a
  "two-thirds power" command at all: it drives the flight motor neurons far
  past anything the animal reaches.

## 3. The SHAPE is not the defect

`body.rs` maps activation to stroke amplitude linearly with a floor:
`amp = STROKE_AMP_MAX * (0.12 + 0.88 * a)` (`STROKE_AMP_MIN = 0.12 * MAX`),
then a 20 ms first-order lag. Is a linear map with a floor justified?

- **The steep/saturating part of the relation is real but belongs to a stage
  this model does not have.** In skinned *Drosophila* IFM fibres, positive power
  generation starts at pCa 5.8 and reaches its maximum at pCa 5.25 —
  i.e. a steep, threshold-like, saturating dependence on calcium
  (Wang, Zhao & Swank 2011, Biophys. J. 101:2207,
  `doi:10.1016/j.bpj.2011.09.034`).
- **In vivo the operating point sits on that steep flank, where the relation is
  linear.** Intramuscular calcium varies ~2-fold during flight (pCa 5.7 -> 5.4)
  and muscle power is linear in calcium with R² ≈ 0.95 (DLM) / 0.97 (DVM) over
  20-120 W/kg (Lehmann, Skandalis & Berthe 2013, J. R. Soc. Interface 10:20121050,
  `doi:10.1098/rsif.2012.1050`). A 2-fold calcium change gives a 2-3 fold power
  change (Wang et al. 2011).
- **Stroke amplitude is approximately linear in drive and then saturates.**
  DNg02 optogenetic recruitment raises wingbeat amplitude ~linearly
  (1.8-2.8 deg per cell pair, r² = 0.79-0.84) and peaks at ~160° — which is
  `STROKE_AMP_MAX` (2.757 rad = 158°) — where further drive trades amplitude for
  frequency at constant power (Namiki et al. 2022, `op. cit.`).

A linear rise from a resting floor to a 160° ceiling is therefore the right
*shape* for this model: it reproduces the in-vivo linearity over the working
range and saturates at the measured maximum. The 0.12 floor is an uncited [E]
holdover from the pre-embodiment `0.12 + 0.95 * power` curve; it is documented
as such and left alone, because it only *adds* force and so cannot be masking a
shortfall. No shape change is justified, and none is made.

## 4. Consequences of correcting the scale — including the coincidence

The anchored read-out saturates. Measured `flight_power_*` rate ≈ 334 Hz/neuron
against a 20 Hz full scale is 16.7, clamped to 1.0, so `a -> 1` and the wings
run at full stroke amplitude. **The fly flies.** That is not evidence that the
model is right, and it is stated here rather than buried:

- A read-out that sits at 1.000 with min 0.000 and mean 0.995 is a **bang-bang
  switch**, not a graded muscle activation. A real fly hovers with its power
  motor neurons near 5 Hz of a ~20 Hz maximum, i.e. around `a ≈ 0.25`, not at
  the rail.
- The physiology that grounds the scale and the behaviour that the scale
  produces therefore **contradict one another**, and that contradiction is the
  real finding: a 2 ms spike-count window over a 12-neuron pool in this network
  reports a rate (334 Hz) that is 17x outside the range the biology allows. No
  choice of full scale can make that signal both physiological and graded —
  anchoring it at 20 Hz pins it at 1.0, leaving it at 500 Hz reports "0.67 of
  full" for a drive that is far beyond full.
- So the scale defect is real and is corrected, but **correcting it alone does
  not produce a physiological flight controller**. The remaining defect is the
  absolute firing-rate scale of the flight-power pools in the LIF/connectome
  model, which lives in `lif.rs`/the pack, not in the read-out or the body.

## 5. The change

One change: the flight-power read-outs are normalised by the power motor
neuron's physiological maximum firing rate, not by the spike-count window's
arithmetic ceiling.

- `src/groups.rs`: `POWER_MN_MAX_HZ = 20.0` with the citations above;
  `phys_full_scale_hz(name)` anchors `motor_flight_power_left/right`;
  `Groups` carries `group_scale_hz`; `GroupRates::commit` copies it into
  `scale_hz` (falling back to the window ceiling for unanchored groups) and
  `norm` divides by it. `full_scale_hz()` keeps its name and its meaning — the
  measurement ceiling — because `analyze/yaw.rs` uses it for the steering
  fraction and the steering pools are deliberately *not* scaled here.
- `src/body.rs`: comment only. The map is unchanged; the doc now records the
  scale, the shape justification, and the [E] status of the 0.12 floor.
- `src/wing.rs`: untouched.
- One test added (`flight_power_readout_is_scaled_by_the_motoneuron_max_rate`):
  **33 tests pass, 32 pre-existing + 1 new.** No existing test's expected value
  changed. The doc comment of `readout_keeps_the_graded_spike_count` was amended
  because its claim — "the read-out's full scale is one spike per member per
  window" — is precisely the bug; it is now scoped to groups with no
  physiological anchor, and its 500 Hz assertion still holds there.

## 6. Before / after

12 s, seed 7, full 166,700-neuron model, same tree; the only difference is the
normalisation of the flight-power read-out.

| | before (500 Hz scale) | after (20 Hz scale) |
|---|---|---|
| takeoffs | 0 | 1 |
| cruise % | 0.0 | 97.0 |
| ground % | 100.0 | 0.8 |
| altitude mean / max, mm | 0.0 / 0.0 | 208.9 / 220.0 |
| path (horizontal), mm | 3 | 995 |
| mean speed, mm/s | 0 | 85 |
| wing stroke amplitude, mean / max, rad | 1.941 / 2.011 | 2.742 / 2.757 (rail) |
| wing-power read-out mean / max / min | 0.6677 / 0.7284 / 0.0000 | 0.9951 / 1.0000 / 0.0000 |
| free-flight body-up force / weight, mean | 0.856 | 1.292 |
| free-flight world-up force / weight, mean | 0.805 | 1.176 |
| windows above weight (world frame), % | 0.0 | 89.5 |

`flight-test --seconds 12 --seed 7 --altitude 1000`: before, the fly sinks from
1000 mm at -14.9 mm/s; after, mean vz -0.9 mm/s at 1.176x weight (world), i.e. it
very nearly holds altitude in mid-air, and 1.292x in the body frame sits inside
the spec's own `F_lift/weight ≈ 1-1.3` hover band (§6.3). The steering read-out
is untouched by the change (`flight_steer_l` mean 0.518 -> 0.496), confirming the
anchor is scoped to the flight-power pools.

Note on numbers: `flight-test` measures force with the fly moving, so the
`0.5 * airspeed^2` term in `wing_force_magnitude` inflates it; the zero-airspeed
value at `a = 0.67` is `1.327 * ((0.12 + 0.88*0.67))^2 = 0.663`, i.e. level
flight from rest needs `a ≈ 0.85` at zero tilt (`~0.865` at the few degrees of
mean tilt the earlier 12 s run measured). The task's `0.639` for the pre-fix
body-frame lift/weight was not reproducible on this tree; the same run gives
`0.856`, and the qualitative result (0 takeoffs, 100 % ground) is confirmed.

## 7. Verdict

- **SCALE is the defect.** 500 Hz is the spike-count window's arithmetic ceiling
  (one spike per member per 2 ms window); the physical full scale of a flight
  power motor neuron is **20 Hz/neuron** — a **25x** error. Level flight's
  required `a ≈ 0.865` is 17.3 Hz, a plausible 87 % of the maximum a DLM motor
  neuron is observed to fire.
- **SHAPE is not the defect.** Linear-in-`a` between a resting floor and the
  158° ceiling reproduces the in-vivo linearity of power in calcium over the
  flight range and saturates at the measured amplitude maximum. The 0.12 floor
  is uncited [E] but only adds force. Left unchanged.
- **Not "neither".** The connectome's drive is *not* a two-thirds command that
  genuinely cannot fly; 0.667 `norm` is 334 Hz/neuron, ~17x the physiological
  maximum. The demand for level flight (17.3 Hz) is well inside what the animal
  does.
- **Stated plainly:** the corrected scale makes the fly fly, and it does so by
  saturating. The read-out goes to 1.000 because the model's power pool is not
  in the physiological range at all. That is the next defect, and it is not in
  these files: the flight-power pools of the LIF/connectome model need their
  absolute rates brought into the 3-20 Hz band before `a(t)` can be a graded
  muscle activation.

## 8. Interaction with the steering read-out (reported, not fixed)

`motor_flight_steering_*` is 3 neurons/side (b1-b3) while the spec names tp1,
tp2 and hg1-hg4 (9/side), and 37 traced `wm` vc_motor neurons sit in no read-out
at all. That is a separate defect and is not touched here: the steering pools
keep the window ceiling, so their read-out is still the fraction of the pool
that fired, quantised to thirds, and my change does not alter it
(`flight_steer_l` before/after: 0.518 -> 0.496).

If the steering pool is widened, it should get its **own** physiological anchor
rather than inheriting the window ceiling — and it will be a very different
number from the power pools, because the steering muscles are *synchronous*:
each is innervated by a single motor neuron firing about one spike per wingbeat,
so their rates run to ~200 Hz, an order of magnitude above the power pools'
20 Hz. The scale mechanism added here (`phys_full_scale_hz`) is per-group and can
carry that without touching the power pools.