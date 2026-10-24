# Implementation ledger — docs/implementation-plan.md

- Approved spec: the proposed plan in the conversation; implementation plan saved above.
- Ruling: work in the user's existing checkout without commits or branch changes, preserving supplied untracked fixtures and schema. The user requested implementation in this workspace; no integration or publication is needed.
- Parser/graph/worker share serializable equipment identities, source diagnostics, and active-terminal definitions.
- Parser complete: five tests passed, including both fixture counts and Windows-1252 decoding.
- Graph engine complete: six tests passed, including multigraph bridges, inactive equipment, and equipment-hop transformer paths.
- WASM worker and browser UI implemented; browser import/inspection/path/export/replacement tests pass with both supplied files.
- Ruling: rustworkx cut algorithms use a simple projection whose edge weights preserve circuit multiplicity; bridges with multiplicity >1 are filtered. This respects the upstream simple-graph precondition while retaining multigraph degree and identity.
- Ruling: graph neighborhoods count equipment hops, consistently with shortest paths; three-winding connections include their synthetic junctions.
- Independent read-only review completed: no Critical issues; three Important findings reproduced and fixed with regression tests (numeric-circuit transformer truncation, premature Q, stale path replacing a newer graph selection).
- Mobile overflow reproduced and fixed: the visually hidden table heading needed a containing block inside its horizontal scroll region.
- Final verification passed: 16 native Rust tests; 3 frontend unit tests; full WASM/TypeScript/Vite production build; 5 actual Chrome acceptance tests against the production static build (both supplied cases, graph limit, CSV/JSON exports, topology warnings, mobile layout, delayed path delivery, and file replacement). No upload requests observed.
- Formatting and tracked diff whitespace checks passed. No hosting, commits, branch changes, or fixture edits performed.

## Field filters and configurable colors — 2026-10-10

- Implemented the approved in-chat design as a frontend extension: AND rules over bus/equipment fields, numeric comparisons and inclusive ranges, text matching, categories, and explicit missing-value checks. Graph filters apply within the selected scope before the 500-node display limit; topology calculations retain the full active case.
- Bus/equipment tables and CSV exports honor their corresponding rules, including every matching row beyond pagination. Neighborhood CSV exports honor bus filters. JSON includes view scope, rules, schemes, and displayed graph alongside original source data and full analysis.
- Added separate bus/equipment palettes, editable category colors, configurable numeric gradients and bounds, and live legends for visible categories, missing values, and mixed merged-member fields. Selection, paths, and critical findings retain their markers. Color edits preserve graph positions and viewport.
- Color preferences persist locally; custom overrides keyed by names or circuits remain in memory to avoid saving those source strings. Case-specific rules clear on replacement, and merged source rules must match together on one original member.
- Regression checks cover filters, exports, capped views, merged members, preferences, editor state, both themes, and narrow layouts. Independent read-only review found no remaining issues after fixes for select restoration, disclosure state, numeric category operators, large numeric domains, and preference privacy.
- Final verification passed: 30 Rust tests, 39 frontend unit tests, and 32 production browser tests; TypeScript checking, production build, Rust formatting, and whitespace checks passed. Desktop/light and narrow/dark screenshots were inspected. No commits, branch changes, publication, or fixture edits were performed.

## Tauri packaging — 2026-10-10

- Approved design: `docs/superpowers/specs/2026-10-10-tauri-packaging-design.md`; implementation plan: `docs/superpowers/plans/2026-10-10-tauri-packaging.md`.
- Implemented Tauri 2 shell, Vite build hooks, macOS application/DMG and Windows NSIS configuration, application icons, scoped main-window export permission, and native save dialogs. Browser builds remain available; analysis remains in the existing in-memory WASM worker.
- Fixed a pre-existing path-form event listener lost on interface redraw. A production regression checks path submission after an export error, active-case preservation, and rejection of stale export failures after case replacement.
- Verification passed on macOS arm64: 30 root Rust tests, 3 native export-writing tests, 20 frontend unit tests, and 20 production Playwright tests. Root and desktop Rust formatting, TypeScript checking, whitespace checks, full frontend build, and final macOS desktop build passed.
- Final artifacts: `src-tauri/target/release/nodal`, `src-tauri/target/release/bundle/macos/Nodal.app`, and `src-tauri/target/release/bundle/dmg/Nodal_0.1.0_aarch64.dmg`. System access was required for local test-server binding and DMG tooling in the sandbox.
- Native Tauri WKWebView smoke harness loaded the actual production frontend with its configured CSP and embedded assets, imported both supplied fixtures through synthetic File input, analyzed their expected counts (993/975/407 buses/lines/transformers, then 2,751/3,993/1,351), and verified case replacement. The harness returned explicit success with zero CSP violations. This verifies native worker/WASM loading without a separate frontend HTTP server.
- Native file chooser, real file dragging, save-dialog interactions/cancellation, and error handling against an unwritable destination still require manual platform smoke tests. Windows builds and runtime behavior are unverified on this macOS host. App signing/notarization and release publishing are outside scope.
- Independent task and whole-branch reviews approved the implementation without remaining defects. RAW fixtures and root parser/topology interfaces were preserved; generated build/test outputs are ignored.
