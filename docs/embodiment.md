# Embodiment: replacing the engineered flight layer

This note records what changed when the surrogate flight layer was removed, what
replaced it, and what the fly actually does now.

## What was removed

The previous body contained a flight controller. Every behaviour it produced was
specified by the authors of this repository rather than by the connectome:

| Removed | Was |
|---|---|
| `LIFT_MAX`, `WING_AMP_HOVER` | lift invented as an amplitude-squared curve, scaled to hover |
| `THRUST_MAX` | thrust applied along the heading, capped |
| `DRAG_Q`, `DRAG_L` | two drag coefficients tuned until the flight looked right |
| `CRUISE_ALT`, `ALT_GAIN`, `alt_target`, `stab` | a proportional altitude controller with an altitude setpoint |
| `roll_target = -0.55 * steer` | commanded bank angle |
| `YAW_GAIN` | steering motor read-out added directly to the yaw rate |
| takeoff timer + `vel[2] = 40` | liftoff fired on a threshold after a delay, then a vertical impulse |
| mode timers | gait, landing and feeding driven by hand-chosen durations |

There is no longer any altitude setpoint, stabiliser gain, bank command, or
takeoff timer anywhere in `src/body.rs`. `no_altitude_setpoint_remains` is a test
that fails if an altitude controller is reintroduced.

## What replaced it

**Wing aerodynamics** (`src/wing.rs`). A quasi-steady blade-element cycle mean:

```
F = 1/2 * rho * C_F(alpha) * <U^2> * S
<U^2> = (Phi/2)^2 * omega^2 / 2 * R^2 * k2 + 1/2 * V^2
C_F = sqrt(C_L^2 + C_D^2), from the measured Drosophila force coefficients
```

The wing flips its angle of attack at each stroke reversal, so the force keeps
its sign through both half strokes and the cycle mean does not cancel. Every
constant is measured or derived, not tuned:

| Constant | Value | Source |
|---|---|---|
| `WING_LEN` | 2.406 mm | mesh bounding box |
| `WING_CHORD` | 1.126 mm | measured in-plane chord extent |
| `WING_AREA` | 2.036 mm² | measured from the mesh (closed blade; planform = half the 4.072 mm² surface) |
| `WING_K2` | 0.3335 | measured second moment of the planform |
| `WINGBEAT_HZ` | 200 | literature |
| `STROKE_AMP_MAX` | 158° | free-flight measurement |
| `FLY_MASS` | 0.983 mg | flybody / Vaxenburg et al. 2025 |
| `C_L`, `C_D` | functions of alpha | Sane & Dickinson 2002, Re ≈ 115 |

At full stroke one wing produces about 1.8× half the body weight. That surplus is
what lets a fly climb and carry a load; the earlier estimated planform (1.19 mm²)
put it at 1.05×, which is why the fly could barely hover. That was a measurement
correction, not a tuning knob: the assumed chord was 29 % short.

**Stroke-plane tilt.** The steering motor neurons set each wing's stroke-plane
tilt, and the force vector rotates with it. Common drive tilts both planes
fore/aft; differential drive tilts them oppositely. That is the entire steering
mechanism, and it is geometry rather than a push in a chosen direction.

**Torque.** Each wing's force is applied at its own root, `r = (0, ±0.395,
0.20) mm`, so `tau = r x F` gives roll from an amplitude difference, yaw from a
tilt difference, and pitch from forward thrust acting above the centre of mass.
No handedness correction is applied: the cross product and the quaternion
rotation both follow the right-hand rule and already agree.

**Rotational damping.** A flapping wing resists body rotation because rotation
adds a relative airflow that does not reverse with the stroke. Integrating that
over the cycle gives `rho * S * C_D * u_bar * R^2` per wing. This is the physical
effect that stops the body spinning up, and it is not a control law: it has no
offset term and cannot hold an attitude, only oppose a rate.

**Halteres** (`src/wing.rs`). Each haltere's Coriolis deflection is linear in the
component of body angular velocity in its plane, so the afferent rate is
`base * (1 + |omega| / full_scale)`, lagged with a 1 ms time constant. The left
and right signals are split so a roll rate drives them differentially and a yaw
rate drives them in common, which is what the real pair does. The afferents are
the real neurons, from `assets/male_cns_v1_mechanosensory_io.json`, merged at
load time. `FLYVERSE_NO_HALTERE=1` disconnects them and is the control condition.

## What it does now

60 s, seed 7, full model, haltere afferents connected (175 M spikes):

| | halteres on | halteres off |
|---|---|---|
| takeoffs / landings | 1075 / 1074 | 795 / 794 |
| wall hits | 8 | 36 |
| mean speed | 328 mm/s | 426 mm/s |
| altitude mean (p05) | 39.8 mm (40.0) | 36.6 mm (0.4) |
| mean \|yaw rate\| | 3.4 rad/s | 5.4 rad/s |
| path tortuosity | 43.5 | 60.8 |

Liftoff is physical: the wings beat, and the fly leaves the surface when the
aerodynamic force beats its weight. It does so within a few hundred milliseconds,
with no takeoff timer.

The fly then travels at a few hundred mm/s, turning at a few rad/s, and it does
not fall out of the air. What it does not do is fly: it skitters a millimetre or
so above the table top (the room's table is 40 mm high and the fly spends most of
its time at z ≈ 41 mm), touching down roughly every 50 ms. It does not navigate,
climb away, avoid walls, or reach the food.

## What this does and does not show

The haltere-on and haltere-off runs differ, and they differ in the direction a
working stabiliser would move them: fewer wall hits, less turning, a steadier
altitude. That is suggestive and it is not proof. This system is chaotic, so two
runs that differ in any input diverge, and the divergence alone would produce
differences of this size. Establishing a real reflex needs a perturbation test
that holds everything else fixed, which has not been run.

The honest summary of the flight is that it is *not* emergent flight. The fly
holds itself up and moves about, which the previous controller also achieved by
construction. The difference is that nothing in the code now specifies that it
should: the altitude, the speed and the turn rate are consequences of the wing
forces and the body's inertia rather than of a gain. That is a weaker claim than
"the connectome knows how to fly", and it is the claim the evidence supports.

## Still surrogate

- Ground locomotion: leg motor read-out sets a capped walking speed. No leg
  kinematics, no contact model.
- The takeoff, landing and feeding state machine, including the feeding timer
  and the landing label.
- The odour field, the optic-flow proxy, the looming-wall channel.
- `vnc_sensory`: still replayed at a fixed 150 Hz rather than generated by the
  body. The brain's main input remains canned.
- Collision: a position clamp with 0.15 restitution, no avoidance.
- Wing inertia is not modelled. The wings' reaction torques are internal to the
  body and cycle-average to near zero for a symmetric stroke, but the
  instantaneous torques are large and are omitted.
- `WING_DZ` (the wing root's height above the centre of mass) is an estimate at
  0.20 mm, and it sets the pitching moment. The fly's pitch behaviour is
  sensitive to it.

## Bugs found while doing this

Two, both in this repository's own code, both caught by measurement:

1. **The wing planform was 42 % too small.** The area was estimated as
   `length * chord * 0.62` with a literature chord of 0.80 mm. Measuring the
   actual mesh gives a 1.126 mm chord and a 2.036 mm² planform. The estimate made
   a full-power fly produce only 1.05× its weight, so it could not reliably leave
   the ground, and the reason looked like the connectome.

2. **The rotational damping had the wrong sign.** The roll and yaw components of
   the torque were negated to "correct" for the left-handed (x forward, y left,
   z up) frame. That negation also flipped the damping term, turning the
   aerodynamic damping into positive feedback. The body spun up until it hit the
   rate clamp and its speed pegged the speed clamp. The reading that exposed it
   was `max_speed_mm_s = 3999.6875` against a 4000 mm/s clamp. `aerodynamic_
   damping_slows_a_spinning_body` now pins the direction.

A third fix was to the analysis rather than the model: the turn rate was being
computed by differencing the Euler yaw angle. Once the body pitches, the Euler
chart wraps and the quotient invents rates that the body never had. Every
turning number now comes from the body's actual angular velocity.
