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

Solve the case with the AC model first. The bar above the results sums up
the setup in one line; **Set up…** opens the contingency setup, a dialog with
five steps:

1. **Outages**: what to take out, and where.
2. **Monitor**: what is checked after each outage.
3. **Subsystems**: named sets of buses (areas, zones, owners, bus numbers
   and a kV range) that Outages and Monitor pick from.
4. **Limits & reporting**: ratings, the overload limit, and what counts as
   a problem.
5. **Solution**: the screen, and the AC solve options.

**Run contingencies** runs from the dialog or from the bar. The choices are:

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
  the chosen rating is unrated.
- **Overloaded above**: the loading, as a percent of the chosen rating, that
  counts as an overload.

- **Within** (Outages): a subsystem, to take out only the elements in it:
  a branch counts when either end is in it. With N-2, pairs are of
  branches only.
- **Overloads worse by** and **Voltages worse by** (Limits & reporting):
  how much worse a problem already there before the outage must get to
  count (1 point and 0.001 p.u. by default). **Include existing problems**
  counts every problem after the outage.
- **Generator Q limits**, **Transformer taps** and **Switched shunts**
  (Solution): **Same as Power flow** follows the Power flow tab's settings;
  **On** or **Off** sets them for the contingency solves only.

The bus filters above the tab narrow the outages to branches and generators
touching the matching buses, for example one area or one voltage level. A
subsystem chosen under **Within** takes their place.

## Subsystems

The **Subsystems** step builds the sets of buses the other steps pick from.
**New** adds one; tick areas, zones and owners (each with its bus count),
list bus numbers, and give a kV range. Listed members combine, and the kV
range narrows them; with nothing listed, every bus in the range is in it.
**Save…** writes them as a `.stnl` lists file and **Load…** reads the
subsystems of one, so the same subsystems work on any case. A subsystem
that includes other subsystems loads without those links.

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
syntax and an example.

## Checking only what you monitor

By default every rated branch and every bus is checked after each outage.
Under **Monitor**, choose **A subsystem** to check only the branches with an
end in it (at the limit and ratings of Limits & reporting) and, with **Bus
voltages too**, its buses. Or choose **Monitors from a lists file** and a file (the
lists file you run, or another) to check only what its `monitor` records
name, as a PSS/E `.mon` file does:

- a `flow` monitor watches one branch, or every branch with an end in its
  subsystem, at its own `limit_pct` and, when it names one, its own rating
  slot (1 to 3 for rates A to C);
- a `voltage` monitor watches one bus, or every bus in its subsystem, with
  its own `vmin_pu` and `vmax_pu` in place of the bus's limits;
- an `interface` monitor sums an interface's weighted branch flows after
  each outage, measured from the first bus of each id as written, and
  flags it when the sum goes past `limit_pct` of the interface's
  `limit_mw` and is new or worse. The Overloads column counts interfaces
  beside branches, and an outage's detail lists every monitored interface.

Branches and buses no monitor names aren't checked, so a file with only
voltage monitors reports no overloads. Monitors naming equipment the case
doesn't have are skipped and noted with the results.

## Reading the results

The figures at the top count outages that cause overloads, voltage problems
(AC only), AC solves that didn't converge, and outages that split buses off
from the rest of the network.

Each contingency is named the way PSS/E names one: the bus name and number
at each end of a branch, then its circuit (`NORTH 101 SOUTH 102 1`); a
generator's bus name and number, then its id; a bus by its name and
number; `BASE CASE` for P0. A contingency from a lists file keeps the name
the file gives it. Search finds a contingency by its name.

The table lists one row per outage, worst first: AC results first, then the
linear-only ones. An overload counts when a branch goes over the limit, or
was already over it and gets more than one point worse, so a branch that is
overloaded before any outage doesn't fill the list. **Worse by** sets that
one point, and **Include existing** reports every branch and bus outside its
limits after the outage, whether or not it was before. **Worst branch** is the
most loaded such branch (or the most loaded branch when nothing is over the
limit) and **Before** its loading without the outage. **Splits off** gives
the buses that lose their path to the rest of the network and the load and
generation on them. **Problems only** hides the rest. **Export** saves
**Every violation (CSV)**, one line per overload, voltage problem and
interface violation of every outage (and one per outage whose AC solve
failed), **One row per outage (CSV)**, the table as shown, or **Compact
results (.stnr)**, the whole run in a small binary file to keep or share:
on Texas 2k, 73,000 violations take 13 MB as CSV and 450 KB as `.stnr`.
pandas, polars and DuckDB read it directly; see the
[results file format](/StannumRevival/reference/results-file/).

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
