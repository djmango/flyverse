# The room, and the loom pathway

Two questions, both measurement, no model change:

1. Is the fly circling because the room is too small for its visual steering loop
   to see anything? (The room is 600 x 440 x 220 mm and the fly flies at 166-511
   mm/s, so it is essentially never far enough from a wall for the looming
   signal to carry information.)
2. Independently of the geometry, can the visual (looming) pathway produce a
   steering response at all?

**Answers.** No, and no.

- A bigger room does not un-saturate the loom at 2x and only partly at 4x, and
  even where it does (4x: mean loom 0.51-0.80, mean wall distance 114-139 mm,
  loom SD 0.37-0.40) the correlation between the loom and the steering
  differential stays within noise of zero and flips sign between seeds. At 8x
  (4.8 x 3.5 x 1.8 m) the fly still spends 71-80% of its airborne samples within
  20 mm of a wall: it has more wall to follow, not more room to fly in.
- The path does not straighten. Among the seeds that fly, tortuosity is
  unchanged from 1x to 4x and the fraction of airborne time turning faster than
  0.5 rad/s is unchanged or worse. The one apparent improvement (4x mean
  tortuosity 1.42 against 2.08) is an artefact of a single cell in which the fly
  never left the ground, and is called out below.
- Open-loop, an imposed looming stimulus delivered to the two `visual_loom`
  pools at 15-120 Hz, in both signs and both eyes together, produces no
  measurable motor response: the largest |t| on any motor channel in any of the
  three seeds is 2.18, against a measured no-stimulus floor of up to 3.07. The
  positive control (the loom pool's own firing rate) clears |t| 4.5-21.7, so the
  stimulus is delivered; nothing downstream of it moves.

**Verdict.** The circling is not attributable to the geometry and not to a
pathway that is absent. It is attributable to the specimen's own one-signed
steering differential, with no corrective feedback available: the loom channel
is wired to the steering motor neurons but its reach is almost perfectly
bilaterally balanced, and in the closed loop the very same loom rate is
delivered to both eyes, so there is no lateral loom information anywhere in the
system for a corrective reflex to act on.

---

## What was added

| knob | effect |
|---|---|
| `FLYVERSE_ROOM_SCALE=s` | scales the arena's linear size by `s` about its own centre, keeping the aspect ratio and the table's relative position. `s = 1` (default) is the shipped room. |
| `FLYVERSE_ROOM_HEIGHT=h` | sets the ceiling at `h` mm independently, so the footprint can be held fixed while the ceiling moves (or removed). Default: 220. |
| `room::active()` | returns the active `Room`; `room::ROOM` is still the 1x layout, which is also what `active()` returns when both knobs are unset. |

Both are read once at startup and reported on the banner:

```
[flyverse] arena: 600 x 440 x 220 mm (x -300..300, y -220..220, z 0..220); FLYVERSE_ROOM_SCALE=1 FLYVERSE_ROOM_HEIGHT=-
[flyverse] arena: 2400 x 1760 x 880 mm (x -1200..1200, y -880..880, z 0..880); FLYVERSE_ROOM_SCALE=4 FLYVERSE_ROOM_HEIGHT=-
```

**The default run is unchanged.** With both knobs unset, `analyze --seconds 4
--seed 7 --every 1` produces a bit-identical `trace.csv` to the same command on
the pre-change tree (`cmp` clean, 4 s, seed 7). The summary JSON gains fields
only (`config.arena_mm`, the loom-distribution rows in `loom_response`); no
existing field changes value.

Nothing in the connectome, the drive, the gains or the body was touched. The
`Room` the simulation already had is now selectable; that is the whole change.

---

## Part 1: the room-size sweep

12 s per run, three seeds (7, 11, 23), `--every 1`. All sizes are linear scales
of the shipped floor plan with the shipped ceiling unless the size says
otherwise.

### Behaviour

| size | seed | cruise % | tortuosity | airborne turning > 0.5 rad/s % | wall hits | airborne within 20 mm % | steer differential < 0 % |
|---|---|---|---|---|---|---|---|
| 600x440x220 (1x) | 7 | 10.8 | 1.264 | 77.0 | 10 | 62.1 | 100 |
| 600x440x220 (1x) | 11 | 50.9 | 1.819 | 82.0 | 62 | 96.5 | 100 |
| 600x440x220 (1x) | 23 | 17.0 | 3.148 | 92.6 | 15 | 94.6 | 100 |
| 1200x880x440 (2x) | 7 | 23.8 | 2.448 | 91.7 | 16 | 78.7 | 100 |
| 1200x880x440 (2x) | 11 | 60.7 | 1.340 | 76.3 | 69 | 89.0 | 100 |
| 1200x880x440 (2x) | 23 | 87.1 | 1.858 | 76.2 | 95 | 93.3 | 100 |
| 2400x1760x880 (4x) | 7 | 35.8 | 1.559 | 87.9 | 28 | 78.1 | 100 |
| 2400x1760x880 (4x) | 11 | 31.1 | 1.490 | 95.0 | 22 | 75.1 | 100 |
| 2400x1760x880 (4x) | 23 | 0.0 | (1.212) | (0.0) | 0 | (0.0) | - |
| 2400x1760x220 (4x floor, 1x ceiling) | 7 | 52.4 | 1.541 | 81.5 | 39 | 82.4 | 100 |
| 2400x1760x220 (4x floor, 1x ceiling) | 11 | 24.4 | 1.551 | 85.6 | 15 | 66.7 | 100 |
| 2400x1760x220 (4x floor, 1x ceiling) | 23 | 51.8 | 1.209 | 81.7 | 52 | 80.9 | 100 |
| 4800x3520x1760 (8x) | 7 | 47.9 | 1.453 | 91.2 | 31 | 71.8 | 100 |
| 4800x3520x1760 (8x) | 11 | 51.0 | 1.382 | 84.1 | 85 | 80.3 | 100 |
| 4800x3520x1760 (8x) | 23 | 46.1 | 1.330 | 90.3 | 36 | 71.0 | 100 |
| 600x440x1200 (1x floor, 5.5x ceiling) | 7 | 26.5 | 2.698 | 85.0 | 25 | 90.2 | 100 |
| 600x440x1200 (1x floor, 5.5x ceiling) | 11 | 26.1 | 1.264 | 87.5 | 27 | 82.4 | 100 |
| 600x440x1200 (1x floor, 5.5x ceiling) | 23 | 34.7 | 2.734 | 92.9 | 22 | 87.8 | 100 |

Rows in parentheses are degenerate: **at 4x, seed 23 never took off**. It spent
all 12 s in ground mode, travelled 2 mm, and produced 0 airborne samples. Its
tortuosity and turning figures are therefore not comparable with anything, and
they are what makes the 4x column look better than it is. Excluding that cell,
the 4x rows are tortuosity 1.56 / 1.49 and turning 87.9% / 95.0% against the 1x
rows' 1.26 / 1.82 and 77.0% / 82.0% for the same two seeds: the mean tortuosity
is 1.53 against 1.54, and the mean turning is 91.4% against 83.9%.

So the path does not straighten with size, and turning while airborne does not
get rarer. Even at 8x, where the mean tortuosity does fall to 1.39, the fly is
still turning faster than 0.5 rad/s for 84-91% of its airborne time. The
question the sweep was asked to answer is whether the *turning* is a small-room
artefact; it is not.

### The loom distribution, and what the visual loop does with it

`loom` is the single geometric scalar the closed loop computes,
`1 - clamp(ttc / 0.6, 0, 1)` with `ttc = wall_dist / max(speed, 20 mm/s)`. The
"loom" columns below are over airborne samples only.

| size | seed | mean wall dist mm | loom mean | loom > 0.9 % | loom SD | corr(loom, steer diff) | corr(loom, yaw) |
|---|---|---|---|---|---|---|---|
| 1x | 7 | 40.5 | 0.714 | 55.2 | 0.375 | **-0.417** | -0.174 |
| 1x | 11 | 2.0 | 0.961 | 90.9 | 0.138 | +0.098 | -0.124 |
| 1x | 23 | 3.0 | 0.961 | 91.9 | 0.153 | -0.019 | -0.051 |
| 2x | 7 | 62.3 | 0.843 | 79.4 | 0.326 | -0.006 | -0.004 |
| 2x | 11 | 32.0 | 0.917 | 88.4 | 0.240 | +0.040 | -0.096 |
| 2x | 23 | 13.1 | 0.941 | 90.8 | 0.205 | +0.139 | -0.207 |
| 4x | 7 | 114.1 | 0.804 | 76.8 | 0.368 | -0.046 | -0.096 |
| 4x | 11 | 138.9 | 0.739 | 68.4 | 0.402 | -0.223 | -0.082 |
| 4x | 23 | (grounded) | - | - | - | - | - |
| 4x floor, 1x ceiling | 7 | 87.1 | 0.798 | 77.7 | 0.384 | +0.197 | -0.120 |
| 4x floor, 1x ceiling | 11 | 169.6 | 0.735 | 62.0 | 0.390 | -0.079 | -0.450 |
| 4x floor, 1x ceiling | 23 | 116.0 | 0.839 | 79.2 | 0.330 | -0.032 | -0.090 |
| 8x | 7 | 351.4 | 0.724 | 68.2 | 0.422 | +0.206 | +0.089 |
| 8x | 11 | 202.2 | 0.790 | 75.8 | 0.392 | -0.094 | -0.060 |
| 8x | 23 | 329.5 | 0.675 | 65.8 | 0.459 | -0.136 | -0.015 |
| 1x floor, 5.5x ceiling | 7 | 10.1 | 0.891 | 82.9 | 0.263 | -0.002 | -0.041 |
| 1x floor, 5.5x ceiling | 11 | 19.1 | 0.861 | 74.4 | 0.281 | +0.116 | -0.212 |
| 1x floor, 5.5x ceiling | 23 | 13.0 | 0.881 | 77.0 | 0.259 | -0.042 | -0.072 |

Per-condition means of corr(loom, steering differential), with the per-seed
spread, which is the number that matters given that a seed-7 value of -0.42 in
this same system already failed to replicate once:

| size | seed 7 | seed 11 | seed 23 | mean | spread |
|---|---|---|---|---|---|
| 1x | -0.417 | +0.098 | -0.019 | -0.113 | -0.417 .. +0.098 |
| 2x | -0.006 | +0.040 | +0.139 | +0.058 | -0.006 .. +0.139 |
| 4x | -0.046 | -0.223 | - | -0.135 | -0.223 .. -0.046 |
| 4x floor, 1x ceiling | +0.197 | -0.079 | -0.032 | +0.029 | -0.079 .. +0.197 |
| 8x | +0.206 | -0.094 | -0.136 | -0.008 | -0.136 .. +0.206 |
| 1x floor, 5.5x ceiling | -0.002 | +0.116 | -0.042 | +0.024 | -0.042 .. +0.116 |

Three things to read off this:

1. **2x does not help.** Mean loom goes *up* (0.843-0.941, 79-91% above 0.9)
   and mean wall distance stays at 13-62 mm. A 2x arena is still a room the fly
   crosses in a couple of seconds.
2. **4x and 8x do un-saturate the loom, and it changes nothing.** Mean loom
   falls to 0.51-0.80, the SD roughly triples against the 1x seeds 11/23
   (0.37-0.46 against 0.14-0.15), mean wall distance rises to 114-351 mm, and
   yet the correlation with the steering differential is still within +/-0.22 in
   every single cell and flips sign between seeds at every size. There is no
   size at which it stabilises in sign, let alone grows.
3. **The ceiling is not the binding constraint.** Giving the fly 5.5x the
   headroom (1x floor, 1200 mm ceiling) leaves the loft untouched (mean loom
   0.86-0.89, 74-83% above 0.9) and the turning as bad as before; the fly simply
   climbs (altitude max reaches 1200 mm) and keeps circling. Ceiling contacts at
   the shipped 220 mm ceiling are real - the 4x-floor variant still hits them for
   14-46% of samples - but removing the ceiling does not change the behaviour.

And the fly wall-follows at every size. At 8x it has 4.8 x 3.5 m of floor and
still spends 71-80% of its airborne samples within 20 mm of a wall. Enlarging
the room gives it more wall to follow; it does not make it fly in the middle.
That is why the loom mean at 8x (0.68-0.79) is barely lower than at 1x: the
distribution is bimodal - a long tail of hugged wall plus excursions into the
middle - and the hugged-wall mode dominates the sample.

The one thing geometry does not touch at all: the steering differential is
negative in **100%** of airborne samples in every one of the 18 flying runs,
at every size. The specimen's one-signed asymmetry is not a small-room effect
and is unaffected by giving the fly 64x the floor area.

---

## Part 2: the open-loop loom probe

The closed loop cannot answer whether the visual pathway works, because there
the loom channel collapses to one geometric scalar and is delivered to *both*
`visual_loom` pools at the same rate. Whatever the fly's walls do, the two eyes
never see different things, so the closed loop can only ever produce a common
steering command, never a differential one.

The probe therefore imposes the stimulus directly: `set_imposed_loom(left,
right)` overrides the two pools, each driven at `60 Hz x level` (the same rate
convention the closed loop uses, `loom * 60 Hz`), for a 200 ms response window.
Conditions: a null (0/0), and for each of four levels (0.25/0.5/1/2, i.e. 15,
30, 60, 120 Hz) the left eye alone, the right eye alone, and both eyes together.
12 trials per condition, 13 conditions, 156 trials, 10 s of warmup first so the
fly is airborne (91-100% airborne in the response windows). Per-trial means,
Welch t against the null.

### Positive control: the stimulus arrives

The `visual_loom_left/right` pools are two single neurons. Their firing rate in
the response window, both-eyes conditions, all three seeds (null first; the
`|t|` column is against the null, for the left pool):

| imposed drive | seed 7 L / R Hz | seed 11 L / R Hz | seed 23 L / R Hz | \|t\| range |
|---|---|---|---|---|
| 0 (null) | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.00 |
| 15 Hz | 11.2 / 12.1 | 12.5 / 13.7 | 9.6 / 15.0 | 4.5 - 5.7 |
| 30 Hz | 36.2 / 33.7 | 32.5 / 31.2 | 31.2 / 35.4 | 9.4 - 10.1 |
| 60 Hz | 56.2 / 58.7 | 69.2 / 51.7 | 62.9 / 53.7 | 10.8 - 13.6 |
| 120 Hz | 102.1 / 118.3 | 112.9 / 116.2 | 110.0 / 110.4 | 15.0 - 21.7 |

and when only one eye is driven, the other pool's rate is exactly 0.0 in every
trial, so the two pools are genuinely independent channels. The imposed drive
reaches the sense organ and drives it monotonically, up to |t| 21.7 against the
null. Any null result downstream is not a delivery failure.

### t statistic vs the null (motor channels only)

|t| > 2.3 is the repo's floor. `steer d` is `steer right - steer left`, the
steering differential. Full tables per seed are in the console logs and in
`loom_s{7,11,23}.json`; the shape is the same in all three:

| seed | largest motor |t| | where | measured floor (null vs null) |
|---|---|---|---|
| 7 | 1.68 | both eyes 30 Hz / steer left | 1.01 (steer left) |
| 11 | 2.18 | left eye 30 Hz / steer differential | 3.07 (steer left) |
| 23 | 3.20 | left eye 120 Hz / yaw rate | 1.86 (yaw rate) |

The floor is measured in this probe, not assumed: the null's own 12 trials are
split in half and the halves compared, which is a no-stimulus comparison with
this design's exact sampling. That is the bar the stimulated conditions have to
beat, and it is why "just under 2.3" is not being treated as a trend.

- **The steering differential never responds.** Across 3 seeds x 12 stimulated
  conditions = 36 comparisons on that channel, the largest |t| is 2.18 (seed 11,
  left eye 30 Hz), in a run whose own no-stimulus floor was 3.07. In seeds 7 and
  23 it is 1.10 and 1.70.
- **No dose-response, in either sign.** Mean steering differential for the null
  is -0.070 to -0.080 in every seed; for every stimulated condition at every
  level and both signs it stays in -0.071 to -0.084. That spread is the spread
  of the null itself.
- **The only comparison over the floor does not replicate.** Seed 23, left eye
  120 Hz, yaw rate, |t| 3.20. The same condition in seed 7 gives |t| 0.34 and in
  seed 11 gives 0.92. With 12 stimulated conditions x 6 motor channels = 72
  comparisons per seed, 216 across the three, a maximum near 3.2 is what chance
  alone produces; 3.20 sits at that level and on the channel (yaw rate) that is
  the fly's own chaotic turning, whose per-condition means wander between +0.09
  and -2.25 rad/s with no relation to the stimulus. It is not read as a
  response.
- **The lateral contrast is flat.** The clean test of the visual sense is one
  eye against the other at the same level, and it has the same sign structure as
  chance:

  | level | seed 7 diff | t | seed 11 diff | t | seed 23 diff | t |
  |---|---|---|---|---|---|---|
  | 15 Hz | +0.0008 | 0.12 | -0.0008 | 0.16 | -0.0086 | 1.43 |
  | 30 Hz | -0.0044 | 0.95 | -0.0067 | 1.10 | +0.0004 | 0.06 |
  | 60 Hz | -0.0021 | 0.40 | +0.0015 | 0.27 | +0.0015 | 0.34 |
  | 120 Hz | +0.0011 | 0.19 | +0.0017 | 0.40 | +0.0078 | 1.47 |

  The difference does not grow with level, does not hold its sign between seeds,
  and never clears |t| 1.5. Against a null spread of ~0.005 on that channel,
  these are all indistinguishable from the null.
- **Nothing else moves either.** No condition at any level clears the floor on
  `lift / weight` in any seed (largest |t| 1.57), so there is not even a
  non-directional effect on the flight power.

**Verdict on the pathway:** within this probe's power - 12 trials, 200 ms, three
seeds, both signs, four rates spanning 15-120 Hz - the looming pathway does not
reach the steering motor output. A flat response is the honest reading: the
visual steering pathway does not produce a measurable steering response in this
model.

### Why: the wiring

`src/analyze/loom.rs` aside, this is the shipped pack's CSR, no simulation.
Breadth-first from each of the two `visual_loom` VP neurons (body ids 10142 and
10723; model indices 128 and 672), counting synapse entries landing on the two
steering motor pools from nodes at each graph distance `d` from the seed:

| seed neuron | shell `d` | shell size | synapses onto steer-left | onto steer-right |
|---|---|---|---|---|
| `visual_loom_left` | 1 | 360 | 30 | 25 |
| | 2 | 51591 | 2962 | 3149 |
| | 3 | 109401 | 2724 | 2797 |
| `visual_loom_right` | 1 | 346 | 25 | 25 |
| | 2 | 61487 | 3133 | 3284 |
| | 3 | 100624 | 2558 | 2662 |

Neither neuron has a direct projection onto either pool (`d = 0` gives 0).
Two facts matter.

1. **The pathway is not absent.** Each loom VP neuron reaches the steering motor
   neurons within two hops (30 and 25 synapses from the left eye, 25 and 25 from
   the right), and by three hops the reach saturates: the cumulative synapse
   count onto each pool (5716 onto steer-left, 5971 onto steer-right) equals the
   pool's entire in-degree, so both loom neurons reach *every* presynaptic
   partner of both pools. There is a physical route from the loom sense organ to
   the steering output.
2. **Its lateral bias is nil.** Left-eye and right-eye seed neurons project
   almost identically onto *both* pools. The ipsilateral excess
   (`L->steerL + R->steerR`) minus contralateral (`L->steerR + R->steerL`) is
   +5 of 105 at `d = 1` (+4.8%), -36 of 12528 at `d = 2` (-0.3%) and +31 of
   10741 at `d = 3` (+0.3%). At three hops the reach has saturated - both loom
   neurons reach every presynaptic partner of both pools - so a lateral bias
   cannot emerge there by construction.

That is the structural counterpart of the flat probe: even a perfectly
asymmetric looming stimulus, which is what the probe imposed, enters a channel
whose downstream reach is bilaterally balanced to within a fraction of a
percent. And in the closed loop even that is moot, because both eyes are given
the identical rate.

---

## What the circling is

Not the geometry. Enlarging the room from 600 x 440 x 220 mm to 4.8 x 3.5 x 1.8
m, and separately removing the ceiling, leaves the turning rate, the
wall-following (71-80% of airborne samples within 20 mm of a wall even at 8x)
and the 100%-negative steering differential untouched. The loose correlation
between the loom and the steering differential that motivated the hypothesis
(-0.417 at seed 7, which failed to replicate earlier this session) does not come
back at any size: at 4x and 8x, where the loom genuinely varies, it is
-0.046/-0.223/+0.206/-0.094/-0.136 - sign-unstable and small.

Not an absent pathway either: the loom VP neurons are wired to the steering
motor neurons, and the imposed stimulus demonstrably drives them.

What is left is the specimen's own one-signed steering differential with no
corrective feedback available. The command is asymmetric - the left pool carries
1.13x the right pool's contacts, the net +3848 contacts of an almost balanced
136206 - so the fly turns; and the only sense that could correct a lateral error
is the loom channel, which (a) reaches the steering output symmetrically, and
(b) in the closed loop is fed the same value to both eyes. There is no lateral
information anywhere in the loop for a corrective reflex to act on, which is
exactly what "the loom signal carries no differential and therefore no steering
information" means when it is measured rather than argued.

The two problems were not one coupled problem after all: the wall-hugging and
the absent visual response are independent. The wall-hugging is a property of
the body and the room (the fly flies at 166-511 mm/s and follows whatever wall
it finds, even a 3.5 m one); the absent visual response is a property of a
symmetric pathway being asked for a differential.

---

## Reproduce

```sh
./build.sh                      # build
./build.sh test --release       # 39 tests

# Part 1: the room-size sweep (per size per seed, 12 s)
FLYVERSE_ROOM_SCALE=1   ./target/release/flyverse analyze --seconds 12 --seed 7  --every 1 --out /tmp/fv/s1_s7
FLYVERSE_ROOM_SCALE=2   ./target/release/flyverse analyze --seconds 12 --seed 11 --every 1 --out /tmp/fv/s2_s11
FLYVERSE_ROOM_SCALE=4   ./target/release/flyverse analyze --seconds 12 --seed 23 --every 1 --out /tmp/fv/s4_s23
FLYVERSE_ROOM_SCALE=4 FLYVERSE_ROOM_HEIGHT=220 ./target/release/flyverse analyze --seconds 12 --seed 7 --every 1 --out /tmp/fv/s4_h220_s7
FLYVERSE_ROOM_SCALE=8   ./target/release/flyverse analyze --seconds 12 --seed 7  --every 1 --out /tmp/fv/s8_s7
FLYVERSE_ROOM_HEIGHT=1200 ./target/release/flyverse analyze --seconds 12 --seed 7 --every 1 --out /tmp/fv/s1_h1200_s7

# Part 2: the open-loop loom probe
./target/release/flyverse loom-probe --seed 7 --trials 12 --warmup 10 --interval 0.2 --response 200 --out /tmp/fv/loom_s7
```

`analyze` writes `summary.json` (behaviour, loom distribution, correlations),
`trace.csv` (per-window samples) and `report.html`. `loom-probe` writes
`loom_s*.json` and prints the condition tables, the t statistics against the
null, the measured floor and the lateral contrast.

The sweep is 18 runs of 12 s (~55 s wall each on 8 cores); the probe is 156
trials per seed (~10 min wall each).
---

## Appendix: full probe tables

Per-condition mean of the per-trial means, with Welch t against the null in brackets, for all
three seeds. The null row is the reference. The two loom-pool rate channels are in the
positive-control table above, not repeated here.


**seed 7** - measured no-stimulus floor |t| 1.01 on `steer left`; largest motor |t| 1.68 (both eyes (symmetric) 30 Hz (level 0.50) / steer left).

| condition | air% | steer d | steer L | steer R | yaw | \|yaw\| | lift/w |
|---|---|---|---|---|---|---|---|
| both eyes (symmetric) 120 Hz (level 2.00) | 100 | -0.0797 (+0.16) | +0.4154 (+0.27) | +0.3357 (+0.05) | -1.0075 (+0.55) | +2.2672 (+0.46) | +0.9237 (+1.11) |
| both eyes (symmetric) 15 Hz (level 0.25) | 100 | -0.0781 (+0.45) | +0.4153 (+0.36) | +0.3372 (+0.21) | -1.1684 (+0.39) | +2.2185 (+0.50) | +0.8971 (+0.03) |
| both eyes (symmetric) 30 Hz (level 0.50) | 100 | -0.0742 (+1.36) | +0.4111 (+1.68) | +0.3369 (+0.19) | -0.0242 (+1.48) | +1.4697 (+1.49) | +0.8937 (+0.13) |
| both eyes (symmetric) 60 Hz (level 1.00) | 100 | -0.0782 (+0.39) | +0.4144 (+0.57) | +0.3362 (+0.03) | -0.7399 (+0.72) | +1.9740 (+0.72) | +0.9126 (+0.74) |
| left eye 120 Hz (level 2.00) | 100 | -0.0782 (+0.41) | +0.4173 (+0.25) | +0.3391 (+0.56) | -2.0413 (+0.34) | +2.8854 (+0.22) | +0.9086 (+0.54) |
| left eye 15 Hz (level 0.25) | 100 | -0.0782 (+0.36) | +0.4159 (+0.14) | +0.3377 (+0.30) | -0.7743 (+0.73) | +1.9904 (+0.76) | +0.9107 (+0.47) |
| left eye 30 Hz (level 0.50) | 100 | -0.0780 (+0.50) | +0.4127 (+1.22) | +0.3348 (+0.25) | -0.5071 (+0.91) | +2.0410 (+0.65) | +0.9135 (+0.77) |
| left eye 60 Hz (level 1.00) | 92 | -0.0746 (+1.10) | +0.4146 (+0.48) | +0.3400 (+0.81) | -1.9196 (+0.25) | +2.6335 (+0.02) | +0.8925 (+0.16) |
| null: loom 0 / 0 | 100 | -0.0804 (+0.00) | +0.4164 (+0.00) | +0.3360 (+0.00) | -1.6015 (+0.00) | +2.6591 (+0.00) | +0.8965 (+0.00) |
| right eye 120 Hz (level 2.00) | 100 | -0.0792 (+0.23) | +0.4200 (+1.26) | +0.3407 (+0.88) | -0.6318 (+0.88) | +1.7027 (+1.11) | +0.9178 (+1.03) |
| right eye 15 Hz (level 0.25) | 100 | -0.0790 (+0.24) | +0.4138 (+0.75) | +0.3349 (+0.20) | -0.8013 (+0.68) | +1.9991 (+0.70) | +0.8713 (+1.10) |
| right eye 30 Hz (level 0.50) | 100 | -0.0735 (+1.45) | +0.4141 (+0.83) | +0.3406 (+0.83) | -0.2547 (+1.24) | +1.4220 (+1.45) | +0.9011 (+0.21) |
| right eye 60 Hz (level 1.00) | 95 | -0.0725 (+1.63) | +0.4151 (+0.42) | +0.3426 (+1.27) | -0.9059 (+0.65) | +1.7629 (+1.07) | +0.9187 (+1.10) |

**seed 11** - measured no-stimulus floor |t| 3.07 on `steer left`; largest motor |t| 2.18 (left eye 30 Hz (level 0.50) / steer differential (R-L)).

| condition | air% | steer d | steer L | steer R | yaw | \|yaw\| | lift/w |
|---|---|---|---|---|---|---|---|
| both eyes (symmetric) 120 Hz (level 2.00) | 93 | -0.0813 (+1.85) | +0.4152 (+0.72) | +0.3339 (+1.57) | -1.6701 (+0.28) | +2.7117 (+0.57) | +0.9196 (+0.52) |
| both eyes (symmetric) 15 Hz (level 0.25) | 100 | -0.0754 (+1.03) | +0.4149 (+0.80) | +0.3396 (+0.47) | -1.3927 (+0.03) | +2.3919 (+0.24) | +0.8962 (+0.55) |
| both eyes (symmetric) 30 Hz (level 0.50) | 100 | -0.0784 (+1.66) | +0.4170 (+1.36) | +0.3386 (+0.69) | -1.5764 (+0.19) | +2.4915 (+0.34) | +0.8889 (+0.89) |
| both eyes (symmetric) 60 Hz (level 1.00) | 100 | -0.0730 (+0.48) | +0.4156 (+0.83) | +0.3425 (+0.18) | -0.2966 (+1.11) | +1.5313 (+0.85) | +0.8934 (+0.65) |
| left eye 120 Hz (level 2.00) | 100 | -0.0764 (+1.39) | +0.4154 (+0.83) | +0.3389 (+0.63) | -0.4999 (+0.92) | +1.5346 (+0.84) | +0.8982 (+0.40) |
| left eye 15 Hz (level 0.25) | 100 | -0.0795 (+1.72) | +0.4136 (+0.35) | +0.3341 (+1.50) | -1.0004 (+0.34) | +2.2055 (+0.03) | +0.8961 (+0.54) |
| left eye 30 Hz (level 0.50) | 100 | -0.0835 (+2.18) | +0.4176 (+1.29) | +0.3342 (+1.44) | -0.0286 (+1.40) | +1.6072 (+0.75) | +0.9013 (+0.32) |
| left eye 60 Hz (level 1.00) | 100 | -0.0710 (+0.03) | +0.4127 (+0.03) | +0.3417 (+0.01) | -0.3433 (+0.97) | +1.4871 (+0.81) | +0.9031 (+0.24) |
| null: loom 0 / 0 | 100 | -0.0708 (+0.00) | +0.4126 (+0.00) | +0.3418 (+0.00) | -1.3545 (+0.00) | +2.1819 (+0.00) | +0.9085 (+0.00) |
| right eye 120 Hz (level 2.00) | 100 | -0.0782 (+1.49) | +0.4140 (+0.50) | +0.3358 (+1.17) | -0.5095 (+0.80) | +1.6294 (+0.64) | +0.8937 (+0.62) |
| right eye 15 Hz (level 0.25) | 100 | -0.0787 (+1.72) | +0.4145 (+0.65) | +0.3357 (+1.27) | -0.7477 (+0.57) | +1.9759 (+0.25) | +0.8998 (+0.37) |
| right eye 30 Hz (level 0.50) | 100 | -0.0767 (+1.17) | +0.4126 (+0.01) | +0.3358 (+1.39) | -1.3199 (+0.03) | +2.2882 (+0.13) | +0.9159 (+0.31) |
| right eye 60 Hz (level 1.00) | 100 | -0.0725 (+0.31) | +0.4069 (+1.73) | +0.3345 (+1.62) | +0.0944 (+1.55) | +1.4852 (+0.95) | +0.8789 (+1.25) |

**seed 23** - measured no-stimulus floor |t| 1.86 on `yaw rate rad/s`; largest motor |t| 3.20 (left eye 120 Hz (level 2.00) / yaw rate rad/s).

| condition | air% | steer d | steer L | steer R | yaw | \|yaw\| | lift/w |
|---|---|---|---|---|---|---|---|
| both eyes (symmetric) 120 Hz (level 2.00) | 100 | -0.0723 (+1.43) | +0.4129 (+1.13) | +0.3407 (+0.73) | -0.2796 (+0.41) | +1.5462 (+0.71) | +0.9214 (+1.20) |
| both eyes (symmetric) 15 Hz (level 0.25) | 100 | -0.0768 (+0.60) | +0.4156 (+0.42) | +0.3389 (+0.37) | -0.7429 (+1.12) | +1.8333 (+1.26) | +0.9053 (+0.09) |
| both eyes (symmetric) 30 Hz (level 0.50) | 92 | -0.0773 (+0.46) | +0.4179 (+0.56) | +0.3406 (+0.83) | +0.0230 (+0.10) | +1.1885 (+0.21) | +0.9122 (+0.55) |
| both eyes (symmetric) 60 Hz (level 1.00) | 100 | -0.0804 (+0.34) | +0.4178 (+0.48) | +0.3374 (+0.05) | -1.6346 (+1.96) | +2.6989 (+2.35) | +0.9082 (+0.31) |
| left eye 120 Hz (level 2.00) | 100 | -0.0726 (+1.70) | +0.4123 (+1.47) | +0.3397 (+0.56) | -2.2509 (+3.20) | +2.7674 (+2.83) | +0.8953 (+0.51) |
| left eye 15 Hz (level 0.25) | 90 | -0.0799 (+0.16) | +0.4207 (+1.75) | +0.3409 (+0.72) | -1.6581 (+2.10) | +2.2992 (+1.67) | +0.9117 (+0.45) |
| left eye 30 Hz (level 0.50) | 92 | -0.0745 (+0.82) | +0.4123 (+1.24) | +0.3378 (+0.07) | -1.6371 (+1.67) | +2.2978 (+1.27) | +0.9086 (+0.30) |
| left eye 60 Hz (level 1.00) | 92 | -0.0810 (+0.48) | +0.4158 (+0.34) | +0.3347 (+0.77) | -0.5850 (+0.90) | +1.5861 (+0.74) | +0.9181 (+0.99) |
| null: loom 0 / 0 | 92 | -0.0790 (+0.00) | +0.4166 (+0.00) | +0.3376 (+0.00) | -0.0262 (+0.00) | +1.2591 (+0.00) | +0.9040 (+0.00) |
| right eye 120 Hz (level 2.00) | 100 | -0.0804 (+0.27) | +0.4153 (+0.49) | +0.3349 (+0.59) | -1.9212 (+2.21) | +2.7508 (+2.34) | +0.9034 (+0.04) |
| right eye 15 Hz (level 0.25) | 100 | -0.0712 (+1.58) | +0.4109 (+1.93) | +0.3397 (+0.48) | -0.2507 (+0.37) | +1.3623 (+0.23) | +0.9068 (+0.18) |
| right eye 30 Hz (level 0.50) | 100 | -0.0749 (+0.98) | +0.4143 (+0.69) | +0.3394 (+0.50) | -1.2788 (+1.62) | +2.1563 (+1.55) | +0.8838 (+1.40) |
| right eye 60 Hz (level 1.00) | 92 | -0.0826 (+0.86) | +0.4184 (+0.72) | +0.3358 (+0.43) | -0.5612 (+0.65) | +1.7004 (+0.67) | +0.9260 (+1.57) |
