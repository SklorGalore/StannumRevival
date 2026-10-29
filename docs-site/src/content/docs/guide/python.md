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

Each [release](https://github.com/SklorGalore/StannumRevival/releases/latest)
on GitHub has a wheel for Windows x64, Macs with Apple silicon and Linux x64
(see [Download](/StannumRevival/start/download/) for the file names). One
wheel per platform works on Python 3.10 and newer. NumPy comes with it;
pandas is optional.

```sh
pip install stannum-0.3.0-cp310-abi3-win_amd64.whl
```

The package isn't on PyPI yet. To build a wheel from a checkout (Rust and
Python needed):

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
case.export_raw("case_v33.raw", revision=33)   # any case, as RAW 33 or 35

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

## What's public

Everything in `stannum.__all__`, and the documented methods and attributes
of its classes, stays compatible across 1.x. Get cases, diffs and apps from
`sn.open`, `sn.diff` and `sn.attach` rather than their constructors; names
starting with `_`, such as `stannum._native`, can change in any release.
[Versions and compatibility](/StannumRevival/reference/versioning/) has the
full rules, and how anything is retired.

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

## Driving the running app

`sn.attach()` connects to the Stannum app open on your computer instead of
opening a workspace of its own. The script then works on the case in the
window, and the window shows what it does as it goes: a case it opens,
study edits it makes, results it solves. A **Script attached** chip in the
case bar says a script is connected.

```python
import stannum as sn

app = sn.attach()                     # the newest app running
case = app.case                       # the case in the window, or None
case = app.open("Texas2k.RAW")        # or open one in the window

case.edits = [sn.outage("line:6137:6207:1")]
report = case.solve()                 # the window shows the study solve

app.draw_around("bus:6137")           # a one-line around it in the Diagram
app.show("bus:6137")                  # select it in every module
app.go("power")                       # switch module
app.say("Outage studied")             # a line in the app's log
app.commands()                        # the Ctrl+K commands, by id
app.run("theme")                      # run one
app.state                             # module, selection, case, result
```

A case from `attach()` has every method `sn.open()` gives; the difference
is that it is the window's case. `with` blocks and `app.detach()` let go of
the app without closing it or its case. If the person at the window opens
another case, the script's old case raises `CaseNotOpen`. Ctrl+C stops the
app's solve or contingency run, as it does in-process.

**How it connects.** The app listens on a local port (127.0.0.1 only) and
writes the port and a random token into a file only you can read:
`%LOCALAPPDATA%\Stannum\attach` on Windows,
`~/Library/Application Support/Stannum/attach` on macOS and
`$XDG_RUNTIME_DIR/stannum/attach` (or `~/.local/state/stannum/attach`) on
Linux. `STANNUM_ATTACH_DIR` moves it. `sn.attach(pid=...)` picks one app
when several run. Start the app with `STANNUM_NO_ATTACH=1` to turn attaching
off. From 1.0 on, any 1.x package attaches to any 1.x app (see
[Versions and compatibility](/StannumRevival/reference/versioning/)); before
1.0 they must come from the same release.
