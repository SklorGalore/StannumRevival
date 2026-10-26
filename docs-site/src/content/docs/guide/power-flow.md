---
title: Power flow
description: Solve settings, the overview and its checks, result tables, filters, DC results, sensitivities and export.
---

![Power flow overview of a 300-bus case: convergence, totals, operating-limit checks, voltage profile and largest flows](../../../assets/screens/overview-charts.png)

## Solve settings

The settings panel on the left (hide it with the button at its top right)
holds:

| Setting | Meaning | Default |
| --- | --- | --- |
| **Model** | **AC** Newton–Raphson, or **DC** linear flow | AC |
| **Mismatch tolerance (MVA)** | Largest power mismatch allowed at any bus, in MW or Mvar. Hover it to see the value in per unit on the case's base. | 1e-6 on a 100 MVA base (1e-8 p.u.) |
| **Newton iterations per pass** | Newton steps allowed in each pass | 10 |
| **Flat start** | Start from 1 p.u. and reference angles. When it is off, each solve starts where the last one ended, with shunts, taps and generators at Q limits where it left them, and the first solve starts from the voltages saved in the file | Off |
| **Enforce generator Q limits** | Switch generators to fixed Q at their limits, and turn on remote voltage regulation | Off |
| **Switch shunts** | Let switched shunts in voltage control move (RAW only) | On |
| **Adjust transformer taps** | Let tap changers in voltage control move (RAW only) | On |
| **DC line model** | **Detailed converters** or the simplified generator pair | Detailed |
| **Adjust DC converter taps** | Let converter taps step to hold scheduled DC voltage and firing angles | On |

**Solve** runs the power flow. You can also solve from the palette or the
diagram toolbar. The [technical manual](/StannumRevival/manual/ac-power-flow/)
explains what each setting does to the solve.

A nonconverged result is kept and labelled, so you can inspect the final
iterate and see where the mismatch is.

## Overview

The overview shows convergence, the residual (MVA and p.u.), iterations, how
many shunts and taps moved, and totals for generation, load and losses. Below
it:

- **Operating limits**: voltage limits, generator reactive limits and branch
  loading against rate A. Each check is a link that opens the matching rows,
  worst first.
- **Voltage profile** and **Most loaded branches** charts.

![Checks leading to the voltage violations, worst first](../../../assets/screens/findings-violations.png)

## Tables

**Buses**, **Generators** and **Branches** list every element with its solved
values. Tables are searchable and sortable, show names, and export to CSV.
Cells carry small bars and colours for voltage against its limits and branch
loading, and rows outside limits are highlighted. The checkbox above each
table (for example **Voltage violations only**) narrows it to the problems.

Selecting a row puts the element in the selection chip and opens its details,
with **Draw around** and the [study](/StannumRevival/guide/studies/) actions.

![Branch table with names and loading bars](../../../assets/screens/tables-branches.png)

### Filters

**Filters** above the tables narrows buses, generators and branches together
by field rules, such as base kV = 230 or area = 2. Numbers compare with =, ≠,
>, ≥, <, ≤ or *between*; text with *contains*, *is* or *is not*; any field can
be *missing* or *present*. A branch passes when it touches a bus that passes.

Filters also narrow the overview charts. The operating-limit checks always
count the whole case.

![Filter chips narrowing the tables to 230 kV](../../../assets/screens/filter-chips.png)

## DC results

When the case has DC lines, two more tabs appear:

- **DC lines**: each two-terminal LCC or VSC line with its control mode, DC
  current and voltages, firing angles and taps, losses, and both ends' P and Q.
- **DC network**: each multi-terminal system with its DC bus voltages, DC link
  currents, flows and losses, and each converter's operating point.

![DC lines tab for a case with LCC and VSC lines](../../../assets/screens/power-dc-lines.png)

The models behind these numbers are in [HVDC](/StannumRevival/manual/hvdc/overview/).

## Sensitivities

**Calculate sensitivities** linearizes the solved case. Pick an injection bus
and P or Q, then a response: voltage angle, voltage magnitude, or one of the
four branch-end flows. Results are in engineering or raw units, ordered by
magnitude, signed value or input order. **How to read these results**
explains the sign conventions, and the [technical
manual](/StannumRevival/manual/sensitivities/) gives the mathematics.

![Sensitivities of branch flows to an injection](../../../assets/screens/power-sensitivities.png)

## Import warnings

For RAW files, **Import warnings** lists everything the conversion
approximated or skipped, with its line number.

## Export MATPOWER

**Input case…** writes the case as loaded (with study edits applied);
**Solved case…** writes the latest solved case. Both are MATPOWER version 2
files that MATPOWER's `loadcase` reads.
