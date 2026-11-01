---
title: QV curves (reactive margin)
description: How much reactive power a bus can give up before its voltage collapses, found by holding the bus at one voltage after another with full AC solves.
---

A QV curve asks how close a bus is to voltage collapse, in Mvar. A
fictitious synchronous condenser, with no active power and no reactive
limits, holds the bus at one voltage after another. Its reactive output at
each voltage, **Q**, is plotted against the voltage, **V**:

- At the voltage the last solve left the bus at, the condenser gives
  nothing.
- Higher voltages need it to inject.
- Lower voltages need it to absorb, down to the **bottom** of the curve.
  Below the bottom the network can't keep the bus up any more.

How far the bottom lies below zero is the bus's **reactive power margin**:
how much more reactive load the bus can take before its voltage collapses.
A bottom above zero (a negative margin) means the bus can't hold its
voltage without the condenser at all.

The study starts from the converged AC solution of the case with the
study's edits, and solves every voltage with the full AC power flow and the
same settings: every control (taps, switched shunts, phase shifters, DC
lines) and generator reactive limits act as in a normal solve.

It runs from the **QV curve** tab in
[Power flow](/StannumRevival/guide/qv-curves/),
[run files](/StannumRevival/guide/run-files/) and the command
registry (`run_qv_curve`), [Python](/StannumRevival/guide/python/#qv-curves)
(`Case.qv_curve`) and the
[MCP server](/StannumRevival/guide/ai-assistants/) (`qv_curve`).

## The condenser

Each bus in `buses` is swept on its own, from the same base solution. For
the sweep:

- the bus holds its voltage with the condenser (a generator with no MW and
  no reactive limits);
- generators on the bus keep the reactive output they had, so the condenser
  alone holds the voltage, and the curve is what the bus needs beyond what
  it already makes;
- generators elsewhere that held the bus's voltage hold their own terminal
  instead, at the voltage it had;
- switched shunts and transformer taps that held the bus stay where the
  last solve left them.

This is how PSS/E's QV analysis draws the curve. A reference (swing) bus
already holds its voltage, so it has no curve.

## The sweep

The sweep starts at the bus's solved voltage, goes up to `v_high_pu` and
then, from the solved voltage again, down towards `v_low_pu`, `step_pu`
apart. The steps land on multiples of `step_pu` (0.95, 0.94, ...). Each
solve starts from the one before. When a step down fails to converge it is
halved and tried again, down to `min_step_pu` (an 8th of the step by
default); the sweep then stops at the last voltage that converged, which
is usually well past the bottom.

Each solve uses full Newton. Generators the last step held at a reactive
limit start there, which saves finding the same limits again at every step.
When that leaves one on the side of its set point that should take it off
the limit, or doesn't converge, the step is solved again with every limit
looked at afresh. The mismatch tolerance is 0.1 MVA, or the solve
tolerance when that is tighter, so that the flat part near the bottom is
smooth.

The bottom of the curve is the point with the least reactive output.
When the curve is still falling at the lowest voltage reached, the bottom
is lower than that: the curve says so (`bottom_found` is false), and the
margin is at least the one given.

## What a study returns

- `summary`: the buses with a curve, the weakest bus (the smallest margin)
  and its margin, the voltages and steps used, solves made and failed, and
  notes (buses left out and why, curves without a bottom).
- `curves`, one per bus, in the order asked for:
  - `base_vm_pu`: the bus's solved voltage;
  - `v_pu` and `q_mvar`: the curve, lowest voltage first; `q_mvar` is
    positive when the condenser injects;
  - `q_limited`: generators elsewhere held at a reactive limit at each
    point, which explains kinks in the curve;
  - `q_min_mvar` and `v_at_q_min_pu`: the bottom;
  - `margin_mvar`: how far the bottom lies below zero;
  - `bottom_found`, `limited_by` (`collapse`, `v_low`, `max_points`,
    `cancelled` or `failed`), and the bus's solves.

## Example

```text
# Reactive margin at four 115 kV buses in Coast (area 7).
open_case path=Texas2k.RAW
solve_power_flow
run_qv_curve settings={"buses": [7314, 7340, 7050, 7239]}
```

| Setting | Default | |
| --- | --- | --- |
| `buses` | required | buses to draw a curve for, 100 at most |
| `v_high_pu` | 1.1 | the highest voltage set |
| `v_low_pu` | 0.3 | the lowest voltage tried |
| `step_pu` | 0.01 | the voltage step |
| `min_step_pu` | an 8th of `step_pu` | the smallest step tried on the way down |
| `max_points` | 300 | most solves for one bus, failed ones included |

`cancel_qv_curve` stops a study before its next solve (keeping the curves
so far) and `qv_progress` counts the buses done.

On the Texas 2000-bus case each bus takes about 5 seconds (81 solves).

## Verification

`crates/stannum-core/scripts/qv_reference.py` adds the same condenser to
the case in PYPOWER and solves each voltage there. The curves at three
load buses agree within 1e-7 Mvar, with generator reactive limits off; see
the [Verification](/StannumRevival/reference/verification/#qv-curves) page.

## Limits of the method

- The base case only for now: QV curves after contingencies come next.
- FACTS devices that hold the bus keep holding it, against the condenser.
- Attached VSC DC grid files are left out, as in contingency analysis.
