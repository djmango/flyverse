# The activation -> stroke amplitude -> force chain

This note grounds the three-stage chain that decides whether the fly can leave the
ground, after the motor read-out was made faithful (`groups.rs`/`sim.rs`, commit
"Fix the motor read-out, and find the fly never flew"). It answers one question:
is the chain correctly calibrated, or is it a modelling error?

Short answer: **one real error, in `wing.rs`, now fixed**; the exponent is right; and
the remaining gap is a **connectome-command shortfall**, not physics.

---

## 1. Does the force scale with amplitude squared?

Yes, and it did before the fix too. `wing_force_magnitude` (src/wing.rs) computes

```
<U^2> = (Phi/2)^2 * omega^2 / 2 * R^2 * k2          (cycle-mean squared section speed)
omega = 2*pi*f,  Phi = full peak-to-peak stroke angle
F     = 1/2 * rho * C * <U^2> * S
```

so the section speed is proportional to amplitude x frequency and the force to the
speed squared: **quadratic in amplitude, quadratic in frequency**, with no offset.
For `Phi = (Phi/2)*cos(omega*t)`, `<dtheta/dt^2> = Phi^2 omega^2/8`, and
`sqrt(<U^2>) = pi f Phi / sqrt(2)`, which is *exactly* the spec's own
`omega_rms = pi f Phi / sqrt(2)` (physical-model-spec.md §6.3). At `Phi = 160 deg`,
`f = 200 Hz` that is 1.70 m/s at the second-moment radius — the spec's independently
stated `U_bar = 1.7 m/s` (§2, §6.3). So the amplitude/frequency convention and the
velocity are both right; a factor-of-2 or `r2`-vs-`R` error would have shown up here.

`wing::tests::stroke_force_is_quadratic_in_amplitude` now pins the exponent: doubling
the amplitude multiplies the force by 4.000, halving it divides by 4.000, and zero
amplitude gives exactly zero.

## 2. What was wrong: the stroke-plane-normal force used the resultant coefficient

The spec integrates the blade element as a **vector** sum (§1.3):

```
dF = 1/2 rho c |U|^2 [ C_L(alpha) n_L + C_D(alpha) n_D ] dr ,  n_D = -U/|U|
```

The wing sweeps *in* the stroke plane, so `U` lies in that plane, `n_D` therefore lies
in it as well, and `n_L` (perpendicular to `U`) is the stroke-plane **normal**. That
normal is exactly the direction the code applies the force along (`wing_force_vector`,
rotated by the steering tilt), so the quantity it needs is the normal component:

```
<F_n> = 1/2 rho C_L(alpha) <U^2> S
```

The drag term contributes **nothing** to the cycle mean: it reverses with `U` at every
half stroke and averages to zero in every direction, while the lift term keeps one sign
because the wing flips its angle of attack at each reversal.

The pre-fix code computed `C_F = sqrt(C_L^2 + C_D^2)` and applied the whole resultant
**along the normal**. At `alpha = 40 deg` that is `2.3837 / 1.7696 = 1.347`, i.e. the
lift force was **35 % too large** — the drag magnitude was being spent in the lift
direction.

The spec calls this out as a *mandatory* self-check (§6.3): a hovering configuration
must give `F_lift/weight ~ 1-1.3`, integrated as `F = 1/2 rho C_L omega_rms^2 S R^2/3`,
and "if the implemented model does not reproduce this within a few percent ... a
coefficient convention is wrong". Measured on the pre-fix code, one wing at full stroke
gave **1.79 x half the body weight** (spec target 1.22 for the spec's 1.8 mm^2 wing,
1.39 at the planform measured from the mesh). After the fix: **1.33**.

`wing::tests::normal_force_reproduces_the_specs_mandatory_self_check` recomputes that
integral from the geometry constants and pins the convention. It **fails** on the
resultant form and passes on `C_L`; it is the only test that distinguishes the two.

### Change

- `src/wing.rs`, `wing_force_magnitude`: `0.5 * RHO * cf * u2 * WING_AREA` ->
  `0.5 * RHO * cl * u2 * WING_AREA`, with the derivation above in the doc comment.
- `wing_damping()` still uses `C_D`: that term is the *in-plane* drag resisting body
  rotation, where the drag coefficient is the right one. Unchanged.

## 3. The activation -> amplitude map (body.rs)

```rust
let span = wing::STROKE_AMP_MAX - wing::STROKE_AMP_MIN;
let amp_l = wing::STROKE_AMP_MIN + span * m.flight_power_l.clamp(0.0, 1.0);
```

i.e. `amp = STROKE_AMP_MAX * (0.12 + 0.88 * a)`, then a 20 ms first-order lag
(`MUSCLE_TAU`) stands in for the thorax/muscle that the dataset does not contain.

- The **a = 1 endpoint is the spec's, not a fit.** §6.5 pins the muscle activation gain
  as "scale to give `F/W ~ 1.2` at `a = 1`", and §5.1-5.3 require only that `a(t)` change
  the *amplitude* while leaving the 200 Hz beat constant (the defining signature of
  asynchronous flight muscle). The map does both; it is documented in `body.rs` as the
  surrogate it is.
- The **floor** (`STROKE_AMP_MIN = 0.12 * max`) is inherited from the pre-embodiment
  `0.12 + 0.95 * power` curve and is an [E] choice, not a measurement. It can only *add*
  force, so it is not masking a shortfall.
- The **rate -> a conversion is the one unpinned link.** The read-out is the fraction of
  the wing-power pool that fired in the 2 ms control window, and `body.rs` uses that
  fraction directly as `a`. The spec fixes what `a = 1` *means* (full stroke, hovering
  force plus margin) but not how many spikes per second of motor drive that is, and the
  repo holds no measurement that fixes it. Choosing a different conversion here — e.g.
  normalising by the physiological wing-power rate of one spike per wingbeat (200 Hz) —
  would raise `a` by ~2.5x and *would* make the fly leave the ground. That is a surrogate
  picked to produce a result, so it is deliberately **not** done here. It is, however,
  the honest statement of what the conclusion below rests on.

  > **Superseded.** This conversion has since been grounded in the asynchronous-flight
  > physiology and fixed: the full scale of a flight power motor neuron is **20 Hz per
  > neuron** (a DLM/DVM motor neuron fires ~5 Hz in flight, 3-12 Hz working range, up to
  > ~20 Hz manoeuvring), not the 500 Hz the spike-count window can express. See
  > [`activation-scale.md`](activation-scale.md). Note the direction: the grounded answer
  > is *not* the 200 Hz "one spike per wingbeat" figure rejected above — asynchronous
  > muscle fires far *below* the wingbeat, which is the whole point of §5.1 — and it makes
  > the read-out saturate, because the model's power pool runs at ~334 Hz/neuron. The
  > conclusion of §5 below is therefore revised: the shortfall is not that the connectome
  > issues two-thirds drive, but that the pool's absolute rate is far outside physiology.

## 4. Before / after

Controlled A/B, 12 s, seed 7, full 166,700-neuron model. Same tree, same commit
snapshot; the **only** difference is the coefficient in `wing_force_magnitude`.

| | before (`C_F`) | after (`C_L`) |
|---|---|---|
| takeoffs | 0 | 0 |
| cruise % | 0.0 | 0.0 |
| ground % | 100.0 | 100.0 |
| altitude mean / max, mm | 0.0 / 0.0 | 0.0 / 0.0 |
| path (horizontal), mm | 2 | 3 |
| mean speed, mm/s | 0 | 0 |
| wing stroke amplitude, mean / max, rad | 1.918 / 1.999 | 1.941 / 2.011 |
| wing-power read-out mean / max | 0.659 / 0.706 | 0.668 / 0.728 |
| wing lift / weight, mean / peak (body frame) | 0.842 / 0.911 | 0.639 / 0.683 |
| one wing at full stroke, x half weight | 1.79 | 1.33 |
| width of the shortfall | needs a = 0.73 | needs a = 0.865 |

The read-out itself moved *up* slightly (0.659 -> 0.668 mean) because `sim.rs` feeds
`(weight - wing_lift)/weight` back into the network while the fly is on the ground: less
lift means more sensed load, which the network answers with more drive. A real
load/unload loop, and it compensates only part of the way.

`cargo test --release`: 32 passed (30 pre-existing + the 2 added here). The other
surrogate-free invariants are unchanged — full-stroke lift still beats weight even at
the steering tilt limit, the fly still lifts off under full commanded power in
`grounded_fly_lifts_off_when_its_wings_beat`, and the exponential scaling is intact.

## 5. Verdict

- **The exponent was never the problem.** Force is quadratic in amplitude before and
  after. There is no linear-in-amplitude error to fix.
- **There was one physics error, and it was in the fly's favour.** The resultant
  coefficient overstated the stroke-plane-normal force by 35 %. Fixing it *reduces* the
  lift and widens the gap. It flips no conclusion.
- **The 0.84 lift-to-weight at 66 % recruitment was not wrong in kind, only in
  magnitude** (it is correctly 0.64). A fly at two-thirds of full stroke amplitude does
  not hover, and the model says so: level release needs the wings to carry 1.0 weight,
  which needs `a ~ 0.87` at the measured mean stroke-plane tilt (`AMP = 2.43 rad =
  139 deg`).
- **The remaining gap is a connectome-command shortfall.** The pool is at 0.67 mean /
  0.73 peak against a required 0.87. It is also close to tonic — 74 % of samples sit
  above 0.65 and the 12 s range is 0.00-0.73 — so it is issuing a steady two-thirds
  drive, not a graded power command that peaks short. No physical constant was moved to
  close the gap, and none should be.
- **Caveat, stated because it is load-bearing:** the conclusion depends on taking the
  read-out fraction as the muscle activation directly. That conversion is [E]; see §3.