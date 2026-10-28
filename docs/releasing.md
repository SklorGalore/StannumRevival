# Releasing Stannum

Releases are built by `.github/workflows/release.yml` when a `v*` tag is
pushed. The workflow builds the installers, the Python wheels and `stannum-cli` on
each platform and attaches them all to a **draft** GitHub release, so
nothing is public until you publish it.

| Platform | Runner | Installers |
| --- | --- | --- |
| Windows x64 | `windows-latest` | `.msi`, `-setup.exe` (NSIS) |
| macOS, Apple silicon | `macos-latest` | `.dmg` |
| Linux x64 | `ubuntu-22.04` | `.deb`, `.rpm`, `.AppImage` |

Beside the installers, the **Python wheel and CLI** jobs build, for the same
three platforms:

| File | What it is |
| --- | --- |
| `stannum-<version>-cp310-abi3-<platform>.whl` | The `stannum` Python package, for Python 3.10 and newer. The Linux wheel is linked against an old glibc with zig, so it is tagged `manylinux2014`. Each wheel's tests run against it before it is kept. |
| `stannum-cli-<version>-<platform>.zip` / `.tar.gz` | `stannum-cli`, which runs run files without the app. |

The wheels go on the release page only; they are not published to PyPI.

## Cutting a release

1. Make sure `CHANGELOG.md` has a line under **Unreleased** for everything
   worth telling users about.
2. Set the new version and turn the Unreleased notes into its section:

   ```powershell
   pwsh scripts/release/new-version.ps1 -Version 0.3.0
   ```

   This updates `package.json`, `package-lock.json`,
   `src-tauri/tauri.conf.json`, the `Cargo.toml` of the app, `stannum-cli`
   and the Python package (`crates/stannum-py`), and `Cargo.lock`,
   regenerates `THIRD-PARTY-LICENSES.txt`, and runs the same check the
   workflow runs.
3. Open a pull request with those changes and merge it.
4. Tag the merge commit on `main` and push the tag. Tags matching `v*` are
   protected by the `release tags` ruleset, so only admins can create them.

   ```powershell
   git switch main; git pull
   git tag v0.3.0
   git push origin v0.3.0
   ```

5. Wait for the **Release** workflow. Its first job fails fast if the tag,
   the three version fields and the changelog don't agree, or if
   `THIRD-PARTY-LICENSES.txt` no longer matches the dependencies (run
   `pwsh scripts/release/third-party-licenses.ps1` and commit the result).
6. Open the draft on the
   [releases page](https://github.com/SklorGalore/StannumRevival/releases),
   try an installer or two (and `pip install` a wheel), then press **Publish release**. The docs site's
   Download page links to the latest published release.

A version with a suffix (`0.3.0-beta.1`) is published as a pre-release.

## Licence files

Every installer, wheel and `stannum-cli` archive carries `LICENSE`
(BSD-3-Clause), `THIRD-PARTY-NOTICES.md` (hand-written: MATPOWER, ELK,
MapLibre, Natural Earth and the copyleft-licensed packages) and
`THIRD-PARTY-LICENSES.txt` (generated: every Rust crate and npm package
built in, with its licence text). The installers list them as Tauri
resources, `build-cli.ps1` packs them, and `scripts/ci/test-python.ps1`
copies them beside `pyproject.toml` so maturin puts them in the wheel's
`.dist-info/licenses`. When a new dependency with an unusual licence
(anything beyond MIT, Apache-2.0, BSD, ISC, Zlib or Unicode) arrives, add a
line for it to `THIRD-PARTY-NOTICES.md`.

## Test builds

- Pull requests that touch the release workflow, `scripts/release/`, the
  Tauri config or the icons run the build jobs and keep the installers as
  workflow artifacts (on the run's summary page) for 90 days.
- **Actions → Release → Run workflow** does the same for any branch.

Neither creates a GitHub release.

## Checking locally

```powershell
pwsh scripts/release/check-release.ps1                  # versions agree, notes exist
pwsh scripts/release/check-release.ps1 -Tag v0.3.0      # as the tag push will
npm run tauri -- build --bundles deb                    # one installer for this machine
pwsh scripts/ci/test-python.ps1 -Manylinux -Out wheels  # the Linux wheel, as the release builds it
pwsh scripts/release/build-cli.ps1 -OutDir cli          # stannum-cli, packed
pwsh scripts/release/collect-bundles.ps1 -OutDir out    # gather what was built
```

## Signing (not set up yet)

The installers are unsigned, so the first launch shows a warning:

- **macOS:** "Stannum can't be opened because Apple cannot check it". Right-
  click the app in Applications, choose **Open**, then **Open** again. Only
  needed once.
- **Windows:** SmartScreen says "Windows protected your PC". Choose **More
  info**, then **Run anyway**.

To sign later:

- **macOS** needs an Apple Developer account. Add the repository secrets
  `APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD`,
  `APPLE_SIGNING_IDENTITY`, and for notarization `APPLE_ID`, `APPLE_PASSWORD`
  (an app-specific password) and `APPLE_TEAM_ID`, then pass them as `env` to
  the **Build installers** step. Tauri signs and notarizes when they are set.
- **Windows** needs a code-signing certificate. Import it on the runner and
  set `bundle.windows.certificateThumbprint` in
  `src-tauri/tauri.windows.conf.json`, or use `bundle.windows.signCommand`
  for a cloud signing service such as Azure Trusted Signing.
