# Changelog

All notable changes to Stannum are listed here. Each release's section is
also its release notes on GitHub. Add a line under **Unreleased** in the same
pull request as the change; `scripts/release/new-version.ps1` turns that
section into the next version's when a release is cut (see
[docs/releasing.md](docs/releasing.md)).

## Unreleased

### Added

- **Contingency names.** Contingencies are named the way PSS/E names them
  when no name is given: bus name and number at each end, then the circuit
  (`NORTH 101 SOUTH 102 1`), a generator's bus then its id, a bus outage by
  its bus, and `BASE CASE` for P0; elements out together are joined by
  `&`. Lists file contingencies keep their own names. The name leads each
  row, the outage panel and both CSV exports (which add the element ids in
  an `outage` column), and the table search finds it.

- **Contingency setup.** The Contingencies tab's settings moved into a
  setup dialog with five steps: Outages, Monitor, Subsystems, Limits &
  reporting, and Solution. The tab keeps a one-line summary, **Set up…**
  and **Run**. New in it: subsystems built from areas, zones, owners, buses
  and a kV range (saved to and loaded from `.stnl` lists files), N-1 and
  N-2 within a subsystem, monitoring one subsystem's branches and voltages,
  a voltage "worse by" threshold, and generator Q limit, tap and switched
  shunt settings for the contingency solves alone. The core takes lists
  text inline (`lists_text`, `monitors_text`) and reads a file's
  subsystems (`lists_subsystems`); a contingency set's `name` now names
  its category.

- **Every contingency violation in one CSV.** The Contingencies tab's
  **Export CSV** can now save every violation: one line per overload,
  voltage problem and interface violation of every outage, written by the
  core straight to the file (command `export_contingency_violations`), as
  well as the one-row-per-outage table. How much worse a problem already
  there before the outage must get to count is now a setting (**Worse by**,
  `worse_by_pct` and `worse_by_pu`; 1 point and 0.001 p.u. as before), and
  **Include existing** (`report_existing`) reports every problem after the
  outage. Each overload now carries the limit it was judged against.

- **Transformer magnetizing and impedance correction tables.** RAW cases'
  magnetizing admittance (`MAG1`, `MAG2`, either `CM`) now draws at the
  winding one bus, and impedance correction tables (`TAB1`–`TAB3`) scale a
  winding's impedance with its ratio or phase shift angle, worked out again
  each time a tap or shifter moves in the solve. Both were ignored before,
  as MATPOWER does; RAW export writes them back. Against the voltages saved
  in the PSS/E sample case, the largest angle difference drops from 1.19 to
  0.50 degrees.

- **Monitored elements and interfaces.** Contingency runs can check only
  what a `.stnl` lists file's `monitor` records name: branches at their own
  limit and rating, buses at their own voltage limits, and interfaces,
  whose summed flow is reported and flagged against their MW limit after
  each outage. In the app, set **Checked** to **Monitors from a lists
  file**; from Python, the CLI and run files, set `monitors` to the path.
- **Transfer limits (DFAX and FCITC), preview.** A study moves power from a
  source subsystem to a sink (scaled by generation, generator reserve or
  load) and finds the first change of total transfer capability: the
  transfer at which a monitored branch or interface reaches its rating, in
  the base case or after each contingency, from linear PTDFs and OTDFs on
  top of the AC solve. It also reports the generation limit of each side.
  Runs from the new Transfers tab in Power flow (source and sink by area,
  zone, owner or bus list), run files (`run_transfer_limits`), Python
  (`Case.transfer_limits`) and the MCP server (`transfer_limits`).
  Lists files (`.stnl`) gain `transfer` records (source and sink
  subsystems, scaling, cutoff, ratings); a run with `lists` studies the
  file's contingencies and watches its flow and interface monitors.
- **Your own contingency list.** Contingencies can run from a `.stnl`
  lists file (preview): its named contingencies (`open` and `trip_bus`
  events) and its `contingency_set` rules for single branches, generators,
  branch pairs or buses in a subsystem. Equipment the case lacks is noted
  and skipped. In the app, pick **From a lists file** under Outages; from
  Python, the CLI and run files, set `lists` to the file's path.
- **Mvar flow direction on the one-line.** Each branch shows an open violet
  arrow for the way its reactive power flows, beside the filled MW arrow;
  the Mvar number beside it is violet too, and now a magnitude, like MW.
- **Animated flow arrows.** **Canvas > Animated flow arrows** (also under
  **View** and in Ctrl+K) moves the MW and Mvar arrows along the wires, each
  the way its flow goes and in its own colour, faster with more flow. Off by default; exports and reduced motion
  keep still arrows.
- **N-2 and bus outages.** Contingency runs can take the chosen branches
  and generators out two at a time (every pair once, up to 250,000 pairs;
  the bus filters narrow them), and can take out each bus with every
  branch and DC line connected to it. Any elements can also be listed to go
  out together (`line:1:2:1&line:2:3:1`), and Trip and compare takes a bus.
- **Skip zero-impedance lines in contingencies.** A toggle leaves out lines
  with no resistance and a reactance at or below a threshold (0.0001 p.u.
  by default, PSS/E's THRSHZ) from N-1, N-2 and planning event runs.
- **Ratings B and C in contingency analysis.** Contingency runs and Trip
  and compare measure loading against rate A, B or C, chosen separately for
  the base case and after each outage (for example rate A before, rate B
  after). Both stay rate A by default. The choice is saved in the study file
  and is open to run files, Python (`rating=`, `base_rating=`) and the MCP
  server.

### Changed

- **Study files are retired; save work as a run file, and keep a history.**
  **Save study** is now **Save as run file**: a `.stnrun` that opens the
  case, attaches its DC grid and bus coordinates files, sets the study edits
  and solves with the app's settings. It leaves out saved solves (study
  files kept every one as a full report) and the drawing, which has its own
  file. Older `.stannum` files still open, with their edits, saved solves
  and drawing, but are no longer written. **Record a run file** is replaced
  by **History** in the log drawer (**File → History…**): every change and
  export is kept as you go, and any part of it can be copied or saved as a
  run file without starting anything first.

- **Solve keeps the current view.** Solving from the toolbar, the menu bar or
  Ctrl+K no longer switches to Power flow; the diagram and tables update in
  place. Power flow comes into view only when the solve fails or does not
  converge.
- **Final residual rounded to 0.001 MVA.** The Power flow overview shows the
  final residual to three decimals (for example 0.669 MVA) instead of six
  significant digits. Only the display changed; the solver and the per-unit
  value under it are unchanged.
- **Shorter sensitivity values.** Flow sensitivities show three decimals
  (factors below 0.001 show as 0.000). Voltage and angle sensitivities, which
  are small by nature, show three significant digits (0.000149 p.u./MW).
  Hover a value for the full number. The VSC grids "Converters settled to"
  figure shows three decimals of MVA, like the residual.

## 1.0.0-rc.1 - 2026-10-30

### Added

- **Stannum for AI assistants (MCP, preview).** `stannum-cli mcp` serves
  Stannum to Claude and other Model Context Protocol clients: open a case,
  solve, find violations, page through results, run N-1 or the NERC planning
  events, look at topology, compare RAW files and export, by asking in plain
  words. `--attach` drives the running app instead, so the window shows
  every case and result, and the assistant can select elements and draw
  one-lines around them. See
  [AI assistants (MCP)](https://sklorgalore.github.io/StannumRevival/guide/ai-assistants/).

- **NERC TPL-001 planning events.** Contingencies can run the planning events
  Stannum generates from the case instead of every branch and generator: the
  base case (P0), each generator, line, transformer, shunt and DC line pole
  (P1.1 to P1.5), each section of a multi-section line (P2.1) and each
  bipolar DC line (P7.2), at or above a voltage (100 kV by default). Choose
  **NERC TPL-001 planning events** under **Outages** in the Contingencies
  tab; results show each event's category, with counts and a filter per
  category. Scripts pass `planning` in the contingency settings, and
  `planning_event_counts` says how many events each category gives.
  Multi-section line groupings in RAW files are now read, so a planning event
  takes a multi-section line out whole.
- **Binary Stannum case files (`.stnb`, preview).** The same case as a
  `.stn` file, in columns (Apache Arrow tables in a ZIP archive): about a
  quarter of the size, and readable with pyarrow, pandas or polars with no
  Stannum code. Open one like any case; **Save case as** and **Export
  Stannum case** write one when the name ends in `.stnb`, as do
  `stannum-cli convert` and `export_stn`.
- **Stannum cases open faster than RAW.** A `.stn` or `.stnb` case is read
  straight into the power flow model, with large tables split across
  processor cores: at 12,000 buses, about 0.11 s for `.stnb` and 0.12 s for
  `.stn`, against 0.14 s for the same case as RAW.
  Warnings about a record point at its line in a `.stn` file, also for
  records that aren't equipment.
- **Stannum case files (`.stn`, preview).** An open text format for whole
  cases: one record per line with named fields, stable ids beside bus
  numbers, units in the field names, comments, and a fixed order so version
  control diffs show only what changed. Open a `.stn` file like any case;
  **Save case as** writes `.stn` or `.raw`; **Export Stannum case** (Power
  flow's side panel, **Ctrl+K**) writes any case with its edits;
  `stannum-cli convert case.raw case.stn` converts from the command line,
  and `export_stn` does it in run files and Python. RAW 33 and 35 cases
  convert both ways exactly. The format is a preview and may change before
  its 1.0; see the Case file page in the docs. Lists files (`.stnl`) for
  subsystems, contingencies, monitors and interfaces are defined and checked,
  and will be applied by the contingency module in the next preview.

### Changed

- **Newton line search.** Each Newton step is checked before it's taken and
  shortened while it would leave the mismatch larger than the last few
  iterations did, so solves near the edge of what the network can carry
  don't overshoot and wander. Ordinary solves are unchanged.
- **Generators return from a reactive limit.** A generator bus fixed at a
  reactive limit holds its voltage again when that voltage moves past the
  setpoint on the side the limit pushes (at Qmax and above the setpoint, or
  at Qmin and below), as PSS/E does. Before, only generators holding a
  remote bus did, so a solve could end with units at a limit on the wrong
  side of their setpoints.
- **Synthetic cases are written as Stannum cases (`.stn`).** MATPOWER has no
  column for the bus a generator regulates, so the `.m` file made every
  synthetic unit hold its own terminals instead of its plant's high-voltage
  bus, and the 80,000-bus case no longer solved with reactive limits on. The
  large-case timing test keeps its case as `.stnb` and solves with reactive
  limits on again.
- **The Records tab and the topology open much faster on large RAW and
  Stannum cases.** Records are read on every core, in parts, and no longer
  copy their field layout one by one; the topology reader decodes its
  records on every core too; and the app and the command line use a faster
  memory allocator (mimalloc). On an 80,000-bus Stannum case, Records
  opens in about 3 s instead of 12–18 s, the topology in about 3 s instead
  of 4–8 s, and exporting it takes 5–6 s instead of 14–18 s.
- **Map places for missing buses come from line lengths.** A bus the
  coordinates file and the case leave out is now put where its lines'
  lengths say: the RAW `LEN`, or the line's reactance where the case gives
  none, scaled to kilometres by the lines whose ends are placed. A bus
  behind a transformer sits exactly on its placed side. Hiding half the
  substations of a synthetic 2,000-bus case, the estimates land 5.5 km from
  the true places on average, against 17.8 km for neighbour averages. Too few placed lines
  to scale by, and buses go at their neighbours' average as before.
- **The desktop app only reads and writes files you choose.** Opening,
  saving and exporting accept files picked in a dialog, dropped on the
  window, found next to an opened study, or chosen in an earlier session
  (so Recent files keep working); anything else is refused. The window also
  has a content security policy now, and web links open through Tauri's
  opener plugin, limited to Stannum's releases page. Scripts, run files and
  the command line are unchanged.

- **Enforce generator Q limits** is on by default in the app, run files
  and Python, as real cases are solved in PSS/E and PowerWorld. With it off,
  SyntheticUSA showed a 25 Mvar unit pushing 3,500 Mvar and a 1,584% branch;
  with it on, the case matches its saved solution. Run files and scripts
  that leave `enforce_q_limits` out now get Q limits; set it to `false` for
  the old behaviour. Large cases take a few seconds longer to solve.

### Fixed

- A sample case a script opened while attached (`open_sample`) now shows in
  the window; before, the window kept saying no case was open.
- RAW revision 35 cases no longer warn that ordinary generators are wind
  machines: the wind-machine flag was read two columns early (from F4).
- The version at the foot of the module rail keeps a pre-release suffix
  (v1.0-rc.1), so a release candidate never passes for the release.
- Sensitivities work on 80,000-bus cases. They used to hold every injection
  column at once, over 13 GB at 82,000 buses, and the app was shut down by
  the system; now the case is factored once and the picked column is solved
  when asked (about 3 s and 0.5 GB for SyntheticUSA).
- A solve that starts from the last solution and doesn't converge tries
  again from the case voltages. Ticking **Enforce generator Q limits** after
  a solve without them used to stop at the iteration limit on SyntheticUSA.
- A three-winding transformer's winding loading is measured at its own bus,
  not at the internal star bus, in results, checks and contingencies.
- RAW transformers with a 1.0 ratio and no phase shift show as transformers
  in Branches and in network drawings, not as lines.
- Branches results show **Loading** next to **Status**, where it is in view.
- Sorting a table high to low keeps rows without a value (such as unrated
  branches' loading) at the bottom instead of the top.
- Case diff compares cases whose records repeat a key, such as
  SyntheticUSA's nine DC lines all named 'DC Line 1'. Repeats are numbered
  in file order ("#2" on) and matched by that number, with a note on each.
- Case diff reads RAW files that end with Q where a section would start, as
  SyntheticUSA does before INDUCTION MACHINE. The missing sections are read
  as empty, as PSS/E does; Q partway through a section is still an error.
- One-line labels clear the result halo and layer ring around a symbol.
- A MATPOWER bus picked on a one-line shows its **Drawing** fields again.
  Its load and shunt symbols stand for the same bus and are picked with it,
  and the inspector used to leave the drawing fields out.

## 0.5.0 - 2026-10-29

### Added

- **Updates in the app.** A little after it starts, Stannum checks GitHub
  for a newer release and offers to install it and restart; click the
  version at the foot of the module rail (or **Check for updates** in
  **Ctrl+K**) to check by hand. Updates are signed, and Stannum only
  installs one signed with the project's key. Turn the startup check off in
  **Ctrl+K**. Takes effect from the first release after this one.
- Reference pages for every Stannum file format: study files (`.stannum`),
  diagrams (`.sld`), run files (`.stnrun`) and bus coordinates, each with
  how its version number works. Studies, diagrams, run files and coordinates
  saved by 0.3.0 and 0.4.1 are kept in `tests/compat`, and the tests open
  them on every change.
- Run files and bus coordinates files can say which version of their format
  they follow with a first comment line (`# stannum-run-file version 1`,
  `# stannum-coordinates version 1`). Saved run files start with it. A file
  from a newer Stannum is refused with a message saying so, instead of being
  read wrongly; so is a diagram with a newer major `schemaVersion`.
- **Sample cases** to try on first run: with no case open, Power flow and
  Records offer MATPOWER's nine-bus case, two five-bus RAW cases and a
  synthetic 200-bus grid, the last three with bus coordinates for the map.
  Also in **Ctrl+K** ("Open sample"), and as `sample_cases` and
  `open_sample` in run files and scripts.
- **Crash handling**: a bug inside Stannum no longer takes the window down.
  The step that hit it fails with a notice and an error line in the log, and
  everything else keeps working. A view that fails while drawing can be drawn
  again on its own. Python and run files get an `internal` error instead of a
  crash.
- **Save diagnostics…** (log toolbar, crash notice and command palette)
  saves the log with Stannum's, rs-power's and nodal's versions and the
  platform, for bug reports. No case data. Also the `diagnostics` command.
- **Keyboard and screen reader pass.** Tables are one Tab stop: arrow keys,
  Page Up/Down and Home/End move between rows, Enter opens a row, Up from
  the first row reaches the column headers to sort, and **F2** edits a
  Records cell. Rows of tabs switch with the arrow keys, menus open and move
  with the arrow keys, and dialogs keep focus inside and give it back when
  they close. **Ctrl+1** to **4** show the modules (with Shift, side by
  side), the menu key or **Shift+F10** opens the diagram's case actions on
  the selected symbol, **?** lists every shortcut, and the first Tab offers
  **Skip to the module**. One teal focus ring everywhere, landmarks and
  labels for screen readers, darker small grey text in both themes, and
  less motion when the system asks for it.
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

- The macOS app is signed ad hoc, so a downloaded copy no longer says
  "Stannum.app is damaged and can't be opened". macOS still asks once
  before the first launch; [Download](https://sklorgalore.github.io/StannumRevival/start/download/)
  has the steps, and the Terminal command that opens older builds.
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
