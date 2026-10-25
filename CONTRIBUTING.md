# Contributing

## Branches

`main` is the only long-lived branch. It is protected and should always
build, pass CI and be ready to release.

| Branch | Purpose | Merges into |
|---|---|---|
| `main` | Released and releasable code | — |
| any other name, e.g. `feature/topology-filters`, `fix/export-blank`, `claude/…` | One change each, short-lived | `main` by pull request |
| `release/x.y` | Only when an old release needs a fix | made from the `vx.y.0` tag; fixes merge in by PR, then are also merged to `main` |

Releases are tags on `main` named `vMAJOR.MINOR.PATCH` (for example `v0.3.0`).
Only the maintainer can create release tags, and they can't be moved or
deleted.

This is a lighter form of git flow: there is no `develop` branch, because
every change already passes CI in its own PR before it reaches `main`.

## Pull requests

1. Branch from the latest `main`.
2. Open a PR into `main`. Fill in the template, including screenshots for
   anything visible in the app.
3. CI must pass: **Rust core and desktop shell** and **Frontend build and
   browser smoke test**. Review conversations must be resolved.
4. Merge with **Create a merge commit** (the default) or **Squash and merge**.
   Rebase merging is off.
   Use a merge commit for anything that pulls a subtree into `crates/`, so
   `git subtree` can still find the upstream history.
5. The branch is deleted after merging.

The branch does not need to be up to date with `main` before merging, so
parallel PRs don't have to keep re-running CI. If two PRs conflict, GitHub
shows it and the later one needs `main` merged in.

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

- **Rulesets** from [`.github/rulesets/`](.github/rulesets): `main` and
  `release/**` require a PR with passing CI and block force pushes and
  deletion; `v*` tags can be created only by an admin and never moved or
  deleted. Admins may bypass the branch rules only by merging a PR, never by
  pushing straight to the branch.
- **Security**: Dependabot alerts and security updates, secret scanning with
  push protection, private vulnerability reporting, and CodeQL default setup.
- **Actions**: the `GITHUB_TOKEN` is read-only by default, workflows can't
  approve PRs, only GitHub-owned, verified and listed actions can run, and
  workflows from outside contributors wait for approval.

When a ruleset JSON changes, re-run the script. If you rename a CI job, update
the required check names in `.github/rulesets/` in the same PR, or PRs will
wait forever for a check that never reports.

Dependency updates arrive from Dependabot each Monday
([`.github/dependabot.yml`](.github/dependabot.yml)). Actions in workflows are
pinned to commit SHAs; Dependabot keeps the pins current.
