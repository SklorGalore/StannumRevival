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

A generator fixed at a limit stays there in this solve, except a generator
holding a remote bus, which can [take the bus
back](/StannumRevival/manual/remote-regulation/#returning-to-voltage-control).

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
