# Haltere perturbation probe

Does the haltere channel actually stabilise the fly, or is the fly merely
carrying 205 haltere afferents that do nothing?

The on/off comparison in `docs/embodiment.md` cannot answer this. Two runs of a
chaotic system diverge, and that divergence alone produces differences of the
size observed between haltere-on and haltere-off. Reporting those differences as
evidence of a reflex would be reporting chaos.

This document describes the probe that was built to answer the question
properly, states in advance what would count as evidence and what would count as
failure, and reports what the probe measured.

## The measurement

The fly's body is given an angular velocity it did not generate:

```
world.inject_rotation(axis, magnitude)
```

That is the only thing the probe touches. It does not inject spikes, does not
change the odour, does not change the room, and does not alter any drive. The
imposed rotation changes the body's state, and from there the only route to the
motor neurons is through the sensory channels the connectome is actually wired
to. If the wing motor output changes, it changed because the network read the
rotation and responded.

### Why both signs

The network state at the moment of perturbation is whatever the fly happened to
be doing; it is not controlled. So the same perturbation is applied with both
signs, and the response is averaged separately for each sign.

Chaotic variation is independent of the sign, so it averages out over trials. A
reflex is a function of the sign, so it survives. The quantity reported is:

```
response_signature = mean(response | perturbation positive)
                   - mean(response | perturbation negative)
```

The sign schedule is balanced: half of each axis's trials are positive and half
negative, shuffled with a seed. With few trials an unbalanced draw would put most
of the weight on one side of a difference of means.

### The control

The identical probe is run twice:

| condition | what it means |
| --- | --- |
| `haltere-probe` | haltere afferents delivered to the network |
| `FLYVERSE_NO_HALTERE=1 haltere-probe` | haltere model still computed, spikes never delivered |

The second run is the control for the claim that any signature is *haltere*
mediated. The body physics and the other sensory channels are unaffected by the
flag, so both conditions still receive the imposed rotation.

### The null control

A third run repeats the probe with `--amplitude 0`. Nothing is imposed, so the
positive and negative groups are physically identical and the true signature is
zero. Whatever this run reports is the noise floor of the statistic itself. If
the 25 rad/s runs report signatures no larger than the zero-amplitude run, the
probe has measured nothing.

### The attribution control

Two sensory channels can report an imposed rotation within the same control
window, and only one of them is the connectome's own sense organ:

| channel | responds to rotation | status |
| --- | --- | --- |
| haltere afferents | yes, through the Coriolis model | real, driven by measured geometry |
| optic flow | yes, through its `turn = yaw_rate * 26` term | **engineered proxy** |

The optic flow channel is a surrogate (see `docs/embodiment.md`), and its turn
term saturates across the full perturbation range this probe uses, so it can
easily dominate the response. `FLYVERSE_NO_FLOW=1` zeroes that channel's drive
and leaves everything else alone.

Running the four-way combination is what makes the result attributable:

| halteres | optic flow | what a signature here would mean |
| --- | --- | --- |
| on | on | baseline |
| off | on | response carried by something other than the halteres |
| on | off | **response carried by the connectome's own haltere pathway** |
| off | off | should be at the noise floor; if not, the probe is measuring itself |

The third row is the measurement the original question asks for. If it sits at
the noise floor while the first row does not, then the connectome receives the
rotation through its haltere afferents and produces no steering response with
it.

## Reading the direction

The sign convention has to be fixed before the numbers mean anything. In this
model:

- A steering motor neuron pair drives stroke-plane tilt: `tilt_l = TILT_MAX *
  (common + 0.5 * diff)` and `tilt_r = TILT_MAX * (common - 0.5 * diff)`, with
  `diff = steer_r - steer_l`.
- A positive tilt leans that wing's force vector forward: `wing_force_vector`
  returns `[f * sin(tilt), 0, f * cos(tilt)]`.
- So a positive `diff` gives the left wing more forward thrust than the right.
- Yaw torque is `tau_z = WING_DY * (fr[0] - fl[0])`, so left wing ahead means
  `tau_z < 0`.

A positive yaw perturbation raises `omega_z`. Opposing it requires negative
`tau_z`, which requires more thrust on the left, which requires a **positive**
steer differential.

**Therefore, for a yaw perturbation, stabilisation means
`response_signature(steer differential) > 0`.**

Roll is not assigned a direction here. The two halteres are driven
differentially by roll and in common by yaw, and the mapping from the imposed
roll to a desirable wing response involves the wing root geometry in a way that
would need its own derivation to state a sign. The roll result is reported as a
measured number, not as a pass or fail.

## What would count as evidence

The claim "the haltere channel stabilises the fly" is supported only if all of
these hold:

1. With halteres connected, the yaw perturbation produces a positive steer
   signature (the direction derived above).
2. That signature is clearly larger than the zero-amplitude noise floor.
3. The same probe with halteres disconnected produces a substantially smaller
   steer signature, so the response is haltere mediated rather than carried by
   some other channel.

The claim is **refuted** if the signature is indistinguishable from the noise
floor, or if it has the wrong sign.

## Results

Ten probe runs, 60 perturbations each, 30 trials per axis, 10 min per run.
`scripts/probe_table.sh` tabulates the JSON artefacts.

The yaw steer-differential signature, which is the quantity that has to move for
the claim to hold:

| run | seed | halteres | optic flow | amplitude | yaw steer signature | t |
| --- | --- | --- | --- | --- | --- | --- |
| `hal-on` | 7 | on | delivered | 25 rad/s | −0.0192 | 1.45 |
| `a10s7` | 7 | on | delivered | 10 rad/s | **+0.0211** | 1.47 |
| `a50s7` | 7 | on | delivered | 50 rad/s | −0.0092 | 0.77 |
| `s11on` | 11 | on | delivered | 25 rad/s | **+0.0188** | 1.37 |
| `s23on` | 23 | on | delivered | 25 rad/s | −0.0093 | 0.75 |
| `null` | 7 | on | delivered | **0** | 0.0000 | 0.00 |
| `nf-hal-on` | 7 | on | **silenced** | 25 rad/s | −0.0139 | 1.21 |
| `hal-off` | 7 | off | delivered | 25 rad/s | −0.0182 | 1.16 |
| `s11off` | 11 | off | delivered | 25 rad/s | −0.0007 | 0.05 |
| `nf-hal-off` | 7 | off | silenced | 25 rad/s | +0.0060 | 0.28 |
| `nf-null` | 7 | on | silenced | **0** | −0.0067 | 0.50 |

The sign flips between seeds. The magnitude does not grow with amplitude: 10 and
25 and 50 rad/s give +0.0211, −0.0192, −0.0092. No run reaches |t| = 2.

Aggregated over the five haltere-connected runs: mean **+0.00044**, sd 0.0183,
sem 0.0082, **t = 0.05**. Per-run 95% confidence intervals are about ±0.022.

The perturbation did land. The yaw-rate residue channel — how much of the
imposed rotation survives into the response window — is positive in 6 of the 7
runs with a non-zero amplitude (1.65, 1.66, 2.90, 3.32, 2.23, 0.93 against
−0.35), and the zero-amplitude runs sit at 0.08 and 0.23. So the body really was
given a rotation, and the sign-conditioned statistic really does detect the
physical consequence. It detects it on the body and not on the motor neurons.

## Verdict

**The evidence does not support the claim that the haltere channel stabilises
the fly, and the probe rules out a large effect.**

Reading the results against the criteria set out above:

1. Criterion 1 fails. The signature does not have the sign that opposition
   requires. It flips sign across seeds, so there is no sign to speak of.
2. Criterion 2 fails. The haltere-connected signatures are not larger than the
   zero-amplitude runs; they are within the same band.
3. Criterion 3 fails. Silencing the engineered optic-flow channel, which was the
   obvious alternative explanation, does not change the picture: `nf-hal-on`
   gives −0.0139, inside the spread of the flow-delivered runs.

There is a real negative result here and it should not be over-read. The probe
bounds the steering response to an imposed 25 rad/s yaw at roughly **±0.02** in
the steer differential, against a baseline differential of about −0.09, so a
reflex smaller than about a fifth of the standing steering command would be
missed. This is an upper bound, not a proof of absence.

What it does establish is that the earlier haltere on/off behaviour differences
(328 vs 426 mm/s, 3.4 vs 5.4 rad/s, 8 vs 36 wall hits) were not evidence of
anything. A response that does not replicate across seeds and does not scale with
the stimulus cannot be used to support a reflex, and those run-to-run differences
are what chaos alone produces.

The likely reading is that 205 haltere afferents project into a network that is
being driven much harder by the surrogate channels — taste fixation, the odour
plume, and above all the optic-flow proxy, which saturates on exactly this
stimulus — and that the rotation signal is simply not reaching the wing motor
neurons with enough weight to see. The honest next step is not a better statistic
but a better fly: real optic flow, real wing mechanics, and a stimulus that puts
the fly in a flight regime where a stabilising reflex would matter.

## Reproducing

Every run quoted above is committed under `docs/probe-results/`, so the numbers
can be checked without re-running anything. Each file records the amplitude and
whether the haltere afferents were delivered, plus the per-channel means and
Welch t for both axes. The seed and the optic-flow condition come from the run
names in the table above; runs made with the current build also record them as
`seed` and `optic_flow_delivered`, which is why `scripts/probe_table.sh` prints
`?` for the older artefacts.

```sh
./build.sh build --release
scripts/probe_table.sh docs/probe-results/*.json    # tabulate what is committed

# or regenerate from scratch (about 10 min per run)
./target/release/flyverse haltere-probe --seed 7 --out runs/haltere-probe/s7
FLYVERSE_NO_FLOW=1 ./target/release/flyverse haltere-probe --seed 7 --out runs/haltere-probe/nf
scripts/probe_table.sh runs/haltere-probe/*.json
```

