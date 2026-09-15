# Per-cell electrophysiological calibration of the flight power motor neurons

This is the follow-up to [`motor-pool-rate-diagnosis.md`](motor-pool-rate-diagnosis.md).
That document measured the failure: at the closed-loop operating point the flight
power pool fires at **330.1 Hz/neuron**, 16.5x its physiological maximum, because the
network's 19.1 Hz mean rate crosses the pool's 7 mV threshold gap **7.6x over**. The
pool was a switch. This document describes the narrow, per-cell fix, and reports
whether it makes the fly fly.

**Scope, stated up front.** This is *not* a global re-tune of `w_syn`. Re-calibrating
all 166,700 neurons would need an f-I target the project does not have. This changes
the flight power motor neurons only, for the reason a real motor neuron is a specific
cell type with a measured input-output relation. Every other neuron keeps gain 1.0 and
the connectome artifact is untouched.

---

## 1. What is measured about these cells

The cells are the DLM (wing depressor) motor neurons MN1-5, i.e. the flight power
motor neurons, and their DVM (wing elevator) counterparts. Three things about them are
measured, and all three matter.

**1.1 They have a measured f-I relation, and it is LINEAR over the working range.**

> Huerkey, Schueler, Ryglewski, Duch, Schreiber, Silies, et al. (2023),
> "Gap junctions desynchronize a neural circuit to stabilize insect flight",
> *Nature* **618**, 118-124. doi:10.1038/s41586-023-06099-0
>
> **Fig. 1d** — "The firing responses of MNs (top traces) to current injections of
> different amplitudes (bottom traces)."
> **Fig. 1e** — "The mean MN response frequency (f) and injected current amplitude (I)
> are approximately linearly related for **2-30 Hz** (n = 15 animals), therefore
> exceeding the normal MN firing frequencies observed during flight (inset; **~3-12 Hz**,
> data from c)."
> Main text — "a nearly linear input-output relationship as observed in f-I curves
> (**3-30 Hz**; Fig. 1e) covers and even exceeds the working range observed during
> tethered flight (approximately **3-12 Hz**; Fig. 1c). The excitability of MN1-5 is
> therefore tuned to linearly translate synaptic input into tonic firing to regulate
> wingbeat in the working range of flight."

This is a measured rate-vs-drive curve for exactly these cells — not a rate range. The
paper's own model of the same cells adds why the slope is finite rather than a cliff:
the MNs sit near the saddle-node-loop (SNL) point, where "the slope of the neurons'
firing-rate versus current-input (f-I) curves is not as steep as deeper in the HOM
regime, therefore enabling smooth control of MN firing frequency and wingbeat power,
as observed in vivo (Fig. 1c-e)."

**1.2 Their in-flight rate is a slow, graded command, and the muscle reads it out as
force.**

> Gordon & Dickinson (2006), "Role of calcium in the regulation of mechanical power in
> insect flight", *PNAS* **103**(11):4317. doi:10.1073/pnas.0510109103
>
> The A-IFM motor neurons "fire at a rate (**~5 Hz in Drosophila**) well below
> contraction frequency (~200 Hz)"; in tethered flight, "An **~2-fold change in power
> accompanied a 3-fold change in steady-state spike frequency**" (Fig. 1c).
> And from the same system, PMC5555410: "myogenic DLMs (firing at **~5 Hz**)" and the
> wingbeat is ~200 Hz.

So the command is a *rate code* whose top is ~25-30 Hz and whose working value is
~5-12 Hz. The paper is explicit that this is the whole control channel: the CNS "does
neither control a-IFM power output by the recruitment of different motor units, nor on
the scale of single wingbeats, but the frequencies of a-IFM-MN population firing are
the key regulator of wing power production."

**1.3 The cells are large, and their input resistance is measured and low.**

| cell | R_in | source |
| --- | --- | --- |
| MN5 (DLM flight power MN) | **97 +/- 31 MOhm** | Duch, Vonhoff & Ryglewski (2008), *J. Neurophysiol.* **100**:2525, doi:10.1152/jn.90758.2008 |
| antennal-lobe projection neuron | 598.0 +/- 69.3 MOhm (n = 14) | Gouwens & Wilson (2009), *J. Neurosci.* **29**:6239 |
| MBON-alpha3 | 926 +/- 55 MOhm | eLife (2022), doi:10.7554/eLife.77578 |

The two small-neuron rows are the two best-measured small *Drosophila* central
neurons; the Gouwens & Wilson paper states the general case — "*Drosophila* neurons are
extremely small and have high input resistances (598.0 +/- 69.3 MOhm, n = 14, measured
with antennae removed)". MN5, by contrast, carries ~4,000 dendritic branches in 23
subtrees (Ryglewski et al. 2017, *Neuron* **93**:632), i.e. a large membrane area and a
correspondingly large input conductance.

**No R_in has been measured for MN9**, the cell the global `w_syn` was calibrated on.
That gap is the source of the honest uncertainty in §2, and it is why the calibration
is reported as a *bracket*, not a number.

---

## 2. The mechanism, and what it models

**Mechanism.** `lif::Lif::gain`, a new per-cell multiplicative factor on the *arrival*
(synaptic) term of that cell's own membrane equation. It is applied per destination
cell, so the packed connectome (weights, delays, contacts) is bit-identical to before;
the gain changes only how much somatic depolarisation a given synaptic conductance
buys *this* cell. Default 1.0 for all 166,700 neurons, so the change is a no-op unless
the calibration is applied. It is applied to **24 cells — the 12 members of
`motor_flight_power_left` and the 12 of `motor_flight_power_right`** — and to nothing
else.

**What it models: the cell's input resistance.** In a conductance-based cell, the
somatic voltage response to synaptic drive is `R_in * I_syn`, so the per-cell synaptic
efficacy — mV of depolarisation per unit presynaptic drive — *is* the input resistance
times the synaptic conductance. This is not a read-out scale and not a magic scalar:
it acts on the membrane equation upstream of the spike, and every spike the pool emits
is then counted and normalised exactly as before. It is not a bias current, and there
is no bias current anywhere in this change.

**Why a single global weight is the wrong object here.** The published model applies
one `w_syn = 0.275 mV` per contact to all 166,700 neurons and calls it "the single free
parameter" (Shiu et al. 2024, *Nature* **634**:210, doi:10.1038/s41586-024-07763-9).
It was calibrated to a **saturation** target — a strong stimulus puts MN9 at ~80 % of
its maximal rate. A saturation target is the appropriate criterion for a cell whose job
is to report that a stimulus is present. It is the wrong criterion for a flight motor
neuron whose job, measured in §1.1, is to be *linear* across its whole working range.
The one global parameter cannot be both, and for these cells the measurement says which
one it should be.

### The arithmetic

Two factors, each a ratio of a measured quantity to a constant of this model.

**Factor A — input resistance, against the reference cell the global weight stands for.**

```
MN5 measured:                    97 +/- 31 MOhm   (Duch et al. 2008)
reference (PN, Gouwens & Wilson):598 +/- 69 MOhm   -> 97/598 = 1/6.2
reference (MBON-a3, eLife 77578):926 +/- 55 MOhm   -> 97/926 = 1/9.5
```

**Factor B — the threshold gap, against the model's uniform gap.**

MN5 is driven into repetitive tonic firing by roughly **0.3-0.4 nA** of injected
current (the MN5 model calibrated to the Duch et al. patch-clamp data; see
Herrera-Valdez, "Analysis of Signal Propagation and Excitability in Computational
Models of an Identified Drosophila Motoneuron", ASU, hdl.handle.net/2286/R.I.25956,
Fig. 2.11, where < 0.4 nA is "the typical amplitude that induces repetitive spiking in
MN5"). At the measured input resistance:

```
0.35 nA * 97 MOhm = 34 mV   of steady depolarisation, rest -> rheobase
this model's gap  =  7 mV   (threshold -45 mV, rest -52 mV)
                    7/34 = 1/4.9
```

In a mean-field LIF the input appears only through its ratio to the gap (a cell fires
when the effective drive reaches the gap), so `gain` and `1/gap` are interchangeable
and this is the same kind of per-cell factor. It is also the factor that explains the
observed over-drive directly: the model's threshold is ~5x easier to reach than a real
MN5's.

**The two factors together:**

```
MN_POWER_INPUT_GAIN = (97/926) / (34/7) = 0.0216      (MBON-a3 reference)
MN_POWER_INPUT_GAIN = (97/598) / (34/7) = 0.0334      (PN reference)
```

The shipped default is **0.0216**. The range **0.0216-0.0334** is the honest
uncertainty, and both ends are grounded: the MN side (97 +/- 31 MOhm) and the rheobase
(0.3-0.4 nA) are measurements of *this* cell type, the reference side is a measured
small central neuron, and the only judgement is *which* measured small central neuron
best stands in for MN9. Both ends are reported below, and — this is the point — both
ends land the pool inside the measured operating range without being tuned to anything.

Override for measurement with `FLYVERSE_MN_GAIN=<x>`; it sets the same per-cell value on
both power pools and nothing else.

---

## 3. Is the pool now graded? (the honesty test)

The test is whether the pool's OUTPUT is graded, not whether the fly flies. Measured
with `mn-audit` on `motor_flight_power_left` (12 members; the right pool is symmetric),
4 s, seed 7, default senses, 2 ms control windows; `a` is the actuator command the wing
receives (`GroupRates::norm` of the pool, then the 30 ms `MOTOR_TAU_S` transduction
lag).

### 3.1 Before (gain 1.0, the shipped state)

```
pool mean 330.1 Hz/neuron = 16.505 x the 20 Hz physiological maximum, 0.726 x the
                            455 Hz refractory ceiling
per-neuron: min 263.1  q1 277.4  median 328.4  q3 372.7  max 417.0 Hz
  ==    0 Hz       0    0.0%
  [3, 12) Hz       0    0.0%     <- the measured flight working range
  [12, 20) Hz      0    0.0%
  [20,100) Hz      0    0.0%
  >=100 Hz        12  100.0%
predicted depolarisation 241.7 mV = 34.5x the 7.0 mV threshold gap
a: mean 0.9907  sd 0.0703  p05 0.9900  median 1.0000  p95 1.0000
```

Across the sense-ablation ladder (from the diagnosis doc) the pool rate was
**23.0, 153.2, 154.5, 290.0, 330.1 Hz** — it varied by 14x but its *minimum over every
drive tested* was 23.0 Hz, 1.15x the physiological maximum. **It never entered its own
working range at any drive.** Every neuron was on the ceiling; the read-out was pinned
at its rail in 84.7 % of the flight trace's samples.

### 3.2 After (gain 0.0216, the calibrated default)

```
pool mean 8.0 Hz/neuron = 0.400 x the 20 Hz physiological maximum, 0.018 x the
                          refractory ceiling
per-neuron: min 0.0  q1 0.0  median 0.0  q3 14.8  max 32.3 Hz
  ==    0 Hz       8   66.7%     (silent)
  (0,  3) Hz       0    0.0%
  [3, 12) Hz       0    0.0%     <- the measured flight working range
  [12, 20) Hz      1    8.3%     (in range: manoeuvring rates)
  [20,100) Hz      3   25.0%
  >=100 Hz         0    0.0%
predicted depolarisation 5.6 mV = 0.8x the 7.0 mV threshold gap
a: mean 0.1727  sd 0.0450  min 0.0000  p05 0.0970  median 0.1716  p95 0.2449  max 0.2921
   windows at the rail (a == 1.0): 0.0%      windows at zero: 0.3%
```

From the actual 12 s flight trace (`runs/analyze/trace.csv`, column `pow_l`, 601 samples):

| | before (gain 1.0) | after (gain 0.0216) |
| --- | --- | --- |
| actuator mean | 0.9951 | 0.1727 |
| sd | 0.0496 | 0.0450 |
| p05 / p50 / p95 | 0.9913 / 1.0000 / 1.0000 | 0.0970 / 0.1716 / 0.2449 |
| max | 1.0000 | 0.2921 |
| **samples pinned AT the rail** | **509 / 601 = 84.7 %** | **0 / 601 = 0.0 %** |
| samples at zero | 1 / 601 = 0.2 % | 2 / 601 = 0.3 % |

**It is graded. It is not a switch.** Before, the read-out was at 1.0000 for five
sixths of the run; now it spans 0.00-0.29 with a real distribution. The pool's *rate*
distribution moved from "12/12 neurons above 100 Hz" to "8/12 neurons at 0 Hz, one in
the 12-20 Hz manoeuvring band, three above 20 Hz" — a spread, not a rail.

### 3.3 Does the pool's rate vary with drive? Before vs after

This is the part the rail-pinning destroyed. Varying the stimulus rate
(`FLYVERSE_STIM_HZ`), gain 0.0216:

| vnc stimulus | pool mean | `a` mean | predicted depol. | network mean |
| --- | --- | --- | --- | --- |
| 0 Hz | 0.0 Hz | 0.000 | 1.6 mV = 0.2x gap | 13.86 Hz |
| 75 Hz | 0.0 Hz | 0.000 | 3.1 mV = 0.4x gap | 18.22 Hz |
| 150 Hz (default) | 8.0 Hz | 0.170 | 5.6 mV = 0.8x gap | 22.30 Hz |
| 300 Hz | 15.0 Hz | 0.301 | 7.6 mV = 1.1x gap | 29.04 Hz |

The pool now traverses its measured range as the drive changes: **0 -> 8 -> 15 Hz**,
and the actuator command with it (0 -> 0.17 -> 0.30). Before, the same ladder sat at
154.5 -> 330.1 Hz (the diagnosis doc's first table), i.e. it moved but never left the
rail region. The *ratio* of the pool rate to the network mean rate is now < 1 and
stimulus-dependent (~0.36 at the default), where before it was ~17.

### 3.4 The honest caveats on the gradedness

Three things that a reader should hold onto, none of which are hidden in the numbers
above:

- **The pool mean is carried by a minority of members.** 8 of 12 neurons are silent at
  the default drive and the per-neuron *median* is 0.0 Hz. Graded at the pool level,
  but a within-pool recruitment effect, not every member tracking the drive. That is
  what the master-slave DLM architecture does in reality (MN5 alone drives two fibres
  and the pool is electrically coupled), but it is not something this change models,
  and it is the largest remaining structural gap.
- **The raw 2 ms window read-out is near-binary.** One member firing in a 2 ms window
  already reads 41.7 Hz/neuron = 2.08x the pool's 20 Hz full scale, so a single spike
  clamps the window. 82.9 % of raw windows read exactly zero; 17.1 % contain >=1 spike;
  0 % contain all 12. The graded command exists because the 30 ms `MOTOR_TAU_S`
  transduction lag integrates the window occupancy — `a` is effectively
  `P(>=1 member fires in a window)` smoothed. That is a real muscle-transduction lag
  (DLM fibre AP decay tau ~14 ms; myoplasmic Ca decay tau ~79 ms; wingbeat change tau
  ~83 ms, Huerkey et al. Fig. 4b), not a filter bolted on to manufacture a graded
  signal — but it does mean the command's gradedness is bought at the window, not in
  the spike count.
- **The gap factor (1/4.9) is the weaker of the two justifications.** It compares a
  measured voltage excursion to this model's arbitrary 7 mV constant. It is
  defensible — in a mean-field LIF only the ratio matters — but it is not a measured
  per-cell *gain*; it is a measured *discrepancy* absorbed into a per-cell gain.

---

## 4. Does the fly fly?

Command: `./build.sh run --release -- analyze --seconds 12 --seed 7` (the default-run
numbers, seed 7, 12 s, everything on).

| | before (gain 1.0) | after (gain 0.0216) |
| --- | --- | --- |
| mode % | ground 0.8, takeoff 2.2, **cruise 97.0** | **ground 100.0**, takeoff 0, cruise 0.0 |
| takeoffs | 1 | **0** |
| altitude mean / max | 210.7 / 220.0 mm | **0.0 / 0.0 mm** |
| path | 995 mm (net 638, tortuosity 1.56) | 5.5 mm (net 5.5, tortuosity 1.00) |
| mean speed | 85 mm/s (p95 224) | **0 mm/s** |
| wall hits | 123 (615/min) | **0** |
| airborne time near a wall | 95.8 % | n/a (never airborne) |
| turning | 7.96 circles, mean abs yaw 5.36 rad/s | 0.00 circles, 0.00 rad/s |
| neural mean rate | 18.79 Hz | 22.21 Hz |

**No. The fly does not fly.** With the pool graded, it stays on the ground for the
whole 12 s: zero takeoffs, zero altitude, zero speed. The wing still beats —
`wing_amp` reaches 0.875 — but the force is far short.

This is the outcome the task said to expect, and it is reported as such rather than
tuned past. Graded at 3-12 Hz is a *weak* command in a body calibrated to be driven by
a rail.

### 4.1 Why, in arithmetic

**The read-out is exactly the window occupancy of the pool.** One member firing in a
2 ms window already reads 41.7 Hz/neuron = 2.08x the pool's 20 Hz full scale, so the
window clamps; the 30 ms lag then smooths it. The resulting actuator command is
therefore

```
a(f) = 1 - (1 - 0.002 * f)^12          f = pool rate, Hz/neuron
```

and this closed form reproduces every measurement in this document (to within 0.03
absolute; it slightly over-predicts above ~40 Hz because the 12 members are not
independent):

| pool rate f | a, formula | a, measured |
| --- | --- | --- |
| 8.0 Hz | 0.1745 | 0.1727 |
| 15.5 Hz | 0.3153 | 0.3152 |
| 27.7 Hz | 0.4968 | 0.5040 |
| 42.0 Hz | 0.6528 | 0.6417 |
| 80.8 Hz | 0.8795 | 0.8491 |
| 110.0 Hz | 0.9500 | 0.9281 |
| 154.5 Hz | 0.9883 | 0.9733 |

Its consequence is the whole result: the read-out is *compressive*, so `a = 1.0` is
approached only asymptotically, and the body needs a high `a`. The actuator-to-force
chain (from `wing-force-calibration.md` §3) is stroke amplitude
`amp = STROKE_AMP_MAX * (0.12 + 0.88 * a)` with aerodynamic force quadratic in
amplitude, so **lift ∝ (0.12 + 0.88 a)^2**. At `a = 1.0` (before) that factor is 1.00;
at `a = 0.17` (after) it is `(0.12 + 0.88*0.17)^2 = 0.270^2 = 0.073`. The calibrated run
delivers **7 % of the lift** the railed run delivered, against unchanged weight. That is
the whole story of the behaviour table.

**And the required command is outside the measured range.** Sweeping the per-cell gain
(`FLYVERSE_MN_GAIN`, everything else fixed) locates the takeoff threshold exactly:

| gain | pool f, Hz/neuron | x physiological max (20 Hz) | a mean | fly? |
| --- | --- | --- | --- | --- |
| **0.0216 (calibrated)** | **8.0** | **0.40** | **0.1727** | **no — ground 100 %, cruise 0 %, alt max 0.0 mm** |
| 0.03 | 15.5 | 0.77 | 0.3152 | no — ground 100 % |
| 0.04 | 27.7 | 1.39 | 0.5040 | no — ground 100 % |
| 0.05 | 42.0 | 2.10 | 0.6417 | no — ground 100 % |
| 0.10 | 80.8 | 4.04 | 0.8491 | **yes, but ragged**: takeoff 21.5 %, cruise 77.2 %, 7 takeoffs, alt mean 51.3 / max 219.3 mm, speed 218 mm/s |
| 0.15 | 110.0 | 5.50 | 0.9281 | yes: cruise 97.0 %, alt 200.7 / 220 mm |
| 0.25 | 154.5 | 7.72 | 0.9733 | yes: cruise 97.0 %, alt 212.4 / 220 mm |
| 1.00 (before) | 330.1 | 16.51 | 0.9951 | yes: cruise 97.0 %, alt 210.7 / 220 mm |

The takeoff threshold sits between `a = 0.64` (no flight) and `a = 0.85` (ragged,
intermittent flight), and *sustained* cruise needs `a >= 0.93`. In pool-rate terms:

```
takeoff requires     f_pool >= ~80 Hz/neuron   = 4.0x the measured physiological maximum
sustained cruise     f_pool >= ~110 Hz/neuron  = 5.5x the measured physiological maximum
```

against a measured in-flight range of 3-12 Hz. A physiologically-graded pool delivers
3-12 Hz, i.e. `a = 0.07-0.29` and 0.5-6 % of the lift. **There is no gain anywhere in
the measured range that lifts this body**, and the graded region (0.0216-0.0334) is a
long way below the takeoff region (>= 0.10).

---

## 5. Verdict: grounded or surrogate?

**Grounded.** The case, point by point, against the three ways this could have been a
surrogate:

1. *The citation is a real measured f-I, not a rate range.* Huerkey et al. 2023 *Nature*
   618, Fig. 1d,e: measured firing responses to current injection, approximately linear
   over 2-30 Hz in n = 15 animals, in-flight range ~3-12 Hz. Cited to figure.
2. *The mechanism is a measured per-cell property applied where the property acts.*
   The gain is the cell's input resistance (measured, 97 +/- 31 MOhm, this cell type),
   applied to the synaptic arrival term of that cell's own membrane equation — the
   place where R_in acts — with the artefact untouched and every other neuron at 1.0.
   Not a read-out multiplier, not a bias current, not a body-side fudge.
3. *The calibration was not fitted to the behaviour.* It was fixed by two
   independently measured ratios and one modelling choice (which measured small neuron
   stands in for the unmeasured MN9), and the choice is reported as a bracket: 0.0216
   against an MBON-alpha3 reference, 0.0334 against a projection-neuron reference. Both
   ends were computed before the behaviour was looked at. Both ends put the pool in the
   measured band (8.0-15.5 Hz) — and the fly does not fly at either. If the calibration
   had been reverse-engineered from the behaviour, the behaviour would look better.
4. *The change makes the target quantity graded, and the fly flies less.* A surrogate
   would have moved the scale so the fly flies; this moved the cell's gain so its rate
   is physiological, and the fly stopped flying, which is the honest cost and is
   reported as a failure of the behaviour, not as a success.

Two honest limitations, stated rather than buried:

- The gap factor (1/4.9) is a measured discrepancy folded into a per-cell gain, not a
  directly measured per-cell gain. It is the weakest link in §2.
- The graded pool's mean is carried by 4 of 12 members with 8 silent; the command is
  graded at the pool but the per-neuron distribution is bimodal (silent or 14-32 Hz).
  The pool's *rate* is graded over drive, but the pool is not a graded population in
  the sense of all members tracking.

---

## 6. The network operating point: compatible with a graded motor pool, or not?

**It is compatible — and that is the surprising half of the result.** At the closed-loop
operating point (network mean 19-22 Hz), the calibrated pool lands at **8.0 Hz/neuron**,
inside the measured 3-12 Hz in-flight range and comfortably inside the linear f-I out
to 30 Hz. The pool's rate is ~0.36x the network mean, and the relationship is
stimulus-dependent. Nothing about the whole-brain 19-22 Hz rate has to change for the
flight motor neurons to produce a physiological, graded command. The diagnosis's
implication that the network rate might be the thing to change is **not** what the
measurement shows: a graded pool exists at the current operating point.

**What is incompatible is the body.** The failure is downstream of the motor neurons.
Sweeping the gain (§4.1) puts the takeoff threshold at `a ~= 0.85`, i.e.
`f_pool >= ~80 Hz/neuron` — **4.0x the measured physiological maximum and 7-27x the
measured in-flight range** — and sustained cruise needs `>= ~110 Hz/neuron` (5.5x). A
physiologically-graded command can never satisfy that. So the project-level finding is:

> **This body model can only be lifted by a rail-level motor command: the flight force
> law, and the read-out scale it is fed by, were implicitly calibrated against a
> saturated pool. Making the motor neurons physiological removes the saturation that the
> body was flying on. Fixing this is a body/read-out task (the 2 ms window's 20 Hz
> full scale, the `0.12 + 0.88 a` amplitude map, or the lift gain), not a motor-neuron
> task, and it is out of scope for the per-cell calibration.**

The alternative — sliding the per-cell gain up until the fly flies again — would put the
pool back on the rail (gain >= 0.10, §4.1) and would be exactly the surrogate this work
is supposed to avoid.

---

## 7. Reproduce

```sh
cd flyverse

# gradedness + pool rate distribution, before and after
FLYVERSE_MN_GAIN=1.0   ./build.sh run --release -- mn-audit --seconds 4
                       ./build.sh run --release -- mn-audit --seconds 4

# does the pool rate track the drive?
FLYVERSE_MN_GAIN=0.0216 FLYVERSE_STIM_HZ=0   ./build.sh run --release -- mn-audit --seconds 4
FLYVERSE_MN_GAIN=0.0216 FLYVERSE_STIM_HZ=300 ./build.sh run --release -- mn-audit --seconds 4

# behaviour, before and after
FLYVERSE_MN_GAIN=1.0 ./build.sh run --release -- analyze --seconds 12 --seed 7
                     ./build.sh run --release -- analyze --seconds 12 --seed 7

# where does the fly start flying again?
FLYVERSE_MN_GAIN=0.04 ./build.sh run --release -- analyze --seconds 12 --seed 7

./build.sh test --release     # 33 tests
```

`mn-audit` now also reports the applied gain and cell list, the mean-field prediction
with the per-cell gain folded in, and the full gradedness block (per-neuron rate
distribution, actuator-command distribution, rail and zero fractions). `analyze`
prints the applied gain and records it in `summary.json` under
`neural.motor_power_mn_input_gain`.

---

## 8. Follow-up: the body-side task §6 handed off, now done

§6's project-level finding was that a physiologically-graded command can never lift a
body calibrated against a saturated pool, and that fixing it is a body/read-out task.
That task has been carried out; see **`docs/force-rate-map.md`**. In brief:

- The amplitude map `MAX * (0.12 + 0.88 a)` was replaced by the measured compressive
  relation `MAX * a^0.2415`, so lift goes as `f^0.483` — Gordon & Dickinson 2006's
  measured 1.7x power per 3x spike rate — instead of the `f^2` the linear map produced.
- The anchor moved from 20 Hz to **12 Hz** (`POWER_MN_MAX_HZ` -> `POWER_MN_FULL_STROKE_HZ`),
  the top of the measured 3-12 Hz in-flight band, because `a = 1` pins the full-stroke
  endpoint. Every "x physiological max (20 Hz)" column **in this note** should be
  re-read against 12 Hz (multiply by 1.67); the Hz figures are unaffected.
- The pool now hovers at **6.69 Hz/neuron** and cruises at ~5.9 Hz/neuron: inside the
  measured 3-12 Hz band, where before the same body needed 73 Hz to hover (6.1x the top
  of the band) and 110 Hz under §4.1's gain sweep for sustained cruise.
- Behaviour, 12 s seed 7: cruise 38.4 % (0 % before), 1 takeoff, altitude mean 31.2 /
  max 220 mm, speed 228 mm/s. Marginal rather than strong flight: the pool cruises at
  `lift/W ~ 0.94` against a 1.0 hover threshold, and 95 % of airborne samples are within
  20 mm of a wall in the 220 mm room.
- Tests are now **38**, not 33 (5 new).