# Why the flight-power pool fires at 150-334 Hz when a real DLM fires at 3-20 Hz

Diagnosis only. No default behaviour was changed; the two instruments added are
flag-gated and off by default (see §9).

**Verdict in one line.** The pool's rate is not a wiring, sign, or read-out
defect. It is the network's *operating point* against the LIF's threshold: the
power motor neurons carry ~2,024 net signed contacts each, so with
`w_syn = 0.275 mV` per contact they reach threshold when the contact-weighted
mean rate of their partners is **2.52 Hz**, and the closed loop runs the network
at a mean of **19.1 Hz** — 7.6x that. The pool is therefore driven ~34x past
threshold and sits at 0.73 of its refractory ceiling. There is **no grounded fix
in these files**, and the honest finding is that this LIF model, at the published
value of its single free parameter, cannot produce a 3-20 Hz motor neuron under
any sustained whole-brain drive this project has measured.

---

## 1. Measured baseline, per neuron

All numbers: full 166,700-neuron pack, seed 7, 4.0 s, `mn-audit`. Rates are
`spike_count / model_seconds` per neuron.

### `motor_flight_power_left` (12 neurons), 0 Hz vnc_sensory drive

`pool mean 154.5 Hz/neuron` = **7.73x** `POWER_MN_MAX_HZ` (20 Hz) = **0.340x** the
455 Hz refractory ceiling. Network mean 10.22 Hz/neuron; 27,725 of 166,700
neurons ever active (16.6%).

| model idx | spikes / 4 s | Hz |
|---|---|---|
| 144620 | 788 | 197.1 |
| 144854 | 449 | 112.3 |
| 144977 | 477 | 119.3 |
| 145741 | 457 | 114.3 |
| 145797 | 630 | 157.6 |
| 146566 | 252 | 63.0 |
| 147328 | 714 | 178.6 |
| 148280 | 776 | 194.1 |
| 150486 | 606 | 151.6 |
| 162037 | 669 | 167.3 |
| 164375 | 805 | 201.4 |
| 164389 | 789 | 197.3 |

### `motor_flight_power_left`, default drive (150 Hz vnc_sensory)

`pool mean 330.1 Hz/neuron` = **16.5x** the 20 Hz physiological maximum =
**0.726x** the refractory ceiling. Per neuron: 263.1, 274.9, 276.9, 277.4, 312.2,
316.4, 328.4, 329.7, 372.7, 384.4, 408.2, 417.0 Hz. Network mean 19.12 Hz;
35,400 neurons ever active (21.2%).

**Correction to the framing this task started from.** "The pool runs at ~150 Hz
with *no input at all*" is not what 0 Hz vnc_sensory means. Zeroing
`FLYVERSE_STIM_HZ` zeroes **one** of six external channels; the odour (up to
120 Hz on 2,045 olfactory neurons), the optic-flow proxy, the loom channel, the
haltere afferents and the leg tactile bristles all keep driving. Measured
ladder (all seed 7, 4 s):

| configuration | pool Hz/neuron | x 20 Hz max | network mean Hz |
|---|---|---|---|
| **every channel zeroed** (`FLYVERSE_NO_SENSE=1`) | **0.0** | 0.00 | **0.00** |
| vnc 0 Hz, all senses on | 154.5 | 7.73 | 10.22 |
| vnc 0 Hz, retina off | 153.2 | 7.66 | 8.62 |
| vnc 0 Hz, odour+retina+flow+haltere off (loom, leg tactile left) | 23.0 | 1.15 | 2.27 |
| vnc 150 Hz, odour+retina+flow+haltere off | 290.0 | 14.50 | 10.13 |
| vnc 150 Hz, everything on (default) | 330.1 | 16.51 | 19.12 |

With **every** drive channel at zero the network emits **exactly 0 spikes**
(`spikes total: 0`) and the fly stays on the ground. That is not an accident: the
LIF reset/rest state (`v = -52 mV`) is below threshold (`-45 mV`) and, with
`g = 0`, it is a fixed point of the update, so an unforced network has no
mechanism to ever fire. **The network has no spontaneous activity and no
self-sustaining recurrent loop.** Every spike in every run above is forced from
outside.

---

## 2. The neuron model: the actual parameters and their sources

`src/lif.rs:36-43` and `src/pack.rs:18`:

| repo | value | published original | source |
|---|---|---|---|
| `lif::DT_MS` (lif.rs:36) | 0.1 ms | integration step | — |
| `lif::REST_MV` (lif.rs:37) | −52 mV | `v_0 = v_rst = -52 mV` | Kakaria & de Bivort 2017, doi:10.3389/fnbeh.2017.00008 |
| `lif::THRESHOLD_MV` (lif.rs:38) | −45 mV | `v_th = -45 mV` | idem |
| `lif::TAU_M_MS` (lif.rs:39) | 20 ms | `t_mbr = 20 ms` ("capacitance * resistance = .002 uF * 10. Mohm") | idem |
| `lif::TAU_S_MS` (lif.rs:40) | 5 ms | `tau = 5 ms` | Jürgensen et al., doi:10.1088/2634-4386/ac3ba6 |
| `lif::REFRACTORY_MS` (lif.rs:41) | 2.2 ms | `t_rfc = 2.2 ms` | Lazar et al. 2021, doi:10.7554/eLife.62362 |
| `lif::DELAY_MS` (lif.rs:42) | 1.8 ms | `t_dly = 1.8 ms` | Paul et al. 2015, doi:10.3389/fncel.2015.00029 |
| `pack::WEIGHT_MV` (pack.rs:18) | 0.275 mV / contact | `w_syn = .275 * mV` — **labelled "Free parameter"** | Shiu et al. 2024, Nature 634:210-218, doi:10.1038/s41586-024-07763-9 |
| `lif::POISSON_MV` (lif.rs:43) | 68.75 mV | `w_syn * f_poi = 0.275 * 250` — "250 is sufficient to cause spiking" | idem |
| `sim::VNC_HZ_DEFAULT` (sim.rs:47) | 150 Hz | `r_poi = 150*Hz` — "default rate of the Poisson inputs" | idem |
| update order | `v = REST + (v-REST)*dm + g*coup` | `dv/dt = (v_0 - v + g)/t_mbr` | idem |

The published source is the transitive engine this repo ports: `philshiu/Drosophila_brain_model`
(`model.py`, `default_params`), which drives the paper's Brian2 model. Its
equations are

```
dv/dt = (v_0 - v + g) / t_mbr
dg/dt = -g / tau
```

and `src/lif.rs`'s discrete update is the **exact** solution of that system:
`coupling = TAU_S/(TAU_M - TAU_S) * (decay_m - decay_s)` is precisely the
convolution coefficient of the second equation into the first (with tau_s = 5,
tau_m = 20 that is `(5/-15)(dm - ds) = +0.014812/3 = 0.0049376`, i.e. the code's
`coupling`, and `coupling/(1 - decay_m) = 0.99`, which is the steady state
`v - v_0 = g` up to a 1 % discretisation residue). **The port of the neuron
model is faithful; the parameters are the published ones.** So the answer to
"is it the neuron model?" is not "a parameter was transcribed wrong".

Two things I checked and can rule out:

- **No unconditional bias current.** `lif.rs` adds nothing to `v` except the
  external stimulus (`POISSON_MV * count`, lif.rs:217). There is no tonic drive;
  the fixed-point argument in §1 confirms it operationally.
- **The 2.2 ms refractory is applied uniformly**, including to stimulated
  neurons — the published model exempts Poisson targets (`neu[i].rfc = 0 * ms`,
  "no refractory period for Poisson targets"). At `r_poi = 150 Hz` (one event
  per 6.67 ms) the 2.2 ms refractory is never in the way, so this is a real but
  **inert** port difference at these rates. It would matter only above 455 Hz.

### The arithmetic that sets the rate

Every constant collapses into one number. With `k` the voltage that one
contact-Hz of input produces:

```
k = dt * WEIGHT_MV / (1 - decay_s) * coupling / (1 - decay_m)
  = 1e-4 * 0.275 / 0.019801 * 0.0049376 / 0.0049875
  = 1.375e-3 mV per (contact * Hz)
```

A power motor neuron averages **2,024 net signed contacts**, so it reaches the
7 mV gap when its contact-weighted mean partner rate is

```
(gap / k) / C_net = (7 / 1.375e-3) / 2024 = 2.52 Hz
```

**That is the whole disease.** A motor neuron in this model is over threshold
whenever its partners average two and a half spikes per second, and the
histogram at the end of §4 shows the network does not operate anywhere near so
quietly.

Measured against the same formula (mean-field predicted depolarisation from the
partners' own measured rates, vs the observed pool rate):

| configuration | predicted (v−REST) | x gap | observed pool Hz |
|---|---|---|---|
| every channel zeroed | 0 mV | 0.0 | 0 |
| vnc 0 Hz, odour/retina/flow/haltere off | 8.9 mV | 1.3 | 23.0 |
| vnc 150 Hz, odour/retina/flow/haltere off | 198.4 mV | 28.3 | 290.0 |
| vnc 150 Hz, default | 241.7 mV | 34.5 | 330.1 |

Per neuron at the default drive the prediction ranges 140-415 mV against a 7 mV
gap (20.1x-59.2x). The pool's rate tracks the mean-field drive and **saturates**
by about 4x gap; it carries a graded signal only in the narrow band just above
threshold (~1x-3x gap) and a rail everywhere above. A real motor neuron's usable
range is two to three orders of magnitude of input; this one's is a factor of
about three.

---

## 3. The wiring: signs, balance, and loops

Measured from the pack, `motor_flight_power_left`:

```
5471 incoming edges; 47,959 excitatory contacts, 23,677 inhibitory contacts
E fraction 0.669; 1257 distinct presynaptic neurons
```

Per neuron: 319-588 presynaptic partners, 1,982-6,585 E contacts,
710-3,702 I contacts, E fraction 0.624-0.772, largest single edge 94-186
contacts, out-degree 0-11 (7 of the 12 are not presynaptic to anything).

- **Sign.** Inhibitory contacts enter with a negative `edge_write_mv`
  (`pack.rs:96-101`, `signed_counts * 0.275`) and are added into `g`, so they
  hyperpolarise. The pool's E fraction, 0.669, is close to the pack's global
  0.600 (14,745,137 / 24,559,135), i.e. slightly *more* excitatory than average
  but not anomalous. **No sign is inverted.**
- **Concentration.** 1,257 presynaptic neurons; the top-10 drivers hold only
  **12.2 %** of the pool's 71,636 contacts. There is no small dominant input.
- **Hot inputs.** At the default drive, 187 of 1,257 drivers run above 100 Hz
  and hold 24.2 % of the contacts — consistent with a network where a minority
  of well-connected cells sits near the rail (see the histogram below).
- **Loops.** Falsified operationally: with all input zeroed the network emits 0
  spikes, so nothing here self-sustains. The pool is driven, not oscillating.

Whole-network rate histogram at the default drive — the signature of a network
whose threshold is far below its drive:

```
      == 0 Hz   131300  78.76%      [50,100) Hz     4619   2.77%
     (0,1) Hz     3307   1.98%     [100,200) Hz    11071   6.64%
     [1,5) Hz     1617   0.97%     [200,455) Hz     3301   1.98%
    [5,10) Hz     1885   1.13%       >=455 Hz          0   0.00%
   [10,20) Hz     2049   1.23%     (refractory ceiling 454.5 Hz, 0 neurons above)
   [20,50) Hz     7551   4.53%
```

79 % of the brain is silent and 2 % is pinned between 200 Hz and the refractory
ceiling, with almost nothing in a physiological band. That is not a wiring
defect; it is a threshold-versus-drive mismatch. (The zero neurons above 455 Hz
is a clean sanity check on the engine.)

---

## 4. The read-out timescale: real, measured, and not the cause

The read-out is a spike **count** over the 2 ms control window
(`sim::WINDOW_STEPS = 20` steps of 0.1 ms), so its arithmetic
ceiling is `1000 / 2 = 500 Hz` and the 2.2 ms refractory guarantees at most one
spike per member per window. For a 12-neuron pool one spike in a window reads
**41.7 Hz**; the quantisation is 12 levels.

Measured over 1,999 windows at the default drive: mean **7.92 of 12** members
firing per window, and **0.2 % of windows (4 of 1999) read exactly zero**. Count
histogram: `0:4 1:6 2:18 3:46 4:71 5:152 6:225 7:267 8:341 9:337 10:290 11:171 12:71`.
So the window's *mean* is a valid rate estimator — it agrees with the
per-neuron counts of §1 — but each individual window is a coarse, almost never
zero, integer.

The same arithmetic for the signal the biology actually needs:

| true rate | P(>=1 spike in a 2 ms window) | period |
|---|---|---|
| 5 Hz | 0.010 | 200 ms |
| 12 Hz | 0.024 | 83 ms |
| 20 Hz | 0.039 | 50 ms |

A 12-neuron pool firing at a physiological **5 Hz** would have **P(>=1 of the 12
in a window) = 1 − 0.99^12 = 11.4 %**, i.e. the read-out would read **exactly
zero in 88.6 % of control windows**; at 12 Hz it would be zero in 75 %, at 20 Hz
in 62 %. To *contain* one spike of a 5 Hz command you need a window of at least
~200 ms. Because one extra spike per window is the read-out's unit of
resolution — a window holds `round(R * W / 1000)` spikes — a window of `W` ms
can separate two rates `R` and `R + Δ` only when `Δ * W / 1000 >= 1`, i.e.
`W >= 1000 / Δ`. Separating 3 Hz from 6 Hz (`Δ = 3 Hz`) therefore needs
**>= 333 ms**. A 50 ms window gives 20 Hz per spike, which is the shortest that
matches the actuator's own full scale but still cannot resolve the bottom of the
range.

**But the window is not the cause, and fixing it would not fix the rate.** The
rate is a property of the spikes; the window only estimates it, and its mean
estimator is unbiased. A 50 ms window would still report the pool at 330 Hz
(0.73 of ceiling) and `norm` would still read 1.0. The window is sized for a
signal of up to 500 Hz because the network produces one; it is a *symptom* of
the same mismatch. It matters in the other direction: with the rate corrected, a
2 ms window could not carry a graded command at all.

---

## 5. Root cause

**The neuron model's effective threshold, not the wiring, not the read-out.**

The LIF parameters are the published ones and the integrator is faithful. What
they imply, with the connectome's 2,024 net contacts on each power motor neuron,
is that threshold is reached at a contact-weighted partner rate of **2.52 Hz**.
The pool's own transfer function saturates within a factor of ~4 of that, so any
sustained whole-brain drive above a couple of Hz puts these neurons near the
refractory rail: 330 Hz at the default drive, 0.73 of the ceiling.

There is a second, independent reason the same conclusion is not an artefact of
this repo's configuration: **the published model's single free parameter,
`w_syn = 0.275 mV`, was calibrated for a saturating response to sparse
activation.** The paper states (doi:10.1038/s41586-024-07763-9): "The baseline
firing of each neuron in our model is 0 Hz"; "We chose `W_syn` such that
activation of sugar GRNs at 100 Hz resulted in roughly 80 % of maximal MN9
firing"; robustness was tested by moving `W_syn` by ±30 %; and the model is
described as predicting downstream firing "by driving activity in a sparse set
of neurons". Its own calibration datum is therefore "a 100 Hz drive on a small
population already gets one target near its maximum". This project asks the same
parameterisation for something it was never calibrated to give: a *sustained,
graded* motor command from a closed loop that drives 6,370 neurons (3.8 % of the
brain) at 150 Hz continuously and adds five more channels on top.

Ranked:

1. **Neuron-model operating point** (governing). `w_syn` x convergence vs the
   7 mV gap puts threshold at a 2.5 Hz partner rate; the closed loop runs the
   network at 10-19 Hz mean. Rank cause.
2. **Not the wiring.** Signs correct, E/I 0.669 near the pack's 0.600, input
   spread over 1,257 partners with the top 10 holding 12.2 %.
3. **Not a loop.** 0 spikes with all input zeroed.
4. **Not the read-out timescale.** A measurement limit that would matter only
   after the rate is fixed, and cannot change the rate.

---

## 6. Is there a grounded fix?

**No — not in these files, and not by any change I can ground in physiology or
in a demonstrable porting bug.** The reasoning, so the next session does not
have to re-derive it:

- The read-out cannot fix it. Its scale was already anchored correctly
  (`POWER_MN_MAX_HZ = 20`); at a true 330 Hz a 20 Hz anchor pins `a` at 1.0, and
  at 500 Hz it "reports" 0.67 of a drive that is 16x over full. No divisor turns
  a rail into a graded signal.
- The drive cannot be lowered. 150 Hz is not an engineered guess: it is the
  published model's own `r_poi = 150 * Hz`, and the vnc target set is documented
  as the reference engine's working point. Lowering it until the pool reads
  5 Hz is the surrogate the task forbids.
- `w_syn` is the only real lever (the published authors call it "the single free
  parameter", and note that scaling it "essentially results in scaling the
  distance between the resting potential and the firing threshold potential").
  But its only available calibration target is the published one — sugar GRNs at
  100 Hz -> ~80 % of maximal MN9 — and that target *is* a saturation target.
  Choosing a new `w_syn` requires a measured motor-neuron f-I target that this
  project does not have, and it re-tunes the whole 166,700-neuron network, not
  the flight pool. That is a research decision, not a diagnosis.
- Nothing else is on the table: no bias current to remove, no inverted sign, no
  runaway loop, no porting error in the parameters (§2) or the integrator.

**The reframing, stated plainly.** This LIF model is a stimulus-response
predictor: sparse activation in, downstream firing-rate change out, baseline
0 Hz, one free parameter calibrated to saturate one pathway. This project is
using it as a *behaviour generator* — a sustained closed loop with a dense,
continuous drive whose mean rate (10-19 Hz) is several times the network's own
threshold scale (2.5 Hz). In that role the motor pools cannot be graded, and
the flight-power pool in particular is structurally clamped near its refractory
ceiling. A 3-20 Hz DLM command is not reachable from this model as
parameterised; the fix, if the project wants one, has to come with a stated
physiological calibration target and be made at the network's operating point
(`w_syn`, or the drive regime), not in the read-out or the body.

---

## 7. Reproducing this

```
./build.sh build --release
FLYVERSE_STIM_HZ=0    ./build.sh run --release -- mn-audit --seconds 4 --top 15
FLYVERSE_STIM_HZ=150  ./build.sh run --release -- mn-audit --seconds 4 --top 12
FLYVERSE_NO_SENSE=1   ./build.sh run --release -- probe --seconds 2      # 0 spikes
FLYVERSE_STIM_HZ=0    FLYVERSE_NO_ODOR=1 FLYVERSE_NO_RETINA=1 FLYVERSE_NO_FLOW=1 \
                      FLYVERSE_NO_HALTERE=1 ./build.sh run --release -- mn-audit --seconds 4
```

## 8. Instruments added (all off the normal run path)

**Default behaviour is unchanged.** 12 s, seed 7, full model, after these edits
reproduces the state `docs/activation-scale.md` §6 records for the same run, to
the digit:

| | documented (activation-scale.md §6) | this tree, after the edits |
|---|---|---|
| takeoffs / cruise % | 1 / 97.0 | 1 / 97.0 |
| path (horizontal) | 995 mm | 995 mm |
| mean speed | 85 mm/s | 85 mm/s |
| wall hits | 123 (one every 0.1 s) | 123 (one every 0.1 s) |
| `flight_power_l` mean | 0.9951 | 0.9951428771018982 |
| `flight_steer_l` mean | 0.496 | 0.4961925745010376 |
| wing-power read-out | 0.9951 / 1.0 / 0.0 (mean/max/min) | rail: a 330 Hz pool on a 20 Hz anchor |
| tests | 33 | **33 passed** |

- `src/analyze/mn_audit.rs` (new) + `mn-audit` subcommand in `src/main.rs`:
  per-neuron rates, E/I input composition, per-window count histogram, the
  read-out resolution arithmetic, and the mean-field drive from the partners'
  measured rates. Measurement only; never called by `serve`, `analyze` or any
  other path.
- `FLYVERSE_NO_SENSE=1` (`src/sim.rs`, `World::no_sense`): zeroes every sensory
  drive channel and stops the retina emitting, for the total-ablation control.
  Unset by default; when unset the `sense()`/`advance()` code paths are
  bit-identical to before (the flag only guards the new branches).

## 9. Sources

- Shiu, Sterne, Spiller et al. (2024). *A Drosophila computational brain model
  reveals sensorimotor processing.* Nature 634:210-218.
  doi:10.1038/s41586-024-07763-9 — `W_syn = 0.275 mV` as the single free
  parameter, 0 Hz baseline, sugar-GRN/MN9 calibration, ±30 % robustness.
  Code: `philshiu/Drosophila_brain_model` (`model.py`, `default_params`), which
  carries the per-parameter citations reproduced in the table in §2.
- Gordon & Dickinson (2006), PNAS 103:4311. doi:10.1073/pnas.0510109103 —
  DLM motor neurons fire ~5 Hz in *Drosophila*, up to ~20 Hz in manoeuvres.
- Nature 618:118-124 (2023). doi:10.1038/s41586-023-06099-0 — 1 spike per ~20th
  to 40th wingbeat; tethered working range ~3-12 Hz.
- Lehmann, Skandalis & Berthe (2013), J. R. Soc. Interface 10:20121050.
  doi:10.1098/rsif.2012.1050 — 5-20 Hz maintains intramuscular calcium.
## 10. Follow-up: the per-cell motor-neuron calibration

This diagnosis is acted on in [`motor-mn-calibration.md`](motor-mn-calibration.md),
which gives the flight power motor neurons a per-cell input gain derived from the
measured input resistance of MN5 and the measured rheobase, and reports the resulting
gradedness and behaviour. Summary of that result: the pool becomes genuinely graded
(8.0 Hz/neuron at the default drive, actuator command spanning 0.00-0.29 instead of
pinned at 1.00 in 84.7 % of samples), and **the fly no longer flies** — because the
body's read-out and force chain require a rail-level command (pool >= ~80 Hz/neuron to
take off, against a measured maximum of 20 Hz). The network operating point is
compatible with a graded motor pool; this *body* is not.
