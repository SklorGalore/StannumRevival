# Merged System Topology Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the zero-impedance checkbox select the topology used by every analysis, graph, path, table, and export, with the option available before import.

**Architecture:** Contract bus groups in Rust before constructing equipment adjacency and running analysis. Keep the original Case as source data, return representative nodes with explicit membership, and rebuild the worker's topology when the global option changes. The frontend renders and exports the returned analysis rather than independently applying zero-distance groups.

**Tech Stack:** Rust, rustworkx-core/petgraph, wasm-bindgen, TypeScript, Vite, Cytoscape, Vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-10-10-merged-system-topology-design.md`

## Global Constraints

- Rust 1.85+ and Node.js 22.12+; existing dependencies suffice.
- Group only active nontransformer lines with R = 0 and |X| <= THRSHZ; THRSHZ = 0 disables merging.
- Each merged node uses the smallest member bus number as its stable representative and retains a sorted list of all original bus numbers.
- Switching devices and transformers do not create groups.
- The existing 500-node display limit applies after merging; analysis and exports remain complete beyond that limit.
- Keep original RAW records on-device and preserve CSV formula neutralization and worker/WASM cleanup.
- Preserve existing uncommitted feature work and unrelated `docs/design-philosophy.md`. Leave changes reviewable in the working tree; do not publish or merge.

## Review Focus

- Multiple external circuits become parallel after contraction: none is individually a bridge.
- Transformer terminals map to repeated representatives: no false articulation or counted internal hop.
- Original member bus numbers remain valid path/search/inspection inputs.
- Rapid option changes or file replacements invalidate stale worker/path responses.
- Empty active topology and THRSHZ = 0 produce coherent metrics and exports in either mode.

---

### Task 1: Contract topology and return canonical identities

**Files:** Modify `src/topology.rs`; test `tests/topology.rs`. Existing THRSHZ parsing and source models are reused.

**Interfaces:**
- Add `TopologyOptions { merge_zero_impedance: bool }`, deriving Default with false.
- Keep `Topology::new(&Case)` unmerged; add `Topology::with_options(&Case, TopologyOptions) -> Self`.
- Add `GraphNode.member_buses: Vec<i32>`; synthetic nodes have an empty vector.
- Add `GraphData.merge_zero_impedance: bool`; retain zero_impedance_groups as source membership metadata.
- `Analysis.active_buses` counts canonical active nodes; add `original_active_buses: usize` and `absorbed_equipment: usize`. Excluded counts refer to original inactive/excluded records; active_equipment counts equipment that connects distinct effective nodes.
- Islands, degrees, articulation_buses, Path.buses and Path.node_ids use representative identities. `shortest_path(&self, &Case, from, to)` accepts any active original member number.

- [x] Write failing tests for a chain `1--zero--2--normal--3`: enabled graph has nodes `[1,3]`, members `[1,2]` and `[3]`, one connection, original_active_buses 3, active_buses 2, absorbed_equipment 1. Unmerged mode remains unchanged.
- [x] Write failing tests for representative 1 from an unordered group `[4,1,2]`, paths 2→4 having zero hops, and 2→3 resolving through the representative and external equipment.
- [x] Write failing tests for a zero-connected center whose contraction changes articulation/bridge results; compare results to a manually equivalent graph. Add parallel circuits created by contraction, an excluded member, THRSHZ = 0, and all-excluded cases.
- [x] Write failing tests for three-winding transformers resolving to three, two, or one distinct representative. Three distinct nodes retain the junction; two distinct nodes use a direct transformer edge; one node absorbs the equipment. Assert one-hop external paths and no false bridge from duplicate winding edges.
- [x] Run `cargo test --test topology`; confirm new tests fail for the missing contraction behavior.
- [x] Construct sorted representative mapping before equipment insertion. Deduplicate active transformer terminal representatives, remove internal equipment, preserve external circuit IDs, and reconstruct paths from effective graph terminals rather than original bus indices.
- [x] Run `cargo test` and `cargo fmt --check`; expect all tests passing and clean formatting.
- [x] Review Task 1 changes before proceeding; record the verification and any corrections in the execution ledger.

### Task 2: Make topology mode a worker operation

**Files:** Modify `src/wasm.rs`, `web/types.ts`, `web/worker.ts`, `web/engine.ts`; add `web/engine.test.ts` if an isolated worker protocol test is practical, otherwise cover protocol behavior in Task 3 browser tests.

**Interfaces:**
- WASM `NodalCase.set_merge_zero_impedance(enabled: bool)` rebuilds Topology from the retained Case. Existing snapshot serialization includes GraphData.merge_zero_impedance and Task 1 fields.
- `WorkerRequest` load adds `mergeZeroImpedance:boolean`; add `{ id:number; type:'topology'; mergeZeroImpedance:boolean }`. Both load and topology requests return the existing loaded snapshot response.
- `BrowserEngine.load(bytes:ArrayBuffer, mergeZeroImpedance=false):Promise<Snapshot>`; add `setMergeZeroImpedance(enabled:boolean):Promise<Snapshot>`.
- `GraphNode.member_buses:number[]` and new analysis/graph fields mirror Rust. Update test fixtures rather than weakening required production types.

- [x] Add or extend tests to prove load carries its selected option, topology requests reuse the retained case, and errors reject the corresponding request without hanging unrelated work.
- [x] Run the relevant test command and observe the new behavior failing before implementation.
- [x] Implement the bindings and worker request handling, preserving queued operations, request IDs, original-byte transfer, and worker disposal. The topology operation must not fetch, reopen, or retain a separate copy of RAW bytes.
- [x] Run `npm run wasm`, `npm test`, and `npx tsc --noEmit`; expect successful WASM generation, passing tests, and no type errors once fixtures are updated.
- [x] Review Task 2 interfaces and verify they match Task 1 and Task 3 requirements.

### Task 3: Use one global topology throughout the UI and exports

Execution note: Implementation and regression coverage are complete. The requested red-before-production sequence was not fully followed for this task; this procedure deviation is recorded in the execution report. Review identified and verified correction of a rebuild-control gap; final checks and whole-change review passed.

**Files:** Modify `web/main.ts`, `web/graph.ts`, `web/view-model.ts`, `web/style.css`, `web/view-model.test.ts`, `tests/browser/application.spec.ts`; add `web/topology-view.ts` and `web/topology-view.test.ts` for membership lookup, merged rows, member search, and inspection helpers.

**Interfaces:**
- `nodeForBus(graph:GraphData,bus:number):GraphNode|undefined` resolves representative or original membership.
- `busRows(snapshot:Snapshot):Record<string,unknown>[]` returns one row per canonical node plus excluded source buses; representative metadata comes from its original source bus. Include member_buses in merged-mode rows and map all analysis values through the representative.
- `neighborhood(graph:GraphData,bus:number,hops=2):GraphData` traverses the effective graph. Remove the independent fourth merge argument and zero-closure algorithm, since contraction is now owned by Rust.
- Global option defaults false, survives file replacement, and is passed to load/rebuild. Snapshot.graph.merge_zero_impedance is the authority for displayed results and exports.

- [ ] Write failing unit tests for member lookup, one row for members `[1,2]`, excluded rows, merged degree/articulation values, member-name search, and neighborhoods selected with a nonrepresentative member number.
- [ ] Write failing browser tests using a RAW case with GENERAL THRSHZ and transitive zero branches. Check the option before import and assert contracted node counts, initial graph mode, merged rows, member inspection, member-number path inputs, and zero-hop paths within a group.
- [x] Add browser assertions for Show full network, islands, changed critical results, CSV representative/member columns, JSON graph/option/member mapping, toggling back to original node counts, and persisted option on case replacement.
- [x] Extend stale-response browser coverage to delay topology replies and path replies: the latest option/file/selection must win. Assert controls are disabled while a topology rebuild is pending.
- [ ] Run `npm test` and the focused browser tests; observe feature failures before implementing UI changes.
- [x] Add a global checkbox to the import and loaded workspace, with an identical option reflected in the inspector. Implement a rebuild handler that invalidates paths, tracks request generations, remaps current view selection, and reports errors without mixing modes.
- [x] Track the selected graph view explicitly (`full`, `island`, `neighborhood`, `path`) so topology rebuilds restore the corresponding view or clear stale paths. Add Show full network using the existing post-contraction 500-node guard.
- [x] Use canonical member lookup for inspector actions and Cytoscape callbacks. Label merged nodes by representative and member count; list member names and provide individual RAW source inspection. Aggregate group equipment, loads, and generators without duplicating records.
- [x] Use shared merged rows for the bus table and neighborhood CSV; make member names/numbers searchable. Update metrics, island rows, absorbed-equipment status, graph titles, and JSON reports to reflect the applied topology. Include original Case plus graph, analysis, mode, and path in JSON.
- [x] Run `npm test`, `npm run build`, and `NODAL_TEST_PRODUCTION=1 npm run test:browser`; expect all tests passing, including unchanged unmerged fixture acceptance and mobile layout.
- [x] Review Task 3 changes for every graph/table/export path and correct material findings.

### Task 4: Verify integration and deliver the working tree

**Files:** Review all Task 1–3 files; update this plan and the execution ledger with completion evidence.

- [x] Run `cargo test`, `cargo fmt --check`, `npm test`, `npm run build`, `NODAL_TEST_PRODUCTION=1 npm run test:browser`, and `git diff --check` on the final implementation.
- [x] Request a whole-change review of contracted transformer behavior, stale response handling, and consistent original-to-representative mapping. Resolve material findings and rerun affected checks.
- [x] Confirm no generated WASM, dist, target, or browser reports are staged; preserve unrelated user changes.
- [x] Report the completed behavior, validation, and any remaining material limitation with links to the changed source files. Leave the implementation uncommitted for review.
