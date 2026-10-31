---
title: Run file format
description: "The .stnrun text format: one Stannum command per line, with name=value arguments."
---

A run file is UTF-8 text with the extension `.stnrun`. Each line is one
command from Stannum's command list, followed by its arguments. The commands
are the same ones the app sends to its core, so anything the app does to a
case can be written down. `stannum-cli commands` lists them all with their
arguments.

## Example

```text
# stannum-run-file version 1
# Solve case9 as filed, then with generator 2 backed down to 120 MW,
# and write the solved study case out as MATPOWER.
open_case path=../case9.m
solve_power_flow settings={"start": "flat"}

set_study_edits edits=[
  {"kind": "generation", "element": "generator:2:1", "mw": 120}
]
solve_power_flow
run_contingencies settings={"method": "ac", "branches": true, "generators": false}
export_matpower path=case9_study.m solved=true
```

## Lines

- A line starts with a command name, then its arguments as `name=value`,
  separated by spaces.
- `#` starts a comment, at the start of a line or after a space. Blank lines
  are ignored.
- Argument names are the commands' own: `case_path` and `casePath` both
  work.
- The `case` argument is left out. Every line works on the case open at that
  point, which is the one the last `open_case` opened.
- `run_file`, `cancel_run_file`, `save_run_file` and `run_file_text` can't be used inside a
  run file.

## Values

| Written as | Read as |
| --- | --- |
| `case9.m`, `bus:12`, `line:4:5:1` | Text. A bare word has no spaces. |
| `"cases/my case.raw"` | Text in JSON quotes, for spaces or `#`. |
| `3`, `2.5`, `true`, `false`, `null` | A JSON number, true/false or null. |
| `[...]`, `{...}` | A JSON array or object. It may run over several lines. |

A bare word that reads as JSON (`12`, `true`) is that value; quote it
(`"12"`) to make it text.

## Paths

A relative path is read from the run file's folder, not the folder Stannum
was started in. So `open_case path=../case9.m` opens `case9.m` one folder up
from the run file, wherever the folder is moved.

## Settings

`solve_power_flow`, `calculate_sensitivities` and `run_contingencies` take
their settings as a JSON object. The whole argument, and any field in it,
may be left out: a missing field takes its default.

| Setting | Default |
| --- | --- |
| `model` | `"ac"` (or `"dc"`) |
| `method` | `"auto"` (or `"newton"`, `"fixed_slope"`, `"fast_decoupled_xb"`, `"fast_decoupled_bx"`); see [AC solution methods](/StannumRevival/manual/ac-methods/) |
| `start` | `"last"`: from the last solution (or `"flat"`, `"case"`) |
| `tolerance_mva` | `2` |
| `max_iterations` | `10` Newton iterations per pass |
| `decoupled_iterations` | `30` decoupled iterations per pass |
| `enforce_q_limits` | `true` |
| `dc_model` | `"detailed"` |
| `adjust_dc_taps`, `switch_shunts`, `adjust_transformer_taps`, `adjust_phase_shifters` | `true`; `false` locks them where they are |
| `area_interchange` | `"off"` (or `"tie_lines"`, `"tie_lines_and_loads"`); see [Area interchange control](/StannumRevival/manual/area-interchange/) |

For example, `solve_power_flow settings={"method": "fast_decoupled_xb", "adjust_transformer_taps": false}`
solves with the fast decoupled method and the transformer taps locked.

`run_contingencies` settings: `method` (`"screen"`, the default, or `"ac"`),
`branches` and `generators` (both `true`), `elements` (a list of ids, instead
of every branch and generator), `ac_count` (`50`: how many of the worst screened outages get an AC solve), `limit_pct` (`100`), `solve`
(the solve settings above), and `planning` (run NERC TPL-001 planning events
instead: `categories` such as `["P0", "P1", "P7.2"]`, every one Stannum
generates when left out; `kv_min`, `100`; `areas`, every area when empty).
For example, `run_contingencies settings={"planning": {"categories": ["P1.2", "P1.3"], "kv_min": 230}}`.
`planning_event_counts settings={...}` takes the same `planning` settings and
says how many events each category gives.

## When a line fails

Every line is checked before the first runs: an unknown command, an unknown
argument, or a value that isn't closed stops the file with its line number,
and nothing runs. While running, the first command that fails stops the
run; the rest are reported as not run. `stannum-cli run --keep-going` runs
them anyway.

## Versions

A run file may start with a comment line that says which version of the
format it follows:

```text
# stannum-run-file version 1
```

It must come before the first command, among any other comment lines at the
top. Files that Stannum writes (**Save as run file**, or **Save as run file…** in the history) start
with it from 0.5; a file without it is version 1, so run files written by
hand or by 0.4 run as before.

- The version goes up only when a run file that worked would run
  differently or stop working: a command or argument is removed or renamed,
  or a value is read differently. New commands, arguments and settings don't
  change it; a run file that uses them fails on an older Stannum with the
  usual "no command" or "no argument" line error, before anything runs.
- A Stannum refuses a run file with a higher version than it reads, before
  any line runs: "Line 1: this run file is version 2, from a newer Stannum."
- Because the line is a comment, a Stannum from before 0.5 ignores it.

| Version | Read by | Changes |
| --- | --- | --- |
| 1 | 0.4 and later | The first version. |

Throughout 1.x, Stannum keeps running every run file a 1.x release wrote or
accepted. Run files from 0.4.1 are kept in the repository under
[`tests/compat`](https://github.com/SklorGalore/StannumRevival/tree/development/tests/compat)
and run by the tests on every change.
