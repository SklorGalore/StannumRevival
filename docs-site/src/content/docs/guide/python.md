---
title: Python
description: Open cases, solve, run contingencies and read results from Python with the stannum package.
---

The **stannum** Python package runs Stannum's analysis core inside Python:
open PSS/E RAW and MATPOWER cases, solve power flow, run N-1 contingencies,
read and edit records, check topology and compare RAW files, in a script, a
notebook or CI. It needs no app window. Every call goes through the same
command list the app uses, so a script and the app give the same answers.

```python
import stannum as sn

case = sn.open("case9.m")
report = case.solve(start="flat")
print(report.converged, report.iterations, report.total_losses_mw)
```

## Install

From the next release on, wheels are attached to each
[release](https://github.com/SklorGalore/StannumRevival/releases) on GitHub.
One wheel per platform works on Python 3.10 and newer. NumPy comes with it;
pandas is optional.

```sh
pip install stannum-0.3.0-cp310-abi3-win_amd64.whl
```

Until then, build one from a checkout (Rust and Python needed):

```sh
pwsh scripts/ci/test-python.ps1      # wheel in dist-py/, tests run against it
```

## Cases

`sn.open(path)` returns a `Case`. Each case is independent: open a base case
and a variant side by side, and use them from several threads at once. A
`with` block closes the case at the end.

```python
with sn.open("base.raw") as base, sn.open("summer.raw") as summer:
    a = base.solve()
    b = summer.solve()
```

`case.summary` says what the case holds; `case.id` is the handle every
command names it by.

## Solving

`case.solve()` takes the same settings as the Power flow module, as keyword
arguments, and returns the solve report:

```python
report = case.solve(
    model="ac",             # or "dc"
    start="flat",           # "last" (default), "flat" or "case"
    tolerance_mva=0.1,      # 2 MVA by default
    enforce_q_limits=True,
)
report.converged            # also report["converged"]
report.limits.branch_overloads
```

`report.buses`, `report.generators` and `report.branches` are **tables**:
one NumPy array per field.

```python
buses = report.buses
buses["vm"]                          # voltage magnitude at every bus
low = buses[buses["vm"] < 0.95]      # a smaller table
low.rows()                           # as dicts
buses.to_pandas()                    # a DataFrame, with pandas installed
```

## Study edits

The same edits as a study in the app: outages, generator output and bus load.
Every later solve uses them; the file isn't touched.

```python
case.edits = [
    sn.outage("line:4:5:1"),
    sn.generation("generator:2:1", 150),
    sn.bus_load(5, mw=100, mvar=40),
]
after = case.solve()
case.edits = []                      # back to the case as filed
```

Elements are named by their shared id, the same one the inspector and the
tables show (`line:4:5:1`, `transformer:…`, `generator:2:1`, `bus:5`).

## Contingencies

```python
run = case.contingencies(method="ac", limit_pct=100)
run.with_overloads
run.rows                             # every outage, worst first, as a table
case.contingency(1)                  # the worst outage's flow and voltage changes
case.trip("line:4:5:1")              # Trip and compare
```

Settings are those of the Contingencies module: `elements`, `branches`,
`generators`, `method`, `ac_count`, `limit_pct` and `solve`.

## Records, elements and topology

```python
case.records()                       # record types and counts
buses = case.records("BUS")          # every bus record, one column per field
case.edit_records([{"section": "BUS", "row": 0, "column": "NAME", "value": "NORTH"}])
case.save_as("edited.raw")

case.element("bus:5")                # data and limits, as the inspector shows them
case.topology()                      # islands, bridges, articulation buses
case.shortest_path(1, 9)
```

## Comparing RAW files

```python
d = sn.diff("before.raw", "after.raw")
d.differing
d.records("BUS")                     # one record type's differences
```

## Run files and every other command

`sn.run_file("study.stnrun")` runs a [run file](/StannumRevival/guide/run-files/)
and returns its report. Any command in the list runs with `case.call`,
arguments in snake_case:

```python
[c["name"] for c in sn.commands()]
case.call("diagram_network")         # the case as a drawable network
```

`sn.schemas()` returns every command's arguments and result as JSON Schema,
and `sn.API_VERSION` the version of the command list.

## Errors

A failed command raises a `stannum.StannumError`. Its class says what went
wrong, so a script can catch one kind and let the rest through:

| Error | When |
|---|---|
| `NotFound` | The element, record type or result isn't there. |
| `NotReady` | Another step comes first, such as solving before sensitivities. |
| `InvalidArguments` | An argument is misspelled, missing or the wrong type. |
| `FileError` | A file couldn't be read or written. |
| `Failed` | The work itself failed; the message says why. |
| `Cancelled` | The work was stopped. |
| `NoCase`, `CaseNotOpen` | The case was closed. |
| `Unsupported` | The command doesn't work on this format or revision. |

`error.kind` holds the same name as text (`"not_found"`), and `str(error)` the
sentence the app would show.

## Long runs and Ctrl+C

A solve or contingency run lets go of Python's lock while it works, so other
threads keep running. **Ctrl+C** stops it, the case keeps its earlier
results, and Python raises `KeyboardInterrupt` as usual.
