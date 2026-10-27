# Releasing Stannum

Releases are built by `.github/workflows/release.yml` when a `v*` tag is
pushed to the merge of a release branch into `main`. The workflow builds the installers on each platform and attaches them
to a **draft** GitHub release, so nothing is public until you publish it.

| Platform | Runner | Installers |
| --- | --- | --- |
| Windows x64 | `windows-latest` | `.msi`, `-setup.exe` (NSIS) |
| macOS, Apple silicon | `macos-latest` | `.dmg` |
| Linux x64 | `ubuntu-22.04` | `.deb`, `.rpm`, `.AppImage` |

## Branches

Releases follow git flow (see [CONTRIBUTING.md](../CONTRIBUTING.md#branches)):

```text
feature/…  ──PR──▶  development  ──cut──▶  release/vX.Y.Z  ──PR──▶  main  ──tag vX.Y.Z
                        ▲                                            │
                        └──────────────── PR (back-merge) ───────────┘
hotfix/vX.Y.Z  ◀──cut── main   (urgent fixes; same PR, tag and back-merge)
```

Merge release, hotfix and back-merge pull requests with **Create a merge
commit**, never squash, so `main` and `development` keep sharing history.

## Cutting a release

1. Make sure `CHANGELOG.md` on `development` has a line under **Unreleased**
   for everything worth telling users about.
2. Cut the release branch:

   ```powershell
   pwsh scripts/release/start-release.ps1 -Version 0.4.0
   ```

   This makes `release/v0.4.0` from the latest `origin/development`, runs
   `new-version.ps1` on it (which updates `package.json`,
   `package-lock.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`
   and `Cargo.lock`, and turns the Unreleased notes into the version's
   section), commits that as "Release v0.4.0", pushes the branch and opens a
   pull request into `main`. Add `-NoPush` to stop after the commit.
3. The push runs the **Release** workflow on the branch, which checks the
   version and builds release-candidate installers as workflow artifacts.
   Try them. Fixes go into `release/v0.4.0` by pull request (the branch is
   protected); each merge builds new candidates. Meanwhile `development`
   stays open for the next release's work.
4. Merge the pull request into `main` with **Create a merge commit**.
5. Tag it and merge it back:

   ```powershell
   pwsh scripts/release/finish-release.ps1 -Version 0.4.0
   ```

   This finds the merge commit on `origin/main`, checks the version and
   changelog there, tags it `v0.4.0` and pushes the tag. Tags matching `v*`
   are protected by the `release tags` ruleset, so only admins can create
   them. It then opens a pull request from `main` into `development` so the
   version bump and release fixes reach `development`; merge it with a merge
   commit. If `development` gained changelog lines meanwhile, `CHANGELOG.md`
   conflicts there: keep them under **Unreleased**, above the new version's
   section. `-NoBackMerge` skips that pull request.
6. Wait for the **Release** workflow on the tag. Its first job fails fast if
   the tag, the three version fields and the changelog don't agree.
7. Open the draft on the
   [releases page](https://github.com/SklorGalore/StannumRevival/releases),
   try an installer or two, then press **Publish release**. The docs site's
   Download page links to the latest published release.

A version with a suffix (`0.4.0-beta.1`) is published as a pre-release.

## Hotfixes

For an urgent fix to the released code, cut from `main` instead:

```powershell
pwsh scripts/release/start-release.ps1 -Version 0.3.1 -Hotfix
# makes hotfix/v0.3.1 from origin/main and stops there.
# Commit the fix with a line for it under "## Unreleased" in CHANGELOG.md, then:
pwsh scripts/release/start-release.ps1 -Version 0.3.1
# on the hotfix branch this sets the version, pushes and opens the PR into main.
```

Run on any `release/v*` or `hotfix/v*` branch, `start-release.ps1` carries
on from where it is instead of cutting a new branch, so it is safe to re-run
(for example after `-NoPush`). Finish with `finish-release.ps1 -Version 0.3.1`
as for a release.

## Test builds

- Every push to a `release/v*` or `hotfix/v*` branch builds the installers
  as workflow artifacts.
- Pull requests that touch the release workflow, `scripts/release/`, the
  Tauri config or the icons run the build jobs and keep the installers as
  workflow artifacts (on the run's summary page) for 90 days.
- **Actions → Release → Run workflow** does the same for any branch.

Neither creates a GitHub release.

## Checking locally

```powershell
pwsh scripts/release/check-release.ps1                  # versions agree, notes exist
pwsh scripts/release/check-release.ps1 -Branch release/v0.4.0  # as on the release branch
pwsh scripts/release/check-release.ps1 -Tag v0.4.0      # as the tag push will
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
