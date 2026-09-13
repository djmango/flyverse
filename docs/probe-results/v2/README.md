# Rotation perturbation probe, v2 — corrected pack, real retina

Collected 2026-09-13 against the corrected connectome (24,559,135 edges, retina
restored) with the retinotopic retina driving the photoreceptors. Supersedes the
JSONs one directory up, which were collected on the blind pack.

`seed 7`, 60 trials per condition, warmup 10 s, interval 0.5 s, 200 ms response
window. Two probed axes (roll, yaw), 30 trials per axis, 15 per sign.

## Conditions

| file | amplitude | retina | visual drive |
|---|---|---|---|
| `on_amp0.json` | 0 | on | retina (null control) |
| `on_amp10.json` | 10 rad/s | on | retina |
| `on_amp25.json` | 25 rad/s | on | retina |
| `on_amp50.json` | 50 rad/s | on | retina |
| `off_amp0.json` | 0 | off (`FLYVERSE_NO_RETINA=1`) | optic-flow proxy (null control) |
| `off_amp25.json` | 25 rad/s | off | optic-flow proxy |

Each `.json` carries `retina_on`, `retina_columns`, `retina_photoreceptors` and
`optic_flow_proxy_delivered` (effective delivery, not the env flag), so a result
is interpretable without its log. The paired `.log` holds the header and the
steer-differential time course by lag.

## Steer differential (R−L): the reflex metric

| condition | axis | diff | t |
|---|---|---|---|
| on_amp0 | roll | +0.0113 | 0.91 |
| on_amp0 | yaw | +0.0022 | 0.16 |
| on_amp10 | yaw | −0.0060 | 0.44 |
| on_amp25 | yaw | −0.0053 | 0.47 |
| on_amp50 | yaw | −0.0261 | 1.68 |
| off_amp0 | roll | +0.0092 | 0.74 |
| off_amp0 | yaw | −0.0012 | 0.11 |
| off_amp25 | yaw | −0.0255 | 1.78 |

Baseline steer differential is about −0.09, so these are responses of 5-30 % of a
large standing bias.

## Result: no attributable reflex

No condition exceeds the noise floor. The null controls (amplitude 0) reach
`t = 2.32` on one channel, so `|t| ~ 2.3` is achievable with no perturbation at
all. The two largest steer-differential effects — `on_amp50` (−0.0261, t 1.68)
and `off_amp25` (−0.0255, t 1.78) — are the same size as each other, and one is
retina-on while the other is retina-off. There is no monotonic dose-response
(−0.0060, −0.0053, −0.0261 across amplitudes 10, 25, 50).

The steer-differential time course hints at a transient that grows from about
zero to −0.06 between 80 and 120 ms in `on_amp50`. The null control shows the
same shape at roughly a third the amplitude, which is why it is not attributed.
It also appears in `off_amp25`, where the retina is absent.

## Do not read the yaw-rate channel as a reflex

`yaw rate rad/s` is trivially significant: `t = 4.03` at amplitude 25 and
`t = 4.60` at amplitude 50, against `t = 0.30` in the null. That channel is
measuring the perturbation itself — the probe imposes ±25 or ±50 rad/s, so the
two sign groups must differ. It confirms the perturbation was applied. It is not
a response of the fly.

## Limits of this measurement

- A null at the 200 ms response window bounds what happens **within 200 ms**. A
  visual-to-motor loop through descending neurons could be slower than that, so
  this is not evidence that the retina has no effect at all.
- 30 trials per axis is enough to reach |t| ~ 2.3 by chance. Anything real but
  smaller than that is invisible here.
- The flies were airborne throughout: `lift / weight` averages 1.64.
