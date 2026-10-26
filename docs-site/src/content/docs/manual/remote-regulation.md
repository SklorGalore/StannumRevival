---
title: Remote voltage regulation
description: Generators that hold a remote bus (PSS/E IREG) and share its reactive power (RMPCT).
---

PSS/E lets a generator hold the voltage of another bus (`IREG`) and share that
duty with other plants by percentage (`RMPCT`). MATPOWER's importer ignores
this and holds every setpoint at the generator's own terminal. Stannum keeps
the remote control: it turns `remote_regulation` on when it imports RAW files.

Remote regulation is active only with **Enforce generator Q limits** on.
Without limits, nothing would stop a small unit from driving its terminal to
an extreme voltage to hold a distant bus, so every generator holds its own bus
as in MATPOWER.

## Formulation

A group of generator buses that hold one remote bus $r$ at setpoint
$V_r^\text{set}$ changes the Newton system:

- Bus $r$ loses its magnitude variable: $\lvert V_r\rvert = V_r^\text{set}$ is
  fixed, but its reactive equation stays.
- Each member bus $a$ gains a magnitude variable $\lvert V_a\rvert$, freeing
  its terminal voltage, and keeps no reactive equation of its own.
- To fix the split between members, every member $a$ after the first adds a
  sharing row with the first member $b$

$$
\frac{\Delta Q_a + q_a}{w_a} - \frac{\Delta Q_b + q_b}{w_b} = 0,
$$

where $\Delta Q$ is the bus's reactive mismatch, $q$ its scheduled generator
output, so each numerator is the reactive power the bus's generators must
supply, and $w$ is the sum of the `RMPCT` of the bus's generators (100% when
missing). Generators on the same bus share as before, by reactive range.

If the regulated bus has a generator of its own, that bus joins the group as
its first member and fixes the voltage itself, and the other members share
with it.

With $n$ members there are $n$ new magnitude variables against $1$ removed
magnitude and $n-1$ sharing rows, so the system stays square.

## Falling back to local control

A group holds its own buses instead when the regulated bus:

- is a reference bus,
- is in another island, or
- itself regulates a remote bus.

## At a limit

A member that reaches a reactive limit is fixed there and stops holding the
bus, like any generator at a limit. A generator whose output already sits at a
limit in the case starts at that limit as PQ, since solved cases often leave
small units pinned there.

### Returning to voltage control

A member at a limit takes its remote bus back when the bus's voltage moves
past the setpoint on the side the limit pushes:

$$
\text{at } Q_{\max}: \lvert V_r\rvert > V_r^\text{set} + 10^{-4},
\qquad
\text{at } Q_{\min}: \lvert V_r\rvert < V_r^\text{set} - 10^{-4}.
$$

Each group can return at most three times in one solve, so the limit loop
cannot cycle.

## Results

On the Texas A&M SyntheticUSA case (82,000 buses, 2,730 remote-regulating
generators), the reactive-limit solve diverged with local setpoints and
converges with remote regulation, landing within 0.001 p.u. on average of the
voltages saved in the case.

[Sensitivities](/StannumRevival/manual/sensitivities/#remote-regulation)
computed with Q limits on linearize this same formulation.
