---
title: Transfer limits (DFAX and FCITC)
description: How much power can move from a source subsystem to a sink before a monitored branch or interface reaches its limit, in the base case and after each contingency.
---

A transfer limit study asks how many MW can move from a **source** subsystem
to a **sink** subsystem before a monitored branch or interface reaches its
limit, in the base case or after any of a list of contingencies. The answer
is the **first change of total transfer capability** (FCITC). It starts from
the converged AC solution of the case with the study's edits, and adds a
linear (DC) transfer on top, as PSS/E's DFAX and TLTG do.

The study runs from [run files](/StannumRevival/guide/run-files/) and the
command registry (`run_transfer_limits`),
[Python](/StannumRevival/guide/python/#transfer-limits)
(`Case.transfer_limits`) and the
[MCP server](/StannumRevival/guide/ai-assistants/) (`transfer_limits`); the
app module follows.

## The transfer

The source and the sink are sets of buses, written like a
[lists file](/StannumRevival/reference/lists-file/) subsystem: areas, zones,
owners and buses combine as a union (every bus when all are empty), then
`kv_min` and `kv_max` narrow it. The **scaling** method shares the transfer
out over each side:

| Scaling | Source | Sink | Room to move |
| --- | --- | --- | --- |
| `generation` (default) | online generators raise output, in proportion to their MW now | online generators lower output, in proportion to their MW now | to Pmax (source) and Pmin (sink), summed |
| `reserve` | in proportion to the room to Pmax | in proportion to the room to Pmin | the same sums; every unit reaches its limit at the same transfer |
| `load` | loads drop, in proportion to their MW | loads grow | the source's total load; the sink is unbounded |

One MW of transfer injects $s_i$ at source bus $i$ and $-k_j$ at sink bus
$j$, each side's shares summing to one. The injection sums to zero, so the
result doesn't depend on the slack bus.

## Distribution factors

From the one factorization of the DC network used by
[contingency analysis](/StannumRevival/manual/contingencies/), the transfer's
**PTDF** on branch $m$ is the flow change per MW of transfer,
$d_m = \sum_i s_i\,\text{PTDF}_{mi} - \sum_j k_j\,\text{PTDF}_{mj}$.
After an outage of branch $k$, the **OTDF** is

$$
d_m^{(k)} = d_m + \text{LODF}_{mk}\, d_k ,
$$

found the same way as flows after an outage: outage factors are linear, so
tripping the PTDFs gives the OTDFs, with one sparse solve per contingency
for both the flows and the factors. Several branches out together use the
same $(I - T_{KK})$ system as contingency analysis. A generator outage moves
the base flows (its output is picked up at the reference bus) and leaves the
factors as they are. An interface's factor and flow are the weighted sums of
its branches'.

## FCITC

For a monitored element with flow $F$ before the transfer (the AC from-end
MW, moved by the outage when there is one), limit $L$ and factor $d$, the
transfer that brings it to its limit is

$$
T = \frac{L - F}{d} \;\; (d > 0), \qquad T = \frac{-L - F}{d} \;\; (d < 0).
$$

$L$ is the branch rating (rate A, B or C, chosen separately for the base
case and after an outage) times `limit_pct`, or an interface's `limit_mw`.
Elements whose factor is smaller than the **cutoff** in size (5% by default)
are left out, as are unrated branches. The FCITC is the smallest $T$ over
every monitored element in the base case and after every contingency.

An element already past its limit before any transfer, in the direction the
transfer pushes it, has a negative $T$. By default such elements are listed
apart (`past_limits`) and don't set the FCITC, which is then the transfer
that first takes an element within its limit past it. With
`include_past_limits` they count, and the FCITC can be negative, as PSS/E
reports it.

The **generation limit** is the smaller of the source's and the sink's room
to move; the **transfer limit** is the smaller of the FCITC and the
generation limit.

## What a study returns

- `summary`: the FCITC and the element and contingency that set it, the
  generation and transfer limits, each side's buses, units and room, and
  notes (contingencies left out because they split the network, elements
  already past their limit).
- `limits`: each monitored element's lowest $T$ over the base case and every
  contingency, lowest first.
- `past_limits`: elements already past their limit, each at its worst.
- `contingencies`: each contingency's lowest $T$, lowest first; the base case
  is `""`.
- `factors`: base case PTDFs at or above the cutoff, largest first.

Each list keeps at most `max_rows` rows (200 by default).

## Example

```text
# Far West (area 1) to Coast (area 7), every branch outage at 100 kV and up.
open_case path=Texas2k.RAW
solve_power_flow
run_transfer_limits settings={"source": {"areas": [1]}, "sink": {"areas": [7]}, "monitor": {"kv_min": 100}, "contingencies": {"kv_min": 100}}
```

| Setting | Default | |
| --- | --- | --- |
| `source`, `sink` | required | subsystems |
| `scaling` | `generation` | `generation`, `reserve` or `load` |
| `monitor.branches` | true | every rated branch in service |
| `monitor.subsystem`, `monitor.kv_min` | none, 0 | only branches with a terminal bus in it, or at or above it |
| `monitor.elements` | none | branches watched as well, by id |
| `monitor.interfaces` | none | `{name, elements: [[id, weight], ...], limit_mw}` |
| `contingencies` | none (base case only) | `{}` for every branch outage |
| `contingencies.elements` | every branch | ids, one at a time; `a&b` together |
| `contingencies.generators` | false | also every generator |
| `contingencies.kv_min` | 0 | only branch outages with a terminal bus at or above it |
| `contingencies.skip_zero_impedance` | false | as in contingency analysis |
| `cutoff` | 0.05 | smallest factor that limits the transfer |
| `rating`, `base_rating` | `a` | after an outage, and in the base case |
| `limit_pct` | 100 | percent of the rating that is the limit |
| `include_past_limits` | false | let elements already past their limit set the FCITC |
| `max_rows` | 200 | rows kept in each list |

`cancel_transfer_limits` stops a study between contingencies (keeping what
is done) and `transfer_progress` follows it.

## Limits of the method

- Linear: losses, reactive flow and voltage are left out of the transfer,
  and flows are compared in MW against MVA ratings.
- Contingencies that split the network are left out (and counted); outages
  of shunts and DC lines aren't studied.
- The generation limit sums each side's room; once a unit reaches its limit
  the real shares change, which the linear factors don't follow (the
  `reserve` method avoids this).
