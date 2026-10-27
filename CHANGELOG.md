# Changelog

All notable changes to Stannum are listed here. Each release's section is
also its release notes on GitHub. Add a line under **Unreleased** in the same
pull request as the change; `scripts/release/new-version.ps1` turns that
section into the next version's when a release is cut (see
[docs/releasing.md](docs/releasing.md)).

## Unreleased

### Added

- One result shown in every view: showing an outage on the diagram (from
  Contingencies or Trip and compare) switches the diagram's flow labels, the
  Power flow tables and the inspector to that outage's solution, and the
  case bar says which result is shown, with a menu to switch back.
- A Map drawing style: attach a bus coordinates CSV and the case's
  substations and branches are drawn at their latitude and longitude over
  country and state outlines that ship with the app and follow the theme.
  Buses the file leaves out are placed next to their neighbours.

### Changed

- Solves, sensitivities, "Trip and compare" and Records reads and edits no
  longer hold up the rest of the app: on a 30,000-bus case, Records opens
  in a fraction of a second during a 5 s solve instead of waiting for it.

### Fixed

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
