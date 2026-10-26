---
title: AC Newton power flow
description: The polar Newton–Raphson formulation, convergence, step limiting, and the sequence of passes that wraps it.
---

The AC power flow solves the full nonlinear bus power balance with a sparse
Newton–Raphson method in polar coordinates. It follows MATPOWER 8.1's legacy
AC power/polar Newton solver, extended with the controls described in the
following pages.

## Equations

For every bus $i$, the scheduled injection is generation less load,
$S_i^\text{sch} = \sum_g S_g - S_{d,i}$, in per unit. The calculated injection
from the [network model](/StannumRevival/manual/network-model/) is

$$
S_i^\text{calc} = V_i \sum_k \overline{Y_{ik} V_k},
\qquad V_i = \lvert V_i\rvert e^{j\theta_i}.
$$

The mismatch $\Delta S_i = S_i^\text{calc} - S_i^\text{sch}$ must vanish.
The unknowns and equations are:

- an angle $\theta_i$ and an active-power equation $\Delta P_i = 0$ for every
  non-reference bus;
- a magnitude $\lvert V_i\rvert$ and a reactive-power equation
  $\Delta Q_i = 0$ for every PQ bus.

Reference buses supply whatever $P$ and $Q$ balances their island; PV buses
supply whatever $Q$ holds their voltage.

## Newton iteration

With state $x = [\theta_{pv,pq};\ \lvert V\rvert_{pq}]$ and mismatch
$F(x) = [\Delta P_{pv,pq};\ \Delta Q_{pq}]$, each iteration solves

$$
J(x^{(k)})\,\Delta x = -F(x^{(k)}),
\qquad
J =
\begin{bmatrix}
\dfrac{\partial \Delta P}{\partial \theta} & \dfrac{\partial \Delta P}{\partial \lvert V\rvert} \\[2ex]
\dfrac{\partial \Delta Q}{\partial \theta} & \dfrac{\partial \Delta Q}{\partial \lvert V\rvert}
\end{bmatrix}
$$

and updates $x^{(k+1)} = x^{(k)} + \Delta x$. The Jacobian is assembled
analytically from sparse admittance rows:

$$
\frac{\partial S}{\partial \theta} = j\,\mathrm{diag}(V)\,\overline{\mathrm{diag}(I) - Y\,\mathrm{diag}(V)},
\qquad
\frac{\partial S}{\partial \lvert V\rvert} = \mathrm{diag}(V)\,\overline{Y\,\mathrm{diag}(V_n)} + \overline{\mathrm{diag}(I)}\,\mathrm{diag}(V_n),
$$

with $I = YV$ and $V_n = V/\lvert V\rvert$. The linear system is factored with
a sparse LU (from the `faer` crate) at every iteration.

### Convergence

A solve converges when the infinity norm of the mismatch is below the
tolerance:

$$
\lVert F(x) \rVert_\infty < \varepsilon.
$$

The interface sets the tolerance in MVA and converts it on the case's base.
Its default is 2 MVA, which is $0.02$ p.u. on 100 MVA. (PSS/E's default is
0.1 MVA.) The solver library on its own defaults to $10^{-8}$ p.u., which
the reference checks use. The reported residual is this norm for the equations of the final
solve, shown in both MVA and per unit.

### Step limiting

A Newton step that would change any voltage magnitude by more than half of
its present value is shortened so that none does:

$$
\alpha = \min\!\left(1,\ \min_i \frac{0.5\,\lvert V_i\rvert}{\lvert\Delta \lvert V_i\rvert\rvert}\right),
\qquad x^{(k+1)} = x^{(k)} + \alpha\,\Delta x .
$$

Ordinary steps are far smaller and keep the full Newton step; the limit
protects badly initialized solves from jumping to a low-voltage solution.

### Starting point

**From case** starts at the voltages saved in the file. **Flat** starts every
bus at 1 p.u. and its island's reference angle. Either way, buses held by
generators start at the generators' voltage setpoints.

## Passes

Controls that change the problem's structure (a generator reaching a reactive
limit, a shunt switching, a tap moving) act **between** Newton solves. Each
Newton solve is a **pass**, and the [log](/StannumRevival/guide/log/) names
every pass and why it ran. The iteration limit applies to each pass; the
iteration count reported is the total.

After each converged Newton solve, the solver checks, in this order:

1. **DC converter taps** (detailed HVDC with tap adjustment): if any
   converter tap moved, solve again. At most 20 tap passes.
2. **VSC converter limits**: if a converter exceeds its MVA or current limit,
   cut its output and solve again.
3. **Generator reactive limits** (when enforced): fix violating generators at
   their limits and solve again. See [Generator reactive
   limits](/StannumRevival/manual/reactive-limits/).
4. **Remote regulation restores**: a generator group that left voltage
   control returns to it when its remote bus's voltage moves past the setpoint
   the other way.
5. **Switched shunts and transformer taps**, only once reactive limits have
   settled: move them and solve again. At most 30 control passes. See
   [Switched shunts and taps](/StannumRevival/manual/voltage-controls/).

When nothing moves, the solve ends.

### Loose passes

While reactive limits may still switch generators, passes stop at a loose
tolerance of $0.01$ p.u. (1 MVA on 100 MVA) instead of the full tolerance, and
only violations larger than the remaining mismatch count. Once nothing
switches, the solve finishes at full tolerance and checks the limits again,
so the answer always meets the requested tolerance. A requested tolerance at
or above the loose one, such as the 2 MVA default, skips this stage: every
pass already stops there, and limits are checked at face value. A reference bus at its
limit is switched only after a full-tolerance solve, since it keeps the active
power it has at that moment, as in MATPOWER. On the 80,000-bus synthetic
cases this cuts Newton iterations by about 40%.

### Undo

If a Newton solve fails after a shunt and tap pass, the moves of that pass are
undone, the devices it moved are held where they were, and the solve
continues from the saved state.

## Outcomes

| Status | Meaning |
| --- | --- |
| **Converged** | All equations within tolerance and every control settled. |
| **Iteration limit** | A pass used its Newton iterations. The final iterate and its residual are kept and shown. |
| **Reactive limits infeasible** | An island has no generator left that can hold its voltage. |

Invalid input (for example an island without a reference bus), unsupported
configurations and numerical failures such as a singular Jacobian are
reported as errors rather than results.

## Not included

Optimal power flow, continuation power flow, voltage-dependent (ZIP) load
models in the solve, alternative AC formulations, radial methods and
three-phase models are outside the solver. See [What isn't
modelled](/StannumRevival/reference/limitations/).
