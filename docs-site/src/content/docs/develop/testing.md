---
title: Tests and CI
description: Where Stannum's tests live, how to run each kind, what CI runs on a pull request, and how a change lands and ships.
---

## Kinds of test

| Kind | Where | Run with |
| --- | --- | --- |
| Rust unit tests | `#[cfg(test)]` modules in each crate | `cargo test -p <crate>` |
| Rust integration tests | `crates/*/tests/` | `cargo test -p <crate>` |
| Reference checks (the [Verification](/StannumRevival/reference/verification/) page) | `reference.rs`, `raw_reference.rs`, `transformer_detail.rs` in rs-power; `saved_solutions.rs`, `matacdc_benchmark.rs`, `contingency.rs` in stannum-core; listed in `scripts/verify.ps1` | part of `cargo test`; `pwsh scripts/verify.ps1` also rewrites the page's tables |
| Large cases (ignored) | `crates/rs-power/tests/large_cases.rs` | `cargo test --release -p rs-power --test large_cases -- --ignored` after `pwsh scripts/fetch-test-cases.ps1` |
| Time and size budgets (ignored) | `crates/stannum-core/tests/budgets.rs` | `cargo test --release -p stannum-core --test budgets -- --ignored --nocapture` |
| Bad files (field sweeps ignored) | `crates/stannum-core/tests/bad_files.rs` | `cargo test -p stannum-core --test bad_files`; sweeps with `--release ... -- --ignored` |
| Fuzzing the file readers | `fuzz/` (cargo-fuzz, nightly) | `pwsh scripts/fuzz.ps1`; see `fuzz/README.md` |
| Frontend unit tests | `src/**/*.test.ts` | `npm test` |
| Files from earlier releases | `tests/compat/`, read by `crates/stannum-core/tests/compat.rs` and `src/lib/compat/compat.test.ts` | part of `cargo test -p stannum-core` and `npm test` |
| Browser smoke test | `tests/browser/smoke.mjs` | see below |
| Python package | `crates/stannum-py/tests/` | `pwsh scripts/ci/test-python.ps1` |

Test fixtures live in `crates/stannum-core/tests/fixtures/` and
`crates/rs-power/tests/fixtures/`. MATPOWER cases keep their licence
(`MATPOWER-LICENSE` beside the rs-power fixtures, and the stannum-core
fixtures' README says where each case comes from).
Cases too large or not licensed for the repository go in `tests/large/` by
name and SHA-256 only.

`tests/compat/` holds studies, diagrams, run files and coordinates files
written by released versions of Stannum, one folder per release. They are
never edited: a change that stops one opening breaks the 1.x promise to open
every file a 1.x release wrote. When a release changes one of these formats,
add a folder with files that release wrote (its README says how the current
ones were made). The formats and their version rules are in the
[file format reference](/StannumRevival/reference/study-file/).

Some tests write files the app reads, and check them when they run
normally:

- `stannum-core --test schemas` with `STANNUM_UPDATE_SCHEMAS=1` (or
  `pwsh scripts/update-schemas.ps1`) rewrites the command schema and
  `commands.gen.ts`.
- `stannum-core --test topology_wire` with `STANNUM_WRITE_FIXTURES=1`
  rewrites `src/lib/analysis/__fixtures__/topology-wire.json`.
- The reference tests, with `STANNUM_VERIFY` naming a file, append their
  largest differences to it; `pwsh scripts/verify.ps1` runs them that way
  and rewrites `docs-site/src/data/verification.json`.

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
| **Rust core and desktop shell** | `cargo fmt --check`, `cargo clippy --all-targets -D warnings` (including `src-tauri`), `cargo test` for stannum-core, stannum-cli, stannum-synth, stannum-verify, rs-power and nodal | Yes |
| **Frontend build and browser smoke test** | `npm ci`, `npm run build`, `npm test`, then the smoke test | Yes |
| **Large case budgets** | The budgets test on a 12,000-bus synthetic case; the timings go in the run summary | No |
| **Large case budgets (80,000 buses)** (`budgets-large.yml`, weekly and on request) | The same test on an 80,000-bus synthetic case, the size of the large cases people open | No |
| **Fuzz file readers** (`fuzz.yml`, weekly, on request, and briefly on PRs that change `fuzz/`) | Ten minutes of cargo-fuzz on each of the RAW, MATPOWER, run file and coordinates CSV readers; crashing inputs are uploaded | No |
| **Python package** (Linux, Windows, macOS) | Builds the abi3 wheel, installs it in a fresh venv, runs its tests, uploads the wheel | No |

`.github/workflows/docs.yml` builds this site on pull requests that touch
`docs-site/` or the Rust crates, and publishes it to GitHub Pages from
`main`. It first runs `scripts/verify.ps1`, so the Verification page's
numbers come from the commit being built. The build checks
every internal link and heading anchor
([starlight-links-validator](https://github.com/HiDeoo/starlight-links-validator))
and fails on one that goes nowhere, including a link written without the
`/StannumRevival/` base. It isn't required, but a broken docs build shouldn't
be merged.

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
request into `main`; once it is squashed in, `finish-release.ps1` tags it and
merges `main` back into `development`. The steps, signing and checks are in
[docs/releasing.md](https://github.com/SklorGalore/StannumRevival/blob/main/docs/releasing.md).
