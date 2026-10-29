# Sample cases

Small cases built into Stannum (`stannum_core::samples`, commands
`sample_cases` and `open_sample`), so there is something to open on first
run. They are compiled into the core with `include_str!`; opening one writes
a fresh copy to disk and opens that.

| File | Where it comes from |
| --- | --- |
| `case9.m` | MATPOWER 8.1 `data/case9.m`, unchanged, with its provenance header. MATPOWER's case data is excluded from its software licence and from Stannum's; see `THIRD-PARTY-NOTICES.md`. |
| `five_bus.raw` | Stannum's synthetic five-bus RAW 35 test case (`tests/fixtures/five_bus.raw`), retitled. Made-up data. |
| `five_bus_devices.raw` | `tests/fixtures/raw_export/five_bus_everything.raw`, retitled: the five-bus case with a three-winding transformer, a breaker, switched shunts and an impedance correction table. Made-up data. |
| `five_bus.coordinates.csv` | Made-up Texas places for the five-bus buses (both five-bus samples use it; the tertiary bus of the devices case is placed from its neighbours). |
| `synthetic_200.m`, `synthetic_200.coordinates.csv` | `pwsh scripts/new-synthetic-case.ps1 -Buses 200 -Seed 1`, with a header comment added. Synthetic data from the method of Birchfield et al. (IEEE TPWRS 32(4), 2017); describes no real grid. |

`tests/samples.rs` checks that every sample opens, places every bus and
solves.
