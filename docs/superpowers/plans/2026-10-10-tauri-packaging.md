# Tauri Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Build Nodal as a local desktop application for macOS and Windows while preserving browser usage.

**Architecture:** A separate Tauri 2 crate embeds the existing Vite frontend and WASM worker. A small export adapter selects browser downloads or a native save command. Native Rust owns save-path selection and writing.

**Tech Stack:** Tauri 2, Rust, TypeScript, Vite, wasm-pack, Vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-10-10-tauri-packaging-design.md`

## Global Constraints

- Product name Nodal; bundle identifier `com.nodal.desktop`.
- Preserve the root Rust library, WASM worker lifecycle, in-memory analysis, acceptance fixtures, and CSV formula neutralization.
- Retain browser scripts; add `desktop:dev` and `desktop:build`.
- macOS bundles: application and DMG. Windows bundle: NSIS setup executable plus native executable, built on Windows.
- No arbitrary frontend filesystem destination, general filesystem permissions, or network access for case data.
- Report unavailable native checks explicitly; do not claim Windows verification from macOS tests.

## Review Focus

- Save cancellation must leave the case and files untouched (Task 2).
- Unwritable export destination must produce a visible error without losing the case (Task 2).
- Browser exports must retain filenames, content, and Blob cleanup (Task 2).
- Packaged worker/WASM asset resolution and CSP must allow offline analysis (Task 3).
- Case replacement and file drops must work in the native webview (Task 3).

---

### Task 1: Desktop shell and platform packaging

**Files:** Create `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/build.rs`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`, `src-tauri/tauri.macos.conf.json`, `src-tauri/tauri.windows.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/icons/*`, and `vite.config.ts`. Modify `package.json`, `package-lock.json`, and `.gitignore`.

**Interfaces:** Consumes existing `npm run wasm` and `npm run build`. Produces `npm run desktop:dev` and `npm run desktop:build`; desktop crate `run()` bootstrap.

- [ ] Read worktree, TDD, and subagent execution skills; establish isolated implementation workspace. Delegate shell ownership to a Luna subagent per AGENTS.md.
- [ ] Add Tauri 2 CLI/API dependencies and a separate desktop crate with `tauri-build` and the native dialog plugin. Preserve root library dependencies.
- [ ] Configure `frontendDist: ../dist`, `devUrl: http://127.0.0.1:5173`, development WASM build and Vite startup, and production full frontend build. Fix Vite to port 5173 with strict port checking and preserve browser test ports via CLI overrides.
- [ ] Configure platform-specific bundle targets, Nodal identity, resizable 1440-by-1000 window with 900-by-640 minimum, application icons, and `dragDropEnabled: false` for HTML drops.
- [ ] Apply CSP for bundled assets, module workers, WASM, and development HMR; grant only main-window capabilities needed for the custom save command. Do not expose general filesystem access.
- [ ] Ignore `src-tauri/target/` and other desktop build artifacts. Commit dependency lockfiles and icons.
- [ ] Verify `npm run build`, `cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo fmt --manifest-path src-tauri/Cargo.toml --check` exit successfully. Report changed files and results for orchestration review.

### Task 2: Cross-platform exports

**Files:** Create `web/export.ts`, `web/export.test.ts`, and `src-tauri/src/export.rs`. Modify `web/main.ts` and `src-tauri/src/lib.rs`.

**Interfaces:** Consumes Tauri bootstrap and API from Task 1. Produces `saveExport(data: string, mimeType: string, filename: string): Promise<void>` and native `save_export(app: tauri::AppHandle, filename: String, data: String) -> Result<(), String>`. Cancellation resolves successfully without writing.

- [ ] Delegate export ownership to a Luna subagent after shell interfaces are stable; avoid concurrent edits to `src-tauri/src/lib.rs`.
- [ ] Write and run failing Vitest cases for browser Blob content and filename, URL revocation, desktop invocation arguments, successful cancellation, and propagation of save errors.
- [ ] Implement runtime detection with Tauri `isTauri`; desktop invokes `save_export` with text and suggested filename. Browser retains Blob downloads and delayed URL revocation.
- [ ] Implement the asynchronous native command with a native save dialog, extension filters, and suggested filename; path selection remains inside Rust. Write only to the selected path, return clear errors, and avoid blocking the UI thread.
- [ ] Add native tests for the pure write helper: exact UTF-8 contents, empty contents, and an invalid parent path yielding an error. Verify save-dialog cancellation through a native smoke test.
- [ ] Replace the existing `download` body with the adapter while preserving naming. Catch export rejections and present them using the existing application error UI without clearing the current case.
- [ ] Run `npm test`, `cargo test --manifest-path src-tauri/Cargo.toml`, and frontend type checking. Review export changes and test evidence before integration.

### Task 3: Integrated builds, desktop smoke checks, and documentation

**Files:** Modify `README.md`; update relevant browser tests only if integration exposes a behavioral gap. Record validation results in `docs/progress.md`.

**Interfaces:** Consumes the commands and export adapter from Tasks 1 and 2. Produces documented build workflow and reviewed platform artifacts.

- [ ] Document Rust and Node requirements, macOS tools, Windows MSVC/C++ Build Tools/WebView2 prerequisites, browser and desktop commands, artifact locations, local data handling, and unsigned distribution scope.
- [ ] Run `cargo test`, `cargo fmt --check`, `npm test`, and `npm run build`; require successful exit codes.
- [ ] Run `NODAL_TEST_PRODUCTION=1 npm run test:browser`; require all production browser tests passing, including import, replacement, paths, and exports.
- [ ] Run `npm run desktop:build` on macOS; confirm the native executable, `.app`, and `.dmg` exist. Record their paths.
- [ ] Launch the packaged app with network unavailable and check both fixtures, worker initialization, case replacement, drag-and-drop, and analysis views. Check CSV/JSON saving, cancellation, and unwritable destination errors. If GUI access is unavailable, record those checks as unverified.
- [ ] On Windows when available, run `npm run desktop:build` and repeat native smoke checks; confirm `.exe` and NSIS installer. Otherwise document the exact unverified Windows checks and prerequisites.
- [ ] Request independent review per repository collaboration instructions, resolve findings, then run any checks affected by fixes. Use verification-before-completion and finishing-a-development-branch skills before reporting completion.

## Execution and review

Use Sol orchestration and Luna implementation subagents with explicit file ownership as required by AGENTS.md. Task 2 depends on Task 1's native bootstrap; Task 3 reviews the integrated result. Do not publish installers or introduce signing credentials as part of this plan.
