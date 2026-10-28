---
title: Tests and CI
description: Where Stannum's tests live, how to run each kind, what CI runs on a pull request, and how a change lands and ships.
---

## Kinds of test

| Kind | Where | Run with |
| --- | --- | --- |
| Rust unit tests | `#[cfg(test)]` modules in each crate | `cargo test -p <crate>` |
| Rust integration tests | `crates/*/tests/` | `cargo test -p <crate>` |
| MATPOWER reference checks | `crates/rs-power/tests/reference.rs`, `raw_reference.rs` | part of `cargo test -p rs-power` |
| Large cases (ignored) | `crates/rs-power/tests/large_cases.rs` | `cargo test --release -p rs-power --test large_cases -- --ignored` after `pwsh scripts/fetch-test-cases.ps1` |
| Time and size budgets (ignored) | `crates/stannum-core/tests/budgets.rs` | `cargo test --release -p stannum-core --test budgets -- --ignored --nocapture` |
| Frontend unit tests | `src/**/*.test.ts` | `npm test` |
| Browser smoke test | `tests/browser/smoke.mjs` | see below |
| Python package | `crates/stannum-py/tests/` | `pwsh scripts/ci/test-python.ps1` |

Test fixtures live in `crates/stannum-core/tests/fixtures/` and
`crates/rs-power/tests/fixtures/`. MATPOWER cases keep their licence
(`MATPOWER-LICENSE` beside the rs-power fixtures, and the stannum-core
fixtures' README says where each case comes from).
Cases too large or not licensed for the repository go in `tests/large/` by
name and SHA-256 only.

Some tests write files the app reads, and check them when they run
normally:

- `stannum-core --test schemas` with `STANNUM_UPDATE_SCHEMAS=1` (or
  `pwsh scripts/update-schemas.ps1`) rewrites the command schema and
  `commands.gen.ts`.
- `stannum-core --test topology_wire` with `STANNUM_WRITE_FIXTURES=1`
  rewrites `src/lib/analysis/__fixtures__/topology-wire.json`.

## The browser smoke test

`tests/browser/smoke.mjs` drives the built interface in Chromium with
Playwright, against the real Rust core served by `dev_bridge`. It replaces
Tauri's IPC with a shim that forwards commands to the bridge, answers file
dialogs from a queue and keeps saved files in the page.

```sh
cargo build --release -p stannum-core --example dev_bridge
./target/release/examples/dev_bridge &
npm run build && npx vite preview --port 1420 --strictPort &
F=crates/stannum-core/tests/fixtures
STANNUM_MATPOWER=$PWD/$F/case9.m \
STANNUM_RAW=$PWD/$F/five_bus.raw \
STANNUM_RAW_B=$PWD/$F/five_bus_modified.raw \
  node tests/browser/smoke.mjs
```

The three cases above are required. CI also sets `STANNUM_RAW_DC`,
`STANNUM_DC_A`, `STANNUM_DC_B`, `STANNUM_VIOLATIONS`, `STANNUM_VSC_CASE` and
`STANNUM_VSC_GRIDS` (see `.github/workflows/ci.yml`) to cover HVDC,
violations and VSC grids; the steps that need them are skipped otherwise.
Set `STANNUM_SHOTS=<dir>` to save screenshots on the way; that is where the
screenshots on this site and in pull requests come from.

When you add something visible, add a step to the smoke test that uses it
and checks the result, not only that it renders.

## What CI runs

`.github/workflows/ci.yml` runs on every pull request and on pushes to
`main` and `release/**`:

| Job | Steps | Required |
| --- | --- | --- |
| **Rust core and desktop shell** | `cargo fmt --check`, `cargo clippy --all-targets -D warnings` (including `src-tauri`), `cargo test` for stannum-core, stannum-cli, stannum-synth, rs-power and nodal | Yes |
| **Frontend build and browser smoke test** | `npm ci`, `npm run build`, `npm test`, then the smoke test | Yes |
| **Large case budgets** | The budgets test on a 12,000-bus synthetic case; the timings go in the run summary | No |
| **Python package** (Linux, Windows, macOS) | Builds the abi3 wheel, installs it in a fresh venv, runs its tests, uploads the wheel | No |

`.github/workflows/docs.yml` builds this site on pull requests that touch
`docs-site/` and publishes it to GitHub Pages from `main`. It isn't
required, but a broken docs build shouldn't be merged.

The required check names are in `.github/rulesets/`. If you rename a job,
change them there in the same pull request, or every PR will wait for a
check that never reports. Actions are pinned to commit SHAs, and
Dependabot keeps them current.

## Landing a change

The full rules are in
[CONTRIBUTING.md](https://github.com/SklorGalore/StannumRevival/blob/main/CONTRIBUTING.md).
In short:

1. Branch from the latest `development` and open the pull request into
   `development`. Both `development` and `main` are protected and only
   change by pull request; `main` changes only when a release is merged.
2. Fill in the pull request template: summary, **screenshots** for
   anything visible in the app (as `<img src="https://…">` with full
   absolute URLs, so they render), and what you tested.
3. Change the docs in the same pull request as the feature, and add a line
   under **Unreleased** in `CHANGELOG.md` for anything users will notice.
4. Both required checks pass and review conversations are resolved.
5. Merge with a merge commit or squash. Use a merge commit for anything
   that pulls a subtree into `crates/`.

New scripts in the repository are PowerShell (`.ps1`), so they run the
same on every platform.

## Releasing

Pushing a `v*` tag runs `.github/workflows/release.yml`, which checks the
version and changelog, builds installers for Windows, macOS and Linux, the
Python wheels and `stannum-cli`, and attaches them all to a draft GitHub
release. `pwsh scripts/release/start-release.ps1 -Version x.y.z` cuts
`release/vx.y.z` from `development`, sets the version and opens its pull
request into `main`; once that merges, `finish-release.ps1` tags it and
merges `main` back into `development`. The steps, signing and checks are in
[docs/releasing.md](https://github.com/SklorGalore/StannumRevival/blob/main/docs/releasing.md).
