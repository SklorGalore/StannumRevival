# Nodal

A local application for AC topology analysis of PSS/E RAW v35 files, available in a browser or as a desktop app. Rust parses and analyzes the case inside a WebAssembly worker; TypeScript and Cytoscape display the results. File contents are never uploaded.

## Run in a browser

Requires Rust 1.85 or newer and Node.js 22.12 or newer.

```sh
rustup target add wasm32-unknown-unknown
npm install
npm run wasm
npm run dev
```

Open the localhost URL printed by Vite and choose a `.RAW` file. The two supplied cases, `MemphisCase2026_Mar7.RAW` and `Texas2k_series25_case1_summerpeak.RAW`, are acceptance fixtures. Case files are selected locally rather than bundled into the application.

For an optimized static build:

```sh
npm run build
npm run preview
```

`dist/` contains the application, including its WASM module. It needs a static HTTP server; opening `index.html` directly with `file://` does not support module workers. Run `npm run wasm` after changing Rust code; Vite handles TypeScript/CSS changes automatically.

## Desktop app

The Tauri desktop wrapper packages the same local interface and WebAssembly worker. Browser development and static builds remain available as described above. Desktop development and packaging use these commands:

```sh
npm install
rustup target add wasm32-unknown-unknown
npm run desktop:dev
npm run desktop:build
```

`npm run desktop:build` defaults `CI=true` on macOS to build the `.app` and DMG without opening Finder. App and volume icons remain included, but the DMG's custom Finder layout is skipped. Run `CI=false npm run desktop:build` to enable that layout. Windows environment settings are preserved. This default applies only to the npm wrapper; running `cargo tauri build` directly is unaffected.

The desktop wrapper uses the native save dialog for CSV and JSON exports. Imported case contents stay on the device and in memory; only files explicitly saved through an export are written to disk. RAW acceptance fixtures are not included in the app bundle.

The root parser and analysis library require Rust 1.85 or newer. The desktop save-dialog dependency currently requires Rust 1.90 or newer, so use Rust 1.90+ for desktop commands. Node.js 22.12 or newer and the `wasm32-unknown-unknown` Rust target are required on both platforms. Tauri's platform setup also requires [Xcode Command Line Tools on macOS](https://v2.tauri.app/start/prerequisites/#macos) and, on Windows, the [MSVC Rust toolchain, Microsoft C++ Build Tools with the Desktop development with C++ workload, and WebView2](https://v2.tauri.app/start/prerequisites/#windows). WebView2 is included with current Windows 10 and Windows 11 releases; install its Evergreen Runtime if it is missing.

Build on the platform you want to package. A macOS build produces the native executable in `src-tauri/target/release/`, the `.app` under `src-tauri/target/release/bundle/macos/`, and a DMG under `src-tauri/target/release/bundle/dmg/`. A Windows build produces the native `.exe` in `src-tauri/target/release/` and an NSIS setup executable under `src-tauri/target/release/bundle/nsis/`. Windows artifacts must be built and smoke-tested on Windows; macOS does not verify Windows packaging. Builds are unsigned and are not automatically notarized.

The macOS native executable, `.app` bundle, and DMG were generated successfully with `npm run desktop:build`. A native Tauri WKWebView harness loaded the bundled production frontend under its configured CSP with zero CSP violations. Using synthetic file input, it analyzed both RAW fixtures (993 buses, then 2,751 buses) and passed case replacement. The 73 automated tests also pass: 30 root Rust, 3 desktop Rust, 20 frontend unit, and 20 browser tests. Manual file chooser, drag-and-drop, save-dialog cancellation/error behavior, and Windows build/runtime checks remain unverified.

## Explore a case

- **Overview / Buses:** search bus numbers and names, inspect equipment, and open two-hop neighborhoods.
- **Islands:** inspect connected components. Graph selections above 500 nodes prompt for a smaller neighborhood or path; analysis and exports still cover the whole case.
- **Critical connections:** inspect bridges and articulation buses. Parallel circuits remain distinct and contribute separately to degree.
- **Equipment:** inspect active and excluded lines, transformers, and system switching devices, including original RAW fields and source lines.
- **Diagnostics:** review encoding information and sections not used for topology. Nonempty unsupported DC/FACTS/substation connectivity produces a coverage warning.
- **Shortest path:** find the fewest equipment hops between active buses. Traversing a three-winding transformer counts as one hop; synthetic junctions appear separately in the graph.
- **Filters & colors:** expand the panel above the graph to combine bus or equipment field rules and choose separate color fields. Numeric fields support comparisons, inclusive ranges, and configurable gradients; categorical fields support exact matches and editable palette colors. Missing values and merged nodes with mixed member values have separate colors.
- **Export CSV:** download every filtered row in the current results view, including rows beyond the current page. **Export JSON:** download the parsed case, analysis, and current path.

Field filters narrow the selected graph scope and the corresponding bus or equipment table; search combines with those rules. Filtering can bring a full-network or island view below the 500-node display limit. Islands, critical connections, and shortest paths continue to describe the full active topology; a notice identifies paths partly hidden by filters. Neighborhood CSV exports honor bus filters, and JSON reports include the current view, rules, color settings, and displayed graph alongside the full case and analysis.

Color fields and schemes are remembered locally, and edits update graph colors without resetting its viewport. Custom colors for names and circuit identifiers last only for the session so those source strings are not saved in browser storage. Case-specific filters clear when another file is opened. In merged mode, source-field rules must match together on at least one original member bus; mixed color values are shown explicitly.

The graph uses buses with `IDE != 4` and active equipment terminals. Open switches and out-of-service connections do not connect buses. Two-winding transformers are edges; three-winding transformers use a synthetic star junction. A reported bridge on a three-winding transformer is a specific winding connection, not an equipment outage study. Synthetic articulation junctions are included separately in JSON results, not counted as articulation buses.

RAW field positions and section order come from the supplied `raw.schema.json`. Section comments and `@!` headers are optional. UTF-8 is attempted first, with a Windows-1252 fallback. Quoted string content and original records are preserved; identifier padding is normalized for comparisons. Missing status fields default to in service, bus type to PQ, circuit/record identifiers to `1`, injection power to zero, and frequency to 60 Hz; required bus references, base voltage, and series reactance must be present. Defaults are explicit in the parser rather than inferred from example records.

Revision 35 is supported. RAWX, other revisions, DC connectivity, substation node-breaker topology, power-flow calculations, outage studies, centrality, and RAW editing/export are outside this version. Files exist only in memory; reload the page to clear the case. CSV text that could be interpreted as a spreadsheet formula is prefixed with an apostrophe.

## Verify

```sh
cargo test
npm test
npm run build
npm run test:browser
# Run the same browser checks against the production build:
NODAL_TEST_PRODUCTION=1 npm run test:browser
```

Browser tests use local Google Chrome on macOS when available. Otherwise install Playwright Chromium with `npx playwright install chromium`, or set `PLAYWRIGHT_CHROMIUM_EXECUTABLE` to an existing Chrome/Chromium executable. Tests cover both supplied cases, worker replacement, diagnostics, paths, graph selection, and exports.

## Rust API

```rust
let case = nodal::parse_raw(&bytes)?;
let analysis = nodal::analyze(&case);
let path = nodal::shortest_path(&case, from_bus, to_bus)?;
```

Parsing returns a diagnostic containing severity, section, physical line number, and message. An unreachable path returns `Ok(None)`; missing or inactive endpoint buses return an error. `topology::Topology` caches the graph for repeated analyses and paths. The browser worker owns a WASM `NodalCase` handle and frees it when another case is opened.
