---
title: Secure transfer limits
description: The Secure transfer tab in Power flow, which grows a transfer step by step with every contingency solved in AC at each step, finds the secure limit, and draws any recorded bus voltage or flow under every contingency.
---

The **Secure transfer** tab in Power flow answers "how far can this
transfer go with every contingency still secure?". The transfer grows step
by step. At each step the base case and every contingency in the
[contingency setup](/StannumRevival/guide/contingencies/) are solved in AC,
until a contingency shows a new problem or stops solving. Every step's
values of the recorded channels are kept, so afterwards you can draw any
recorded bus's PV curve, or a flow, under every contingency. The method is
in the [Secure transfer limits](/StannumRevival/manual/secure-transfer/)
manual page.

![Secure transfer on the IEEE 118-bus case: load at buses 44 to 48 grows; the secure limit is 150 MW, where the Sporn 69 unit outage stops solving, against a base nose of 465 MW. The chart shows bus 44's voltage for the base case and the seven most limiting contingencies, with the values at 205 MW](../../../assets/screens/secure-transfer.png)

## Running

Solve the case with the AC model first. Then:

- **Source** and **Sink**: an area, zone, owner or a list of buses each,
  and how each shares out its part (as for
  [PV curves](/StannumRevival/guide/pv-curves/)). **Every bus** as the
  source lets all generation pick up the sink's load.
- **Major step**: the step while nothing new turns up (5% of what the sink
  starts with if blank). **Minor step**: the step that pins down where a
  problem starts (a tenth of the major step). **Stop at**: the largest
  transfer.
- **Problems that stop it**: **Thermal** (new branch overloads, and
  monitored interfaces over their limit) and **Voltage** (new bus voltages
  outside their limits). A contingency, or the base case, that stops
  solving always counts.
- **Stop at the first problem**: on, the run stops there. Off, it pins the
  first problem down, then carries on with major steps to the nose of the
  base case and logs every problem it meets.
- **Contingencies**: the one-line summary is the contingency setup;
  **Contingency setup…** opens it, with its outages, monitors,
  subsystems, limits and solve options. Narrow the monitors on a big case:
  every contingency is solved at every step.
- **Also record**: the sink's bus voltages (or the voltage monitors'
  buses) are always recorded. Add any bus voltage or angle, branch MW,
  Mvar or loading, generator MW or Mvar, or monitored interface by its id
  (`line:101:102:1`, `generator:101:1`, or a bus number).

Then press **Find the secure limit**. The bar shows the step, its
transfer and how many contingencies have solved there; **Cancel** stops
the study and keeps the steps done.

## Results

- **Secure limit**: the last clean transfer before the first problem that
  counts, what kind of problem it was and the minor step it lies within.
- **Limiting contingency**: the contingency behind it, and the element.
- **Studied up to**: the largest transfer solved and why the run stopped,
  with the base case's nose when it got there.
- **Solves**: contingency solves, steps, the time and the size of the
  results file.

The chart draws one recorded **Channel** against the transfer: the base
case in black and the contingencies ticked in the list below, each ending
where it stopped solving. It starts with the most limiting ones. The
secure limit and the channel's limits (a bus's voltage limits, a branch's
100%) are marked. Hover for every curve's value at a step; click a name
in the legend to hide it. **CSV** saves the channel's curves.

The tabs under the chart:

- **Contingencies**: each one's own limit and what set it, the most
  limiting first. Tick **Plot** to draw it.
- **Problems**: every problem the first time it was seen, with the
  transfer where it appeared and the last clean one before it. Problems
  that don't stop the run (with Thermal or Voltage off) are listed too.
  Click one to show its element.
- **Steps**: each step, major or minor, with the lowest base case voltage
  and counts of contingencies that didn't solve or had problems.
- **Channels**: what was recorded; click one to draw it.

Problems already there with no transfer don't count unless they get worse.
The results clear when the case or the study changes.

## Also from

Run files (`run_secure_transfer`), Python (`Case.secure_transfer`) and the
[MCP server](/StannumRevival/guide/ai-assistants/) (`secure_transfer`) run
the same study; see the [manual](/StannumRevival/manual/secure-transfer/).
