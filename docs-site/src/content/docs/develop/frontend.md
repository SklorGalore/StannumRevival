---
title: Frontend
description: How the React interface in src/ is organised - the shell, modules, stores, typed core calls, wire decoding, shared selection and styling.
---

The interface is React 19 and TypeScript, bundled by Vite, with Zustand for
state and Tailwind 4 for styling. The one-line editor uses React Flow
(`@xyflow/react`) with ELK for layout; the network view uses Cytoscape; the
map uses MapLibre. Entry points are `index.html` → `src/main.tsx` →
`App.tsx` → `shell/Shell.tsx`, plus `splash.html` for the splash window.

## Folders

| Folder | Holds |
| --- | --- |
| `shell/` | The frame around every module: `ModuleRail`, `CaseBar` (toolbar), `ContextBar` (shown result and selection), `CommandPalette` (Ctrl+K), `LogDock`, `RunFileBar`, `StudyFileControls`, the result and selection chips, `WindowControls` (Windows title bar), and side-by-side layout in `Shell.tsx`. |
| `modules/` | One folder per rail entry: `diagram/` (one-line, network and map styles, layers, tables), `power/` (power flow, sensitivities, contingencies, warnings, VSC grids), `records/`, `diff/`. `topology/` holds the network graph and field filter code the Diagram uses. |
| `lib/analysis/` | Everything that talks to the core: `api.ts`, the generated `commands.gen.ts`, `types.ts`, and decoders for column tables (`wire.ts`, `topologyWire.ts`). |
| `lib/` (other) | Pure logic with unit tests next to it: diagram generation and routing (`diagram/`), map model (`map/`), exports (`export/`), study files (`study/`), shared element ids (`elements.ts`), search (`search.ts`), the log model (`log/`). |
| `store/` | Zustand stores (below). |
| `components/` | The hand-drawn one-line editor from the original StannumSLD: canvas, node and edge symbols, palette, property forms, dialogs. |
| `ui/` | Shared building blocks: `DataTable` (virtualised, sortable, filterable, CSV), tabs and buttons (`primitives.tsx`), `ElementInspector`, `FilterChips`, `TripCompare`, `DrawAround`. |
| `hooks/` | `useKeyboardShortcuts` for the diagram editor. |

Keep logic out of components where you can: a function in `lib/` with a
`*.test.ts` beside it is easier to test than a component.

## Calling the core

All core calls go through `src/lib/analysis/api.ts`:

- `invokeCore(command, args)` sends one core command through the Tauri
  command `stannum_command`. Its argument and result types come from
  `commands.gen.ts`. A rejection becomes a `StannumError` with the core's
  error `kind`; test it with `isErrorKind(e, "cancelled")`.
- `api.*` are the wrappers the app uses: `api.openCase`, `api.solve`,
  `api.runContingencies` and so on. They go through `call()`, which shows
  the step in the log strip with the label from `LOG_LABELS` and pulls the
  core's log lines once it finishes.
- `watchCoreCalls` is told about every core command that worked; the history
  uses it to keep the steps of a run file.

Results that arrive as columns are decoded here before anything else sees
them: `decodeSolveReport` and `fromColumns` in `wire.ts`, and
`decodeTopology` in `topologyWire.ts`. Components work with rows.

Never call `invoke("…")` for analysis directly. If the core has no command
for what you need, [add one](/StannumRevival/develop/add-a-command/).

## Stores

Each store is a Zustand `create(...)` hook. Components select the slices
they need (`useWorkspace((s) => s.caseSummary)`); actions outside React use
`useWorkspace.getState()`.

| Store | Holds |
| --- | --- |
| `workspaceStore` | The open case's summary and version, the module shown and the side-by-side pane, the latest solve report, the *shown result* (an outage or trip shown in place of the latest solve), the shared selection, and requests other modules act on (`requestSolve`, `drawAround`, `focus`). |
| `studyStore` | The study's edits; the core applies them on every solve. |
| `contingencyStore` | What the contingency tab shows: settings, summary, the page of ranked rows, the open outage. |
| `drawingStyleStore`, `layerStore`, `diagramStore` | The Diagram's style and neighbourhood, its topology layers and their marks, and the editor's nodes, edges and undo history. |
| `logStore` | Log lines from the core (`stannum://log`) and the app's own steps. |
| `studyFileStore`, `runFileStore`, `historyStore` | Save as run file and opening older `.stannum` study files; playing `.stnrun` run files; the history of actions, saved or copied as run files. |
| `attachStore` | Answers a Python script's `ui.*` commands and refreshes after a script changes the case. |
| `themeStore` | Light, dark or system. |

The core is the source of truth for the case, its study and its results.
The stores hold what is on screen and refresh from the core when
`caseVersion` changes.

## Modules and the rail

Modules are listed in `MODULES` (`shell/ModuleRail.tsx`) with `ModuleId`
in `workspaceStore.ts`, and mounted in `Shell.tsx`. Every module stays
mounted when it isn't shown, so its state survives switching: hidden
modules get the `module-hidden` class, which hides every descendant
(React Flow sets its own `visibility`, hence the `!important`). Side by
side shows two modules at once; `SPLIT_PARTNER` names the default partner.

To add a module: add its id to `ModuleId` and `SPLIT_PARTNER`, an entry to
`MODULES`, a "Go to" entry to `MODULE_COMMANDS` in `CommandPalette.tsx`,
and mount it in `Shell.tsx`. Put a heavy library behind `React.lazy`, as
`modules/topology/LazyNetworkGraph.tsx` does for Cytoscape, and give it a
chunk in `vite.config.ts` (`codeSplitting.groups`).

## Shared selection

An element picked in one module is selected in all of them. Ids are the
core's shared element ids (`bus:101`, `line:101:102:1`, see
`lib/elements.ts`, generated from `elements::KINDS`). A module publishes a
pick with `select(element, from)` and follows `selection` from the store;
`modules/diagram/useSharedSelection.ts` is the Diagram's version.
`select` only reaches the other modules while **Linked** is on; `focus`
shows an element everywhere regardless (a Ctrl+K result).

## The command palette

`paletteCommands()` in `shell/CommandPalette.tsx` builds the list Ctrl+K
offers from the current state. Each entry is `{ id, title, icon, keywords,
run }`. The same ids are what a script runs with `app.run(id)` over attach,
so keep an entry's `id` stable once it ships.

## Styling and themes

Tailwind utility classes, with a few global rules in `src/index.css`. Dark
mode doesn't use `dark:` variants: under `:root.dark` the slate and teal
scales are flipped (50 ↔ 950 and so on), so `bg-white text-slate-800`
reads correctly in both themes. `force-light` keeps the light palette
inside a dark app (the rail, the diagram's print style). Icons come from
`lucide-react`.

## Tests

Vitest unit tests sit next to the code as `*.test.ts` (`npm test`).
`lib/analysis/__fixtures__/topology-wire.json` is written by a Rust test
(`STANNUM_WRITE_FIXTURES=1 cargo test -p stannum-core --test topology_wire`),
so the TypeScript decoder is tested against real core output. The browser smoke
test covers the whole interface; see
[Tests and CI](/StannumRevival/develop/testing/).
