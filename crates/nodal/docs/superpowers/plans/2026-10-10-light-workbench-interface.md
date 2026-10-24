# Light Workbench Interface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the approved light, graph-first engineering workbench to Nodal while preserving topology analysis, source inspection, local imports, and exports.

**Architecture:** Keep Rust, WASM, and the worker protocol unchanged. Reorganize the existing TypeScript-rendered workspace so graph and results share a left column alongside the inspector; use CSS tokens and a small graph-presentation module for consistent visual semantics. Maintain selection independently of graph scope and synchronize its presentation without reconstructing the graph for ordinary inspection.

**Tech Stack:** TypeScript, CSS Grid, Cytoscape, Vite, Vitest, Playwright; existing dependencies only.

**Spec:** `docs/design-philosophy.md`. Visual reference: `.superpowers/brainstorm/20512-1791661353/content/workbench-layout.html`, selected `graph-first` concept only.

## Global Constraints

- “Nodal is a technical workbench for experienced power-system engineers.”
- “Its primary theme is light: a calm, precise workspace for sustained investigation of electrical network topology.”
- “The focused network graph leads the analysis area. The searchable results table sits directly beneath it, and a persistent inspector occupies the right column.”
- “Place path controls below the analysis area, with explicit endpoint identifiers and an equipment-hop result.”
- “On narrower screens, stack the graph, inspector, and results in a readable order.”
- “A topology bridge is a connectivity finding, not an outage consequence.”
- “Density must not require smaller text to remain usable.”
- Rust 1.85+ and Node.js 22.12+; preserve both supplied RAW fixtures and existing 500-node graph limit.
- Retain global zero-impedance mode, representative identities, member search, source records, stale-response checks, and busy-state controls.
- Keep case contents on-device and in memory; preserve CSV formula neutralization and worker/WASM resource cleanup.
- The reference contains illustrative data; do not copy its sample metrics, network, or scaled dimensions into production.
- Do not commit generated `web/wasm/`, `dist/`, `target/`, or test reports. Preserve the existing untracked design document.
- This work excludes dark mode, new analysis algorithms, sorting, resizable panels, and a new export menu. Keep the current explicit CSV/JSON actions.

## Review Focus

- Long filenames, identifiers, member lists, and RAW values must wrap or scroll within their surface without widening the page (Task 2).
- A bus or equipment item that is selected, on a path, and a topology finding must retain evidence of every state (Task 3).
- Inspection from a filtered/paginated table must preserve graph scope, zoom, search, pagination, and path endpoints (Task 4).
- A selection absent from the displayed graph, including excluded source records and merged members, must remain inspectable without a false graph highlight (Task 4).
- At 390 px width and 200% desktop zoom, graph, inspector, results, and path controls must remain readable and keyboard accessible (Task 5).

---

## File responsibilities and task dependencies

| File | Responsibility | Tasks |
| --- | --- | --- |
| `web/style.css` | Theme tokens, compact shell, typography, responsive layout, interaction states | 1, 2, 4, 5 |
| `web/main.ts` | Workspace markup, selection state, inspector/table synchronization, explanatory copy | 1, 2, 4 |
| `web/icons.ts` (new) | Small local SVG icon set with decorative, hidden-from-accessibility markup | 1 |
| `web/graph-presentation.ts` (new) | Pure graph-state classes, style rules, and displayed legend entries | 3 |
| `web/graph-presentation.test.ts` (new) | Graph visual semantics and overlapping-state tests | 3 |
| `web/graph.ts` | Cytoscape lifecycle, selection synchronization, zoom label visibility | 3, 4 |
| `tests/browser/workbench.spec.ts` (new) | Layout, accessibility, context retention, and visual review coverage | 1–5 |
| `tests/browser/application.spec.ts` | Existing behavior acceptance; adjust only selectors affected by intentional markup changes | 1, 2, 5 |

Execute Tasks 1–5 in order. Main/CSS edits overlap, so do not run those tasks concurrently. Under repository collaboration instructions, Sol scopes and reviews; Luna implements each assigned task with explicit ownership. Task 3's pure module may be delegated independently after Task 1's tokens are settled, but integrate before Task 4.

## Implementation decisions

- Start with workspace `#F3F5F7`, surface `#FFFFFF`, navigation `#142B3C`, selection/action `#0F766E`, path `#2563EB`, finding text `#986315`. Use primary text `#233C50`, secondary text `#526577`, border `#D8E0E6`, and red error text `#B42318`. Check contrast before retaining these values.
- Use system sans serif; 14 px controls/data, 12 px secondary copy, 18 px section titles, 24 px case title. Use tabular numerals for measurements and monospace for identifiers/RAW fields. No external font requests.
- Desktop: 200 px navigation rail; workspace gap/padding 16–24 px; inspector 320 px; left column `minmax(0, 1fr)`; graph starts at 400 px high. These are initial CSS values, adjustable during actual visual review.
- At widths below 1100 px, move navigation into a compact horizontal region. Below 900 px, use one analysis column in graph → inspector → results order. Tables scroll inside their own surface; page content must not scroll horizontally.
- Keep existing button names and test IDs where behavior is unchanged. Use “Open case” as the visible header label while retaining file input accessible name “Open RAW file” and import button “Choose RAW file”.
- Keep ordinary network geometry neutral; selected elements use teal outlines, paths use blue strokes, findings use amber markers/underlays, and junctions remain diamonds. Hover and keyboard focus use separate visible treatments.

### Task 1: Establish the readable light theme and coherent controls

**Files:** Modify `web/style.css`, `web/main.ts`; create `web/icons.ts`, `tests/browser/workbench.spec.ts`.

**Interfaces:**
- Produce `icon(name: IconName): string` in `web/icons.ts`; export `IconName` union for `overview`, `islands`, `bus`, `critical`, `equipment`, `diagnostics`, `open`, `search`, `trace`. Return fixed inline SVG with `aria-hidden="true"`, `focusable="false"`, and `currentColor`; no user input interpolation.
- CSS tokens: `--workspace`, `--surface`, `--navigation`, `--text`, `--text-secondary`, `--border`, `--selection`, `--path`, `--finding`, `--error`. Graph presentation uses the same pinned palette in Task 3.

- [ ] Add browser test `workbench uses readable data and control typography`: import an existing fixture; assert computed font size is at least 14 px for table cells, inputs, and primary buttons; secondary copy is at least 12 px; active navigation has `aria-current="page"`. Assert selected theme surfaces match the pinned initial palette.
- [ ] Run `npm run test:browser -- tests/browser/workbench.spec.ts -g 'readable'`; expect failures on current small typography/missing active semantics.
- [ ] Define CSS tokens and replace scattered tiny label/control/data sizes; set consistent 34 px minimum control height, restrained 4 px radii, visible focus outlines, and enabled/disabled/hover states. Format touched CSS into readable rules so later changes are reviewable.
- [ ] Implement `icon` and replace decorative Unicode navigation, search, import, and equipment glyphs with the local SVG set. Preserve textual action labels and the existing logo. Apply `aria-current` to active navigation and explicit text for equipment status rather than circle-only indicators.
- [ ] Run the focused browser test and `npm run build:web`; expect passes. Manually check text contrast: normal text ≥4.5:1; meaningful non-text controls/focus ≥3:1 against adjacent surfaces. Record measured color pairs in the final validation notes.
- [ ] Review the changed theme and markup, then commit only Task 1 files with subject `Establish light workbench theme and controls`.

### Task 2: Build the graph-first workspace and compact case header

**Files:** Modify `web/main.ts`, `web/style.css`, `tests/browser/workbench.spec.ts`; adjust `tests/browser/application.spec.ts` only if required.

**Interfaces:**
- Preserve `#network-canvas`, `#inspector`, `#table-container`, `#path-form`, `#result-search`, and current data test IDs.
- Produce one `.analysis-workspace` grid containing `.network-panel`, `.results-panel`, and `#inspector` as grid items. Desktop areas put results directly under graph and inspector across both rows; narrow layout uses graph, inspector, results. Path panel follows this grid in DOM order.

- [ ] Add browser test `graph leads results beside a persistent inspector`: at 1440×1000, assert results top is below graph bottom, results and graph have equal left edge/width, inspector lies to their right, and path panel lies below the analysis grid. At 800 px, assert graph → inspector → results → path vertical order.
- [ ] Add test `long case and source values stay within the workspace`: import a case with a long filename and long bus/source values; inspect it and open RAW fields; assert document scroll width ≤ viewport width and internal table overflow remains accessible.
- [ ] Run `npm run test:browser -- tests/browser/workbench.spec.ts -g 'graph leads|long case'`; expect layout failures before markup changes.
- [ ] Consolidate loaded case name, metadata, export buttons, and case-opening access into a compact header. Keep global topology checkbox, case notes, diagnostics warning, and source/analysis distinctions visible. Change metric cards into a compact summary strip, preserving their existing values/test IDs and source/excluded/absorbed descriptions.
- [ ] Reorganize the analysis grid and move results above path controls. Give inspector content its own desktop scroll region with a visible heading; allow natural height when stacked. Use `min-width:0`, wrapping identifiers, and internal table overflow. Make case-opening access available through the existing header file input rather than adding a second import mechanism.
- [ ] Run focused workbench tests and the existing application suite; expect passes. Inspect 1440×1000 and 1024×768 screenshots for useful graph area, compact header, and inspector reachability.
- [ ] Review layout changes and commit with subject `Arrange graph-first analysis workspace`.

### Task 3: Preserve overlapping graph semantics and show a truthful legend

**Files:** Create `web/graph-presentation.ts`, `web/graph-presentation.test.ts`; modify `web/graph.ts`, `web/main.ts`, `tests/browser/workbench.spec.ts`.

**Interfaces:**
- Export `GraphSelection = {bus:number|null; equipment:string|null}` and `LegendEntry = {key:'bus'|'junction'|'selected'|'path'|'bridge'|'articulation'; label:string}`.
- Export `graphLegend(graph:GraphData, snapshot:Snapshot, path:Path|null, selection:GraphSelection):LegendEntry[]` and `graphStyles(): cytoscape.Stylesheet[]` (use the installed Cytoscape type declaration's exact stylesheet type).
- Extend `NetworkGraph` with `setSelection(selection:GraphSelection):void`. Resolve bus members through `nodeForBus`; equipment selection applies to all displayed connections and its synthetic junction.
- Legend copy: “Bus”, “Transformer junction”, “Selected”, “Path”, “Topology bridge”, “Articulation bus”. Only show entries whose state is present in the displayed graph.

- [ ] Add unit test `legend reflects only displayed states`: a subgraph excluding all bridge edges must omit `bridge`; junctions, paths, articulation, and selection appear only when matching elements exist.
- [ ] Add unit test `path and findings remain visible under selection`: assert compound Cytoscape style rules retain blue path strokes, teal selection outlines, and amber finding markers/underlays for overlapping node and edge classes. Assert junction shape remains diamond.
- [ ] Run `npm test -- web/graph-presentation.test.ts`; expect failure before the module exists.
- [ ] Implement the pure legend/style module and use it from graph rendering. Use Cytoscape underlays for bridge/articulation finding context, blue path strokes, teal selection outlines, and separate hover treatment; add compound selectors where needed. Inspect actual renderer output rather than assuming CSS-like properties work.
- [ ] Make labels zoom-sensitive: retain labels for selected/path/finding elements; hide ordinary labels below zoom 0.6 and show them above it. Register zoom/hover handlers only on the current Core and release them through `destroy()`.
- [ ] Render matching line/shape legend swatches, not only colored dots. Add concise visible explanation: “Topology bridges indicate connectivity, not outage consequences. Three-winding transformer junctions are synthetic; a transformer counts as one equipment hop.” Keep coverage warnings associated with the current results.
- [ ] Run unit tests and `npm run build:web`; capture and inspect a graph containing a selected path bridge, articulation node, and transformer junction. Confirm each state survives overlap and the legend matches visible elements.
- [ ] Review and commit with subject `Clarify graph selection paths and topology findings`.

### Task 4: Synchronize inspection while retaining investigation context

**Files:** Modify `web/main.ts`, `web/graph.ts`, `web/style.css`, `tests/browser/workbench.spec.ts`.

**Interfaces:**
- Consume Task 3 `GraphSelection` and `NetworkGraph.setSelection`.
- Add `selectInspection(selection:GraphSelection):void` in `web/main.ts`: update existing selectedBus/selectedEquipment, inspector HTML, row selection attributes/classes, graph selection, and legend; do not call full `render()` or change graph scope.
- Add `renderSelection():void` for the current table DOM and graph. Selected table rows use `data-selected="true"` and an accessible textual indication “Selected”; action buttons remain keyboard-operable.

- [ ] Add browser test `inspection preserves investigation context`: search and paginate a fixture, enter endpoints, set a neighborhood scope, then inspect a visible row by keyboard. Assert search, page, endpoint values, graph title, and node count stay unchanged; inspector and row selection update. Use a development-only direct graph unit test or visual manual check to confirm pan/zoom is unchanged without exposing Cytoscape on `window`.
- [ ] Add browser test `source and off-graph selections remain inspectable`: inspect an excluded bus; verify source fields and disabled neighborhood action. In merged mode search for a member name, inspect its representative, open an individual source bus, and return to the merged node. Assert no extra graph node appears.
- [ ] Run the focused browser tests; confirm context/selection failures against current behavior before implementation.
- [ ] Route graph taps, table inspect actions, and equipment-terminal inspection through `selectInspection`. Keep source-record inspection explicit and preserve its return-to-node action. Synchronize visual selection after table rerenders, full graph renders, and topology remapping; do not implicitly change graph scope on inspection.
- [ ] Add selected row background/leading marker and visible “Selected” text without replacing active/excluded status. Keep hover/focus distinct. Use `setSelection` to clear highlights when the selection is absent from the displayed graph.
- [ ] Preserve existing generation and rebuild protections: source selection must not resurrect old analysis after file replacement; topology busy must still disable inspection and graph actions. Full renders caused by loading/rebuilding may recreate Cytoscape; ordinary inspection must not.
- [ ] Run `npm test`, focused workbench browser tests, and stale-response/merged-topology tests in `application.spec.ts`; expect passes.
- [ ] Review and commit with subject `Preserve context across graph and result inspection`.

### Task 5: Validate responsive, keyboard, and production behavior

**Files:** Modify `tests/browser/workbench.spec.ts`, `web/style.css`, and touched frontend files only for findings; update this plan with evidence.

**Interfaces:** Consume the complete Tasks 1–4 UI; introduce no new product behavior.

- [ ] Add test `small workbench retains readable order and controls`: at 390×844, import a case, inspect a bus, search, trace a path, and export; assert graph → inspector → results → path order, no page horizontal overflow, and usable internal table scrolling.
- [ ] Add test `keyboard inspection and controls retain visible focus`: use Tab/Enter to open the chooser, activate navigation, inspect a result, expand RAW fields, and submit path endpoints; assert visible focus outline, labeled inputs, and no keyboard trap. Verify focus survives any DOM replacement triggered by the action; restore focus to the equivalent control when necessary.
- [ ] Add test `reduced motion retains loading feedback`: emulate reduced motion and verify textual loading/rebuilding states remain visible while animations are disabled.
- [ ] Run focused tests, then manually review desktop at 200% browser zoom, mobile touch targets, long RAW fields, empty/no-match tables, no active topology, parse errors, coverage warnings, and the >500-node display guard. Inspect graph keyboard accessibility: all graph inspection and scope actions must also be reachable through tables/inspector controls.
- [ ] Capture desktop loaded, import, narrow loaded, merged inspector, and overlapping graph-state screenshots using Playwright screenshots in ignored test artifacts. Review against the selected concept; do not introduce the mockup's illustrative data.
- [ ] Run `cargo test`, `cargo fmt --check`, `npm test`, `npm run build`, `NODAL_TEST_PRODUCTION=1 npm run test:browser`, and `git diff --check`; expect all pass. Reuse the complete existing application suite to verify imports, exports, formula neutralization, global topology behavior, and stale-response handling.
- [ ] Request whole-change review focused on spec coverage, graph-state overlaps, preserved selection context, accessibility, and resource cleanup. Resolve material findings and rerun affected checks.
- [ ] Record commands/results, contrast measurements, reviewed screenshot locations, and any limitations here. Check `git status --short` for generated artifacts before the final commit `Verify light workbench interface`.

## Design coverage and plan self-review

| Design requirement | Owning task |
| --- | --- |
| Light surfaces, readable density, coherent icons, focus and numeric typography | 1 |
| Graph first, results beneath, persistent inspector, compact metadata/summary, path below | 2 |
| Semantic graph colors, synthetic junctions, zoom labels, truthful legend, precise claims | 3 |
| Source evidence, synchronized selection, stable scope/search/page/endpoints, merged members | 4 |
| Narrow layout, keyboard access, contrast, diagnostic/empty states, production acceptance | 5 |

The selected design already establishes the direction; this plan fills in implementation values without reopening that decision. It is one frontend workstream with shared state and layout dependencies, so separate subsystem plans would add coordination without independent deliverables. Function names and selection types are shared explicitly across Tasks 3–4. Each Review Focus condition has an owning test or visual check. No implementation changes or application test runs are claimed by writing this plan.

## Execution status

Plan only. All implementation tasks remain unchecked. Repository instructions already specify Sol orchestration and Luna implementation; retain that execution method if implementation is requested. Review this plan before starting code changes.
