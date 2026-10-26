---
title: Build from source
description: Build and run Stannum from source on Windows, macOS or Linux.
---

Stannum is a [Tauri 2](https://v2.tauri.app/) desktop app. To just use it,
[download an installer](/StannumRevival/start/download/). Build it from
source to work on it, or on a platform without an installer.

## Prerequisites

- [Node.js](https://nodejs.org/) 22 or later (22.23.4 or later to build these docs).
- [Rust](https://www.rust-lang.org/tools/install), stable toolchain (edition 2024).
- The [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for
  your platform. On Linux that means `libwebkit2gtk-4.1-dev` and its friends;
  on Windows, the WebView2 runtime and the MSVC build tools.

## Run it

```sh
git clone https://github.com/SklorGalore/StannumRevival.git
cd StannumRevival
npm install
npm run tauri dev      # run the desktop app
npm run tauri build    # package an installer for this platform
```

How releases are cut and signed is in
[docs/releasing.md](https://github.com/SklorGalore/StannumRevival/blob/main/docs/releasing.md).

## Check a build

These are the same checks continuous integration runs on every pull request:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test -p stannum-core -p rs-power -p nodal
npm run build          # type-check and bundle the frontend
npm test               # frontend unit tests (Vitest)
```

The browser smoke test drives the real interface in Chromium against the real
Rust core; see the repository README for how to run it.

## Build these docs

The documentation lives in `docs-site/` and is an
[Astro Starlight](https://starlight.astro.build/) site:

```sh
cd docs-site
npm ci
npm run dev            # live preview at http://localhost:4321/StannumRevival/
npm run build          # static site in docs-site/dist
```

Pages are Markdown files under `docs-site/src/content/docs/`. Equations use
`$...$` and `$$...$$` and are rendered with KaTeX at build time.
