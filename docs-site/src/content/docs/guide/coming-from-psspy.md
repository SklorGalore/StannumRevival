---
title: Coming from psspy
description: The stannum Python calls that do what common psspy calls do, side by side.
---

The [stannum Python package](/StannumRevival/guide/python/) does many of the
jobs psspy scripts do, but it is not a copy of psspy. There is no ierr to
check: a call that fails raises an exception that says why. Results come
back as tables of NumPy arrays, not as `abusreal` lists. Outages,
generation and load changes are study edits: every later solve uses them,
and the case itself is left as it was. Record edits change the case, as
psspy's `_chng` calls do, and are kept when you save.

```python
import stannum as sn

case = sn.open("summer.raw")
report = case.solve()                # raises if the case can't be read
low = report.buses[report.buses["vm"] < 0.95]
print(low.rows())
```

## Calls side by side

| psspy | stannum |
| --- | --- |
| `psspy.psseinit()` | Nothing to do. `import stannum as sn` |
| `psspy.read(0, "case.raw")` | `case = sn.open("case.raw")` |
| `psspy.case("case.sav")` | Save RAW from PSS/E first; `.sav` can't be read |
| `psspy.fnsl([...])` | `case.solve(method="newton")` |
| `psspy.fdns([...])` | `case.solve(method="fixed_slope")` |
| `psspy.fnsl` with flat start | `case.solve(start="flat")` |
| `psspy.solved()` | `report.converged` |
| `psspy.abusreal(-1, 2, "PU")` | `report.buses["vm"]` |
| `psspy.abusint(-1, 2, "NUMBER")` | `report.buses["id"]` |
| `psspy.aflowreal(...)`, `"PCTRATE"` | `report.branches["pf"]`, `report.branches["loading_pct"]` |
| `psspy.busdat`, `psspy.brndat` | `case.element("bus:5")`, `case.element("line:4:5:1")` |
| `psspy.branch_chng_3(i, j, "1", [0, ...])` | `case.edits = [sn.outage("line:4:5:1")]` |
| `psspy.machine_chng_4(...)` for PG | `case.edits = [sn.generation("generator:2:1", 150)]` |
| `psspy.load_chng_6(...)` | `case.edits = [sn.bus_load(5, mw=100, mvar=40)]` |
| Changing any other field | `case.edit_records([{"section": "BUS", "row": 0, "column": "VM", "value": "1.02"}])` |
| `psspy.scal_2(...)` | `case.call("scale_load_generation", settings={...})`, see [Scaling](/StannumRevival/guide/scaling/) |
| `psspy.accc_with_dsp_3(...)` | `case.contingencies(...)`, then `run.rows` |
| `psspy.dfax_2` and `psspy.tltg` | `case.transfer_limits(source, sink, ...)` |
| `psspy.pv_engine` | `case.pv_curve(sink, source)` |
| `psspy.qv_engine` | `case.qv_curve([buses])` |
| `psspy.rawd_2(...)` | `case.export_raw("out.raw", revision=33)` |
| `psspy.save("case.sav")` | `case.save_as("case.stn")` (or `.raw`); it keeps record edits, not study edits |
| `psspy.bsys(...)` | A dict such as `{"areas": [1, 2], "kv_min": 100}` |

Element ids are the same ones the app shows: `bus:5`, `line:4:5:1`,
`transformer:…`, `generator:2:1`. The table columns (`report.buses.columns`)
list every field.

## Subsystems

A psspy subsystem is a numbered slot you fill with `bsys` and then pass by
number. In stannum, pass the subsystem itself where it is needed:

```python
north = {"areas": [1], "kv_min": 115}
south = {"areas": [7]}
run = case.transfer_limits(north, south, contingencies={"kv_min": 115})
```

Areas, zones, owners and buses in one subsystem are combined as a union, and
`kv_min`/`kv_max` narrow it.

## Contingencies

psspy builds a `.dfx` from `.sub`, `.mon` and `.con` files, then runs ACCC
and reads the `.acc` file back. stannum runs in one call and answers with
the ranked results:

```python
run = case.contingencies()                        # every branch and generator
run = case.contingencies(lists="summer.stnl")     # contingencies from a lists file
run.rows.to_pandas()                              # every outage, worst first
case.contingency(1)                               # the worst one's changes
case.export_contingency_results("summer.stnr")    # compact results file
```

The `.sub`, `.mon` and `.con` files are written as one
[lists file](/StannumRevival/reference/lists-file/) instead.

## Scripts without Python

A [run file](/StannumRevival/guide/run-files/) is the closest thing to an
IDV file: one command per line, run by the app or by `stannum-cli run`.
**Save as run file** in the app writes one from what you did.

## What has no equivalent

Dynamics (`psspy.dyre_new`, `psspy.run`), short circuit (`psspy.ascc`), OPF,
and reading `.sav`, `.seq`, `.dyr` and `.acc` files. See
[What isn't modelled](/StannumRevival/reference/limitations/).
