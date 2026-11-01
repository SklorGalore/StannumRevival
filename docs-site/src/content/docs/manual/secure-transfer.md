---
title: Secure transfer limits
description: How far a transfer can go with every contingency still secure, found by solving every contingency in AC at each step of the transfer.
---

A secure transfer limit is the largest transfer from a source to a sink at
which the base case and every contingency still solve, with no new branch
overload, interface over its limit or bus voltage outside its limits. It is
the AC, contingency-by-contingency version of
[transfer limits (DFAX)](/StannumRevival/manual/transfer-limits/), and the
contingency version of a [PV curve](/StannumRevival/manual/pv-curves/):
the transfer grows step by step, and at every step the base case is solved
and then every contingency is solved from it, as PSS/E's PV analysis with
contingencies and VSAT's transfer analysis do.

It runs from the **Secure transfer** tab in
[Power flow](/StannumRevival/guide/secure-transfer/), [run files](/StannumRevival/guide/run-files/) and the command
registry (`run_secure_transfer`),
[Python](/StannumRevival/guide/python/#secure-transfer-limits)
(`Case.secure_transfer`) and the
[MCP server](/StannumRevival/guide/ai-assistants/) (`secure_transfer`).

## The transfer

The source and sink are subsystems (areas, zones, owners, buses, kV), and
move as for PV curves: by default the sink's loads grow at constant power
factor and the source's generation picks it up in proportion to its output
(`sink_scaling` and `source_scaling` choose generation, reserve or load).
With `respect_generation_limits` on (the default) the transfer stops when
the source or sink runs out of room.

Each step of the base case is solved with the full AC power flow from the
step before, with every control and generator reactive limit of a normal
solve, as for PV curves.

## Steps and stops

The transfer grows by the **major step** (`major_step_mw`, 5% of what the
sink starts with by default). At each step every contingency is solved in
AC, starting from that step's base case solution. When a step finds a new
problem, the run goes back to the last clean step and walks forward by the
**minor step** (`minor_step_mw`, a tenth of the major step by default)
until the problem shows up again. The secure limit is the last clean
step, to within the minor step (`resolution_mw`).

What counts as a problem is chosen:

- `stop_on_thermal` (on): a new branch overload, or a monitored interface
  over its limit.
- `stop_on_voltage` (on): a new bus voltage outside its limits.
- A contingency, or the base case, that stops solving always counts.

With `stop_at_violation` on (the default) the run stops at the first
problem. With it off, the run pins down the first problem with the minor
step, goes back to the major step, and carries on to the nose of the base
case (pinned down with the minor step too) or the largest transfer, logging
every problem it meets on the way. A contingency that stops solving is left
out of the steps after it.

Problems already there with no transfer don't count: each is remembered at
the start, and counts again only if it gets worse by the run's `worse_by`
margins (1 percentage point of loading, 0.001 p.u. of voltage). A
contingency that already fails to solve with no transfer is left out, and
the notes say how many.

## Contingencies and monitors

`contingencies` takes the settings of a
[contingency run](/StannumRevival/manual/contingencies/): every branch and
generator by default, or a list of element ids, a lists file
(`lists`, `.stnl`) with its contingencies, and `monitors` to check only the
branches, buses and interfaces named there, each with its own limit. The
rating after an outage (`rating`) and for the base case (`base_rating`),
the limit percentage and the contingency solve settings come from there
too. Every contingency is solved in AC at every step; the linear screen
(`method`, `ac_count`) isn't used.

On a big case, narrow the monitors: with every bus watched, a voltage just
inside its limit anywhere in the system can set the limit. The contingency
setup in the app builds monitors from subsystems.

## Channels and the results file

Every step's values of the **channels** go to a results file as the run
goes, for the base case and every contingency:

- the voltage of each bus in `voltage_buses` (a subsystem), or of the
  voltage monitors' buses when the monitors name some, or else of the
  sink's buses: these give each monitored bus's PV curve under every
  contingency;
- anything else in `channels`, each a quantity of one element:

| `quantity` | `element` | Value |
| --- | --- | --- |
| `vm`, `va` | `bus:101` | Voltage, p.u.; angle, degrees |
| `mw`, `mvar` | a branch id, such as `line:1:4:1` | Flow at the from end |
| `pct` | a branch id | Loading, percent of the contingency rating |
| `pg`, `qg` | a generator id, such as `generator:3:1` | Output, MW and Mvar |
| `interface` | an interface's name in the monitors | Flow, MW |

A study of 5,000 contingencies, 40 steps and 500 channels holds 100
million values, so they never all sit in memory. The file (`output`, or a
new file in the temporary folder) is a `.stnr` results file: one Arrow IPC
stream, zstd compressed, with one row per step and contingency (`point`,
`transfer_mw`, `contingency_index`, `contingency`, `converged`, the counts
of problems, then one column per channel, empty where it didn't solve).
Its metadata holds the format (`stannum-secure-transfer 0.1`), the
channels and the settings. `secure_transfer_channel` reads one channel's
curves back, and any Arrow reader opens the whole file (pyarrow
`pa.ipc.open_stream`, polars `pl.read_ipc_stream`).

## What a study returns

- `summary`: the secure limit (`limit_mw`), what set it (`limited_by`:
  thermal, voltage, not_converged, base_collapse, or generation,
  max_transfer, max_points or cancelled when nothing did), the limiting
  contingency and element, the resolution, the last transfer studied and
  why the run stopped there, the base case's nose when reached, the steps
  used, counts (contingencies, already out, failing at the start, existing
  problems, solves, channels), the results file with its size and rows, and
  notes.
- `points`, one per step: the transfer, whether it was a minor step, the
  lowest recorded base case voltage, contingencies solved and not
  converged, how many have thermal and voltage problems, and new problems.
- `violations`: each problem the first time it was seen, in transfer
  order: its kind, the contingency, the element, the transfer where it was
  first seen and the last clean one before it, its value and limit, and
  whether it counts towards stopping.
- `contingencies`: the base case and each contingency with its own limit
  (the last clean transfer before its first problem that counts), the most
  limiting first.
- `channels`: what was recorded, with units and voltage limits, and each
  one's column in the file.

## Time

Every contingency is solved at every step. On the 2,000-bus Texas case,
5,344 branch contingencies take about 75 s a step on four cores, as long as
a full AC contingency run. The minor steps only run around a problem, so a
run that stops at the first problem takes a handful of major steps and
about ten minor ones.
