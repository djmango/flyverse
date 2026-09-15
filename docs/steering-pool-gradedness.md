# The flight steering read-out: pool, anchor, and whether steering is graded

This is the follow-up the previous note explicitly handed off
(`docs/activation-scale.md` §8, "Interaction with the steering read-out
(reported, not fixed)"):

> **`motor_flight_steering_*` is 3 neurons/side (b1-b3) while the spec names
> tp1, tp2 and hg1-hg4 (9/side), and 37 traced `wm` vc_motor neurons sit in no
> read-out at all.** That is a separate defect and is not touched here: the
> steering pools keep the window ceiling, so their read-out is still the
> fraction of the pool that fired, quantised to thirds, and my change does not
> alter it (`flight_steer_l` before/after: 0.518 -> 0.496).
>
> If the steering pool is widened, it should get its **own** physiological
> anchor rather than inheriting the window ceiling — and it will be a very
> different number from the power pools, because the steering muscles are
> *synchronous*: each is innervated by a single motor neuron firing about one
> spike per wingbeat, so their rates run to ~200 Hz, an order of magnitude above
> the power pools' 20 Hz. The scale mechanism added here
> (`phys_full_scale_hz`) is per-group and can carry that without touching the
> power pools.
> — `docs/activation-scale.md` §8 "Interaction with the steering read-out
> (reported, not fixed)"

Both halves of that hand-off are done here: the pool is widened to the spec's
own steering set, and it gets its own one-spike-per-wingbeat anchor. This note
reports whether that makes the steering command **graded**, whether the fly
flies **straighter**, and what the loom correlation does.

**Answer in one paragraph.** The pool widens cleanly from 3 to 9 per side — all
18 resolve in the annotation table and all 18 are in the compiled pack, zero
missing. The read-out quantisation step falls from **1/3 to 1/9**, and against
its own **200 Hz** one-spike-per-wingbeat anchor the command becomes a
continuous physical quantity with **0 % of samples on the rail**. But the
gradedness is *narrow*: the pool's rate is carried by 2 of the 9 members (b1 and
b3) firing at 350-400 Hz — **1.75-2.0x the one-spike-per-wingbeat maximum** —
while 7 members are silent. And the behaviour does **not** become a straighter
flight path: over 3 seeds the tortuosity is unchanged (2.068 -> 2.065), the
fraction of airborne time turning faster than 0.5 rad/s is unchanged (84.8 % ->
83.8 %), and the fly spends *less* time airborne (cruise 44.0 % -> 26.2 %). The
looming correlation looks dramatic at seed 7 (-0.09 -> -0.42) but does not
replicate at seeds 11 and 23 (+0.09, -0.04): **the looming response is still
absent, and the seed-7 value was a favourable draw.**

---

## 1. The pool: 3/side -> 9/side, all resolved

The steering groups are defined in `assets/male_cns_v1_neural_io.json` by the
same selector form the power pools use — `status == Traced and superclass ==
vnc_motor and subclass == wm and type in {...} and somaSide == {side}` — over the
Janelia MaleCNS annotation table (`annotations.feather`). The shipped set was
`{b1 MN, b2 MN, b3 MN}`; the spec's set (`physical-model-spec.md` §2: "Steering
muscles (tp1, tp2, hg1-hg4, b1-b3) act as **cycle-by-cycle** modulators of
stroke amplitude, stroke-plane tilt, and flip timing") adds `tp1 MN`, `tp2 MN`
and `hg1 MN`-`hg4 MN`.

`scripts/widen_steering_pool.py` re-resolves the two groups with the widened
type set and rewrites them in place. Deterministic; every other group, the group
ordering and the file's formatting are untouched. The diff is 30 insertions /
18 deletions, confined to the two steering entries and the totals they feed.

| | before | after |
|---|---|---|
| `motor_flight_steering_left` | 3 members | **9 members** |
| `motor_flight_steering_right` | 3 members | **9 members** |
| selected / present / missing (per side) | 3 / 3 / 0 | **9 / 9 / 0** |
| artifact totals | 2459 selected root IDs | 2471 |

**Every named muscle resolves.** `tp1 MN`, `tp2 MN`, `hg1 MN`, `hg2 MN`,
`hg3 MN`, `hg4 MN`, `b1 MN`, `b2 MN`, `b3 MN` each appear exactly once per side
in the traced `vnc_motor`/`wm` population, and all 18 selected body IDs are
present in the compiled pack. Nothing was unresolvable, so there is no
"cannot be resolved, and here is why" entry for the spec's steering set.

*(The remaining **25** traced `wm` motor neurons are still in no read-out, up
from 37 before this change: `STTMm` (4), `TTMn` (2), `MNwm35` (2), `MNwm36` (2),
`i1` (2), `i2` (2), `iii1` (2), `iii3` (2), `ps1` (2), `ps2` (2), `tpn` (2) and
1 untyped. The traced `vnc_motor`/`wm` population is 67 = 24 power + 18 steering
+ 25 other, so the 24-and-6 selectors that matched DLM/DVM and b1-b3 exactly are
confirmed again here.)* They are not in the spec's named steering set, so they
are outside this task; on the evidence below they are also silent in the closed
loop, which is why widening to them would not add resolution.

### The read-out resolution

Two different quantities improved, and they should not be conflated:

1. **The pool's spike-count resolution.** For an **unanchored** group
   `GroupRates::norm` is `rate_hz / window_hz`, which reduces exactly to the
   fraction of the pool that fired in the 2 ms control window,
   `count / group_size`. One member firing is therefore **1/3 of full scale**
   for the shipped 3-member pool and **1/9** for the widened 9-member pool; the
   achievable levels go from 4 (`0, 1/3, 2/3, 1`) to 10. Measured per-neuron raw
   count levels in the closed loop (seed 7, 4 s): **3 observed before
   (`[0, 1, 2]`) versus 5 after (`[0, 1, 2, 3, 4]`)** — the pool never fills in
   either case, so the realised level count is 3 vs 5, while the step each level
   represents falls from 1/3 to 1/9 of the pool.
2. **The form of the command.** Unanchored, the command *is* that count
   fraction, so it can only ever express 0, one-third, two-thirds or full
   authority — it cannot express "half of full steering authority" at all.
   Anchored, the command is the pool's tonic firing rate over the rate one spike
   per wingbeat asks for, `smooth_hz / 200` — a continuous physical quantity
   with no fixed quantisation step (see §3).

Both commands are then low-passed by the 30 ms `MOTOR_TAU_S` motor lag before
the wing sees them, so *post-lag* both take hundreds of distinct values and
neither is literally a staircase. The difference that matters is the scale and
the ceiling: the shipped read-out saturates as soon as all 3 members fire in one
window, whereas one member firing every window is only 1/9 of the widened
pool's read-out.

---

## 2. The anchor: 200 Hz, one spike per wingbeat — and why 12 Hz would be wrong

The power anchor is 12 Hz because the DLM/DVM are **asynchronous** indirect
flight muscles: the thorax oscillates at ~200 Hz on its own and the motor
neurons only set the activation level, firing 3-12 Hz
(`POWER_MN_FULL_STROKE_HZ`, Huerkey et al. 2023 *Nature* 618:118-124).

The steering muscles are the **opposite** case, and reusing 12 Hz for them would
be a straight category error: they are **synchronous** control muscles, each
innervated by a single excitatory motor neuron, and their spikes are
**phase-locked to the stroke cycle** rather than setting a slow activation
level — so their rates run at the wingbeat scale, an order of magnitude above
the power muscles.

> **`STEER_MN_FULL_SCALE_HZ = 200.0`** — one spike per wingbeat at the spec's
> pinned wingbeat `f = 200 Hz` (`physical-model-spec.md` line 243:
> "| Wingbeat frequency f | **200-220 Hz**, use **200 Hz** (hovering free
> flight); 212 Hz measured in one free-flight study | [M] | Fry, Sayaman &
> Dickinson 2005, J. Exp. Biol. 208:2303"; and line 525: "Wingbeat period
> T = 1/f = 5.0 ms at 200 Hz [M]").
>
> 200 Hz is the **top of the measured band**, chosen exactly the way
> `POWER_MN_FULL_STROKE_HZ` is the top of the power muscles' measured 3-12 Hz
> band: the measured steering-muscle rates span ~100 Hz (M.b2: "the frequency of
> muscle spikes within a burst is about 100 Hz, or 1 spike for every two wing
> beat cycles", Lehmann & Götz 1996) up to one spike per wingbeat at the spec's
> 200 Hz. The steering muscles are also heterogeneous — b1 is **tonic**, b2 is
> **phasic** (PMC9750141) — so a single full-scale is a band top, not a claim
> that every member fires at 200 Hz.

Citations, all primary statements about the steering muscle class:

- **Melis, J. M., Siwanowicz, I. & Dickinson, M. H. (2024)**, "Machine learning
  reveals the control mechanics of an insect wing hinge", *Nature*
  **628**(8009):795-803, `doi:10.1038/s41586-024-07293-4` (Janelia record
  confirms volume/issue/pages/DOI). This is the source of the "12 control
  muscles, with one neuron connected to each" framing — the Caltech write-up of
  the paper states it directly: "A fly's wing hinge contains 12 control muscles,
  with one neuron connected to each." The paper itself is the mechanical-control
  authority: a network trained on **steering-muscle activity predicts wing
  motion**, and the hinge model reproduces free-flight manoeuvres — i.e. the
  hinge *encodes* the control logic.
- **Melis et al. (2024)**, same paper, for the asynchronous contrast case
  quoted in §2 above: "Contractions of the large, indirect flight muscles (IFMs)
  are activated mechanically by stretch rather than by individual action
  potentials in their motor neurons — a specialization that permits the
  production of elevated power at high wingbeat frequency."
- **Lesser, E. et al. (2024)**, "Synaptic architecture of leg and wing premotor
  control networks in *Drosophila*", *Nature*, `doi:10.1038/s41586-024-07600-z`
  (`PMC11356479` / `PMC10312524`) — the connectome source for the wing motor
  system, and directly relevant to why a widened, independently-weighted
  steering pool is the right structure: "**wing premotor networks lack
  proportional synaptic connectivity, which may enable more flexible recruitment
  of wing steering muscles**", in explicit contrast to the leg's hierarchical
  size-principle recruitment. It also gives the population this repo's selectors
  draw from: "29 wing and thorax MNs that receive 144,668 synapses from 1,784
  preMNs".
- **Lindsay, T., Sustar, A. & Dickinson, M. (2017)**, "The Function and
  Organization of the Motor System Controlling Flight Maneuvers in Flies",
  *Curr. Biol.* — the anatomy of the 12 synchronous steering muscles and their
  four sclerite groups: "the basalars (b1, b2, and b3), the first axillaries
  (i1 and i2), the third axillaries (iii1, iii3, and iii4), and the fourth
  axillaries (hg1, hg2, hg3, and hg4)". This is the anatomical basis for the
  spec's own steering set, and it names 7 of the 9 widened members (b1-b3,
  hg1-hg4).
- **Lehmann, F.-O. & Götz, K. G. (1996)**, "Activation phase ensures kinematic
  efficacy in flight-steering muscles of *Drosophila melanogaster*", *J. Comp.
  Physiol. A* **179**(3):311-322, `doi:10.1007/BF00194985` — the phase-locking
  result and the ~100 Hz measured rate for M.b2 quoted above: "spike activity of
  the second basalar flight-control muscle (M.b2) is correlated with an increase
  in both the ipsilateral wing beat amplitude and the ipsilateral flight force."
- **Dickinson lab (PMC7307274)**, "Flies regulate wing motion via active control
  of a dual-function gyroscope" — the clean statement of the synchronous,
  cycle-by-cycle character: "Flies execute their remarkable aerial maneuvers
  using a set of wing steering muscles, which are **activated at specific phases
  of the stroke cycle**."
- **Generative summary of the class (PMC9750141)**, "Neuromuscular embodiment of
  feedback control elements in *Drosophila* flight" — "a set of **12 small,
  synchronous** muscles ... **each of which receives input from a sole
  excitatory motor neuron**"; also the tonic/phasic split (b1 integral, b2
  proportional) used above.
- **Teoh, H. K. et al. (2025)**, "How tp1, an indirect wing steering muscle,
  stabilizes *Drosophila*'s flight", bioRxiv `10.1101/2025.11.02.686144`
  (`PMC12637562`, preprint) — grounds why tp1/tp2 belong in the steering set even
  though they are tension muscles rather than sclerite muscles: tp1 is "an
  indirect wing steering muscle" whose optogenetic silencing impairs pitch
  stabilisation, and "the indirect wing steering muscles are capable of
  patterning the wing profile within a wing beat cycle".
- Repo-internal agreement already on record: `docs/physical-model-spec.md` §2
  (steering muscles named, cycle-by-cycle), `src/sim.rs`
  (`MN_CALIBRATED_GROUPS`: "the wing steering muscles are *synchronous*, each
  innervated by a single motor neuron firing about one spike per wingbeat
  (~200 Hz), so a 3-20 Hz asynchronous-muscle calibration would be the wrong
  cell type for them"), and `docs/haltere-probe.md`.

*(Both Melis et al. 2024 and Lesser et al. 2024 post-date the 12-muscle
anatomy above, and Lesser et al.'s female FANC reconstruction counts "29 wing
and thorax MNs" — the wing motor population is larger than the 12 sclerite
steering muscles because it also includes the tension muscles (tp, ps, tpn) and
TTM. The repo's annotation table resolves exactly that wider population: 67
traced `wm` neurons.)*

**What changed, in code.** `groups.rs`: new `STEER_MN_FULL_SCALE_HZ = 200.0`,
and `phys_full_scale_hz` now returns it for the two steering groups. The
per-group anchor mechanism (`Groups.group_scale_hz`) and the anchored
`GroupRates::norm` branch already existed; nothing about the read-out mechanics
changed, only which anchor the steering groups carry. Exact bytes of the
read-out change:

```
before:  a = (count / 3) / (500/500)          = count/3      , count in 0..=3
after:   a = tonic_rate_hz / 200              = pool_hz/200  , clamped to 1
```

### It would have been wrong to leave them unanchored *or* to give them 12 Hz

Two controls, both measured (12 s, seed 7, same body, same tree):

| state | steering pool | anchor | result |
|---|---|---|---|
| **A. shipped** | 3/side | none (window ceiling) | flies: cruise 38.4 %, 35 wall hits |
| **B. widened, unanchored** | 9/side | none (window ceiling) | crawls: altitude mean 0.6 mm, max 17.6 mm; cruise "73.5 %" is ground-skimming |
| **C. widened + 200 Hz** | 9/side | 200 Hz | flies higher/faster: altitude mean 85.4 mm, 504 mm/s |
| **D. anchor only, not widened** | 3/side | 200 Hz | **never leaves the ground** (0 takeoffs, 0 mm altitude) |

State B shows why the anchor is necessary: without it the widened pool's command
is the *fraction of the pool that fired*, and adding 7 silent members drops the
command from ~0.50 to ~0.17 — a rescale that has nothing to do with physiology.
State D shows why the **widening** is necessary for the anchor: the 3-member pool
fires at 245-253 Hz/neuron (2 of 3 members at 350-400 Hz), which is *above* the
200 Hz one-spike-per-wingbeat full scale, so anchoring the narrow pool rails the
tilt command at ~0.98 (p05 0.99) and the fly cannot leave the ground. The
widened pool reaches 83.8 Hz/neuron — 0.42 of its anchor — because 7 of the 9
members are silent.

---

## 3. Is the steering command graded? Yes — but narrowly

Measured with `mn-audit --group motor_flight_steering_left` (seed 7, 4 s; the
tool now reports each group against its **own** anchor, and prints a
steering-specific read-out block instead of the power-specific hover table).

| | before (3/side, unanchored) | after (9/side, 200 Hz anchor) |
|---|---|---|
| read-out form | fraction of pool fired, `count/3` | tonic pool rate / 200 Hz |
| raw quantisation step | **1/3 of full scale** | **1/9** nominal; continuous once anchored |
| raw count levels observed (of the pool) | 3 (`0,1,2` of 3) | 5 (`0,1,2,3,4` of 9) |
| one member firing in a window | 166.7 Hz = **0.33** of full scale | 55.6 Hz = **0.28** of the *anchor*, 0.11 of the count scale |
| command `a` mean / sd | 0.5005 / 0.0373 | 0.4078 / 0.0496 |
| command `a` min / max | 0.0000 / 0.5722 | 0.0000 / 0.4407 |
| command `a` p05 / p95 | 0.4621 / 0.5393 | 0.3804 / 0.4318 |
| **windows at the rail (`a == 1.0`)** | **0.0 %** | **0.0 %** |
| windows at zero | 0.1 % | 0.1 % |
| pool mean rate | 252.0 Hz/neuron = 0.504 x the 500 Hz window ceiling | **83.8 Hz/neuron = 0.419 x the anchor** |
| per-neuron: active members | 2 of 3 at 353.4 and 402.5 Hz | 2 of 9 at 350.7 and 402.5 Hz; 7 silent (0.0-0.5 Hz) |

*(Post-lag, 12 s trace, seed 7: the left command takes 446 distinct values
before and 327 after, so neither is literally a staircase once `MOTOR_TAU_S`
has smoothed it — the after command is confined to a **narrower** band
(0.000-0.447 versus 0.000-0.567) because it is a genuine rate that happens not
to move much, not because it is quantised.)*

**Graded: yes.** The command is now the pool's firing rate over the rate the
muscle can be asked for — a continuous physical quantity — with 0 % of samples
at the rail and 0 % essentially at zero. Before, the same quantity was the
fraction of a 3-neuron pool that fired in one window, i.e. a 4-level step
function that could not express "half of full steering authority" at all.

**But the honest caveats, and they matter:**

- **The pool is nominally a mixed tonic/phasic ensemble, but only the tonic part
  speaks.** Lindsay, Sustar & Dickinson (2017) found the fly steers with "large
  **phasically** active muscles capable of executing large changes and smaller
  **tonically** active muscles specialized for continuous fine-scaled
  adjustments" (and PMC9750141 identifies b1 as tonic, b2 as phasic) — exactly
  the two-class arrangement a graded command needs. In the closed loop only two
  members carry any rate at all, so the phasic half of the ensemble is not
  contributing the large changes it exists for.
- **The gradedness is carried by dilution, not by recruitment.** 7 of the 9
  members are silent at this operating point, and the pool's rate is set by b1
  (350.7 Hz) and b3 (402.5 Hz) — **1.75x and 2.0x the one-spike-per-wingbeat
  maximum**. The widened command is graded because the anchor (200 Hz) is high
  and the mean includes 7 near-zero members, not because 9 muscles jointly
  encode a graded command. This is the *same* over-drive defect the power pool
  had before its per-cell calibration (`motor-mn-calibration.md`), uncorrected
  here because the fix there was a measured input resistance for **MN5**
  (97 MOhm, a large DLM cell) and the steering MNs are a different, smaller cell
  class — its gain is not transferable, and inventing one would be the surrogate
  this project forbids.
- **The command's variance is still small.** Airborne `steer_l` spans
  0.372-0.439 (sd 0.018) in the closed loop, versus 0.406-0.567 (sd 0.025)
  before. The command is continuous now, but it does not swing across its range.

---

## 4. Does the fly fly a straighter path? No — and the count-based improvement does not survive normalisation

Frozen A/B, `analyze --seconds 12`, three seeds (7, 11, 23). Baseline binary is
a pristine copy of the pre-change tree (verified bit-identical to the frozen
seed-7 baseline except wall-clock fields). Both states use the identical body
model; the only differences are the two steering groups and their anchor.

| | before (3/side, unanchored) | after (9/side, 200 Hz) |
|---|---|---|
| takeoffs (mean of 3 seeds) | 1.33 | 2.00 |
| mode % ground / takeoff / cruise | 47.3 / 8.9 / **44.0** | 62.8 / 11.0 / **26.2** |
| altitude mean / max, mm | 25.0 / 216.3 | 43.6 / 219.3 |
| path, mm | 1320 | 1196 |
| **tortuosity** | **2.068** | **2.065** |
| mean speed, mm/s | 227 | 335 |
| wall hits | 44 | 29 |
| **wall hits per second AIRBORNE** | **6.75** | **6.12** |
| airborne samples within 20 mm of a wall | 93.9 % | 84.2 % |
| full circles airborne | 2.09 | 1.22 |
| **circles per second AIRBORNE** | **0.34** | **0.28** |
| mean `\|yaw rate\|`, rad/s | 2.306 | 1.709 |
| **% of airborne time turning > 0.5 rad/s** | **84.8 %** | **83.8 %** |
| mean `\|steer differential\|` | 0.090 | 0.072 |
| steering read-out at rail | 0.0 % | 0.0 % |

Reading the table honestly:

- **Tortuosity is unchanged** (2.068 -> 2.065). The fly does not fly a straighter
  path.
- **The wall-hit and circle counts fall (44 -> 29, 2.09 -> 1.22), but that is
  mostly because the fly spends less time airborne** (cruise 44.0 % -> 26.2 %).
  Normalised per airborne second the falls are much smaller and inside the seed
  spread: wall hits 6.75 -> 6.12, circles 0.34 -> 0.28.
- **The turning is still continuous.** 83.8 % of airborne time is spent turning
  faster than 0.5 rad/s, against 84.8 % before. A free-flying *Drosophila*
  saccades at about 0.5 Hz between straighter flights; this fly turns essentially
  always. The one genuine turn-down is the mean rate of turn when turning
  (`|yaw rate|` 2.31 -> 1.71 rad/s).
- The steering differential is still **one-signed** in every airborne sample
  (seed 7: range `[-0.106, -0.026]`), so the yaw torque is persistently in one
  direction — the "yaw attribution" question from §7 of `force-rate-map.md` is
  unchanged: the circling is a standing asymmetry in the connectome's own
  steering output, not a noise-driven random walk.

Note on `net_horizontal_displacement_mm`: it is **exactly 638.201** in all three
baseline runs and in one of the after runs (seed 11), and differs in the other
two (634.3, 523.8). A value that repeats exactly across independent runs is a
property of the wall the fly finally settles against, not of the flight, so it
carries no information about straightness here.

---

## 5. The loom finding: the pathway is live, the signal is not

`corr(loom, steer differential)` was reported as about -0.09 (a previous run).

| seed | before | after |
|---|---|---|
| 7 | -0.094 | **-0.419** |
| 11 | +0.028 | +0.090 |
| 23 | +0.031 | -0.035 |
| **mean** | **-0.012** | **-0.121** |

**The seed-7 value does not replicate.** Across three seeds the baseline mean is
-0.012 and the after mean is -0.121, both dominated by a single seed and with
sign flips. So the honest finding is: **the looming response is still
essentially absent, and the -0.42 at seed 7 must not be reported as a
response.**

Which of the three candidate causes is it? Measured, not assumed:

- **Not the steering command saturating.** The read-out is at the rail in **0 %
  of samples** in both states (`summary.json: yaw.pct_steering_readout_saturated`),
  and the per-sample `steer_l`/`steer_r` histograms show no pile-up at 1.0. The
  command has room to move; it does not use it.
- **Not the visual pathway failing to reach the pool.** The loom channel is
  delivered — `sim.rs` sets `d_loom_l/d_loom_r.rate_hz = loom * 60.0` onto
  `visual_loom_left/right` (1 MeVP24 neuron per side) every control window — and
  the correlation *does* move when the read-out scale changes (-0.012 ->
  -0.121 mean). A dead pathway could not do that. So the connectome does receive
  the looming signal and it does reach the steering motor neurons.
- **The signal itself has almost no dynamic range.** This is the real cause.
  In the baseline run, 85 % of airborne samples have `loom > 0.9` and only 2 %
  have `loom < 0.5`; the mean is 0.948. The fly hugs walls (95 % of airborne
  samples within 20 mm), so the wall-proximity channel is pinned near its
  ceiling almost all the time and there is almost nothing for a correlation to
  track — the baseline "calm" bucket has **3 samples**. The loom proxy is
  `1 - clamp(ttc/0.6, 0, 1)` with `ttc = wall_distance / max(speed, 20 mm/s)`,
  so at flight speeds it is 1.0 whenever the wall ahead is nearer than
  ~0.6 x speed (120-300 mm) — which, in a 600 x 440 x 220 mm room, is
  most of the time.

So: the loom channel is delivered, it reaches the steering pool, the command is
not saturated — and the response is still not measurable, because the stimulus
is a near-constant. That is a **stimulus-and-room** limitation before it is a
motor-pathway one, and it matches `docs/haltere-probe.md`'s conclusion that "the
rotation signal is simply not reaching the wing motor neurons with enough weight
to see" for a signal that *does* vary. A loom correlation cannot be measured in
a regime where loom does not vary.

---

## 6. Verdict

- **The pool is the spec's steering set.** 9 per side (tp1, tp2, hg1-hg4,
  b1-b3), all 18 resolved from the same annotation source the power pools use,
  all present in the pack. Read-out quantisation 1/3 -> 1/9.
- **The anchor is 200 Hz, one spike per wingbeat**, cited to Melis, Siwanowicz &
  Dickinson 2024 (*Nature* 628(8009):795-803, `doi:10.1038/s41586-024-07293-4`),
  Lindsay, Sustar & Dickinson 2017 (*Curr. Biol.* 27(3):345-358), Lehmann & Götz
  1996 (*J. Comp. Physiol. A* 179(3):311-322), Lesser et al. 2024 (*Nature*,
  `PMC11356479`) and the repo's own spec §2 — and it is the **top of the measured
  steering band** (~100 Hz for M.b2 per Lehmann & Götz, up to one spike per
  wingbeat), the same way the power anchor is the top of the measured 3-12 Hz
  band. It is **not** 12 Hz, because the steering muscles are synchronous and the
  power muscles are asynchronous — reusing the power anchor would be a category
  error.
- **The steering command is now graded: continuous in the pool's firing rate,
  0 % of samples on the rail, 5 raw count levels of 9 instead of 3 of 3.** It is
  graded *narrowly*: 7 of the 9 members are silent and 2 carry the rate at
  1.75-2.0x the one-spike-per-wingbeat maximum.
- **The fly does NOT fly a straighter path.** Tortuosity is unchanged
  (2.068 -> 2.065) and 84 % of airborne time is still spent turning faster than
  0.5 rad/s. Wall hits and circles fall only roughly in proportion to the
  airborne time, and the fly is airborne *less* (cruise 44.0 % -> 26.2 %).
- **The looming response is still absent.** The seed-7 -0.42 does not replicate
  across seeds. It is not saturation (0 % at rail) and not a missing pathway
  (the channel is delivered and the correlation moves with the read-out scale);
  it is a **near-constant stimulus** — 85 % of airborne samples sit at
  `loom > 0.9` because the fly hugs the walls, so there is nothing to correlate
  against.

---

## 7. Tests

`./build.sh test --release`: **39 passed, 0 failed** — the 38 pre-existing plus
one new. No pre-existing test's expected value was weakened; one assertion was
**corrected**, and one test added:

- `groups::tests::flight_power_readout_is_scaled_by_the_full_stroke_rate` — the
  assertion `assert_eq!(phys_full_scale_hz("motor_flight_steering_left"), None)`
  was **wrong to exist**: it asserted that the steering pools carry no anchor,
  i.e. it pinned the diagnosed defect (a steering read-out stuck on the 2 ms
  window ceiling, quantised to the fraction of the pool) as if it were a
  property of the read-out. The test's own comment said "Only the
  actuator-driving pools are anchored **so far**". It now asserts the steering
  pools' own anchor, the structural relation
  `STEER_MN_FULL_SCALE_HZ > 10 x POWER_MN_FULL_STROKE_HZ`, and that an
  unanchored group (now exemplified by `motor_walking_left`) still keeps the
  window ceiling. The power assertions are untouched.
- `groups::tests::steering_readout_is_linear_against_the_one_spike_per_wingbeat_anchor`
  (new) — pins the property the whole change exists for: the steering command is
  linear in the pool rate against the 200 Hz anchor (50/100/200 Hz -> 0.25/0.5/1.0),
  clamps above one spike per wingbeat, and that `commit` wires the steering anchor
  from the group name. Previously the same rate would have read a 4-level
  fraction-of-pool value.

`mn_audit.rs` was also corrected so a steering audit is not misreported: the
pool-mean line now names the group's own anchor, and a non-power group gets a
"STEERING READ-OUT RESOLUTION" block instead of the power-specific force/hover
table (a hover rate is not a steering quantity).

## 8. Reproduce

```sh
# widen the two steering groups in the artifact (deterministic, in place)
python3 scripts/widen_steering_pool.py \
    --annotations /path/to/annotations.feather \
    --pack        /path/to/official-pack/neuron_ids.npy \
    --io          assets/male_cns_v1_neural_io.json

# the read-out, against the group's own anchor
./build.sh run --release -- mn-audit --group motor_flight_steering_left  --seconds 4
./build.sh run --release -- mn-audit --group motor_flight_steering_right --seconds 4

# frozen A/B behaviour (seed 7; any seed reproduces bit-for-bit)
./build.sh run --release -- analyze --seconds 12 --seed 7

./build.sh test --release     # 39
```

The four control states in §2 were produced with the same binary by swapping
only the artifact (`FLYVERSE_IO_JSON=`) and the code state, so each difference
is attributable to the pool or the anchor alone.

## 9. Sources

- Melis, J. M., Siwanowicz, I. & Dickinson, M. H. (2024). Machine learning
  reveals the control mechanics of an insect wing hinge. *Nature*
  **628**(8009):795-803. `doi:10.1038/s41586-024-07293-4`.
- Balint, C. N. & Dickinson, M. H. (2001). The correlation between wing
  kinematics and steering muscle activity in the blowfly *Calliphora vicina*.
  *J. Exp. Biol.* **204**:4213-4226.
- Lehmann, F.-O. & Götz, K. G. (1996). Activation phase ensures kinematic
  efficacy in flight-steering muscles of *Drosophila melanogaster*. *J. Comp.
  Physiol. A* **179**(3):311-322. `doi:10.1007/BF00194985` — M.b2 spikes at
  "about 100 Hz, or 1 spike for every two wing beat cycles"; the lower end of
  the steering band the anchor tops out.
- Lesser, E., Azevedo, A. W., Phelps, J. S. et al. (2024). Synaptic architecture
  of leg and wing premotor control networks in *Drosophila*. *Nature*.
  `doi:10.1038/s41586-024-07600-z` (`PMC11356479`, `PMC10312524`) — the wing
  motor connectome: "29 wing and thorax MNs that receive 144,668 synapses from
  1,784 preMNs", and "wing premotor networks lack proportional synaptic
  connectivity, which may enable more flexible recruitment of wing steering
  muscles".
- Lindsay, T., Sustar, A. & Dickinson, M. H. (2017). The function and
  organization of the motor system controlling flight maneuvers in flies.
  *Curr. Biol.* **27**(3):345-358. `doi:10.1016/j.cub.2016.12.018` — the 12
  synchronous steering muscles per wing and the sclerite grouping that names
  b1-b3 and hg1-hg4; also the tonic/phasic functional split: "each of the four
  skeletal elements at the base of the wing are equipped with both large
  **phasically** active muscles capable of executing large changes and smaller
  **tonically** active muscles specialized for continuous fine-scaled
  adjustments."
- "Neuromuscular embodiment of feedback control elements in *Drosophila*
  flight" (`PMC9750141`) — "12 small, synchronous muscles ... each of which
  receives input from a sole excitatory motor neuron"; the tonic (b1) / phasic
  (b2) split.
- "Flies regulate wing motion via active control of a dual-function gyroscope"
  (`PMC7307274`) — steering muscles are "activated at specific phases of the
  stroke cycle".
- Teoh, H. K., Biswas, D., Leung, A. et al. (2025). How tp1, an indirect wing
  steering muscle, stabilizes *Drosophila*'s flight. bioRxiv
  `10.1101/2025.11.02.686144` (`PMC12637562`, preprint) — tp1 is an
  indirect/tension wing steering muscle whose silencing impairs pitch
  stabilisation; the tp1 MN is resolved here from the same annotation table and
  is silent in the closed loop.
- Huerkey, Schueler, Ryglewski, Duch, Schreiber, Silies et al. (2023). Gap
  junctions desynchronize a neural circuit to stabilize insect flight. *Nature*
  **618**:118-124. `doi:10.1038/s41586-023-06099-0` — the **asynchronous** power
  motor neurons, the contrast case (`POWER_MN_FULL_STROKE_HZ`).
- Fry, S. N., Sayaman, R. & Dickinson, M. H. (2005). The aerodynamics of
  hovering flight in *Drosophila*. *J. Exp. Biol.* **208**:2303-2318 — the
  spec's own `[M]` source for the pinned wingbeat frequency (200 Hz) that sets
  the steering anchor.
- This repo: `docs/physical-model-spec.md` §2 (steering muscle set, wingbeat
  200 Hz, cycle-by-cycle), `docs/activation-scale.md` (the hand-off this note
  discharges), `docs/motor-mn-calibration.md` (the per-cell over-drive defect),
  `docs/haltere-probe.md` (the rotation-signal-not-reaching-MNs negative),
  `docs/force-rate-map.md` (the power-side read-out and behaviour).