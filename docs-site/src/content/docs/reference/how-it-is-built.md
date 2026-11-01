---
title: How Stannum is built and tested
description: Who builds Stannum, how a change gets in, what is checked on every change, and how releases are supported.
---

Stannum is young, open source and built by a small team, with much of the
code written by an AI coding assistant under review. This page says what
stops a wrong answer from shipping, so you can judge how far to rely on it.

## How a change gets in

- Every change is a pull request into `development`. Nothing is pushed
  straight to `development` or `main`.
- Each pull request describes the change with screenshots of anything
  visible in the app, and the maintainer reviews it before merging.
- `main` holds released code only, one squashed commit per release, and
  release tags can't be moved or deleted. See
  [CONTRIBUTING.md](https://github.com/SklorGalore/StannumRevival/blob/main/CONTRIBUTING.md).

## What every pull request must pass

| Check | What it catches |
| --- | --- |
| `cargo fmt`, `cargo clippy -D warnings` | Formatting and common Rust mistakes |
| Rust tests of the core, solver, topology, file formats, CLI and synthetic cases | Wrong results against fixed references, broken commands and files |
| Reference tests on the [Verification](/StannumRevival/reference/verification/) page | Solver results drifting from MATPOWER, saved PowerWorld solutions, a published paper and MatACDC |
| Time and memory budgets on a 12,000-bus synthetic case | A change that makes opening, solving or N-1 much slower |
| Python wheel built and tested on Windows, macOS and Linux | The Python package breaking on any platform |
| Frontend type check, unit tests and a browser smoke test against the real Rust core | The app failing to open, solve or show results |

A failing check stops the pull request from merging.

## What runs on a schedule

- **Fuzzing**, weekly: random and mutated RAW, MATPOWER, run, coordinates
  and Stannum case files fed to every reader. A bad file must give an
  error, never a crash.
- **Large case budgets**, weekly: the same budgets at 80,000 buses.
- **Verification tables**, on every build of this site: the reference tests
  run again and write the numbers the Verification page shows.

## Clean-room rule

Some reference tools forbid redistribution or reuse of their code. Stannum
models are written from published papers and manuals, never from restricted
source code, and tests compare against those tools' results only where
their licences allow.

## What it is for

Stannum is a workbench beside the tool of record (PSS/E or PowerWorld), not
a replacement for compliance studies. [What isn't
modelled](/StannumRevival/reference/limitations/) lists the gaps, and
import warnings name any your case uses. Check results that matter against
your tool of record.

## Releases and support

Versions follow the [compatibility policy](/StannumRevival/reference/versioning/).
Fixes go into the next release; a serious problem in the latest release
gets a patch release. Report security problems as described in
[SECURITY.md](https://github.com/SklorGalore/StannumRevival/blob/main/SECURITY.md)
and anything else as a [GitHub
issue](https://github.com/SklorGalore/StannumRevival/issues).
