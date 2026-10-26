---
title: Sensitivities, PTDF and LODF
description: Linearizing the solved power flow, branch-flow derivatives, and DC PTDF and LODF matrices.
---

**Calculate sensitivities** solves the case and linearizes it at the
converged operating point. The result says how bus voltages and branch flows
respond to a small change in the active or reactive injection at a bus.

## State sensitivities

At the solution, the mismatch equations satisfy $F(x, u) = 0$, where $x$ is
the state (angles and magnitudes) and $u$ the bus injections. Since
$F = S^\text{calc}(x) - u$ in the constrained rows, a small injection
change $\Delta u$ moves the state by

$$
\Delta x = J^{-1}\,\Delta u ,
$$

where $J$ is the reduced power flow Jacobian of the
[Newton solve](/StannumRevival/manual/ac-power-flow/#newton-iteration). The
columns of $J^{-1}$ are the state sensitivities:

| Injection column | State rows |
| --- | --- |
| $P_i$ at every non-reference bus | $\theta_k$ at every non-reference bus |
| $Q_i$ at every PQ bus | $\lvert V_k\rvert$ at every PQ bus |

Reference-bus angles and injections, PV-bus magnitudes and reactive
injections, and isolated buses are left out, because they are fixed or
balancing. A non-reference injection is balanced by the reference bus of its
island. The calculation uses bus types **after** reactive-limit enforcement,
and the final switched shunt and tap settings, held fixed.

## Branch-flow sensitivities

Each branch's four end flows $P_f, Q_f, P_t, Q_t$ depend only on its two end
voltages. Their derivatives with respect to the injections follow by the chain
rule:

$$
\frac{\partial S_\text{br}}{\partial u} = \frac{\partial S_\text{br}}{\partial x}\,J^{-1},
$$

with $\partial S_\text{br}/\partial x$ from MATPOWER's `dSbr_dV` in polar
form. Because each branch row touches only its endpoint states, assembly
avoids dense work even on large cases.

## Units and signs

- Angles are in radians, magnitudes and injections in per unit, in the raw
  matrices. **Engineering units** in the interface show degrees per MW, p.u.
  per Mvar, and MW per MW.
- Divide an MW or Mvar perturbation by $S_\text{base}$ before applying the
  raw state matrix.
- Branch-flow derivatives are in p.u./p.u., numerically MW/MW or Mvar/Mvar.
- An injection is positive into the bus (more generation or less load).
  Branch flows are positive entering the branch at that end.

The **How to read these results** panel in the app restates these rules.

## Remote regulation

With Q limits and [remote
regulation](/StannumRevival/manual/remote-regulation/), the linearization
follows the solve: a remotely held bus keeps its magnitude, so it has a
reactive injection column but no magnitude state, and the generator buses
holding it have magnitude states instead. Each group with $n$ member buses
adds $n-1$ states beyond the injections, so there can be more state rows
than injection columns.

## DC mode: PTDF

In DC mode the Jacobian is the reduced $B_\text{bus}$ and the active
branch-flow sensitivities are **power transfer distribution factors**:

$$
\mathrm{PTDF} = B_f\,B_\text{bus,red}^{-1},
$$

the change in each branch's flow per MW injected at a bus and withdrawn at the
reference bus.

The library also builds a full branch-by-bus PTDF without solving a power
flow, with a choice of slack: each island's reference bus, a distributed slack
with bus weights, explicit transfer columns, or a different slack distribution
per column (MATPOWER's `makePTDF` options).

## LODF

**Line outage distribution factors** give the change in flow on branch $\ell$
per MW of pre-outage flow on branch $k$ when $k$ is taken out:

$$
\mathrm{LODF}_{\ell k} = \frac{\mathrm{PTDF}_{\ell,(f_k,t_k)}}{1 - \mathrm{PTDF}_{k,(f_k,t_k)}},
\qquad \mathrm{LODF}_{kk} = -1,
$$

where $\mathrm{PTDF}_{\ell,(f_k,t_k)}$ is the transfer from $k$'s from bus
to its to bus. Outages that would island the network (bridges, where the
denominator is zero) have no outage factor and are reported as `NaN`.

PTDF and LODF are available in the rs-power library today and will be exposed
in the interface later.
