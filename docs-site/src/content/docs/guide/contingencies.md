---
title: Contingencies
description: N-1 contingency analysis and NERC TPL-001 planning events in Power flow, ranked outages, and showing an outage on the diagram.
---

The **Contingencies** tab in Power flow answers "what if it trips?" for every
branch and generator at once. It takes each one out on its own, starting from
the current AC solution (with the study's edits), and ranks the outages by the
overloads and voltage problems they cause.

![Contingencies on the Texas 2k case: six summary figures, the ranked outages, and the worst one open beside them](../../../assets/screens/contingencies.png)

## Running

Solve the case with the AC model first. Then choose:

- **Take out**: branches, generators, buses, or any mix. A three-winding
  transformer goes out as one element. A bus goes out with every branch and
  DC line connected to it, so its loads, generators and shunts are cut off.
  Elements already out of service are skipped.
- **One at a time (N-1)** or **Two at a time (N-2)**: N-2 takes every pair
  of the chosen branches and generators once (buses stay one at a time).
  The count of pairs shows under the choice; a run takes at most 250,000,
  so on a large case narrow the elements with the bus filters first, for
  example to one area at 230 kV and up.
- **Zero-impedance lines**: tick **Skip** to leave out lines with no
  resistance and a reactance at or below the threshold (0.0001 p.u. by
  default, PSS/E's THRSHZ), as PSS/E does. It applies to N-1, N-2 and the
  planning events; a bus outage still opens them. Transformers never count.
- **Method**: **Linear screen, then AC for the worst** (the default) screens
  every outage with linear outage factors in a fraction of a second, then
  solves the worst N in AC. **AC solve of every outage** is slower on large
  cases but needs no screen.
- **Worst checked in AC**: N, 50 by default.
- **Base case rating** and **Contingency rating**: which of each branch's
  ratings (rate A normal, B short-term, C emergency) the loading is measured
  against before and after an outage. Both are rate A by default; many
  planners use rate A before and rate B after. A branch with no value for
  the chosen rating is unrated. Saved in the study file.
- **Overloaded above**: the loading, as a percent of the chosen rating, that
  counts as an overload.

The bus filters above the tab narrow the outages to branches and generators
touching the matching buses, for example one area or one voltage level.

## Planning events

Set **Outages** to **NERC TPL-001 planning events** to run the events of
TPL-001 Table 1 that Stannum generates from the case, instead of every
branch and generator:

| Event | What goes out |
| --- | --- |
| P0 | Nothing: the base case, checked against the limits as it stands |
| P1.1 | One generator |
| P1.2 | One transmission line; a multi-section line goes out whole |
| P1.3 | One transformer (a three-winding transformer is one) |
| P1.4 | One fixed or switched shunt |
| P1.5 | One pole of a two-terminal DC line: half its schedule |
| P2.1 | One section of a multi-section line |
| P7.2 | A two-terminal or VSC DC line |

Tick the categories to run; each shows how many events it gives on the open
case. **At or above (kV)** (100 kV by default) keeps branches, shunts and DC
lines with a terminal at that voltage or higher, and any bus without a base
voltage. Generators are not filtered by voltage, since most connect below it
through a step-up transformer.

The results list each event's category beside the outage, with a note when
the element alone doesn't say what went out (one pole, or which section). The
row of category buttons above the table counts each category's events and
those with problems; click one, or use **All categories**, to list only that
category. **Export CSV** includes the category.

P0 is solved in AC and judged against the limits, so it lists every overload
and voltage problem already in the case; the other events count only what
they make new or worse, as N-1 does. Shunt outages move voltages rather than
MW, which the linear screen can't see: the largest shunts (by MVAr) get an
AC budget of their own, as many as **Worst checked in AC**.

Events that need breakers and substation layout (P2.2 to P2.4, P4, P5), two
outages with adjustments between them (P3, P6), or tower data (P7.1) are not
generated yet. Fault type and clearing don't change a steady-state result,
so only what goes out matters here.

**Run contingencies** shows progress while it runs, and **Cancel** stops it;
outages it didn't reach in AC keep their linear result. Texas 2k (6,080
outages) screens in about half a second and solves the worst 50 in AC in
under two seconds.

## Your own contingency list

Set **Outages** to **From a lists file (.stnl)** and choose the file to run
the contingencies it defines, the way PSS/E runs a `.con` file:

- each `contingency` record is one outage, named in the results by its
  record name, with its `category` as a chip above the table. `open` events
  take an element out; `trip_bus` takes out a bus;
- each `contingency_set` generates outages for the case at hand: single
  branches, single generators, pairs of branches, or whole buses, in its
  subsystem and above its `kv_min`. Its name becomes the category.

A list doesn't belong to one case. Elements the case doesn't have, and
events Stannum doesn't apply yet (`close`, `set_p_mw`, `set_q_mvar`), skip
their contingency and are listed in a note above the results. The
[lists file format](/StannumRevival/reference/lists-file/) page has the
syntax and an example; the study file remembers which file you chose.

## Reading the results

The figures at the top count outages that cause overloads, voltage problems
(AC only), AC solves that didn't converge, and outages that split buses off
from the rest of the network.

The table lists one row per outage, worst first: AC results first, then the
linear-only ones. An overload counts when a branch goes over the limit, or
was already over it and gets more than one point worse, so a branch that is
overloaded before any outage doesn't fill the list. **Worst branch** is the
most loaded such branch (or the most loaded branch when nothing is over the
limit) and **Before** its loading without the outage. **Splits off** gives
the buses that lose their path to the rest of the network and the load and
generation on them. **Problems only** hides the rest, and **Export CSV**
saves the list.

Click a row to open the outage beside the table: every overload and voltage
problem, what it splits off, and the largest flow changes. Each element name
selects it, and its wand icon draws a one-line around it. **Take out** adds
the outage to the [study](/StannumRevival/guide/studies/), so the main solve
uses it.

When an AC solve doesn't converge with generator Q limits enforced, the
outage solves again without them, and the note says which: "Converges only
without generator Q limits" points at too little reactive support.

## On the diagram

**Show on diagram** draws a one-line around the outage and turns on the
**Contingency** layer, in either drawing style: the tripped element ringed
in red, overloads in red, branches carrying 5 MW or more extra in orange and
5 MW or more less in blue, buses outside their voltage limits ringed in
amber, and split-off buses faded. Hover a branch for its flow after the
outage. **Clear** in the Layers panel turns it off.

![The Contingency layer on a one-line around the tripped line, with the legend](../../../assets/screens/contingency-layer.png)

### The result every view shows

While an outage is on the diagram, every view shows that outage's AC
solution instead of the latest solve: the diagram's flow and voltage labels,
the Power flow tables and overview, the **Results** layer and the element
inspector. The toolbar says which result is shown (**Outage #3 · Line 6–8
circuit 1 out**, or **Base case**); its menu switches between the latest
solve and the open outage, and the cross goes back to the latest solve. A
new solve, a study change or clearing the outage goes back too.

An outage the run only screened is solved in AC when it is shown. Results
have names, which the log and (later) Python scripts use as well: `base` or
`study` for the latest solve, `contingency 3` for the outage ranked third,
and `trip line:6:8:1` for the last **Trip and compare**.

## Trip and compare

To ask about one element without running every outage, select a branch or
generator anywhere and choose **Trip and compare** in its inspector. It takes
the element out of the latest AC solve, solves again in AC with the same
settings, and draws the result with the **Contingency** layer, around the
element. The inspector's **If it trips** box lists the branches pushed over
the limit (new or worse, as above), voltage problems and any split-off
buses. **Keep it out in the study** turns the trip into a study edit;
**Clear** removes it from the diagram. A contingency run's results are left
as they were.

![Trip and compare: the If it trips box in the inspector and the Contingency layer around the tripped line](../../../assets/screens/trip-and-compare.png)

The method is in [Contingency analysis](/StannumRevival/manual/contingencies/).
