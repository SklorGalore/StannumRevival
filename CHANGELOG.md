# Changelog

All notable changes to Stannum are listed here. Each release's section is
also its release notes on GitHub. Add a line under **Unreleased** in the same
pull request as the change; `scripts/release/new-version.ps1` turns that
section into the next version's when a release is cut (see
[docs/releasing.md](docs/releasing.md)).

## Unreleased

### Added

- Run files (`.stnrun`): Stannum commands written one per line, run in
  order. **Ctrl+K → Record a run file** keeps what you do (opening, solving,
  study edits, contingencies, exports) and saves it as one; **Run a run
  file…**, or dropping one on the window, plays it back and shows the case
  it leaves. Every line is checked before the first runs, and a failing
  line stops the run and is named.
- A Python package, `stannum`: open cases, solve, set study edits, run
  contingencies, read and edit records, check topology and compare RAW files
  from a script or notebook (`import stannum as sn; sn.open("case9.m").solve()`).
  Results come as NumPy columns, failures as typed errors (`stannum.NotFound`,
  …), and Ctrl+C stops a long run. See the Python page of the user guide.
- Each release now carries the Python package as a wheel for Windows, Macs
  with Apple silicon and Linux (Python 3.10 and newer, `pip install` the
  file), and `stannum-cli` packed for the same three.
- `stannum-cli`, which runs, checks and lists run files from a terminal
  without the app (`stannum-cli run study.stnrun`).
- One result shown in every view: showing an outage on the diagram (from
  Contingencies or Trip and compare) switches the diagram's flow labels, the
  Power flow tables and the inspector to that outage's solution, and the
  case bar says which result is shown, with a menu to switch back.
- A Map drawing style: attach a bus coordinates CSV and the case's
  substations and branches are drawn at their latitude and longitude over
  country and state outlines that ship with the app and follow the theme.
  Buses the file leaves out are placed next to their neighbours.
- Layers and results on the map: every Diagram layer (colour by result,
  contingency, bridges and cut buses, islands, shortest path, filters and
  colours) draws on the Map style too, and **Flows** adds an arrow on each
  line pointing the way power flows in the shown result, with flows and
  voltages on hover. A shortest path is highlighted where it runs.
- RAW revision 34 and 35 files that carry substation data draw on the map
  without a coordinates file: each substation's latitude and longitude places
  the buses its nodes name. An attached coordinates CSV still wins for the
  buses it lists.

### Changed

- Solve, sensitivity and contingency settings may be left out of the core
  commands, or given in part: what's missing takes its default.
- Core commands report failures as a kind and a message (for example
  `cancelled`, `no_case` or `not_ready`), and every command's arguments and
  result have a JSON Schema. The frontend's types for core commands are
  generated from it, and a test checks every command's real answers
  against it. Command API version 2.
- Solves, sensitivities, "Trip and compare" and Records reads and edits no
  longer hold up the rest of the app: on a 30,000-bus case, Records opens
  in a fraction of a second during a 5 s solve instead of waiting for it.
- Records tables and ranked contingency rows travel as columns, like the
  solve report, through one table type in the core: about 15% less to send
  for the largest Records table of a 12,000-bus case. Command API version 3.
- Opening a case no longer builds its topology: the Network style, the
  diagram's tables and layers build it the first time they need it. A
  12,000-bus case opens in about half the time (0.26 s to 0.12 s).
- A solve after a Records edit starts from the last converged solve, as
  long as the edit kept the same buses: a 5 MW load change on case300
  solves in 1 iteration instead of 4.

### Fixed

- Records: double-clicking a cell of a record that wasn't picked yet edits
  that cell. The first click opens the inspector, which narrows the columns,
  and the edit used to land on the cell that slid under the pointer.
- Generators holding a remote bus no longer leave a solve on a collapsed
  answer: a unit asked for far more reactive power than it has could end a
  pass with its own terminal near 0 pu, and the solve then settled there
  once the unit was held at its limit. A unit held at its limit now restarts
  from the voltage of the bus it held.
- Generated synthetic cases share each substation's reactive power between
  its plants by their size, not equally. The 80,000-bus case now finishes
  its voltage and loading cleanup: 2 buses outside 0.97-1.05 pu and 8
  branches over their limit, down from 1,873 and 308.
- A solve or contingency run that finishes after the case was edited is set
  aside with a message instead of being shown as the edited case's results.

## 0.3.0 - 2026-10-26

The first packaged release. Everything below is new since Stannum was
assembled from StannumSLD, rs-power, nodal and case-diff.

### Added

- Installers for Windows (.msi and setup .exe), macOS on Apple silicon (.dmg)
  and Linux (.deb, .rpm and .AppImage), built by the release workflow when a
  `v*` tag is pushed.
- One shell for every module: a module rail, Ctrl+K command palette and
  element search, side by side panes with linked selection, a log panel,
  light and dark themes, an animated splash and Stannum's own title bar on
  Windows.
- Power flow: AC Newton and DC solves with generator reactive limits, remote
  voltage regulation, switched shunts, transformer taps, ZIP loads, warm
  starts from the last solution, and a Cancel button for long solves.
- HVDC: two-terminal LCC and VSC lines, multi-terminal DC, and meshed VSC
  grids solved together with the AC case.
- Results you can see: result marks on the diagram, colour by loading,
  voltage, sensitivity or the change from another solve, a voltage profile
  and the most loaded branches on the overview, and tables with names,
  loading bars and filter chips.
- Contingencies: N-1 analysis with a linear screen and a full AC check of
  the worst outages, ranked results linked to the diagram, and Trip and
  compare from the inspector.
- Diagram: one-lines generated around any element with compact local
  layouts, a Network drawing style with topology layers (islands, bridges,
  cut buses, paths), a shared element inspector, three-winding transformer
  symbols, a snap grid, and PDF export.
- Studies: outages, generator and load edits with saved solves to compare.
- Study files (`.stannum`): save the study edits, saved solves, solve and
  contingency settings, drawing and layout with a link to the case file;
  opening one finds a moved case and says when the case has changed.
- Recent cases and studies on the empty Power flow and Records screens.
- Case diff of the working case: Records and study edits compared with the
  file on disk.
- Records: every record type of the open case as a table, with cell editing.
- Case diff: record-level comparison of two cases.
- Documentation site with a user guide and technical manual.
- Synthetic test cases of any size, built with the method of Birchfield et
  al. (2017) and checked against its structural criteria
  (`scripts/new-synthetic-case.ps1`).
