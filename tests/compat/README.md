# Files from earlier releases

Stannum 1.x promises to open every study, diagram, run file and coordinates
file that a 1.x release wrote. The files here were written by released
versions of the app, and the tests open them on every change:

- `crates/stannum-core/tests/compat.rs` runs the run files, attaches the
  coordinates and finds each study's case;
- `src/lib/compat/compat.test.ts` opens the studies and diagrams.

| Folder | Written by | Files |
| --- | --- | --- |
| `0.3.0/` | Stannum 0.3.0 | `case9-outage.stannum` (two study edits, a saved solve, a generated drawing), `five_bus.sld` |
| `0.4.1/` | Stannum 0.4.1 | the same two, the study with bus coordinates attached; `case9_outage.stnrun` (recorded in the app); `case9_study.stnrun` (the 0.4.1 run file example, written by hand); `case9-coordinates.csv` (written by hand) |

The studies, diagrams and recorded run file were saved by the browser smoke
test (`tests/browser/smoke.mjs`) of each release, run against that release's
own core and frontend. The only change since is to the paths inside the two
studies, which pointed into a temporary folder: `case.path` and
`coordinates.path` are now Windows paths from another machine, and
`case.relative` leads to `crates/stannum-core/tests/fixtures/case9.m`, which
is the same file (same SHA-256) the study was saved against.

When a release changes a file format, add a folder for it with files that
release wrote, and never edit the files already here. Formats and their
version rules are described under *File formats* in the docs site.
