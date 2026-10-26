---
title: DC (linear) power flow
description: The linearized active-power flow, its assumptions, and how phase shifters and shunts enter it.
---

Choosing **DC** in the solve settings runs MATPOWER's linear DC power flow.
It is fast and always linear, at the cost of ignoring reactive power,
voltage magnitudes and losses.

## Assumptions

- All voltage magnitudes are 1 p.u.
- Branch resistance is negligible next to reactance, $r \ll x$.
- Angle differences across branches are small, $\sin(\theta_f - \theta_t) \approx \theta_f - \theta_t$.

## Equations

Each branch carries

$$
P_{ft} = \frac{1}{x\,\tau}\left(\theta_f - \theta_t - \theta_\text{shift}\right)
$$

on the system base, where $\tau$ is the tap ratio (1 for lines) and
$\theta_\text{shift}$ the phase shift. Collecting the branches gives

$$
B_\text{bus}\,\theta = P_\text{bus} - P_\text{bus,shift} - G_{sh},
$$

where $P_\text{bus,shift}$ is the injection equivalent of the phase shifters
and $G_{sh}$ the shunt conductances. The reference bus angle is fixed and the
remaining rows are solved directly with a sparse factorization. The reference
generator takes up the balance.

## Requirements and limits

- Every active branch needs a nonzero reactance.
- Reactive-limit enforcement, switched shunts, tap control and detailed HVDC
  don't apply; DC lines use the [simplified
  model](/StannumRevival/manual/hvdc/overview/#simplified-model).
- In DC mode, the active branch-flow [sensitivities](/StannumRevival/manual/sensitivities/)
  are power transfer distribution factors (PTDFs).
