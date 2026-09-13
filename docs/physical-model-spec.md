# Physical model specification: Drosophila flight and mechanosensory dynamics

**Target:** `flyverse` (`src/body.rs`, `src/sim.rs`) — a Rust connectome simulation in which
behaviour should emerge from real wing/leg motor-neuron rates acting on real physics,
instead of hand-tuned control loops.

**Status:** this document *specifies* the physics to implement. It replaces the surrogate
block in `body.rs` (`LIFT_MAX`, `THRUST_MAX`, `WING_AMP_HOVER`, the quadratic/linear drag,
and the first-order wing-amplitude lag). It does **not** claim the connectome contains
muscles, a thorax, or haltere afferents — those must be added as explicit physical
elements driven by the neurons the connectome does contain.

**Units:** SI throughout **for the physics layer** (metres, seconds, kilograms, newtons,
radians). The existing `body.rs` runs in millimetres; the conversion is stated in each
section and is a multiply-by-1000 on lengths. It is strongly recommended that the new
physics core be written in SI and only converted at the presentation/wire boundary,
because the published aerodynamic coefficients and inertias are all SI.

**Honesty rule used below:** every numeric parameter is tagged **[M]** measured
(published, with citation), **[D]** derived (computed from measured quantities — the
arithmetic is given), or **[E]** estimated / engineering choice (range given, not
established). Values that are *not* well established in the literature are said so
explicitly rather than pinned to a false precision.

---

## 0. What is being replaced, and the interface to the neurons

The current model (verbatim from `body.rs`):

```
lift   = LIFT_MAX * amp^2 * cos(roll) * cos(pitch)          // LIFT_MAX = G/(0.35^2)
thrust = THRUST_MAX * amp * heading                          // THRUST_MAX = 3000 mm/s^2
drag   = -(DRAG_Q*|v| + DRAG_L) * v                          // tuned to cap speed at 350 mm/s
amp    <- first-order lag (tau = 20 ms) of decoded wing-power MN rate
```

Three fatal problems, in order of importance:

1. **Force direction is decoupled from wing kinematics.** Lift is defined as "up in the
   world frame times cos(roll)cos(pitch)" and thrust as "along the *yaw heading*". A real
   wing produces force in the plane perpendicular to its own motion, in the **body**
   frame; body attitude then rotates that force into the world. With the current model the
   wings cannot produce a pitch or roll torque at all — attitude is a separate hand-tuned
   loop — so the flight posture can never *emerge* from the wingbeat.
2. **The wingbeat is a single scalar.** Real force and torque come from the time course of
   two wings' stroke angle, angle of attack, and stroke-plane orientation. Collapsing this
   to one scalar `amp` discards exactly the degrees of freedom the steering motor neurons
   control.
3. **No inertial or gyroscopic terms**, so the haltere (the fly's angular-rate sensor) has
   nothing physical to sense.

### 0.1 Neural interface (what the connectome actually provides)

`src/groups.rs` selects these readouts; the physics must consume them, not replace them:

| Group (from `male_cns_v1_neural_io.json`) | Use in this model |
|---|---|
| `motor_flight_power_left/right` | drive for the DLM/DVM power-muscle activation `a(t)` (wingbeat amplitude) |
| `motor_flight_steering_left/right` | drive for the direct steering muscles → per-wing stroke-plane tilt and AoA asymmetry (turn/yaw torque) |
| `flight_steering_dn_left/right`, `flight_dng02/07`, `flight_state_sapp` | descending command; may modulate baseline stroke amplitude / pitch bias |
| `motor_walking_left/right`, `motor_landing_*` | leg model (out of scope here; unchanged) |

Crucially, the physics must expose **back-to-the-brain channels** that do not exist today:
haltere afferent drive, wing-hinge campaniform / stretch feedback, and the descending
error signals the fly actually uses. Section 7 gives the concrete form.

---

## 1. Quasi-steady blade-element aerodynamics for Drosophila wings

### 1.1 Choice of framework and its justification

Full Navier–Stokes is out of reach at `flyverse`'s control rate (2 ms window, target
real-time). The defensible substitute is the **quasi-steady blade-element (BET) model**,
which has been validated against dynamically scaled robot-wing measurements at the correct
Reynolds number and against free-flight force measurements:

> Fry, S.N., Sayaman, R., Dickinson, M.H. (2005). *The aerodynamics of hovering flight in
> Drosophila.* J. Exp. Biol. 208, 2303–2318. doi:10.1242/jeb.01612 — "Quasi-steady
> mechanisms could account for nearly all of the mean measured force required to hover."

> Dickson, W.B., Straw, A.D., Dickinson, M.H. (2008). *Integrative model of Drosophila
> flight.* AIAA Journal 46(9), 2150–2164. — the reference whole-fly quasi-steady flight
> model; this spec follows its structure.

The model decomposes the instantaneous force on a wing into **translational +
rotational + added-mass** terms (plus, optionally, wake capture, which is small and can be
omitted at first — see §1.6):

```
F_wing(t) = F_trans + F_rot + F_added + (F_wake)
```

### 1.2 Blade-element geometry

Parameterise the wing by spanwise position `r ∈ [0, R]`, local chord `c(r)`, wing planform
area `S = ∫ c dr`. The local flow speed and angle:

```
U(r,t) = ω_stroke(t) × r          (element velocity, body frame; ω_stroke = dφ/dt ŷ_stroke)
α(r,t) = angle between the chord and U    (local geometric angle of attack)
```

### 1.3 Translational force (the dominant term)

Per unit span, with `n̂_L` the in-wing-plane lift direction and `n̂_D = −U/|U|` the drag
direction:

```
dF_trans(r) = ½ ρ c(r) |U(r)|² [ C_L(α) n̂_L + C_D(α) n̂_D ] dr
```

**Coefficient form.** The accepted Drosophila form is the sinusoid-in-2α fit of
Dickinson/Lehmann/Sane and Sane/Dickinson, with `α` in **degrees**:

```
C_L,t(α) = 0.225 + 1.58 sin(2.13 α − 7.2)          [M]
C_D,t(α) = 1.92 − 1.55 cos(2.13 α − 7.2)           [M]
```

> Sane, S.P., Dickinson, M.H. (2002). *The aerodynamic effects of wing rotation and a
> revised quasi-steady model of flapping flight.* J. Exp. Biol. 205, 1087–1096.
> doi:10.1242/jeb.205.8.1087 — coefficients measured on a dynamically scaled Drosophila
> wing at **Re ≈ 115**.

> Dickinson, M.H., Lehmann, F.-O., Sane, S.P. (1999). *Wing rotation and the aerodynamic
> basis of insect flight.* Science 284, 1954–1960. doi:10.1126/science.284.5422.1954

The **canonical scholarly form** cited in review articles is the generic two-term fit
`C_L = A sin 2α`, `C_D = B − C cos 2α` with `A ≈ 1.6`, `B ≈ 1.9`, `C ≈ 1.5`; the
Sane–Dickinson numbers above are the concrete parameterisation (the `2.13` factor and the
`−7.2°` phase encode a slight asymmetry — the wing stalling earlier at negative α). The
following table is computed from those fits (this document; verify against the hover force
balance in §6.3):

| α (deg) | C_L | C_D |
|---|---|---|
| 25 | 1.47 | 1.24 |
| 35 | 1.68 | 1.32 |
| 37 | 1.72 | 1.43 |
| 45 | 1.80 | 1.88 |
| 50 | 1.78 | 2.17 |
| 60 | 1.66 | 2.67 |

Notes for the implementer:
- These fits were measured for **translational** (steadily sweeping) motion. They already
  embody the lift enhancement from the **leading-edge vortex (LEV)**, i.e. "delayed
  stall": at Re ~ 100–200 the flow does not separate in the usual sense; a stable LEV sits
  on the leading edge and feeds a spanwise-flow-stabilised vortex that keeps C_L high well
  beyond the ~14° flat-plate stall angle.
- The LEV's stabilisation depends on spanwise flow and wing aspect ratio (AR ≈ 7–8 for a
  Drosophila wing pair); at AR below ~3 the LEV sheds and C_L collapses.
  > Ellington, C.P., van den Berg, C., Willmott, A.P., Thomas, A.L.R. (1996). *Leading-edge
  > vortices in insect flight.* Nature 384, 626–630.
  > Birch, J.M., Dickinson, M.H. (2001). *Spanwise flow and the attachment of the
  > leading-edge vortex on insect wings.* Nature 412, 729–733.
- **Reynolds number.** `Re = c Ū / ν` with the mean wing velocity `Ū`. For Drosophila
  (`c ≈ 0.75 mm`, `Ū` at the second-moment radius ≈ 1.7 m/s, ν = 1.5×10⁻⁵ m²/s) this gives
  **Re ≈ 85 at ⅔ span and ≈ 210 at the tip** [D] — the classical "Re ≈ 100–200" regime.
  The coefficients are approximately Re-independent over **Re ≈ 100–1000**
  (Sane & Dickinson 2002); below Re ~ 50 viscosity degrades C_L.

### 1.4 Rotational (pitch-rate) circulation

During pronation/supination the wing rotates rapidly about its own axis, adding a
circulatory force **linear in the pitch rate** `α̇`. Per unit span, in the direction
perpendicular to the wing chord (i.e. adding to or subtracting from the translational lift):

```
dF_rot(r) = ½ ρ C_rot(r) · c(r)² · |U(r)| · α̇(t) · n̂_L(r) dr
C_rot(r)  = π ( 0.75 − x̂(r) )
```

where `x̂(r)` is the distance of the pitching axis **from the leading edge**, measured in
local chords. (Some implementations use `ρ`, not `½ρ`, and fold the factor into a quoted
coefficient — see the convention warning below.) The clean non-dimensional statement is
the one worth implementing:

```
C_rot = π (0.75 − x̂)                     (inviscid theory, "critical axis")
C_rot ≈ 1.5 ± 0.5                         (measured, Sane & Dickinson 2002)   [M/E]
```

where `x̂` is the distance of the pitching axis **from the leading edge**, measured in
chords. Theory puts a *critical axis* at `x̂ = 0.75`: rotation about that axis adds no
circulation; forward of it adds lift, aft of it removes lift.

> Munk, M.M. (1925) and Fung, Y.C. (1969) — the inviscid origin, as reviewed in Sane &
> Dickinson (2002), §Introduction.
> Sane, S.P., Dickinson, M.H. (2002), *op. cit.* — measured rotational coefficients;
> "the rotational coefficient varied linearly with the position of the rotational axis",
> in agreement with `π(0.75 − x̂)`.

For a Drosophila wing hinged at the root (root chord near the leading edge, effectively
`x̂ ≈ 0.2–0.3`), `C_rot ≈ π·0.5 ≈ 1.57` [D], consistent with the measured ≈ 1.55.
Implement as a per-element force proportional to `c(r)² · |U(r)| · α̇`, directed
perpendicular to the wing chord in the stroke plane. **Convention warning:** the factor of
½ that appears in the translational term is absorbed differently in different papers; pick
one convention, and validate by checking that a hovering configuration reproduces
`F_lift ≈ W` (§6.3). Do not mix coefficients from two papers.

### 1.5 Added mass ("virtual mass" / inertia of the air)

Because the wing oscillates at ~200 Hz, the *air* it displaces has appreciable inertia. Per
unit span, for a thin plate (added mass per unit span `m_a = (π/4) ρ c(r)²`):

```
dF_added(r) = (π/4) ρ c(r)² · n̂_w ( n̂_w · a(r) ) dr
```

where `a(r)` is the element acceleration and `n̂_w` the wing-surface normal (only the
normal acceleration component contributes). Sane & Dickinson (2002) include this term
explicitly and call it "the added mass inertia". This is a real, easily-implemented term
and should not be dropped — at 200 Hz it is a non-negligible fraction of the wing's own
inertia (≈ 16 % by our estimate, §3.3).

### 1.6 Wake capture (optional, later)

Fry et al. (2005) and Dickinson et al. (1999) show wake capture contributes a transient
force peak at each stroke reversal. Sane & Dickinson (2002) isolate it as the residual
after subtracting translation + rotation + added mass. It is *not* needed for the mean
force in the flyverse use case; add it only if stroke-reversal transients turn out to
matter for the emergent behaviour. If added, its magnitude is **not** well captured by a
simple closed form — it must be measured or taken from a specific model, so it is left out
of the recommended parameter set.

---

## 2. Wing kinematics for Drosophila in free flight

All values for *Drosophila melanogaster* unless noted. The stroke is parameterised as

```
φ(t) = (Φ/2) cos(2π f t)            stroke angle (in the stroke plane)
α(t) = α_mid ± α_amp  (rapid flip at t = 0, ½ cycle)
β                                    stroke-plane angle (orientation of the stroke plane)
```

| Quantity | Value | Tag | Source |
|---|---|---|---|
| Stroke amplitude Φ (full, peak-to-peak) | **145–165°**, use **160°** | [M] | Altshuler et al. 2005 PNAS state Drosophila amplitude is "≈145–165°"; free-flight hover measured by Fry et al. 2005 |
| Wingbeat frequency f | **200–220 Hz**, use **200 Hz** (hovering free flight); 212 Hz measured in one free-flight study, 213 Hz used in CFD | [M] | Fry, Sayaman & Dickinson 2005, J. Exp. Biol. 208:2303; Sannomiya et al. / Nat. Commun. 9:4005 (2018) uses 213 Hz |
| Midstroke angle of attack α_mid | **~37°** to balance weight (D. virilis CFD); measured ≈ 45° | [M] | Sun & Tang 2002, J. Exp. Biol. 205:2413; Dickinson, Lehmann & Sane 1999 |
| Angle-of-attack sweep | ±~40–50° about α_mid, flips at stroke reversal | [E] | inferred from Fry et al. 2005, Figs; not a single published scalar |
| Mean wing velocity Ū (2nd-moment radius) | ~1.7 m/s at 160°/200 Hz | [D] | computed, §6.3 |
| Wing-tip velocity | ~4.2 m/s at 160°/200 Hz | [D] | computed, §6.3 |
| Reynolds number (chord-based) | **85 (⅔ span) – 210 (tip)**; "Re ≈ 100–200" | [D]/[M] | computed; Sane & Dickinson 2002 use Re ≈ 115 |
| Stroke-plane angle β | not a single well-established number for free-flying Drosophila; reports for tethered/flight vary widely (roughly **40–80°** from horizontal depending on flight mode and body pitch). Treat as a **free parameter** and as the steering degree of freedom. | [E] | see uncertainty register |

> Altshuler, D.L., Dickson, W.B., Vance, J.T., Roberts, S.P., Dickinson, M.H. (2005).
> *Short-amplitude high-frequency wing strokes determine the aerodynamics of honeybee
> flight.* PNAS 102(50), 18213–18218. (states Drosophila stroke amplitude ≈145–165°)

> Fry, S.N., Sayaman, R., Dickinson, M.H. (2005), *op. cit.*
> Sun, M., Tang, J. (2002). *Lift and power requirements of hovering flight in Drosophila
> virilis.* J. Exp. Biol. 205, 2413–2427.

**Key kinematic facts the model must reproduce:**
- The stroke is **U-shaped in the side view and roughly planar in the stroke plane**; most
  vertical force is produced during the *translational* portions, i.e. mid-stroke, with the
  force peak in the early downstroke (Fry et al. 2005).
- **Wing rotation is active and precisely timed**: the fly controls the timing of the
  flip relative to the stroke reversal to shift lift between the two half-strokes, which is
  how it produces pitch/roll torque without changing stroke amplitude.
  > Dickinson, M.H., Lehmann, F.-O., Götz, K.G. (1993). *The active control of wing
  > rotation by Drosophila.* J. Exp. Biol. 182, 173–189.
- Steering muscles (tp1, tp2, hg1–hg4, b1–b3) act as **cycle-by-cycle** modulators of
  stroke amplitude, stroke-plane tilt, and flip timing — i.e. they are *not* a
  slowly-varying amplitude knob. This is why the `motor_flight_steering_*` groups should
  drive wing asymmetry and flip timing, not a scalar.

---

## 3. Wing morphology, inertial and added-mass terms

### 3.1 Measured morphology (D. melanogaster) — [M] unless tagged

| Quantity | Value | Source |
|---|---|---|
| Body mass m_body | **0.98 mg** (0.983 mg, 52 female flies) | Vaxenburg et al. 2024/2025 (flybody) |
| Wing mass m_w (each) | **8 µg** (0.008 mg) | Vaxenburg et al. 2024/2025 (flybody) |
| Wing length R | **2.4–2.9 mm**; use **2.4 mm** (Ellington's D. melanogaster value; flybody/Nat. Commun. give 2.87 mm for a digitised wing) | Ellington 1984b; Sannomiya et al. 2018, Nat. Commun. 9:4005 |
| Single-wing area S | **≈1.5–2.6 mm²**; use **1.8 mm²** | Nat. Commun. 9:4005 gives 2.59 mm² for one digitised wing; Ellington 1984b gives ~1.6 mm². Range is real (size varies with temperature/larval density). |
| Mean chord c = S/R | **0.75 mm** | [D] from S, R above |
| Radius of 2nd moment of area / mass, r₂ | **0.58 R** | Sun & Tang 2002 (r₂ = 0.58 R for the D. virilis wing model) |
| Wingbeat frequency f | 200–220 Hz | see §2 |

> Ellington, C.P. (1984). *The aerodynamics of hovering insect flight. II. Morphological
> parameters.* Phil. Trans. R. Soc. Lond. B 305, 17–40.

> Vaxenburg, R., Siwanowicz, I., Merel, J., Robie, A.A., Morrow, C., Novati, G.,
> Stefanidi, Z., Both, G.-J., Card, G.M., Reiser, M.B., Botvinick, M.M., Branson, K.M.,
> Tassa, Y. (2025). *Whole-body physics simulation of fruit fly locomotion.* Nature,
> doi:10.1038/s41586-025-09029-4 (bioRxiv 2024.03.11.584515). — mass table, 52 flies.

> Sannomiya, K., et al. / the morphologically accurate D. melanogaster CFD model
> (Nat. Commun. 9:4005, 2018) — wing area 2.59 mm², span 2.87 mm.

### 3.2 Wing moment of inertia about the stroke hinge — [D]

Approximating the wing as a uniform rod of mass `m_w` about its base (uniform area→mass
distribution, consistent with `r₂ = 0.58 R` which equals `R/√3`):

```
I_w = m_w R² / 3
    = 8×10⁻⁹ kg · (2.4×10⁻³ m)² / 3
    = 1.54×10⁻¹⁴ kg·m²                        [D]
```

(Using `I = m_w (0.58 R)²` gives the same to within 1 %.) This is an **estimate of
distribution**, not a measurement: the real wing's mass is concentrated proximally, which
*reduces* `I_w`. Implement as a per-element sum `I_w = Σ m_i r_i²` if the wing is modelled
as segments; the rod value is the upper bound.

### 3.3 Added-mass moment of inertia — [D]

```
I_added = (π/4) ρ ∫ c(r)² r² dr ≈ (π/4) ρ c̄² R³/3
        = (π/4)·1.225·(0.75×10⁻³)²·(2.4×10⁻³)³/3
        = 2.5×10⁻¹⁵ kg·m²                     [D]
```

So `I_added / I_w ≈ 0.16` — the air's virtual inertia is ~16 % of the wing's own. Both
terms must be included; neither is negligible.

### 3.4 Inertial vs aerodynamic torque — [D]

Peak stroke angular acceleration at Φ=160°, f=200 Hz:

```
φ̈_max = (2π f)² (Φ/2) = (1257)²·1.396 = 2.2×10⁶ rad/s²
τ_inertial = I_w φ̈_max ≈ 3.4×10⁻⁸ N·m
τ_aero     ≈ F·R/2 ≈ 7×10⁻⁹ N·m           (F ≈ 5.9 µN, see §6.3)
```

**Inertial torque is a few times the mean aerodynamic torque.** This is the single most
important physical fact for this model: the wing's own inertia dominates the instantaneous
torque balance, so the wingbeat waveform is set largely by the flight-muscle/thorax
mechanics (§5) and the aerodynamics supplies the *net* force on the body. A model that
ignores wing inertia (as the current `body.rs` does) cannot correctly represent the
phase relationship between muscle activation and wing force.

---

## 4. Haltere dynamics as an angular-rate sensor

### 4.1 Biology in one paragraph

The halteres are the modified hind wings: club-shaped organs, one per side, that oscillate
**anti-phase to the wings** at the wingbeat frequency, driven by the same indirect flight
muscles through thoracic linkages.
> Pringle, J.W.S. (1948). *The gyroscopic mechanism of the halteres of Diptera.* Phil.
> Trans. R. Soc. Lond. B 233, 347–384.
> Deora, T., Singh, A.K., Sane, S.P. (2015). *Biomechanical basis of wing and haltere
> coordination in flies.* PNAS 112(5), 1481–1486. doi:10.1073/pnas.1412279112

Because the haltere is moving, **body rotation produces Coriolis forces** on it
(`F = 2 m Ω × v`), which deflect it out of its stroke plane and strain campaniform sensilla
at its base. Those strains drive the flight-stabilising reflexes.
> Dickinson, M.H. (1999). *Haltere-mediated equilibrium reflexes of the fruit fly,
> Drosophila melanogaster.* Phil. Trans. R. Soc. Lond. B 354, 903–916.
> doi:10.1098/rstb.1999.0442
> Mohren, T.L., Daniel, T.L., Eberle, A.L., Reinhall, P.G., Fox, J.L. (2019). *Coriolis and
> centrifugal forces drive haltere deformations and influence spike timing.* J. R. Soc.
> Interface 16, 20190035. doi:10.1098/rsif.2019.0035 — shows Coriolis **and centrifugal**
> forces both matter, and that they produce 3-D strain patterns (a point-mass model misses
> the twist and the centrifugal bending).

### 4.2 Physical model to implement — [D] formula, [E] parameters

Model each haltere as a point mass `m_h` on a rigid massless rod of length `L_h`, hinged at
the base, oscillating in its stroke plane with

```
θ_h(t) = Θ_h sin(2π f t)            (anti-phase to the ipsilateral wing)
θ̇_h(t) = 2π f Θ_h cos(2π f t)
v_h(t)  = L_h θ̇_h(t) · ê_t          (velocity of the tip, body frame)
```

Let `ê_t` be the in-plane tangential unit vector at the tip and `n̂_p` the unit normal to
the haltere stroke plane. The **out-of-plane (sensing) deflection** comes from the
stroke-plane-normal component of the Coriolis acceleration:

```
a_⊥(t) = 2 (Ω × v_h(t)) · n̂_p = 2 L_h θ̇_h(t) · (Ω · (ê_t × n̂_p))
```

So the haltere is sensitive to the component of `Ω` along the in-plane radial axis
`ê_d ≜ ê_t × n̂_p` (for a haltere whose stroke plane contains the longitudinal body axis,
`ê_d` is essentially the **roll** axis; the geometry must be set from the haltere's actual
stroke-plane orientation). The base strain (and hence the mechanosensor drive) is

```
ε_h(t) ≈ (L_h / κ) · [ 2 Ω_⊥(t) L_h θ̇_h(t)  +  Ω² L_h (centrifugal term) ]
```

with `κ` an effective base stiffness. **The centrifugal term `∝ Ω²` is real and must not be
dropped** (Mohren et al. 2019 find it produces bending independent of rotation direction).

### 4.3 Afferent encoding — how to drive haltere mechanosensory neurons

Known encoding facts (these constrain the functional form):

- Primary haltere afferents "encode its **oscillation frequency linearly over a wide
  bandwidth** and with **precise phase-dependent spiking**."
  > *Representation of Haltere Oscillations and Integration with Visual Inputs in the Fly
  > Central Complex.* J. Neurosci. 39(21), 4100–4112 (2019).
- Afferents respond with **high temporal precision and short latency** to multiple stimulus
  features, including Coriolis forces; the encoding is nonlinear in the stimulus and driven
  by the derivative pair (position, velocity) — i.e. not a simple rate code.
  > Fox, J.L., Fairhall, A.L., Daniel, T.L. (2010). *Encoding properties of haltere neurons
  > enable motion feature detection in a biological gyroscope.* PNAS 107(8), 3840–3845.
  > doi:10.1073/pnas.0912548107
- Bandwidth: stimuli up to **150 Hz** were used (band-limited white noise 1–150 Hz
  "includes frequencies below, at, and above the frequency of naturally occurring Coriolis
  forces") → afferent low-pass cut-off is at least ~150 Hz; a first-order filter with
  **τ ≈ 1 ms** is a defensible [E] choice.
- The **reflex** the afferents feed acts as a **proportional–integral controller on body
  angular velocity**, with a ~1-wingbeat delay.
  > Ristroph, L. et al. / as summarised in *Timing precision in fly flight control:
  > integrating mechanosensory input and wing control.* Proc. R. Soc. B 287, 20201774
  > (2020).

**Recommended usable functional form** (drives a Poisson population of haltere afferents):

```
Ω_⊥(t)   = Ω_body(t) · ê_d                       (rad/s, body frame)
g(t)     = 2 L_h θ̇_h(t) Ω_⊥(t) + c_cent Ω_body(t)² · (n̂_s · ê_cent)
rate_h(t) = clamp( f_wing · (1 + β · g(t)/(g_ref)) , 0, rate_max )
          then low-pass filtered with τ_h ≈ 1 ms
```

with the affinity field `ê_d` and coefficient `β` set so that the known reflex gain is
reproduced (rotations of order 10–1000 °/s produce clear corrective wing-hitch responses).
The `f_wing` baseline implements the documented fact that afferents fire ~once per
oscillation cycle in the absence of rotation; the rotation term modulates the per-cycle
spike count/timing. This is exactly the interface the connectome simulation needs: a
**rate that is a monotone function of the simulated body angular velocity**, with a
built-in wingbeat-frequency carrier.

**Parameter values (all [E] / not tightly established for Drosophila):**

| Parameter | Value | Basis |
|---|---|---|
| Haltere length L_h | **~0.3–0.4 mm** | haltere morphology is ~1/6–1/7 the wing length; must be taken from a haltere morphology study for the exact strain (Diptera haltere length varies with body size — see *Scaling of sense organs that control flight*, J. Zool., doi:10.1111/jzo.13117) |
| Haltere mass m_h | order **10⁻¹⁰–10⁻⁹ kg** | not pinned here; derive from haltere volume × cuticle density. Even a factor-of-2 error changes only the afferent gain. |
| Haltere stroke amplitude Θ_h | tens of degrees; use ~40–60° | [E], from coupled-oscillator haltere kinematics |
| Haltere frequency | = wingbeat frequency (anti-phase) | Pringle 1948; Deora et al. 2015 |
| Campaniform sensilla count | "hundreds" per haltere | *The role of haltere campaniform sensilla in equilibrium reflexes of the fruit fly*, J. Exp. Biol. 229, jeb250431; exact Drosophila count not pinned here |
| Afferent low-pass τ_h | ~1 ms | [E] from 150 Hz bandwidth |
| Coriolis force magnitude, for scale | 2.3×10⁻¹⁰ N at Ω = 1 rad/s (with m_h=0.5 ng, L_h=0.35 mm) | [D] computed |

The exact mapping of haltere fields to sensed axes (which campaniform field senses roll vs
pitch vs yaw) comes from the haltere connectivity atlas:
> *A neural connectivity atlas for fly flight control.* Current Biology (2025),
> doi:10.1016/j.cub.2025.xx — five morphological afferent subtypes, each targeting muscles
> controlling different aspects of wing motion.

---

## 5. Thoracic muscle and stretch-activated flight-motor dynamics

### 5.1 The mechanism (why Drosophila wingbeat is *not* one spike per beat)

Drosophila uses **asynchronous indirect flight muscles (IFM)**: the dorsolongitudinal
(DLM) and dorsoventral (DVM) muscles are arranged antagonistically and are stretched/
shortened by the thorax's own oscillation. A rapid stretch produces a **delayed increase in
tension** — *delayed stretch activation* (dSA) — so muscle tension lags strain by a
fraction of a cycle. With the thorax as the elastic element, this delayed feedback produces
**self-sustained oscillation at the wingbeat frequency**, decoupled from the motor-neuron
firing rate (the neurons only set the *level* of activation, not the beat).

> Pringle, J.W.S. (1949). *The excitation and contraction of the flight muscles of
> insects.* J. Physiol. 108, 226–232.
> Machin, K.E., Pringle, J.W.S. (1959). *The physiology of insect fibrillar muscle. II.
> Mechanical properties of a beetle flight muscle.* Proc. R. Soc. Lond. B 151, 204–225.
> Pringle, J.W.S. (1978) — reviews the delayed-tension/oscillation argument.

### 5.2 The equations to implement

The **spring-wing oscillator** (validated against dynamically-scaled robot wings and
against insect physiology):

```
I θ̈ + Γ θ̇ |θ̇| + k θ = τ_muscle(θ, θ̇, t)
```

> Lynch, J., Gau, J., Sponberg, S., Gravish, N. (2021). *Emergent wingstroke in asynchronous
> insects and robots is governed by time-delayed strain rate feedback.*
> Gau, J., Gravish, N., Sponberg, S. and related spring-wing work (J. R. Soc. Interface).

where:
- `I` = total rotational inertia of the flapping system (wing inertia + added mass, §3, plus
  thorax–wing transmission ratio),
- `k` = thoracic elastic stiffness,
- `Γ θ̇|θ̇|` = **quadratic** aerodynamic + structural damping (this is where the aerodynamic
  drag from §1.3 feeds back into the oscillator — important: do *not* use linear damping),
- `τ_muscle` = the delayed-stretch-activated muscle torque.

The **dSA is modelled as a 2nd-order low-pass filter acting on strain rate**, with a time
delay — this is the model that reproduces the emergent wingstroke:
> Lynch et al. (2021), *op. cit.*, §3 "Delayed Stretch-Activation acts as a 2nd-Order
> Low-Pass Filter on Strain Rate"; and the same delayed-stretch-activation model used in
> *Bridging two insect flight modes in evolution, physiology and robophysics*, Nature
> (2023), doi:10.1038/s41586-023-06606-3.

Concretely, implement the muscle torque as

```
τ_muscle(t) = a(t) · [ k_a · θ(t) + ∫₀^∞ h(τ) θ̇(t − τ) dτ ]
```

where `a(t) ∈ [0,1]` is the activation set by the decoded `motor_flight_power_*` rate, and
`h` is the impulse response of the second-order low-pass. In practice a discrete form works:

```
y_{n} = y_{n-1} + Δt · ( θ̇_{n-d} − y_{n-1} ) / τ_1
z_{n} = z_{n-1} + Δt · ( y_n      − z_{n-1} ) / τ_2
τ_muscle,n = a_n ( k_a θ_n + g_dSA z_n )
```

### 5.3 Time constants that matter — and their honest uncertainty

- **Wingbeat period T = 1/f = 5.0 ms** at 200 Hz [M]. Every muscle time constant must be a
  fraction of this.
- **dSA tension-rise time.** For stable oscillation the delayed-tension lag must be
  **smaller than ~T/2**, i.e. **≲2.5 ms**, and physically the dominant lag should be of
  order **0.5–2 ms** [E]. The *ratio* is constrained by the physiology: the rate of
  stretch-activation tension generation in Drosophila IFM is **~9× faster than in
  Lethocerus** (which beats at ~25–30 Hz), the fast kinetics being the cost of the higher
  beat frequency.
  > Glasheen, B.M., et al. (2017). *Stretch activation properties of Drosophila and
  > Lethocerus indirect flight muscle suggest similar calcium-dependent mechanisms.*
  > Am. J. Physiol. Cell Physiol. 313, C621–C631. doi:10.1152/ajpcell.00110.2017
- **Molecular strain-response latency** is sub-millisecond:
  > Iwamoto, H. et al. (2017). *The earliest molecular response to stretch of insect flight
  > muscle.* (fast X-ray diffraction evidence for a ~sub-ms structural response.)
- **Calcium activation time constant.** Myoplasmic Ca²⁺ varies ~2-fold during flight and
  correlates with aerodynamic power and wingbeat frequency — so calcium is a *modulator* of
  power, not the beat clock:
  > *Calcium and stretch activation modulate power generation in Drosophila flight muscle.*
  > Biophys. J. (2011), doi:10.1016/j.bpj.2011.06.001.

**Honest statement:** the *individual* dSA time constants `τ_1, τ_2` for Drosophila IFM are
**not** a single agreed published pair in the sources reviewed here. What *is* well
established: (i) dSA exists and is essential, (ii) it is ~9× faster in Drosophila than in
Lethocerus, (iii) the emergent frequency must match `f ≈ 200 Hz`. Recommendation: choose
`τ_1 ≈ τ_2 ≈ 0.5 ms` [E] and then **tune to resonance**: verify the free-running
spring-wing oscillator settles at 200 Hz, and that changing the power-muscle activation
`a(t)` changes *amplitude* while leaving *frequency* nearly constant (this decoupling is
the defining signature of asynchronous flight and a strong test of the implementation).

---

## 6. Recommended concrete parameter set (SI)

Air at 20 °C, sea level: `ρ = 1.225 kg/m³`, `ν = 1.5×10⁻⁵ m²/s` [M].

### 6.1 Body

| Symbol | Value | Unit | Tag | Note |
|---|---|---|---|---|
| m_body | 9.83×10⁻⁷ | kg | [M] | flybody, 52 flies |
| wing mass m_w | 8.0×10⁻⁹ | kg | [M] | flybody |
| gravity g | 9.81 | m/s² | [M] | |
| I_body (roll/pitch/yaw) | ~1×10⁻¹³–3×10⁻¹³ | kg·m² | [E] | not pinned here; take from flybody MuJoCo XML (it has a measured inertia tensor). Do **not** keep the current point-mass attitude. |
| max airspeed | 0.5–1.0 | m/s | [M] | free-flight Drosophila cruise ~0.2–0.5 m/s; escape >1 m/s |

### 6.2 Wing

| Symbol | Value | Unit | Tag |
|---|---|---|---|
| R (wing length) | 2.4×10⁻³ | m | [M] (range 2.4–2.9) |
| S (single wing area) | 1.8×10⁻⁶ | m² | [M] (range 1.5–2.6) |
| c̄ (mean chord) | 0.75×10⁻³ | m | [D] |
| r₂ (2nd-moment radius) | 0.58 R | m | [M] |
| m_w | 8.0×10⁻⁹ | kg | [M] |
| I_w | 1.54×10⁻¹⁴ | kg·m² | [D] |
| I_added | 2.5×10⁻¹⁵ | kg·m² | [D] |
| f (wingbeat) | 200 | Hz | [M] (range 180–230) |
| Φ (stroke amplitude) | 2.79 (160°) | rad | [M] |
| α_mid | 0.65 (37°) | rad | [M] |
| α_flip sweep | ±0.8 | rad | [E] |

### 6.3 Aerodynamic constants

| Coefficient | Value | Tag |
|---|---|---|
| C_L,t(α) | 0.225 + 1.58 sin(2.13α_deg − 7.2) | [M] (Sane & Dickinson 2002) |
| C_D,t(α) | 1.92 − 1.55 cos(2.13α_deg − 7.2) | [M] |
| C_rot | 1.5 ± 0.5 | [M]/[E] |
| added mass / span | m_a(r) = (π/4) ρ c(r)² | [M] (classical) |
| ρ | 1.225 kg/m³ | [M] |
| ν | 1.5×10⁻⁵ m²/s | [M] |

**Mandatory self-check (uses measured quantities only).** At α = 45°, mid-stroke, Φ=160°,
f=200 Hz, using the blade-element integral `F = ½ ρ C_L ω_rms² S R²/3`:

```
ω_rms = π f Φ / √2                      = 1.24×10³ rad/s
S R²/3 = ∫ r² c dr                      = 3.46×10⁻¹² m⁴
F_per_wing = ½·1.225·1.80·(1.24×10³)²·… = 5.9×10⁻⁶ N        [D]
weight/2                                = 4.8×10⁻⁶ N        [M]
→ F/(W/2) = 1.22
```

A hovering configuration must give `F_lift/weight ≈ 1`–`1.3`. If the implemented model
does not reproduce this within a few percent at α ≈ 40–45°, a coefficient convention is
wrong (most likely a factor of ½, or `r₂` vs `R` confusion). This check requires **no
tuning** — it is a physical consistency test.

### 6.4 Haltere (all [E] except where noted)

| Symbol | Value | Note |
|---|---|---|
| L_h | 3.5×10⁻⁴ m | from haltere morphology; verify |
| m_h | 5×10⁻¹⁰ kg | order-of-magnitude; derive from volume |
| Θ_h | ~0.7 rad (40°) | not established |
| f_h | 200 Hz (= f) | [M] anti-phase, Pringle 1948 |
| τ_h (afferent LPF) | 1×10⁻³ s | [E] from ≥150 Hz bandwidth |
| baseline afferent rate | ~f_wing (1 spike/cycle) | [M]-informed |
| sensilla count/haltere | "hundreds" | [M] J. Exp. Biol. jeb250431 |

### 6.5 Thorax / flight muscle (all [E] — engineering choices constrained by [M])

| Symbol | Value | Note |
|---|---|---|
| wingbeat period T | 5.0 ms | [M] |
| dSA time constants τ_1, τ_2 | 0.5 ms each | [E]; tune so free-run = 200 Hz |
| thoracic stiffness k | chosen so `√(k/I) /(2π) ≈ f` | [E], resonance condition |
| damping Γ | small; from quadratic aero drag §1.3 | [E] |
| muscle activation gain k_a | scale to give F/W≈1.2 at a=1 | [D] from §6.3 |
| dSA gain g_dSA | ~k_a | [E] |

---

## 7. Implementation notes for flyverse

1. **Integrate in SI.** Store body state in SI inside the physics core; convert to mm only
   in the JSON/SSE frame (`sim.rs`) and the three.js client.
2. **6-DOF rigid body.** Replace the scalar `yaw/pitch/roll` scalar integration with a real
   rigid-body state (quaternion + angular velocity + inertia tensor `I_body`). Wing forces
   become body-frame forces **and torques** about the centre of mass. This is what allows
   attitude to emerge. `body.rs` currently computes neither a wing force direction nor any
   torque.
3. **Per-wing state.** Two independent wings, each with stroke angle `φ`, AoA `α`, and
   stroke-plane orientation. `motor_flight_power_l/r` → common amplitude `Φ`; their
   difference → roll torque and yaw via stroke-plane tilt. `motor_flight_steering_l/r`
   → asymmetric stroke-plane tilt / flip timing → yaw torque.
4. **Wingbeat oscillator runs at the biological 200 Hz**, decoupled from the 2 ms control
   window. This is a 200 Hz oscillator integrated with, say, 20–50 µs sub-steps inside each
   2 ms window, *not* aliased to a 19 Hz display phase as today. The display can still be
   slowed for visibility.
5. **Motor neurons → physical parameters, not forces.** `motor_flight_power_*` must set
   muscle activation `a(t)` (§5.2) or stroke amplitude; the resulting force is computed by
   the physics. No `THRUST_MAX`.
6. **Haltere → sensory drive.** Add a haltere group (or per-side mechanosensory channel)
   computed from `Ω_body` per §4.3, and inject it as Poisson spikes into the appropriate
   ascending/descending neurons, so the pre-existing reflex circuitry has something
   physical to close the loop around.
7. **Numeric care.** A 200 Hz oscillator with 0.5 ms muscle time constants integrated at
   2 ms is unstable. Sub-step the oscillator; use semi-implicit integration for the stiff
   spring term.
8. **Keep the honesty boundary explicit in the README.** The connectome is real; the
   body/thorax/haltere are added physical elements, now *cited and parameterised* rather
   than hand-tuned.

---

## 8. Uncertainty register (things that are *not* pinned)

1. **Stroke-plane angle β for free-flying Drosophila** — no single agreed value; reports
   span ~40–80° depending on flight mode and body pitch and on the definition used. Treat
   as a free parameter / steering DOF.
2. **AoA time course (the flip waveform)** — the *midstroke* angle (~37–45°) is measured;
   the exact sweep during pronation/supination is not a single published scalar.
3. **C_rot** — theory gives `π(0.75 − x̂)` (≈1.57 for the Drosophila root axis); measured
   value varied with angular velocity in Sane & Dickinson (2002). Use `1.5 ± 0.5`.
4. **dSA time constants for Drosophila IFM** — the mechanism and the ~9×-faster-than-
   Lethocerus scaling are established; a specific `(τ_1, τ_2)` pair is not. Use 0.5 ms and
   verify by the frequency-decoupling test (§5.3).
5. **Haltere length, mass, stroke amplitude, sensilla count, afferent gain β for
   Drosophila** — order-of-magnitude only here. The *form* of the model (Coriolis ∝ Ω,
   centrifugal ∝ Ω²) is established; the gains are not.
6. **Body inertia tensor** — not reproduced here; take measured values from the flybody
   model rather than estimating.
7. **Wake-capture force** — omitted; no simple defensible closed form.
8. **Coefficient fit provenance** — the C_L/C_D fits are from a single dynamically-scaled
   polymer wing model at Re ≈ 115 (Sane & Dickinson 2002); they are the accepted standard
   but are a physical model, not the live fly. Alternative CFD-based coefficients exist
   (Sun & Tang 2002 for D. virilis) and differ by ~10–20 %; do not mix the two.

---

## References

- Altshuler, D.L., Dickson, W.B., Vance, J.T., Roberts, S.P., Dickinson, M.H. (2005). Short-amplitude high-frequency wing strokes determine the aerodynamics of honeybee flight. *PNAS* 102(50), 18213–18218.
- Birch, J.M., Dickinson, M.H. (2001). Spanwise flow and the attachment of the leading-edge vortex on insect wings. *Nature* 412, 729–733.
- Deora, T., Singh, A.K., Sane, S.P. (2015). Biomechanical basis of wing and haltere coordination in flies. *PNAS* 112(5), 1481–1486. doi:10.1073/pnas.1412279112
- Dickinson, M.H., Lehmann, F.-O., Götz, K.G. (1993). The active control of wing rotation by Drosophila. *J. Exp. Biol.* 182, 173–189.
- Dickinson, M.H., Götz, K.G. (1993). Unsteady aerodynamic performance of model wings at low Reynolds numbers. *J. Exp. Biol.* 174, 45–64.
- Dickinson, M.H., Lehmann, F.-O., Sane, S.P. (1999). Wing rotation and the aerodynamic basis of insect flight. *Science* 284, 1954–1960. doi:10.1126/science.284.5422.1954
- Dickinson, M.H. (1999). Haltere-mediated equilibrium reflexes of the fruit fly, Drosophila melanogaster. *Phil. Trans. R. Soc. Lond. B* 354, 903–916. doi:10.1098/rstb.1999.0442
- Dickinson, M.H., Muijres, F.T. (2016). The aerodynamics and control of free flight manoeuvres in Drosophila. *Phil. Trans. R. Soc. B* 371, 20150388.
- Dickson, W.B., Straw, A.D., Dickinson, M.H. (2008). Integrative model of Drosophila flight. *AIAA Journal* 46(9), 2150–2164.
- Ellington, C.P. (1984). The aerodynamics of hovering insect flight. I–VI. *Phil. Trans. R. Soc. Lond. B* 305, 1–181 (esp. II: morphological parameters, 17–40; III: kinematics, 41–78; IV: aerodynamic mechanisms, 79–113).
- Ellington, C.P., van den Berg, C., Willmott, A.P., Thomas, A.L.R. (1996). Leading-edge vortices in insect flight. *Nature* 384, 626–630.
- Fox, J.L., Fairhall, A.L., Daniel, T.L. (2010). Encoding properties of haltere neurons enable motion feature detection in a biological gyroscope. *PNAS* 107(8), 3840–3845. doi:10.1073/pnas.0912548107
- Fox, J.L., Daniel, T.L. (2008). A neural basis for gyroscopic force measurement in the halteres of Holorusia. *J. Comp. Physiol. A* 194, 887–897.
- Fry, S.N., Sayaman, R., Dickinson, M.H. (2003). The aerodynamics of free-flight maneuvers in Drosophila. *Science* 300, 495–498.
- Fry, S.N., Sayaman, R., Dickinson, M.H. (2005). The aerodynamics of hovering flight in Drosophila. *J. Exp. Biol.* 208, 2303–2318. doi:10.1242/jeb.01612
- Glasheen, B.M., et al. (2017). Stretch activation properties of Drosophila and Lethocerus indirect flight muscle suggest similar calcium-dependent mechanisms. *Am. J. Physiol. Cell Physiol.* 313, C621–C631. doi:10.1152/ajpcell.00110.2017
- Iwamoto, H., et al. (2017). The earliest molecular response to stretch of insect flight muscle (fast X-ray diffraction). (PMC5296744)
- Lynch, J., Gau, J., Sponberg, S., Gravish, N. (2021). Emergent wingstroke in asynchronous insects and robots is governed by time-delayed strain rate feedback.
- Machin, K.E., Pringle, J.W.S. (1959). The physiology of insect fibrillar muscle. II. *Proc. R. Soc. Lond. B* 151, 204–225.
- Mohren, T.L., Daniel, T.L., Eberle, A.L., Reinhall, P.G., Fox, J.L. (2019). Coriolis and centrifugal forces drive haltere deformations and influence spike timing. *J. R. Soc. Interface* 16, 20190035. doi:10.1098/rsif.2019.0035
- Nalbach, G. (1993). The halteres of the blowfly Calliphora. I. Kinematics and dynamics. *J. Comp. Physiol. A* 173, 293–300.
- Pringle, J.W.S. (1948). The gyroscopic mechanism of the halteres of Diptera. *Phil. Trans. R. Soc. Lond. B* 233, 347–384.
- Pringle, J.W.S. (1949). The excitation and contraction of the flight muscles of insects. *J. Physiol.* 108, 226–232.
- Sane, S.P. (2003). The aerodynamics of insect flight. *J. Exp. Biol.* 206, 4191–4208.
- Sane, S.P., Dickinson, M.H. (2002). The aerodynamic effects of wing rotation and a revised quasi-steady model of flapping flight. *J. Exp. Biol.* 205, 1087–1096. doi:10.1242/jeb.205.8.1087
- Sun, M., Tang, J. (2002). Lift and power requirements of hovering flight in Drosophila virilis. *J. Exp. Biol.* 205, 2413–2427.
- Vaxenburg, R., et al. (2025). Whole-body physics simulation of fruit fly locomotion. *Nature*, doi:10.1038/s41586-025-09029-4 (bioRxiv 2024.03.11.584515).
- *Representation of haltere oscillations and integration with visual inputs in the fly central complex.* J. Neurosci. 39(21), 4100–4112 (2019).
- *A neural connectivity atlas for fly flight control.* Current Biology (2025).
- *The role of haltere campaniform sensilla in equilibrium reflexes of the fruit fly, Drosophila melanogaster.* J. Exp. Biol. 229, jeb250431.
- *Calcium and stretch activation modulate power generation in Drosophila flight muscle.* Biophys. J. (2011). doi:10.1016/j.bpj.2011.06.001
