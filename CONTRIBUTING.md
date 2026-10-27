# Contributing

## Branches

Stannum uses git flow. `development` is where work comes together; `main`
holds only released code. Both are protected and should always build and
pass CI.

| Branch | Purpose | Made from | Merges into |
|---|---|---|---|
| `main` | Released code; every merge is a release | — | `development` (back-merge after each release) |
| `development` | The next release, as it is built | — | `release/vX.Y.Z` is cut from it |
| any other name, e.g. `feature/topology-filters`, `fix/export-blank`, `claude/…` | One change each, short-lived | `development` | `development` by pull request |
| `release/vX.Y.Z` | One release being stabilised: version bump and fixes only | `development` | `main` by pull request |
| `hotfix/vX.Y.Z` | An urgent fix to the released code | `main` | `main` by pull request |

Releases are tags on `main` named `vMAJOR.MINOR.PATCH` (for example `v0.3.0`),
on the merge commit of a release or hotfix branch. Only the maintainer can
create release tags, and they can't be moved or deleted.
`scripts/release/start-release.ps1` and `finish-release.ps1` do the branch,
tag and back-merge steps; see [docs/releasing.md](docs/releasing.md).

## Pull requests

1. Branch from the latest `development`.
2. Open a PR into `development`. Fill in the template, including screenshots for
   anything visible in the app.
3. CI must pass: **Rust core and desktop shell** and **Frontend build and
   browser smoke test**. Review conversations must be resolved.
4. Merge with **Create a merge commit** (the default) or **Squash and merge**.
   Rebase merging is off. Release, hotfix and back-merge PRs (into `main`, or
   from `main` into `development`) always use a merge commit.
   Use a merge commit for anything that pulls a subtree into `crates/`, so
   `git subtree` can still find the upstream history.
5. The branch is deleted after merging.

The branch does not need to be up to date with `development` before merging,
so parallel PRs don't have to keep re-running CI. If two PRs conflict, GitHub
shows it and the later one needs `development` merged in.

## Documentation

The user guide and technical manual live in [`docs-site/`](docs-site), an
[Astro Starlight](https://starlight.astro.build/) site published to
<https://sklorgalore.github.io/StannumRevival/>. Change the docs in the same PR
as the feature they describe. The **Docs** workflow builds the site on PRs that
touch it and publishes it when they merge; it is not a required check.

```sh
cd docs-site
npm ci
npm run dev     # preview at http://localhost:4321/StannumRevival/
```

Screenshots for the docs come from the browser smoke test
(`STANNUM_SHOTS=dir`); copy the ones a page uses into
`docs-site/src/assets/screens/`.

## Repository settings

The rules above are stored in the repository and applied with
[`scripts/setup-github-repo.ps1`](scripts/setup-github-repo.ps1), which needs
the GitHub CLI signed in as an admin:

```powershell
pwsh scripts/setup-github-repo.ps1 -WhatIf   # show what would change
pwsh scripts/setup-github-repo.ps1           # apply
pwsh scripts/setup-github-repo.ps1 -Audit    # print current settings
```

It sets:

- **Rulesets** from [`.github/rulesets/`](.github/rulesets): `main`,
  `development`, `release/**` and `hotfix/**` require a PR with passing CI and block force pushes and
  deletion; `v*` tags can be created only by an admin and never moved or
  deleted. Admins may bypass the branch rules only by merging a PR, never by
  pushing straight to the branch.
- **Security**: Dependabot alerts and security updates, secret scanning with
  push protection, private vulnerability reporting, and CodeQL default setup.
- **Actions**: the `GITHUB_TOKEN` is read-only by default, workflows can't
  approve PRs, only GitHub-owned, verified and listed actions can run, and
  workflows from outside contributors wait for approval.
- **Pages**: the documentation site is published by the **Docs** workflow
  rather than from a branch.

When a ruleset JSON changes, re-run the script. If you rename a CI job, update
the required check names in `.github/rulesets/` in the same PR, or PRs will
wait forever for a check that never reports.

Dependency updates arrive from Dependabot each Monday
([`.github/dependabot.yml`](.github/dependabot.yml)). Actions in workflows are
pinned to commit SHAs; Dependabot keeps the pins current.
