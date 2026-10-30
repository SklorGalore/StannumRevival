---
title: MATPOWER
description: The MATPOWER case subset Stannum reads and writes.
---

Stannum reads and writes MATPOWER version 2 case files (`.m`) as data,
without running MATLAB or Octave.

## What is read

- `mpc.version = '2'`, a literal `mpc.baseMVA`, and numeric `bus`, `gen` and
  `branch` tables (at least 13, 10 and 13 columns).
- Optional `gencost` (kept, not used) and `dcline` (used as [DC
  lines](/StannumRevival/manual/hvdc/overview/#simplified-model)).
- Literal `bus_name`, `gentype` and `genfuel` string cell arrays.
- `function mpc = name` or `function mpc = name()` wrappers, an optional final
  `end`, `%` comments, `...` continuations, decimal and scientific numbers,
  signed literals, and `Inf`/`-Inf` for reactive limits.

Extra columns are kept without interpretation, including result columns from
an earlier MATPOWER run.

## What is rejected

Arithmetic expressions, indexed assignments, function calls, unknown fields
and general MATLAB code are refused with a source location. `.mat` files and
version 1 cases are not supported.

## Conventions

| Item | Convention |
| --- | --- |
| Bus types | 1 PQ, 2 PV, 3 reference, 4 isolated |
| Transformer tap | From side; 0 means 1 (a line) |
| Branch flow | Positive entering the branch at that end |
| Generator status | Normalized to 0 or 1 on export |
| Limits | Voltage, branch rating, angle-difference and generator P limits are kept and checked in results but not enforced in the solve; Q limits only when requested |

## Writing

**Export case** in Power flow (or Ctrl+K) writes the input case or the latest solved
case. The file defines a function named after the file, ready for MATPOWER's
`loadcase`. Row order, extra columns, costs and labels are kept; comments and
original number formatting are not. The file name must be a valid MATLAB
function name.

A RAW case exported this way carries the conversion described in [PSS/E
RAW](/StannumRevival/reference/psse-raw/), including the DC line table with
solved values.
