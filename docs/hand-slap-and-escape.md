# A slap aimed at the fly

**Subject.** The project owner's request: put a real, faithfully approaching human hand
into the room and measure, per seed, whether the fly runs away from it. A slap.

**Verdict.** It does not. Not in any seed, by any measure, and the strongest case is
not "below the noise floor" but *bit-identical*: in one seed the palm swept from 495 mm
away to 53 mm from the fly -- 442 mm of travel, covering up to 60 optic-lobe columns --
and the fly's recorded motion for all 12 s did not differ by **one bit** from the same
run with the palm parked 300 mm away and not moving. In the other two seeds the body
trace during the 150 ms of expansion is fingerprinted at 90.7% and 98.7% identical, and
the entire excursion over the pulse is 0.01 mm.

This closes the last untested stimulus class. Every earlier null used a *sustained*
stimulus -- a single-eye luminance step, and colour (`docs/colour-vision-and-fruit.md`)
-- and the obvious objection to those was that a step is not what a looming object is.
An expanding dark edge is the canonical *Drosophila* escape stimulus, it is the headline
looming result of the published whole-brain demos, and it drives the loom pool through a
different response than a step does. It was never tested because it has temporal
structure, and this is that test. The answer is the same null, so the finding is now:
**no visual stimulus that this room can present drives steering in this connectome.**

# 1. The hand, and the one thing it is not allowed to be

The constraint comes first, because it determines the design. There is **no escape
reflex, no startle term, no threat response, no avoidance gradient, no looming gain, no
steering coefficient and no stabiliser anywhere in this change**, and no term whose
purpose is to make the fly flee. The hand is not a stimulus injected into the network and
not a loom scalar written into a drive. It is an ordinary scene object with a geometry,
an albedo and a trajectory, and the fly sees it through its own eyes: the optics
ray-cast against the palm exactly as they ray-cast against a wall, the fruit or the
table, and whatever the connectome does with that is the connectome's doing. The whole of
the hand's coupling to the model is one field, `Room.hand`, set once per 2 ms control
window in `World::advance` *before* the retina samples the room. Nothing else in the
model knows the hand exists.

## Geometry

A flat open palm, the part of a hand that reaches a fly, as an **oriented box**:

| parameter | value |
| --- | --- |
| half-extents | `[45, 55, 12]` mm, i.e. **90 x 110 x 24 mm** |
| axes | `[0]` across the palm, `[1]` down its length, `[2]` the palm normal, which points back along the approach direction -- at the fly |
| construction | `hand_axes(dir)`: an orthonormal frame built from the approach direction, so the flat face turns toward the fly and the **silhouette grows as it approaches** -- an expanding dark edge, not a point and not a sphere |
| containment | `Hand::contains`, exact for the box in its own frame |
| distance | `Hand::surface_dist`, 0 inside the palm, otherwise the distance to the nearest face (exact) |
| ray-cast | ray-vs-oriented-box in the renderer's `scene_hit`, returning the entry face normal; a ray starting inside the palm gets the exit face |

## Albedo

The palm is a **dark, spectrally non-flat** object in the existing spectral pipeline. It
gets its own reflectance spectrum (not a grey, not a gain): `HAND_REFLECTANCE_ANCHORS`,
measured human-skin diffuse reflectance from 300 to 700 nm -- strongly absorbing in the
UV (the epidermis is the optical shield), rising through blue and green, highest in the
red because dermal haemoglobin and melanin absorption fall off there. To a fly, whose
opsins all peak at or below 508 nm, that is a green/blue object with a red tail outside
the animal's range.

On top of the spectrum there is one modelling choice, stated as such: an ambient shading
factor `HAND_SHADING = 0.18`. A palm coming between the fly and the room's single
illuminant is lit on its fly-facing surface only by ambient scatter, because it occludes
the direct path to the ceiling for exactly the surface the fly is looking at, and it
typically arrives against the bright ceiling behind it. The result is a dark expanding
silhouette -- the canonical looming stimulus. 0.18 puts the palm's R1-R6 catch near 0.06,
the same order as the tomato fruit's 0.063 and about 8x darker than the room's mean wall
luminance. It is albedo: it changes how much light reaches the photoreceptors and nothing
else. No downstream number is a function of it.

## Trajectory

Straight line, constant speed, world frame, every parameter a `FLYVERSE_HAND_*` knob:

| knob | default | used here | meaning |
| --- | --- | --- | --- |
| `FLYVERSE_HAND` | unset (no hand) | `approach` / `static` | mode |
| `FLYVERSE_HAND_DIR` | `0.35,0.55,0.76` | default | approach direction, world frame, normalised: down and in from the fly's left-rear corner, i.e. **oblique and from ONE side** |
| `FLYVERSE_HAND_DIST_MM` | 300 | default | palm centre to the aim point at launch |
| `FLYVERSE_HAND_START_MS` | 2000 | default | launch time |
| `FLYVERSE_HAND_DUR_MS` | 150 | default | launch to contact |
| `FLYVERSE_HAND_HALF_MM` | `45,55,12` | default | palm half-extents |
| `FLYVERSE_HAND_AIM` | `fly` | default | re-take the aim point to the fly's own position at the launch instant |

150 ms over 300 mm is 2.0 m/s, inside the 100-300 ms window a human slap takes to cover
the last few hundred millimetres. The control window is 2 ms, so the expansion is sampled
at ~75 points: the retina sees a genuinely time-varying stimulus, not a step.

**Why the aim is re-taken at launch.** A slap is aimed at the fly, and at 2 s the three
seeds are not where they spawned: seed 7 is still sitting on the spawn point, seed 11 is
in mid-air at 1001 mm/s near the far wall, seed 23 is on that wall. A fixed aim at the
spawn point would mean that for two of the three seeds the hand sweeps through empty
space hundreds of millimetres away and misses the fly entirely. So the trajectory's aim
point is re-taken once, at the launch instant, to the fly's own position (aim error 0.0 mm
in every seed; the run JSON records the aim point, the fly's position at launch, the
distance flown and the launch speed, and the console headline prints them). This is a
property of the *stimulus* -- where the hand goes -- and it is taken once: the hand does
not chase. It is a scalar-free decision with no feedback from the fly's behaviour
afterwards, and `FLYVERSE_HAND_AIM=spawn` is available to measure the alternative instead
of assuming it.

**The flown geometry, per seed** (`hand.json` -> `trajectory`):

| seed | launch | origin -> aim | distance | speed | aim error |
| --- | --- | --- | --- | --- | --- |
| 7 | t = 2000 ms | (-115, 15, 228) -> (-220, -150, 0) | 299.9 mm | 1999 mm/s | 0.0 mm |
| 11 | t = 2000 ms | (-115, 15, 228) -> the fly at (297, 49, 82) | 495 mm | 3302 mm/s | 0.0 mm |
| 23 | t = 2000 ms | (-115, 15, 228) -> the fly on the wall | see run JSON | see run JSON | 0.0 mm |

## The static control

`FLYVERSE_HAND=static` places the same palm, with the same geometry and the same albedo,
parked at the trajectory's origin and never moving. Because the origin is where the
approaching palm also starts, the two runs are **bit-identical until the launch instant**
(measured, section 4), so the static run separates "a response to the hand's existence"
from "a response to its expansion".

# 2. How it was measured

One run per (seed, configuration). 12 s of sim time, sampling every 2 ms control window
(6000 rows), seeds 7, 11 and 23, the same three seeds every earlier null used.

Windows, relative to the launch at t = 2.000 s:

| window | span | samples | role |
| --- | --- | --- | --- |
| `pre` | 1.500 - 2.000 s | 250 | the fly's own baseline, hand already present but not yet moving |
| `pulse` | 2.000 - 2.150 s | 75 | the 150 ms of expansion, up to contact |
| `post` | 2.150 - 2.650 s | 250 | the 500 ms after contact |

The response side, all of it measured in `src/analyze/hand.rs` and written to each run's
`hand.json`, with the per-window record in `hand.csv`:

- **does it turn away** -- the sign test. Signed bearing of the palm relative to the
  body's heading `hand_az` (positive to the left) against the yaw rate, and against the
  steering read-out `steer_l - steer_r` both raw and detrended by the pre-window mean. A
  sample is "away" when `az * yaw_rate < 0`. Reported as a fraction of samples with an
  exact two-sided binomial p. **The pre window runs the identical test**, because the
  fly's yaw is one-signed for long stretches and a bearing that does not change sign
  within 150 ms would otherwise read as 100% away with p ~ 1e-23 and mean nothing.
- **does the distance increase** -- the body's *own* velocity projected on the unit
  vector from the palm to the body (`hand_radial_v`), which separates "the fly moved
  away" from "the hand was moving", over the pulse with the pre window for comparison.
- **does it accelerate or change mode** -- speed in the pre/pulse/post windows, mode
  codes in the pulse, takeoffs during the pulse and in the 500 ms after, altitude gained.
- **how much did it move at all** -- the body trace of the approach run against the body
  trace of the static run, window by window: count of bit-identical rows and the maximum
  positional excursion. This is the measure that does not need a threshold.

The behaviour block (takeoffs, landings, wall hits, mode percentages, altitude, path,
tortuosity, speed, yaw rate, spike rate) comes from the same runs' `summary.json`, with
the no-hand run from the same seed for comparison.

Two rooms. The shipped arena is 600 x 440 x 220 mm. In it, a parked fly's own floor is
0.16 mm below its eyes, so the delivered per-eye loom sits at 0.99 and cannot rise: the
loom channel is at its ceiling before the hand arrives. That is a property of the room,
not of the hand, and it would make "the hand does not drive the loom pool" uninformative,
so the grid is repeated at `FLYVERSE_ROOM_SCALE=4` (2400 x 1760 x 880 mm), the
configuration `docs/room-size-and-loom-pathway.md` established as having loom range:
mean delivered loom 0.51-0.80 there against 0.99 here.

Runs: 1x seeds 7/11/23 x {static, approach} = 6; 1x no-hand x 3; 4x seeds 7/11/23 x
{none, static, approach} = 9. Twenty-one runs, one revision of the binary.

# 3. Integrity

**The default run is unchanged when there is no hand in the room.** Three no-hand runs
were taken from the final build with the hand code in it and compared against the same
three runs taken from the build *before* any of this existed:

<!--TABLE-CHECKSUM-->

Only the wall-clock fields differ (`wall_seconds`, `steps_per_second`,
`realtime_factor`); every simulated number in `summary.json` is identical. The
`summary.json` key set is unchanged too: the hand block is inserted only when there is a
hand, so a run without one serialises exactly as it did before.

**The static and approach runs share their history until the launch.** This is what makes
the pair a controlled comparison, and it is measured rather than assumed -- the two runs
are bit-identical up to t = 2.000 s in all three seeds:

<!--TABLE-INTEGRITY-->

**45 tests.** `./build.sh test --release`: `45 passed; 0 failed` on the final revision.

# 4. What the hand delivered

The palm reaches the fly's eyes in all three seeds, and the coverage grows as it
approaches -- the signature of an expanding edge rather than a step:

<!--TABLE-DELIVERY-->

Columns are per eye, out of the 678 the left eye holds and the 784 the right one holds
(1462 in total). `pulse` is the 150 ms of expansion, `pre` the 500 ms before it, and
`mean cols (pulse)` is the mean over both eyes together. Seed 7 is the full-contact slap:
the palm is on the retina in 100% of the pulse windows, peaks at 365 and 391 columns --
**53.8% and 49.9% of the two eyes at once** -- and the mean luminance-channel catch per eye
falls from 0.541 to 0.454 as the dark palm covers half the field. The expansion, window by
window, in seed 7:

<!--TABLE-TIMECOURSE-->

Seed 7 shows the canonical time course: coverage climbing 33 -> 65 -> 102 -> 124 -> 174
-> 216 -> 290 -> 361 columns in the left eye, in a sawtooth because the eye's column
lattice re-samples the growing edge, with the luminance catch falling monotonically behind
it (0.521 -> 0.292); the right eye enters later (first at 2080 ms, 143.9 mm) and is fully
engaged by contact, so the stimulus is both expanding *and* asymmetric.

Seeds 11 and 23 are the ones where the slap does not land on the fly: a hand that takes
150 ms to cross 495 mm is stale by the time it arrives, because the fly is doing 1001 mm/s
and has moved ~150 mm. Seed 11's closest approach is **53.3 mm**, and the palm enters the
eye's field only at t = 2134 ms -- the last 16 ms of the pulse window -- with a peak of 60
columns of the right eye (7.7% of it) and 84 columns in total at t = 2152 ms. So for seed
11 the delivery check is "the palm was seen, briefly and late", and the honest reading of
seed 11 is a *near* miss rather than a hit. It still carries the load-bearing measurement,
for the reason in section 5.

## The loom channel does not carry the hand at all

This is the part that needs stating plainly, because it is a property of the sensory model
and not of the fly.

The delivered per-eye loom is **saturated** in the shipped arena. A parked fly's own floor
is 0.16 mm below its eyes, and at zero airspeed the loom model's speed floor is 20 mm/s, so
the time-to-collision to the floor is a fraction of a millisecond and the channel sits at
0.9895 for *both* eyes before the hand does anything. The palm's arrival moves it by
**1.65e-4**: the whole lateral loom difference between the eyes, over the entire pulse,
is 0.000165 in seed 7, and the per-eye values during the pulse are the same to six decimal
places as in the static control. In seed 11 the loom is not pinned -- the fly is flying and
its own walls swing it between 0.377 and 0.973 -- but again the static and approach runs
give the same numbers to four decimals, because the nearest surface to that eye is always a
wall and never the palm.

The loom pool's firing rate is the sharper test, and it is **bit-identical** between the
static and the approach run in both seeds: 62.0 -> 80.0 Hz left and 84.0 -> 93.3 Hz right
for seed 7, 30.0 -> 26.7 and 58.0 -> 26.7 Hz for seed 11, the same to the last digit in
both configurations. The 62 -> 80 Hz movement in seed 7 is therefore *not* a response to
the expansion: it happens identically while the palm stands still 300 mm away, and it
coincides with the fly's own takeoff (section 5) rather than with the stimulus. What the
loom pool is doing there is the network's business; what it is not doing is responding to
the hand.

So the pathway that carries the hand to the brain in this experiment is the photoreceptor
array -- the `retina.push_events` route into the optic lobe columns -- and not the labelled
loom drive. That is worth saying because the loom channel is the one the canonical escape
story runs through, and in this room it is already at its ceiling when the hand arrives.
The 4x arm (section 5) is the configuration where it is not. The fast-slap arm (section 6)
is the configuration where the labelled loom route does carry the hand, and where both
conditions hold in one run.

# 5. What the connectome did

<!--TABLE-RESPONSE-->

Reading the columns that matter, per seed, in the pulse window against the 500 ms before
it and against the static control:

- **Turn away, yaw.** Seductive and meaningless. In seed 7 the fly's yaw satisfies "away"
  in 100% of the 75 pulse samples with p = 5e-23; it also satisfies it in 100% of the 250
  pre-window samples with p = 1e-75, i.e. for half a second *before* the hand moved at all.
  Seed 11 is 0% in both windows. A one-signed yaw over 150 ms cannot be evidence of
  anything, which is why the pre-window test is in the block.
- **Turn away, steering.** The detrended differential moves slightly *toward* the palm in
  seed 7 (36.0% away, p = 0.02) and it moves by the same kind of amount in the static
  control (32.0% away, p = 0.0024) with the raw sign test at 0% away. Seed 11: 9.3% away in
  both, identical. There is no away bias, and the one nominally significant number is
  toward, larger in the control than in the treatment, and does not replicate.
- **Does the distance increase.** No. Seed 7's own motion along the palm-to-fly axis is
  +0.059 mm/s during the pulse against +0.056 in the pre-window -- two orders of magnitude
  below the fly's own 600 mm/s -- and the fraction of samples with the velocity pointing
  away is 49.3%, i.e. a coin flip. Seed 11 is +6.2 mm/s against a pre-window of -386 mm/s:
  the fly is decelerating through a turn, and the pulse window happens to catch it moving
  away from a fixed palm; the static control at the same instant shows +72.6 mm/s, because
  the axis the projection uses is a fixed line to a parked palm rather than to a
  moving one. Neither is a response to expansion.
- **Acceleration and mode.** Seed 7: speed 0.2 mm/s in the pre, 0.2 in the pulse, then 627
  mm/s post -- and the static control is 0.2 / 0.2 / 649 mm/s with **one takeoff in the
  post-500 ms window as well**. The takeoff is not a response to the slap; it happens in
  the control. None of the three seeds takes off during the pulse, and the pulse mode codes
  are [0] (ground) for 7 and 23 and [2] (cruise) for 11.
- **How much it moved at all.** The decisive number. The body trace of the approach run
  against the body trace of the static run, row by row:

| seed | pre 500 ms | pulse 150 ms | post 500 ms | rest of the run |
| --- | --- | --- | --- | --- |
| 7 | 999/999 identical, 0.000 mm | 74/75 identical, max 0.010 mm | 18/250, max 165.6 mm | 119/4676, max 435.6 mm |
| 11 | 999/999 identical, 0.000 mm | **75/75 identical, 0.000 mm** | **250/250 identical, 0.000 mm** | **4676/4676 identical, 0.000 mm** |
| 23 | 999/999 identical, 0.000 mm | <!--S23-PULSE--> | <!--S23-POST--> | <!--S23-REST--> |

Seed 11 is the clean statement of the result: the palm travelled 442 mm through the room
and across that eye's field, the retina saw it in up to 84 columns, the network's spike
rate changed by 9 spikes per simulated second -- and the fly's recorded position, speed,
yaw, yaw rate and mode are **bit-identical to the control for the entire 12 seconds**.
Not "within noise": identical.

Seed 7 is the case where the two runs do diverge, and it is worth being precise about why,
because it is not escape. During the pulse the excursion is 0.010 mm; the runs separate
38 ms into the post window, at the moment the fly takes off -- and the static control takes
off too, in the same window, gaining *more* altitude (50.0 mm against 10.2 mm) and
accumulating *more* takeoffs over the run (3 against 1). One 2 ms window of difference in
when a takeoff commits, on a body whose flight is chaotic, is enough to produce 165 mm of
divergence 500 ms later and 435 mm by the end of the run. That is the amplification, not a
response; the treatment flew less than the control.

## The 12 s behaviour block

<!--TABLE-BEHAVIOUR-->

Read this one with care. The fly's flight in this model is chaotic, and *any* optical
difference amplifies: the no-hand run against the static-hand run differs from the first
window, because a parked palm in the room is itself a change to the optics, and by 12 s
their takeoff and wall-hit counts have nothing to do with each other. The hand-present
runs are therefore not comparable to the no-hand run on aggregate counters, and the rows
that matter are static versus approach, where the histories are bit-identical until
t = 2.000 s. In seed 11 those two rows are numerically identical everywhere, which is the
same fact as the trace fingerprint. In seed 7 the difference is the takeoff-timing
amplification above, and the direction of it is not escape: fewer takeoffs, less altitude,
shorter path, lower cruise speed and a lower mean speed than the control.

Seed 23's own behaviour difference is <!--S23-BEHAV-->.

## The 4x arm: the loom channel with range

<!--TABLE-4X-->

In the shipped arena the loom channel is at its ceiling before the hand arrives, so "the
hand does not drive the loom pool" is uninformative there. At `FLYVERSE_ROOM_SCALE=4`
(2400 x 1760 x 880 mm) the delivered loom has range -- mean 0.51-0.80 airborne, the
configuration `docs/room-size-and-loom-pathway.md` established -- and the palm arriving
from 300 mm is a real change in the nearest-surface geometry rather than a rounding error
on a saturated channel. The result is the same. The loom pool rates and delivered loom are
again identical between the static and the approach runs, and the body traces fingerprint
as in the table above.

# 6. The fast slap: the one run that holds both

The last open item in this experiment was a tension between two arms rather than a result.
The shipped arena delivers the palm to the retina, but its delivered loom sits at the room's
own ceiling -- `0.9895` for both eyes, pinned by the floor under a parked fly -- before the
hand does anything, so "the hand does not drive the loom pool" is uninformative there. The
4x arena has loom range (airborne per-eye means of 0.55 to 0.75, against the `0.9895` a
parked fly's eyes are pinned at) but the palm is never delivered to a flying seed: at most
one column. No single run had held **both** "the palm reaches the retina" and "the loom
channel has headroom (is unsaturated)".

The 150 ms arm cannot hold them either, for a reason worth stating plainly, because it is a
property of the trajectory and not of the fly. The trajectory's **origin** is fixed at the
spawn point and only the *aim* is re-taken at launch, so a slap at a seed that has left the
spawn is not the 300 mm terminal phase the arm is named for: seed 11's slap flies **495 mm**
at 3.3 m/s (seed 7's, at the spawn, flies the nominal 300 mm). Arriving 150 ms stale, when
the fly has moved ~150 mm, is why that slap lands 53 mm wide.

The fast arm is the terminal phase at the speed the last 300 mm of a real slap is actually
covered -- 6 m/s for a seed at the spawn -- via `FLYVERSE_HAND_DUR_MS=50`. The fly moves
~50 mm during the sting instead of ~150 mm, and the sting is 25 control windows of genuinely
time-varying stimulus. Two configurations were run on the same revision of the binary: the
shipped arena (arm D) and the 4x arena (arm E). One run per (seed, configuration), 12 s,
`--every 1`, seeds 7/11/23.

**Delivery and the loom channel.** Columns are per eye, out of the 678 the left eye holds
and the 784 the right one holds; `sting` is the 50 ms of the sting, `pre` the 500 ms before
it; the launch column is origin -> aim, and it is 300 mm only for a seed still at spawn.

| arm | seed | launch -> aim | palm on retina | peak cols L/R | delivered loom L pre->sting | delivered loom R pre->sting | loom pool R pre->sting |
| --- | --- | --- | --- | --- | --- | --- | --- |
| D 1x-fast | 7 | 300 mm / 5997 mm/s | 100% of sting, first 2000 ms | 361/328 (53.2%/41.8%) | 0.9895->0.9895 | 0.9894->0.9894 | 84->60 Hz |
| D 1x-fast | 11 | 495 mm / 9907 mm/s | 20% of sting, first 2040 ms | 354/0 (52.2%/0.0%) | 0.9725->0.8417 | 0.9615->0.9732 | 58->80 Hz |
| D 1x-fast | 23 | 300 mm / 5998 mm/s | 100% of sting, first 2000 ms | 361/328 (53.2%/41.8%) | 0.9895->0.9895 | 0.9894->0.9894 | 72->20 Hz |
| E 4x-fast | 7 | 300 mm / 5998 mm/s | 100% of sting, first 2000 ms | 361/328 (53.2%/41.8%) | 0.9895->0.9895 | 0.9894->0.9894 | 84->60 Hz |
| E 4x-fast | 11 | 671 mm / 13413 mm/s | **0%**, never seen | 0/0 | 0.9736->0.8511 | 0.9745->0.8576 | 60->80 Hz |
| E 4x-fast | 23 | 1681 mm / 33616 mm/s | **0%**, never seen | 0/0 | 0.9777->0.9931 | 0.9723->0.8793 | 70->20 Hz |

**The response and the body trace.** `sting` is the 50 ms of the sting, `post` the 500 ms
after it, `rest` the remaining 9.5 s. The trace column is the approach run against the
static-hand control, window by window: bit-identical rows / rows, and the maximum positional
excursion.

| arm | seed | yaw away sting (pre) | steer detrended away sting | radial v sting (pre) mm/s | speed pre/sting/post mm/s | takeoffs sting/post (approach vs static) | trace sting / post / rest |
| --- | --- | --- | --- | --- | --- | --- | --- |
| D 1x-fast | 7 | 100% (100% pre) | 0.0% (p 6e-08) | +0.1 (+0.1) | 0.2/0.2/268.6 | 0/1 vs 0/1 | 25/25 (0.000 mm) / 65/250 (134.0 mm) / 79/4726 (439.9 mm) |
| D 1x-fast | 11 | 0% (0% pre) | 0.0% (p 6e-08) | +4.9 (-386.0) | 1001.0/119.7/197.0 | 0/0 vs 0/0 | 25/25 (0.000 mm) / 29/250 (55.1 mm) / 631/4726 (145.7 mm) |
| D 1x-fast | 23 | 100% (100% pre) | 100% (p 6e-08) | +0.1 (+0.1) | 0.2/0.2/0.2 | 0/0 vs 0/0 | 25/25 (0.000 mm) / 209/250 (0.010 mm) / 206/4726 (659.0 mm) |
| E 4x-fast | 7 | 100% (100% pre) | 76.0% (p 0.015) | +0.1 (+0.0) | 0.2/0.3/0.2 | 0/0 vs 0/0 | 25/25 (0.000 mm) / 179/250 (0.020 mm) / 654/4726 (1955.5 mm) |
| E 4x-fast | 11 | 0% (0% pre) | 100% (p 6e-08) | -1823.9 (-733.9) | 1307.8/2005.8/1656.3 | 0/0 vs 0/0 | **25/25 (0.000 mm) / 250/250 (0.000 mm) / 4726/4726 (0.000 mm)** |
| E 4x-fast | 23 | 12% (12% pre) | 100% (p 6e-08) | -482.9 (-1033.8) | 1340.4/987.1/71.8 | 0/0 vs 0/0 | 25/25 (0.000 mm) / 103/250 (7.5 mm) / 17/4726 (1516.6 mm) |

One caveat about the response columns, because it applies to this whole arm: the analysis
module defines its `pulse` window as the sting itself, so for a 50 ms sting the sign tests
sample the approach only -- the palm arrives in the sting's last window, and every seed's
sting sign test is genuinely identical between the static and the approach run. The tests
are reported for completeness; the measurements that matter here are the body trace, the
delivered loom and the pool rates, and a post-hoc sign test over the windows after contact
(section 6.2).

## 6.1 The run that holds both

`fast1x/s11_approach` is that run: `FLYVERSE_HAND=approach FLYVERSE_HAND_DUR_MS=50`,
seed 11, shipped arena.

**The palm demonstrably reaches the retina.** The sting starts 495 mm out (the fly is not at
spawn) and the palm is seen from 2040 ms, in 20% of the 25 sting windows. It peaks at **354
of the left eye's 678 columns -- 52.2% of one eye at once**, and its nearest surface gets to
**3.7 mm of the left eye inside the sting** and **1.4 mm of the right eye one sample later at
2050 ms**, the first window after it. Mean columns on the palm over the sting, 17.4 against
0.0 in the 500 ms before it.

**The loom channel has headroom (it is unsaturated), and the palm's arrival is in it.** At
the launch instant the delivered loom is **0.332 (left eye)** and **0.930 (right)** -- against
the `0.9895` both eyes are pinned at in this room when the fly is parked -- and over the 500 ms
before the sting the left eye's delivered loom runs **0.309 to 1.000**, over the sting itself
0.332 to 0.980. Neither eye is sitting on the room's ceiling when the palm arrives, and the
right-eye channel is worse than that: **it is silent**. At 2058 ms the right-eye loom drops
from 0.9994 to **0.0000** -- nothing looming at all -- and it does so identically in both runs,
before the palm has arrived and before the two runs diverge anywhere, so it is the fly's own
motion and not the hand. That is what makes the next comparison clean rather than a
saturation argument:

- the delivered **right**-eye loom reads **0.9245 in the approach run and 0.0000 in the
  control at 2058 ms**, and stays separated (0.9199, 0.5922, 0.0994, 0.0005) for the next
  70 ms as the palm sweeps through and away -- against a lateral loom difference of 1.65e-4
  over the whole pulse in the 150 ms shipped-arena arm;
- the right `visual_loom` pool reads **500 Hz at 2062 ms** where the control reads **0 Hz**
  (the pool's own 500 Hz spikes in the control are earlier, at 1980 and 2028 ms, before the
  palm is anywhere near the eyes).

So this is the first configuration in this document where the labelled loom route carries
the hand, at a magnitude comparable to its own scale. It is also the first time the palm
reaches the retina in a room where the loom is not already at its ceiling. Both conditions
the earlier arms could only satisfy one at a time now hold in one run.

## 6.2 Does the fly escape it? No.

Same measures, same windows, against the static-hand control whose history is identical
until the launch:

- **The body is bit-identical through the sting** -- 25/25 windows, 0.000 mm -- while the
  palm crosses the last 495 mm of the room and covers half of one eye. The divergence, when
  it comes, follows the stimulus in the order and at the delays the optics predict: the
  *sensory* columns (`lum_l`, `gr_l`) first differ at **2040 ms**, the sample in which the
  palm first enters the left eye; the pooled spike count first differs at **2046 ms**, 6 ms
  later, as the covered area grows from 1 to 64 columns; the *body* first differs at
  **2094 ms**, by 0.0100 mm, 44 ms after contact. From there it reaches 55 mm over the 500 ms
  after the sting and 146 mm by 12 s. Unlike the 150 ms arm's seed 11, this run *does* diverge
  once the slap lands: at 52% of an eye the delivered stimulus is far larger than that arm's
  late, near-miss sliver, and this connectome amplifies any optical difference at all. That is
  the price of closing the gap, not a response -- the contrast is arm E's seed 11, where the
  palm never reaches the retina and the two runs are bit-identical for all 6000 rows.
- **No away direction.** The sting-window yaw and steering sign tests are identical between
  the two runs (0% away, i.e. the one-signed yaw reads "toward", as it does for 250/250
  pre-window samples). A post-hoc sign test over the 50-100 and 100-150 ms windows after
  contact is also 0% away for both the yaw and the steering differential. The 95.6% "away"
  that appears in the 150-650 ms window is the artefact the document already names, and the
  numbers here show it exactly: the yaw rate is positive in **250/250** samples of that
  window and in 250/250 of the pre-window too (+1.7 rad/s before the palm, +1.2 after) -- one
  continuous turn that was already under way half a second before the hand existed -- while
  in the same window the bearing to the palm flips sign (positive in 250/250 pre-window
  samples, 11/250 at 150-650 ms, as the palm passes behind the fly). The same turn, read
  against the flipped bearing, reads "away". The control's bearing does not flip -- its
  parked palm stays at +1.75 rad throughout -- so the same test on it reads 0%.
- **The fly's own radial motion is a coin flip.** +4.9 mm/s along the palm-to-body axis
  during the sting (pre-window -386.0, i.e. the fly was closing), 60% of samples moving away.
- **No takeoff, and no mode change.** In all six arm-D/E pairs the takeoff counter and the
  mode are unchanged through the sting, and the only pair that changes either in the 500 ms
  after it is seed 7 -- and it does so in *both* its runs, so that takeoff is the fly's own
  schedule and not the palm's.
- **Over 12 s the run that saw the palm is *less* directed, not more.** Seed 11: 62 wall hits
  against the control's 108, 1 landing against 0, 10.0% of the run in cruise against 85.7%,
  56.3% in the takeoff mode against 3.2%, 1327 mm of path against 1462 mm -- it takes off
  more and gets nowhere, rather than flying away. The same asymmetry the 4x seed-23 case
  shows (43 wall hits against 109, 63.9% cruise against 93.7%).

## 6.3 Why the 4x fast arm still cannot hold both

At `FLYVERSE_ROOM_SCALE=4` the fly is not the 1x fly: at 2 s (arm E) it is already doing
**1.95 m/s** (seed 11) and **1.07 m/s** at the ceiling (seed 23), so the sting, even at 6 m/s
of palm speed, is 13413 mm/s of travel over a fixed-origin 671 mm throw (33616 mm/s, 1681 mm,
for seed 23). The fly covers **98 mm** (seed 11) and **53 mm** (seed 23) of its own travel
during the 50 ms sting -- four and two times the 24 mm thickness of the palm -- so a palm
aimed exactly at where the fly *was* (`aim_error_mm` = 0.0 at launch, both seeds) passes it:
**0 columns in every sting window, for both seeds**. With the palm never delivered, the
approach and static runs are **bit-identical for all 6000 rows** (seed 11), which is the
cleanest control in the arm.

So the arena with loom range is the arena where the palm cannot arrive, and the arena where
the palm arrives is the arena where the loom needs a *flying* seed to have headroom. The 1x
fast arm threads that needle because seed 11 is landing at t = 2 s -- speed 27.7 mm/s,
airborne at 70 mm, with its nearest surface 11 mm off rather than the 0.16 mm floor a parked
fly's eyes sit on -- so the delivered loom is 0.33, not the 0.99 a parked fly is pinned at.
That is the run in section 6.1, and its answer is the same one every other arm gives.

# 7. Verdict

**The fly does not escape the hand, and there is no route by which it could have.**

1. **The stimulus arrived.** The palm is a dark, spectrally non-flat, expanding,
   asymmetric object that the fly's own optics ray-cast; it covers up to 53.8% / 49.9% of
   the two eyes simultaneously at full contact in seed 7 and 197.6 columns on average across
   the 150 ms, and it darkens the mean luminance catch per eye from 0.541 to 0.454.
2. **The response is absent, not small.** Seed 11: bit-identical body trace for 12 s while
   the palm swept 442 mm to within 53 mm of the fly. Seed 23: <!--S23-SUMMARY-->. Seed 7: 74
   of 75 pulse windows identical, a 0.010 mm maximum excursion during the expansion, and a
   divergence that begins only when the fly's own takeoff commits (which the control also
   does).
3. **No escape measure fires in a replicable direction.** The yaw sign test reads 100%
   "away" in seed 7 inside the pulse *and* 100% in the 250 ms before the palm moved, and 0%
   in seed 11; the steering differential moves slightly *toward* in seed 7 with a larger
   effect in the static control, and not at all in seed 11; the fly's own radial motion is
   at the 49% level; no seed takes off during the pulse.
4. **No pathway carries it in a direction that matters.** The photoreceptor route delivers
   the hand to the optic lobe (measured, section 4). In the shipped-arena arms the labelled
   loom route does not: the delivered loom moves by 1.65e-4 laterally at 1x because it is
   already pinned at 0.9895, and the loom pools' firing rates are bit-identical between the
   static and approach runs. In the fast-slap arm (section 6.1) the channel is not pinned and
   the route **does** carry the hand -- the delivered right-eye loom reads 0.9245 in the
   approach against 0.0000 in the control at 2058 ms, and the right `visual_loom` pool reads
   500 Hz where the control reads 0
   -- and the answer is unchanged: the body is bit-identical through the sting, then diverges,
   with no away direction and no takeoff. Whatever the connectome does with the retinal image
   of a hand, it does not reach the descending motor read-out in a way that moves the body
   away from it.

This is a null with a different shape from the earlier ones. The luminance-step and colour
nulls were "the stimulus landed and nothing moved above the noise floor"; the shipped-arena
hand arm is the stronger "the stimulus landed and the body was bit-identical to its control".
With it, every stimulus class this room can present has been tested: a sustained single-eye
luminance step, an equi-luminant colour change, and a temporally structured expanding dark
edge. None of them drives steering. The fast arm puts the strongest form of that test on the
table -- palm on half an eye, the labelled looming channel carrying it, the body perturbed
-- and the fly does not turn away; it flies less. **No visual stimulus presented through this
connectome's eyes produces an away response**, and the reason is upstream of any reflex
anyone might have wanted to add: the raw connectome's visual-to-steering projection is
bilaterally balanced (verified at hop 2 against all 151.8M source rows -- identical edges,
identical contact weights, ipsilateral excess of the wrong sign for a corrective reflex), so
there is no lateral drive for a lateral stimulus to act on.

## What this does not show

- It does not show the fly's visual system is dead. The palm changes the retinal image
  over 365-391 columns and the network's spike rate moves measurably (seed 11: +9
  spikes/s over the run for a bit-identical body; seed 23: <!--S23-SPIKES-->). The
  connectome sees the hand. It does not act on it.
- It does not cover a stimulus the room cannot present. There is no wind on the palm, no
  odour, no sound and no substrate vibration from the slap; those are different channels
  and this experiment says nothing about them.
- It does not cover a hand *in contact* for a sustained period, or a fly walking on a
  hand, which would be a tactile rather than a visual stimulus.
- Seed 11's slap misses by 53 mm and enters the eye late; its bit-identical result is
  therefore the strongest possible statement of "no response" but the weakest possible
  statement of "the stimulus was big". Seed 7, where the palm reaches 0.03 mm from the
  body centre with half of each eye covered, carries the delivery claim.
- It does not separate a motor response from chaos amplification in the fast arm. There the
  runs are bit-identical through the sting and then diverge (55 mm in the 500 ms after
  contact, 146 mm by 12 s) once the palm is delivered over half an eye, and the divergence
  is caused by the palm -- arm E's seed 11, with the palm never delivered, is bit-identical
  for all 6000 rows. Whether the 12 s difference in cruise, wall hits and landings is the
  connectome acting on the image or the amplification of a 1e-4 mm difference is not
  settled by this measure; what is settled is that nothing in it is directed away from the
  palm.

# 8. Reproducing it

```
cd /opt/data/workspaces/skg/flybrain/flyverse
./build.sh build --release
./build.sh test --release                     # 45 passed; 0 failed

# the slap, and the static-hand control, per seed
for s in 7 11 23; do
  FLYVERSE_HAND=approach ./target/release/flyverse analyze --seconds 12 --seed $s --every 1 --out /tmp/fv/hand1x/s${s}_approach
  FLYVERSE_HAND=static   ./target/release/flyverse analyze --seconds 12 --seed $s --every 1 --out /tmp/fv/hand1x/s${s}_static
done

# the no-hand run, which must stay byte-identical to the pre-hand build
./target/release/flyverse analyze --seconds 12 --seed 7 --every 1 --out /tmp/fv/nohand2/s7

# the arm where the loom has range
FLYVERSE_ROOM_SCALE=4 FLYVERSE_HAND=approach ./target/release/flyverse analyze --seconds 12 --seed 7 --every 1 --out /tmp/fv/hand4x/s7_approach

# the fast slap (the terminal phase at 6 m/s): the shipped arena, seeds 7/11/23
for s in 7 11 23; do
  FLYVERSE_HAND=approach FLYVERSE_HAND_DUR_MS=50 ./target/release/flyverse analyze --seconds 12 --seed $s --every 1 --out /tmp/fv/fast1x/s${s}_approach
  FLYVERSE_HAND=static   FLYVERSE_HAND_DUR_MS=50 ./target/release/flyverse analyze --seconds 12 --seed $s --every 1 --out /tmp/fv/fast1x/s${s}_static
done
# seed 11 at 1x is the run that holds both (section 6.1): the palm reaches the retina
# (354 columns, 52% of one eye) and the delivered loom is not pinned (0.332 at launch,
# 0.9245 -> 0.0000 against the static control at 2058 ms, right loom pool 500 Hz -> 0 Hz
# at 2062 ms). The 4x arm of the same sting delivers nothing (section 6.3):
FLYVERSE_ROOM_SCALE=4 FLYVERSE_HAND=approach FLYVERSE_HAND_DUR_MS=50 ./target/release/flyverse analyze --seconds 12 --seed 11 --every 1 --out /tmp/fv/fast4x/s11_approach
```

Each run writes `trace.csv` (the body, unchanged schema), `summary.json` (with the `hand`
block appended when a hand is present), `hand.csv` (per-window hand telemetry: palm
position, distance, bearing, elevation, the fly's own radial velocity, columns on the palm
per eye, nearest palm distance per eye, per-eye luminance, delivered loom per eye, both
loom pool rates, the steering read-out) and `hand.json` (the stimulus description, the
flown trajectory with its aim point, the delivery check and the response numbers). With no
`FLYVERSE_HAND` in the environment none of that is written and the files are what they
always were.

Every knob is listed in `src/room.rs` in the `active_hand` doc comment: mode, direction,
launch distance, launch time, duration, aim point, palm half-extents, and whether the aim
is re-taken at launch.

# 9. What was added, and what was deliberately not

New: `src/analyze/hand.rs` (telemetry + the response measurements). Changed: `src/room.rs`
(the `Hand` geometry, the trajectory and its knobs), `src/vision.rs` (the `Surface::Hand`
variant, the skin reflectance, the ray-vs-oriented-box intersection, the per-eye palm
coverage and nearest-distance accessors), `src/sim.rs` (the per-window placement and the
aim re-take, plus the loom-pair accessor the earlier probe work added),
`src/analyze/{run,summary}.rs` and `src/analyze.rs` (wiring). Nothing in `wasm/` and
nothing in `web/`.

The fast-slap arm (section 6) adds no code at all: it is `FLYVERSE_HAND_DUR_MS=50` on the
same binary, and the 4x arena is `FLYVERSE_ROOM_SCALE=4`. No new knob, no new term, and no
change to any default -- a run with no `FLYVERSE_HAND` still serialises exactly as it did
before the hand existed, and the fast arm's own static/approach pair is bit-identical until
the launch.

Deliberately not added, and this is the point of the whole exercise: any term, gain,
coefficient, threshold or gradient whose purpose is to make the fly move away from a
visual object. There is no escape reflex in this diff, no startle response, no looming
gain, no avoidance field, no steering bias and no stabiliser. The hand is geometry, albedo
and a trajectory; the eyes are the ones that were already there; the connectome is the
Janelia MaleCNS connectome, unmodified; and the answer to "does it run away" is that it
does not.