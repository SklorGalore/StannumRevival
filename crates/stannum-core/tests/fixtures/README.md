# Test fixtures

- `case9.m` is MATPOWER's 9-bus case, copied from rs-power's fixtures with its
  original provenance header. MATPOWER case data is excluded from MATPOWER's
  software license; see `MATPOWER-LICENSE` and the notice in the file.
- `five_bus.raw` and `five_bus_modified.raw` are synthetic PSS/E RAW v35 cases
  written for these tests. The modified case renumbers the spur bus (5 → 6),
  changes one load and one branch rating, so the comparison has added,
  removed and changed records in several sections.
