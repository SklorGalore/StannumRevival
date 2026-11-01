---
title: PV curves (voltage stability)
description: How far load in a subsystem, or a transfer between two subsystems, can grow before the network runs out of voltage support, found with full AC solves.
---

A PV curve asks how far a transfer can go before the network can't carry
it any more. Power **P** grows on the horizontal axis and bus voltages
**V** fall on the vertical one, until the curve turns back on itself at the
**nose**: the most the network can deliver, where voltage collapses. The
study also reports where a monitored bus first drops below its low voltage
limit, which usually comes well before the nose and is often the limit that
matters in planning.

The study starts from the converged AC solution of the case with the
study's edits, and solves every step with the full AC power flow and the
same settings: every control (taps, switched shunts, phase shifters, DC
lines) and generator reactive limits act as in a normal solve.

It runs from the **PV curve** tab in
[Power flow](/StannumRevival/guide/pv-curves/),
[run files](/StannumRevival/guide/run-files/) and the command
registry (`run_pv_curve`),
[Python](/StannumRevival/guide/python/#pv-curves) (`Case.pv_curve`) and the
[MCP server](/StannumRevival/guide/ai-assistants/) (`pv_curve`).

## The transfer

The **sink** and the **source** are sets of buses, written like a
[lists file](/StannumRevival/reference/lists-file/) subsystem, as in
[transfer limits](/StannumRevival/manual/transfer-limits/#the-transfer). Each
side has its own scaling method:

| Side | Default | Others |
| --- | --- | --- |
| Sink | `load`: every load in the sink grows by the same factor, at constant power factor | `generation`, `reserve`: generators lower their output |
| Source | `generation`: online generators raise output, in proportion to their MW now | `reserve`: in proportion to the room to Pmax; `load`: loads drop |

With the source left empty, every bus is in it, so a load increase in the
sink is picked up by all the generation in the case. The reference bus
takes the losses, which grow along the curve. Constant current and
constant impedance loads scale with the constant power part, so their MW
at the solved voltage can fall short of the step near the nose.

By default the curve also stops when one side runs out of room, as
transfer limits count it: the source's generators reach Pmax (summed), the
sink's reach Pmin, or a side's loads reach zero. Set
`respect_generation_limits` to false to let it go on to the nose.

## Finding the nose

The transfer grows by `step_mw` at a time (5% of what the sink starts with
by default). Each step moves the case from the base case by the whole
transfer and solves from the step before. When a step fails to converge,
or generator reactive limits leave an island with no bus to hold its
voltage, the step is halved and tried again, down to `min_step_mw` (a 32nd
of the first step by default). The last step that converged is the nose:
the true nose lies between it and it plus `resolution_mw`, the last step
that failed.

This is how PSS/E's PV analysis finds the nose. It keeps every control and
limit of a normal solve, but it only traces the upper half of the curve:
the lower half, past the nose, is not a state the system can operate at.

Each step solves with full Newton (at least 30 iterations a pass, as it
converges ever more slowly near the nose), whatever method the last solve
used. It starts from the step before's voltages and looks at every
generator reactive limit afresh, as a normal solve does: holding the units
the step before held at a limit can strand the solve short of the nose. The
mismatch tolerance is a 20th of the smallest step at the bus it moves most,
or the solve tolerance when that is tighter, so that a small step can't pass
without moving the case.

Switched shunts and taps step as the voltages fall, so a curve can rise a
little where one switches in.

With generator reactive limits on, the nose is often where the last
reactive reserve near the sink runs out rather than a smooth turn of the
curve: voltages can still look healthy one step before it. Each point
lists the generators held at a limit and the reserve left (`q_limited`,
`q_reserve_mvar`). The summary says what stopped the last step.

## What a study returns

- `summary`:
  - `nose_mw`: the largest transfer that converged;
  - `limited_by`: `collapse` (the nose), `generation` (a side ran out of
    room), `max_transfer`, `max_points` or `cancelled`;
  - `resolution_mw`: how far past `nose_mw` the nose can lie;
  - `voltage_limit_mw` and `voltage_limit_bus`: the transfer at which a
    monitored bus first falls below its low limit (linear between steps),
    and that bus;
  - `weakest_bus`: the bus whose voltage falls fastest over the last tenth
    of the curve, which is the bus nearest collapse;
  - each side's buses, units and room, solves made and failed, and notes.
- `points`: one row per converged step, the base case first: the transfer,
  the sink's load, the losses, the lowest monitored voltage and its bus,
  the generators held at a reactive limit and the reactive reserve left.
- `curves`: for each bus in `buses`, then the `max_curves` buses whose
  voltage falls furthest, the voltage at every point, the drop, the drop
  per 100 MW over the last tenth of the curve, and the transfer at which it crosses its
  low limit.

The low limit is `vmin_pu` for every bus, or each bus's own normal low
limit from the case (RAW `NVLO`, MATPOWER `VMIN`) when that is left out.
Buses already below their limit in the base case are counted and left out
of `voltage_limit_mw`.

## Example

```text
# Grow the load in Coast (area 7) until the nose, every generator picking up.
open_case path=Texas2k.RAW
solve_power_flow
run_pv_curve settings={"sink": {"areas": [7]}, "respect_generation_limits": false}
```

| Setting | Default | |
| --- | --- | --- |
| `sink` | required | a subsystem |
| `source` | every bus | a subsystem |
| `sink_scaling` | `load` | `load`, `generation` or `reserve` |
| `source_scaling` | `generation` | `generation`, `reserve` or `load` |
| `step_mw` | 5% of the sink's starting amount | the first step, MW |
| `min_step_mw` | a 32nd of `step_mw` | the smallest step tried |
| `max_transfer_mw` | none | stop here |
| `respect_generation_limits` | true | stop when a side runs out of room |
| `monitor` | every energized bus | a subsystem of buses to watch |
| `vmin_pu` | each bus's own | low voltage limit |
| `buses` | none | buses that always get a curve |
| `max_curves` | 10 | buses with the furthest falls that get a curve |
| `max_points` | 400 | most steps solved, failed ones included |

`cancel_pv_curve` stops a study before its next solve (keeping the curve so
far) and `pv_progress` counts the solves.

## Verification

An independent arclength continuation power flow, written from the
textbook method in `crates/stannum-core/scripts/pv_reference.py` on top of
PYPOWER, traces the same curves around the nose. Its nose and the sink
voltages from PYPOWER solves along the way are on the
[Verification](/StannumRevival/reference/verification/#pv-curves) page:
the noses agree within the step resolution (0.01 MW in the test).

## Limits of the method

- The base case only for now: PV curves after contingencies, QV curves and
  drawing the results on the one-line and the map come next.
- The curve stops at the nose and doesn't trace the lower half.
- Area interchange control, when the solve has it on, moves area slack
  generation against a transfer between areas.
- Attached VSC DC grid files are left out, as in contingency analysis.
