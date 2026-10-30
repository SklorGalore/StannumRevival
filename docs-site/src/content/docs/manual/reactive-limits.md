---
title: Generator reactive limits
description: How PV buses switch to PQ at generator Q limits, reference-bus reassignment, and infeasibility.
---

With **Enforce generator Q limits** on, a generator that cannot hold its
voltage within its reactive range is fixed at the limit, and its bus stops
holding voltage. This follows MATPOWER's `ENFORCE_Q_LIMS` option, with the
refinements below.

## Allocation

After each Newton solve, a bus's reactive injection is shared among its online
generators in proportion to their reactive ranges, as MATPOWER does.
Zero-width ranges get exactly their limit; infinite ranges share what is left.

## Switching

After a converged pass, every generator is checked against its limits:

$$
Q_g > Q_{g,\max} + \delta \quad\text{or}\quad Q_g < Q_{g,\min} - \delta,
\qquad \delta = 5\times 10^{-6}\ \text{Mvar}.
$$

All current violations are fixed at once: the generator's output is set to the
violated limit, its bus becomes PQ, and another pass runs. During [loose
passes](/StannumRevival/manual/ac-power-flow/#loose-passes) the tolerance
$\delta$ grows by the remaining mismatch (in Mvar), so that only clear
violations switch while the solution is still rough.

### Returning to voltage control

Fixing every violation in one pass can overshoot: once many generators stop
holding voltage, the voltages around them fall or rise, and some of the
generators already fixed find themselves with reactive power to spare. A
generator bus at a limit therefore holds its voltage again when that voltage
moves past the setpoint on the side the limit pushes, as in PSS/E:

$$
\text{at } Q_{\max}: \lvert V\rvert > V^\text{set} + 10^{-4},
\qquad
\text{at } Q_{\min}: \lvert V\rvert < V^\text{set} - 10^{-4}.
$$

A bus can return at most three times in one solve, so the limit loop cannot
cycle. A generator holding a remote bus does the same with [the remote bus's
voltage](/StannumRevival/manual/remote-regulation/#returning-to-voltage-control).
Generators that start at a limit because the solve started from the last
solution stay there.

## Reference bus

If the reference bus reaches a limit, it becomes PQ like any other, keeping
the active power it had in the last full-tolerance solve, and the reference
moves to another voltage-controlled bus in the same island. The log says which
bus takes over.

## Infeasible limits

An island's reactive limits are **infeasible** when no generator is left that
can hold voltage: every remaining generator violates the same side of its
range, or no bus in the island is left that could become the reference. The
solve returns the status *Reactive limits infeasible* with the state from a
full-tolerance solve, so you can see where the voltage collapsed.

## In the results

The overview counts reactive-limit passes, and the generator table flags units
at or beyond a limit. Generator Q limits also appear in the operating-limit
checks, whether or not they were enforced.
