# Releasing Stannum

Releases are built by `.github/workflows/release.yml` when a `v*` tag is
pushed. The workflow builds the installers on each platform and attaches them
to a **draft** GitHub release, so nothing is public until you publish it.

| Platform | Runner | Installers |
| --- | --- | --- |
| Windows x64 | `windows-latest` | `.msi`, `-setup.exe` (NSIS) |
| macOS, Apple silicon | `macos-latest` | `.dmg` |
| Linux x64 | `ubuntu-22.04` | `.deb`, `.rpm`, `.AppImage` |

## Cutting a release

1. Make sure `CHANGELOG.md` has a line under **Unreleased** for everything
   worth telling users about.
2. Set the new version and turn the Unreleased notes into its section:

   ```powershell
   pwsh scripts/release/new-version.ps1 -Version 0.3.0
   ```

   This updates `package.json`, `package-lock.json`,
   `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `Cargo.lock`, and
   runs the same check the workflow runs.
3. Open a pull request with those changes and merge it.
4. Tag the merge commit on `main` and push the tag. Tags matching `v*` are
   protected by the `release tags` ruleset, so only admins can create them.

   ```powershell
   git switch main; git pull
   git tag v0.3.0
   git push origin v0.3.0
   ```

5. Wait for the **Release** workflow. Its first job fails fast if the tag,
   the three version fields and the changelog don't agree.
6. Open the draft on the
   [releases page](https://github.com/SklorGalore/StannumRevival/releases),
   try an installer or two, then press **Publish release**. The docs site's
   Download page links to the latest published release.

A version with a suffix (`0.3.0-beta.1`) is published as a pre-release.

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
