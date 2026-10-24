# nodal

PSS/E RAW v35 parsing and AC network topology analysis for Stannum: islands,
bridges, articulation buses and shortest paths, built on `rustworkx-core`.
Stannum's Topology module (`crates/stannum-core/src/workspace.rs` and
`src/modules/topology/`) is the user interface.

nodal started as its own repository with a TypeScript/Cytoscape browser app,
a WASM build and a Tauri wrapper. Only the Rust library moved into Stannum;
the old frontend is still in this repository's history under `crates/nodal/web/`
(for example `git log --all -- crates/nodal/web/field-view.ts`).

## Rust API

```rust
let case = nodal::parse_raw(&bytes)?;
let analysis = nodal::analyze(&case);
let path = nodal::shortest_path(&case, from_bus, to_bus)?;
```

Parsing returns a diagnostic containing severity, section, physical line number, and message. An unreachable path returns `Ok(None)`; missing or inactive endpoint buses return an error. `topology::Topology` caches the graph for repeated analyses and paths.

## Topology rules

The graph uses buses with `IDE != 4` and active equipment terminals. Open switches and out-of-service connections do not connect buses. Two-winding transformers are edges; three-winding transformers use a synthetic star junction. A reported bridge on a three-winding transformer is a specific winding connection, not an equipment outage study. Synthetic articulation junctions are reported separately, not counted as articulation buses. Parallel circuits remain distinct and contribute separately to degree. Shortest paths count the fewest equipment hops between active buses; traversing a three-winding transformer counts as one hop.

## Parsing rules

RAW field positions and section order come from the bundled `raw.schema.json`. Section comments and `@!` headers are optional. UTF-8 is attempted first, with a Windows-1252 fallback. Quoted string content and original records are preserved; identifier padding is normalized for comparisons. Missing status fields default to in service, bus type to PQ, circuit/record identifiers to `1`, injection power to zero, and frequency to 60 Hz; required bus references, base voltage, and series reactance must be present. Defaults are explicit in the parser rather than inferred from example records.

Revision 35 is supported. RAWX, other revisions, DC connectivity, substation node-breaker topology, power-flow calculations, outage studies and RAW editing/export are outside this crate.

## Verify

```sh
cargo test -p nodal
```

The tests parse and analyze both supplied acceptance cases, `MemphisCase2026_Mar7.RAW` and `Texas2k_series25_case1_summerpeak.RAW`.
