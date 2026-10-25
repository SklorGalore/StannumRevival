# Repository Guidelines

## Project Structure & Module Organization

nodal is the PSS/E RAW v35 parser and topology library used by Stannum's Topology module. `src/model.rs` holds the data model, `src/parser.rs` the schema-driven RAW parser, and `src/topology.rs` the graph analysis. Integration tests live in `tests/`. Root `.RAW` files are acceptance fixtures; `raw.schema.json` defines field positions and section order. Design and progress notes from the original standalone app live in `docs/`.

## Build and Test

- `cargo test -p nodal`: run the parser and topology tests.
- `cargo clippy -p nodal --all-targets -- -D warnings`: lint.
- `cargo fmt --all -- --check`: check formatting.

## Coding Style & Naming Conventions

Use standard Rust formatting with four-space indentation. Functions and modules use `snake_case`, types use `PascalCase`. Keep parsing and topology logic here; presentation belongs in the Stannum frontend.

## Testing Guidelines

Use descriptive `#[test]` names. Cover changed behavior and relevant edge cases. Preserve both supplied RAW fixtures.
