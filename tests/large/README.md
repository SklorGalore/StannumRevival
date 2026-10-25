# Large test cases

Three Texas A&M synthetic grids exercise the solver at real scale. They are too
large to commit, and their license does not say they may be redistributed, so
only their names and SHA-256 hashes are in the repo (`cases.json`).

| File | Size | Used for |
| --- | --- | --- |
| `SyntheticUSA.RAW` | 82,000 buses, RAW rev 33, 9 two-terminal LCC lines | DC lines at scale (plan phases 0 and 1) |
| `Synthetic_East_West_US.RAW` | 80,000 buses, RAW rev 34, no DC | Revision 34 import, no-DC regression, speed |
| `case_ACTIVSg70k.m` | 70,000 buses, MATPOWER, no DC | No-DC regression, speed |

Fetch them from a folder or a private URL, then run the ignored tests:

```powershell
pwsh scripts/fetch-test-cases.ps1 -Source "<folder or https:// base URL>"
cargo test --release -p rs-power --test large_cases -- --ignored
```

`pwsh scripts/fetch-test-cases.ps1 -Check` verifies what is already here. The
tests read this folder, or `STANNUM_LARGE_CASES` when set.
