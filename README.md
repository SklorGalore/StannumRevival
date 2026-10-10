# Nodal

A local browser application for AC topology analysis of PSS/E RAW v35 files. Rust parses and analyzes the case inside a WebAssembly worker; TypeScript and Cytoscape display the results. File contents are never uploaded.

## Run locally

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

## Explore a case

- **Overview / Buses:** search bus numbers and names, inspect equipment, and open two-hop neighborhoods.
- **Islands:** inspect connected components. Graph selections above 500 nodes prompt for a smaller neighborhood or path; analysis and exports still cover the whole case.
- **Critical connections:** inspect bridges and articulation buses. Parallel circuits remain distinct and contribute separately to degree.
- **Equipment:** inspect active and excluded lines, transformers, and system switching devices, including original RAW fields and source lines.
- **Diagnostics:** review encoding information and sections not used for topology. Nonempty unsupported DC/FACTS/substation connectivity produces a coverage warning.
- **Shortest path:** find the fewest equipment hops between active buses. Traversing a three-winding transformer counts as one hop; synthetic junctions appear separately in the graph.
- **Export CSV:** download every filtered row in the current results view, including rows beyond the current page. **Export JSON:** download the parsed case, analysis, and current path.

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
