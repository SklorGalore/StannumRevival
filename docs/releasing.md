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

Releases follow git flow (see [CONTRIBUTING.md](../CONTRIBUTING.md)): work
lands on `development`, a `release/vX.Y.Z` branch is cut from it, and that
branch's pull request into `main` is the release. `main` only ever moves
when a release or hotfix merges.

1. Make sure `CHANGELOG.md` on `development` has a line under
   **Unreleased** for everything worth telling users about. Each feature
   PR into `development` adds its own line there, so this is usually a
   read-through.
2. Cut the release branch:

   ```powershell
   pwsh scripts/release/start-release.ps1 -Version 0.4.0
   ```

   This makes `release/v0.4.0` from the latest `origin/development`, runs
   `new-version.ps1` on it (which updates `package.json`,
   `package-lock.json`, `src-tauri/tauri.conf.json`, the `Cargo.toml` of
   the app, `stannum-cli` and the Python package, and `Cargo.lock`,
   regenerates `THIRD-PARTY-LICENSES.txt`, and turns the Unreleased notes
   into the version's section), commits that as
   "Release v0.4.0", pushes the branch and opens its pull request into
   `main`. The bump goes out with the branch's first push, because the
   `release branches` ruleset lets you create a `release/**` branch but
   takes later changes only by pull request.
3. Every push to the release branch builds release-candidate installers as
   workflow artifacts. Try them. Fixes go into the release branch by pull
   request; nothing else does.
4. Merge the release pull request into `main` with **Create a merge
   commit**.
5. Tag it and merge `main` back into `development`:

   ```powershell
   pwsh scripts/release/finish-release.ps1 -Version 0.4.0
   ```

   This tags the merge commit on `main` as `v0.4.0` and pushes the tag
   (tags matching `v*` are protected by the `release tags` ruleset, so only
   admins can create them), then opens a pull request from `main` into
   `development` so the bump and any release fixes reach `development`.
   Merge that one with a merge commit too.

   Check `CHANGELOG.md` in that pull request first. The release branch
   renamed the old **Unreleased** notes to `## 0.4.0`, and git applies that
   rename on `development` too, so any line a feature PR added under
   **Unreleased** after the release was cut ends up inside the `0.4.0`
   section without a conflict. If the diff shows such lines, merge the
   back-merge pull request and then open a small PR into `development` that
   moves them back under **Unreleased**.
6. Wait for the **Release** workflow. Its first job fails fast if the tag,
   the three version fields and the changelog don't agree, or if
   `THIRD-PARTY-LICENSES.txt` no longer matches the dependencies (run
   `pwsh scripts/release/third-party-licenses.ps1` and commit the result).
7. Open the draft on the
   [releases page](https://github.com/SklorGalore/StannumRevival/releases),
   try an installer or two (and `pip install` a wheel), then press **Publish release**. The docs site's
   Download page links to the latest published release, and the docs site
   itself is published from `main`, so it always describes the released
   version. Publishing is also what offers the update to installed copies:
   the app reads `latest.json` from the latest *published* release, so a
   draft or a pre-release is never offered.

### Hotfixes

An urgent fix to the released version starts from `main` instead:

```powershell
pwsh scripts/release/start-release.ps1 -Version 0.4.1 -Hotfix
```

This cuts `hotfix/v0.4.1` from `main` and stops, because a hotfix has no
changelog notes yet. Commit the fix and its `CHANGELOG.md` line, run the
same command again on that branch to set the version, push and open the
pull request into `main`, then carry on from step 4.

A version with a suffix (`0.3.0-beta.1`, `1.0.0-rc.1`) is published as a
pre-release. The updater never offers a pre-release, so install it by hand;
the next full release is offered to it as usual. Windows Installer versions
are numbers only, so `build-installers.ps1` gives the MSI the suffix's last
number as a fourth field (`1.0.0-rc.1` builds MSI version `1.0.0.1`).

## Updates

Installed copies of Stannum update themselves with the
[Tauri updater](https://v2.tauri.app/plugin/updater/). A little after
startup the app downloads `latest.json` from
`https://github.com/SklorGalore/StannumRevival/releases/latest/download/`,
and when its `version` is newer, offers to install it. The user can also
check from the version button at the foot of the module rail or **Check for
updates** in **Ctrl+K**.

Every update is signed with the project's updater key, a minisign key that
has nothing to do with Apple or Windows code signing. The public half is
built into the app, which refuses an update whose signature doesn't match.
The signature also carries the version it was signed for
(`requireSignedVersion`), so an old installer can't be passed off as a new
one.

How a release gets its updates:

1. **Build installers** (`scripts/release/build-installers.ps1`) writes a
   Tauri config overlay (`scripts/release/updater-config.ps1`) with the
   public key, the `latest.json` address and `createUpdaterArtifacts`, and
   builds with it. Tauri then signs each installer with
   `TAURI_SIGNING_PRIVATE_KEY` and writes a `.sig` beside it; on macOS it
   also makes `Stannum.app.tar.gz`, the app the updater installs.
2. **Updater manifest** (`scripts/release/updater-manifest.ps1`) gathers the
   `.sig` files into `latest.json`, with the release notes and a download
   address on the release for each platform and installer kind
   (`windows-x86_64-nsis`, `-msi`, `darwin-aarch64`, `linux-x86_64-appimage`,
   `-deb`, `-rpm`).
3. **Draft GitHub release** attaches `latest.json` with the installers.

Builds that aren't tag pushes (pull requests, release branches, runs by
hand) sign with the real key when it is set, and otherwise with a throwaway
key made in the job, so every platform's updater build and the manifest are
proven before a tag. Throwaway-signed installers can't install real updates.
A tag push without the key builds without the updater and warns.

### The key

The public key is committed as `src-tauri/updater-key.pub` (key id
`AD2CD0DAC66B8C16`); it isn't secret. The private key lives only with the
maintainer and in two repository secrets:

| Secret | Holds |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | The private key file's contents. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Its password, if it has one. |

To set them, from the repository with `gh` logged in as an admin:

```powershell
pwsh scripts/release/setup-updater-key.ps1 -KeyPath ~/.tauri/stannum.key
```

It checks that the key's `.pub` matches `src-tauri/updater-key.pub`, checks
the password by signing a scratch file, and sets both secrets. By hand: in
**Settings → Secrets and variables → Actions → New repository secret**, paste
the key file's contents as `TAURI_SIGNING_PRIVATE_KEY` and the password as
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

Each build checks that its signatures were made with the private half of
`updater-key.pub` and fails if not, so a wrong secret can't ship unusable
updates.

**Back up the key file and its password** (a password manager is a good
place). Copies of Stannum only accept updates signed with this key. If it is
lost, make a new one (`npx tauri signer generate -w <file>`), commit its
`.pub` as `src-tauri/updater-key.pub`, update the secrets, and tell users to
install the next release by hand once. Never commit the private key.

The first release built with the key is the first one installed copies can
update *from*; copies installed before it update by hand once.

### Trying an update before release

`npm run tauri dev` and builds without the overlay have no updater (the
version button says so). To try one, build an old and a new version with an
overlay whose endpoint points at a local server:

```powershell
npx tauri signer generate --ci -p test -w test.key
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content test.key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = 'test'
pwsh scripts/release/updater-config.ps1 -PubKey (Get-Content test.key.pub -Raw) -Out updater.json
# Edit updater.json: endpoints -> http://127.0.0.1:8765/latest.json, and add
# "dangerousInsecureTransportProtocol": true for plain http.
npm run tauri -- build --bundles appimage --config updater.json
```

Install that build, raise the version, build again, write a `latest.json`
for the new build with `updater-manifest.ps1` (or by hand), serve the folder
(`npx http-server -p 8765`) and choose **Check for updates** in the old one.

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

## Signing

The macOS app is signed **ad hoc**: `src-tauri/tauri.macos.conf.json` sets
`bundle.macOS.signingIdentity` to `"-"`, so Tauri seals the whole bundle
without an Apple Developer account, and the release workflow checks the app
inside the `.dmg` with `scripts/release/verify-macos-signature.ps1`. An app
that isn't signed as a bundle is called **damaged** by macOS once downloaded,
with no way past it in Finder (v0.4.1 and older).

An ad hoc signature isn't from an identified developer, so the first launch
still shows a warning, once:

- **macOS 15 (Sequoia) and later:** "Apple could not verify Stannum is free of
  malware". Choose **Done**, open **System Settings > Privacy & Security**,
  scroll to **Security**, choose **Open Anyway** next to Stannum, and confirm.
- **macOS 14 and earlier:** right-click the app in Applications, choose
  **Open**, then **Open** again.
- **Either, from Terminal:** `xattr -dr com.apple.quarantine
  /Applications/Stannum.app` clears the download flag. This also opens the
  older, unsigned builds that macOS calls damaged.
- **Windows** (unsigned): SmartScreen says "Windows protected your PC".
  Choose **More info**, then **Run anyway**.

To sign later:

- **macOS** needs an Apple Developer account. Add the repository secrets
  `APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD`,
  `APPLE_SIGNING_IDENTITY`, and for notarization `APPLE_ID`, `APPLE_PASSWORD`
  (an app-specific password) and `APPLE_TEAM_ID`, then pass them as `env` to
  the **Build installers** step. `APPLE_SIGNING_IDENTITY` takes the place of
  the ad hoc `"-"`, and Tauri signs and notarizes when they are set.
- **Windows** needs a code-signing certificate. Import it on the runner and
  set `bundle.windows.certificateThumbprint` in
  `src-tauri/tauri.windows.conf.json`, or use `bundle.windows.signCommand`
  for a cloud signing service such as Azure Trusted Signing.
