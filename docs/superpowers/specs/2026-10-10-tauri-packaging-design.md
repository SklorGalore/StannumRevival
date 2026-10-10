# Tauri desktop packaging

## Intent and success criteria

Package Nodal as a standalone desktop application using Tauri 2. The approved approach wraps the existing TypeScript interface and Rust WebAssembly worker, with macOS as the first build and validation target. A packaged application must launch without a separate HTTP server or network connection and support RAW import, analysis, case replacement, and CSV/JSON export. Browser development and production builds remain available.

## Architecture and build

Add a separate Rust desktop crate under `src-tauri/` with the Tauri bootstrap, build script, configuration, capabilities, and application icons. Keep the existing root Rust library and WASM build independent of the desktop runtime. Bundle Vite's `dist/` through Tauri's `frontendDist`; use the Vite development server through `devUrl` and a fixed port with strict port selection. Ignore the desktop crate's generated build output.

Keep the existing browser build scripts. Add `desktop:dev` and `desktop:build` npm commands using the Tauri 2 CLI. Desktop development builds WASM before starting Vite; desktop packaging invokes the full frontend build before compiling the native shell. The macOS bundle targets are an application bundle and a DMG. Other operating systems are outside this validation scope.

Use the product name Nodal and bundle identifier `com.nodal.desktop`. Provide a resizable main window with a minimum size suitable for the existing workbench. The application embeds its interface, worker, and WASM assets; RAW acceptance fixtures are not bundled.

## Analysis and privacy

Continue using `BrowserEngine` and its dedicated module worker. No parsing or topology interface migrates to native Rust. Preserve cancellation on case replacement, explicit WASM handle release, error presentation, and CSV formula neutralization. Imported case contents remain local and in memory, except for exports explicitly saved by the user.

Configure a content security policy that permits the bundled application scripts, worker, WASM initialization and asset requests, plus development-only Vite connections as needed. Grant only capabilities needed by the desktop shell and file export; do not introduce general filesystem, shell, or network privileges.

## File interactions

Retain the existing HTML file chooser and drag-and-drop import. Configure Tauri's drag-and-drop behavior so the frontend can receive normal HTML file drop events. Verify that imported bytes reach the worker in the packaged webview.

The current export implementation uses Blob URLs and anchor downloads. For reliable desktop saves, introduce a small runtime-aware export adapter: browser builds retain their current download behavior; Tauri builds use a native save dialog and a narrowly scoped native command to write the export text to the selected destination. The backend owns path selection and writing, so the frontend cannot submit an arbitrary filesystem destination. Canceling the save dialog does not write a file or show an error. Failed saves report an actionable error through the existing interface. Export filenames and contents remain the same.

## Validation

Run root Rust tests and formatting checks, frontend unit tests, the complete frontend production build, and the production Playwright suite. Cover the desktop export adapter's browser fallback, cancellation, and failure handling where these can be tested meaningfully. Compile and format-check the desktop crate and build the macOS application and DMG.

Launch the packaged application and verify worker/WASM initialization, import of both supplied acceptance fixtures, drag-and-drop, case replacement, analysis views, and CSV/JSON saves. Browser tests alone do not establish desktop compatibility. Report any native UI checks that cannot be performed in the available environment, along with the exact build artifact paths and successful automated checks.

## Distribution scope

This change produces local macOS build artifacts and documents prerequisites, development, packaging, and validation commands in the README. App Store submission, automatic updates, signing credentials, notarization, and Windows/Linux release pipelines are outside this change. Any distribution signing can be configured separately.
