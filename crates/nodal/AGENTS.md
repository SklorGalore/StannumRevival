# Repository Guidelines

## Project Structure & Module Organization

Nodal analyzes PSS/E RAW v35 electrical networks locally in the browser. Rust modules in `src/` handle data models, parsing, topology, and WASM bindings. The TypeScript frontend in `web/` contains the worker, engine, graph rendering, view models, and CSS; `index.html` is the Vite entry point. Rust integration tests live in `tests/`, browser tests in `tests/browser/`, and frontend unit tests alongside their modules. Root `.RAW` files are acceptance fixtures; `raw.schema.json` defines field positions and section order. Design and progress notes live in `docs/`.

## Build, Test, and Development Commands

Use Rust 1.85+ and Node.js 22.12+.

- `rustup target add wasm32-unknown-unknown`: install the WASM compilation target.
- `npm install`: install frontend and build dependencies.
- `npm run wasm`: compile Rust into `web/wasm/`; rerun after Rust changes.
- `npm run dev`: start the local Vite server.
- `npm run build`: build WASM, type-check TypeScript, and produce `dist/`.
- `npm run preview`: serve the production build over HTTP.
- `cargo test`, `npm test`, `npm run test:browser`: run Rust, Vitest, and Playwright tests respectively.

## Coding Style & Naming Conventions

Use standard Rust formatting with four-space indentation; run `cargo fmt --check` before submitting. Rust functions and modules use `snake_case`, types use `PascalCase`. TypeScript uses strict checking, single quotes, semicolons, `camelCase` functions, and `PascalCase` classes/types. Follow surrounding indentation and spacing; no frontend formatter or linter is configured. Keep parsing/topology logic in Rust and presentation logic in TypeScript.

## Testing Guidelines

Use descriptive Rust `#[test]` names, `web/**/*.test.ts` for Vitest, and `tests/browser/*.spec.ts` for Playwright. Cover changed behavior and relevant edge cases; no numerical coverage threshold is configured. Preserve both supplied RAW fixtures. Browser tests require Chrome/Chromium; install with `npx playwright install chromium` if needed. After building, run `NODAL_TEST_PRODUCTION=1 npm run test:browser` to verify production behavior.

## Commit & Pull Request Guidelines

The short Git history uses plain descriptive messages, such as `added rustworkx-core as a dependency`; no formal convention is established. Write concise, action-oriented commit subjects. PRs should explain behavior changes, link relevant issues, list validation performed, and include screenshots for UI changes.

## Privacy & Generated Files

Keep case contents on-device and in memory. Preserve CSV formula neutralization and worker/WASM resource cleanup. Do not commit generated `web/wasm/`, `dist/`, `target/`, or test reports.

## Agent Collaboration

Use Sol (`gpt-6.1-sol`) for orchestration and Luna (`gpt-6-luna`) subagents for implementation. Sol scopes tasks, assigns file ownership, reviews changes, and verifies integration. Delegate independent implementation tasks to Luna with explicit requirements and validation commands. Set `fork_turns: "none"` when selecting a model explicitly, and provide necessary context. Avoid overlapping edits; subagents report changed files and test results before integration.
