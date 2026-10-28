---
title: Set up and run
description: Build Stannum, run the interface in a browser against the real core, and run the checks CI runs.
---

## Prerequisites

- [Node.js](https://nodejs.org/) 22 or later (22.23.4 or later for this
  docs site).
- [Rust](https://www.rust-lang.org/tools/install), stable toolchain. The
  crates use edition 2024. CI uses the latest stable, so new Clippy lints
  can appear without any change in the repository.
- The [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/)
  for your platform, only if you build the desktop app. On Linux that means
  `libwebkit2gtk-4.1-dev` and friends;
  `scripts/ci/install-tauri-deps.ps1` installs the list CI uses.
- [PowerShell 7](https://learn.microsoft.com/powershell/scripting/install/installing-powershell)
  (`pwsh`) for the repository's scripts. Scripts are `.ps1` so they run the
  same on Windows, macOS and Linux.
- Python 3.10 or later and [maturin](https://www.maturin.rs/), only for the
  Python package.

npm (`package-lock.json`) is the package manager CI uses. `pnpm install`
works too: `pnpm-workspace.yaml` lists which dependency build scripts may
run. When you add a dependency with an install script, add it there.

## Run the desktop app

```sh
npm install
npm run tauri dev      # Vite on :1420 plus the Tauri window, with reload
npm run tauri build    # an installer for this platform
```

`npm run tauri dev` rebuilds the Rust side when it changes and hot-reloads
the React side.

## Run the interface in a browser

The Tauri window isn't needed for most interface work. The `dev_bridge`
example serves the core's command list over local HTTP, and a shim in the
page stands in for Tauri's IPC:

```sh
cargo run --release -p stannum-core --example dev_bridge   # 127.0.0.1:1421
npm run dev                                                # Vite on :1420
```

`dev_bridge` answers `POST /invoke/<command>` with the command's JSON
arguments, running the same `Session` the desktop app runs. The browser
smoke test (`tests/browser/smoke.mjs`) installs the shim; copy its `shim`
function into the page if you want to click around by hand. Without the
shim, plain `npm run dev` loads the interface but every command fails,
since there's no Tauri.

## Run the checks

These are what CI runs on every pull request:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test -p stannum-core -p stannum-cli -p stannum-synth -p rs-power -p nodal
npm run build          # tsc type-check and Vite bundle
npm test               # Vitest unit tests
```

Clippy builds `src-tauri` too, and Tauri embeds `../dist` when it compiles.
If you haven't built the frontend, `mkdir dist` is enough for Clippy.

The browser smoke test and Python tests are described in
[Tests and CI](/StannumRevival/develop/testing/).

## Useful scripts

| Script | What it does |
| --- | --- |
| `pwsh scripts/update-schemas.ps1` | Writes the command schemas and `commands.gen.ts` again after a command or a type it returns changes. |
| `pwsh scripts/update-icons.ps1` | Regenerates the Tauri icons, the favicon and this site's logo from `src/assets/stannum-logo*.svg`. |
| `pwsh scripts/fetch-test-cases.ps1` | Fetches the large cases in `tests/large/` and checks their hashes. |
| `pwsh scripts/new-synthetic-case.ps1` | Builds a synthetic case with `stannum-synth`. |
| `pwsh scripts/ci/test-python.ps1` | Builds the Python wheel, installs it in a fresh venv and runs its tests. |
| `pwsh scripts/release/new-version.ps1 -Version x.y.z` | Sets a new version everywhere it appears (see `docs/releasing.md`). |
| `pwsh scripts/setup-github-repo.ps1` | Applies the repository's rulesets and security settings (admin only). |

## Work on these docs

```sh
cd docs-site
npm ci
npm run dev     # http://localhost:4321/StannumRevival/
npm run build   # static site in docs-site/dist
```

Pages are Markdown under `docs-site/src/content/docs/`, and the sidebar is
listed in `docs-site/astro.config.mjs`. Internal links include the
`/StannumRevival/` base. Equations use `$...$` and `$$...$$` (KaTeX at build
time). App screenshots come from the browser smoke test with
`STANNUM_SHOTS=dir`; copy the ones a page uses into
`docs-site/src/assets/screens/`.
