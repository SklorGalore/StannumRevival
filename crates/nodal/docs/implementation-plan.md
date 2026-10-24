# Nodal implementation plan

Implement the approved browser-only PSS/E RAW v35 topology application.

1. Rust parsing library: schema-driven section order and layouts, UTF-8/Windows-1252 decoding, source diagnostics, equipment identities/status, and both supplied fixture counts.
2. Graph engine: active undirected multigraph, transformer junctions, islands, degrees, multigraph-safe bridges/articulations, equipment-hop paths.
3. WASM worker: browser-target build, serializable interface, worker-owned cases, cancellable file replacement.
4. Browser UI: local file import, diagnostics and searchable result tables, equipment inspector, focused Cytoscape views capped at 500 nodes, CSV/JSON downloads.
5. Verification: native tests, WASM and frontend builds, browser acceptance with both fixtures and no file uploads.

Defaults: v35 only, active AC topology, one memory-only case, no hosting, DC/substation connectivity omissions reported. No outage studies, centrality, power-flow calculations, or RAW editing/export.
