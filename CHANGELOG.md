# Changelog

All notable changes to Stannum are listed here. Each release's section is
also its release notes on GitHub. Add a line under **Unreleased** in the same
pull request as the change; `scripts/release/new-version.ps1` turns that
section into the next version's when a release is cut (see
[docs/releasing.md](docs/releasing.md)).

## Unreleased

### Added

- **Crash handling**: a bug inside Stannum no longer takes the window down.
  The step that hit it fails with a notice and an error line in the log, and
  everything else keeps working. A view that fails while drawing can be drawn
  again on its own. Python and run files get an `internal` error instead of a
  crash.
- **Save diagnostics…** (log toolbar, crash notice and command palette)
  saves the log with Stannum's, rs-power's and nodal's versions and the
  platform, for bug reports. No case data. Also the `diagnostics` command.
- **Versions and compatibility** page in the docs: what a version number
  promises from 1.0 on for Stannum's files, the Python package,
  `stannum-cli`, run-file commands and attach, and how anything is
  deprecated before it's removed.
- Stannum is now licensed BSD-3-Clause. The installers, Python wheels and
  `stannum-cli` archives include `LICENSE`, `THIRD-PARTY-NOTICES.md` and
  `THIRD-PARTY-LICENSES.txt`, the licences of the software built into them.
- **Area interchange** control in AC solves: each area's slack generation
  moves until its net interchange is within `PTOL` of the RAW area data's
  `PDES`, counting tie lines, or tie lines and loads that belong to another
  area. Off by default; choose it under **Area interchange** in the solve
  settings, or with `area_interchange` in run files and Python. A new
  **Areas** tab lists every area's interchange, desired value and slack
  movement, also with control off.
- **Export PSS/E RAW** 33 or 35 from any open case, with its Records edits
  and study edits: in Power flow's side panel, **Ctrl+K**, `export_raw` in
  run files and `Case.export_raw` in Python. RAW 33 and 35 cases are written
  record by record, keeping every record and converting between the two
  revisions; MATPOWER and older RAW revisions are written from the power flow
  model, with its two-terminal and VSC DC lines, FACTS devices, phase
  shifters, area interchange targets and every switched shunt mode.
  Anything the file can't keep is listed after the export.
- Large tables read a page at a time. Power flow's bus, generator and
  branch tables, Records, and contingency results are now sorted, searched
  and filtered in the core, and the window reads only the rows in view as
  you scroll. On an 80,000-bus case a Records table no longer copies 19 MB
  into the window, and every contingency result can be sorted and scrolled
  without **Load all**. Under a filtered table the count reads "69 rows of
  80,000". Scripts get the same pages with `result_buses`,
  `result_generators`, `result_branches`, `record_page` and
  `contingency_page` (plus `result_bus_values`, `record_fields` and
  `record_values` for filter choices).
- Generated one-lines draw RAW switching devices: breakers and disconnectors
  with their name, open or closed state (flagged when not in their normal
  position) and flow. Double-click one to open or close it in the study.
  The inspector shows their type, state, normal state and flow, and its
  study action reads **Open** / **Close** for them.
- **DC line transformer taps**: when a two-terminal DC line converter in a
  RAW file names a two-winding transformer with `COD` 4 (`IFR`/`ITR`/`IDR`
  or `IFI`/`ITI`/`IDI`), that transformer's tap now steps in place of the
  converter's own tap, within the converter's tap limits, to keep the
  rectifier's firing angle in range or the inverter's DC voltage at
  schedule. **Adjust DC converter taps** moves and locks them. RAW exports
  written from the model keep these transformers and reactive flow taps
  (`COD` 2).
- **Reactive flow taps**: transformer windings in reactive power flow
  control in RAW files (`COD` 2) now step their taps in AC solves to hold
  their Mvar band, moving with the phase shifters. **Move transformer taps**
  moves and locks them too.
- **Phase shifters**: transformer windings in active power flow control in
  RAW files (`COD` 3, and asymmetric `COD` 5) now move their angles in AC
  solves to hold their MW band. The moves are solved together through the
  network, so parallel shifters share the correction. **Move phase shifters**
  in the solve settings (`adjust_phase_shifters` in run files and Python)
  locks them when off. The overview counts the shifters that moved, and the
  Branches table shows each transformer's tap and phase shift.
- Switched shunts in RAW modes 3, 4 and 5 now switch: they keep a plant's
  reactive output, a VSC converter's, or another switched shunt's setting
  within their band, as PSS/E does. Mode 6 keeps a FACTS device's shunt
  element within its band.
- **FACTS devices** from RAW FACTS control device data: the shunt element
  holds its bus (or `REMOT`) at `VSET` within `SHMX`, like a STATCOM, and a
  series element in normal mode sends `PDES` and `QDES` to its terminal-end
  bus, like a UPFC (also in DC solves). A new **FACTS** tab in Power flow
  lists each device's output. On the PSS/E sample case the largest angle
  error against its saved solution drops from 10.8° to 0.9°.
- Continuous switched shunts wait for generators and FACTS devices to settle
  their reactive limits before they are fixed at the end of their range.

### Changed

- The `stannum` Python package's public API is now exactly what
  `stannum.__all__` lists. The `stannum.errors` module is private
  (`stannum._errors`); import the errors from `stannum` itself, as in
  `stannum.NotFound`. Helpers the package used inside, such as
  `stannum.from_kind` and `stannum.tables_in`, are no longer reachable from
  the top.

### Fixed

- Power flow reads RAW revision 34 and 35 system switching devices instead
  of skipping them, so buses joined only by a breaker or disconnector are no
  longer split apart. Each solves as a lossless branch of the device's
  reactance, closed devices in service. Case diff compares their fields
  too (it used to list them as an unsupported layout).

## 0.4.1 - 2026-10-28

### Added

- AC solution methods: **Auto** (the new default), **Full Newton**,
  **Fixed-slope decoupled (FDNS)**, and **Fast decoupled** XB and BX. Auto
  starts each pass with cheap decoupled iterations and finishes with Newton
  when they slow down or fail, so it solves the 70,000–82,000-bus synthetic
  cases about twice as fast as Newton (1.1 to 4 times), and still converges in
  the stressed cases where the decoupled methods alone do not.
  Every method stops at the same mismatch tolerance. Choose it under **AC
  method** in the solve settings, or with `method` in run files and Python;
  `decoupled_iterations` sets the decoupled iteration limit.
- **Move switched shunts** and **Move transformer taps** (and **Adjust DC
  converter taps**) now lock those devices when turned off: they stay where
  the last solve left them, instead of going back to their case values.

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

- The app's analysis types now come from the core's command schemas
  (`commands.gen.ts`) rather than a hand-kept copy, so a field added in
  Rust can't be missed in the app.
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
- Fixed shunts, switched shunts, induction machines and FACTS devices have
  their own ids (`shunt:101:1`, `switched-shunt:101:1`, `machine:101:1`,
  `facts:SVC_1`): picking one in Records opens it in the inspector with its
  fields as filed, and a bus lists them among its connections. One table in
  the core lists every kind of id (`element_kinds`), and the app reads it.

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
