# Fuzzing the file readers

Stannum opens files people hand it, so every reader must turn a bad file
into an error, never a panic. These [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)
targets feed the readers random and mutated files to check that.

| Target | What it reads |
| --- | --- |
| `raw` | A PSS/E RAW file opened as Stannum opens one: rs-power (power flow), nodal (topology), Records and Case diff, then the summary, topology, diagram and map views |
| `matpower` | A MATPOWER `.m` file opened the same way |
| `run_file` | A `.stnrun` run file, parsed and written back out |
| `coordinates` | A bus coordinates CSV, decoded as UTF-8 or Windows-1252 |
| `stn` | A Stannum case (`.stn`, `.stnb`) or lists (`.stnl`) file: read, written canonically and read back, through the other form, then opened as a case |

The crate has its own workspace, since cargo-fuzz needs the nightly
toolchain and sanitizers; the main build never compiles it.

## Running

```
rustup toolchain install nightly
cargo install cargo-fuzz --locked
pwsh scripts/fuzz.ps1                       # each target for a minute
pwsh scripts/fuzz.ps1 -Target raw -Seconds 3600 -Jobs 4
```

The script seeds each target with the test fixtures of its kind and the
token dictionary in `dicts/`, and keeps the corpus it grows in
`corpus/<target>` for the next run. The
[Fuzz file readers](../.github/workflows/fuzz.yml) workflow runs every target
for ten minutes each week, and on request.

## When it finds a crash

The input lands in `artifacts/<target>/`. Then:

1. Reproduce it: `cargo +nightly fuzz run <target> artifacts/<target>/crash-...`
   (`cargo +nightly fuzz tmin` shrinks it first).
2. Fix the reader so the file gives an error, or is read sensibly.
3. Add the input as a test in `crates/stannum-core/tests/bad_files.rs`, so
   the normal test run keeps it fixed.
