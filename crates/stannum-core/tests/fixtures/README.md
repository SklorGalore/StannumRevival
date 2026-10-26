# Test fixtures

- `case9.m` is MATPOWER's 9-bus case, copied from rs-power's fixtures with its
  original provenance header. MATPOWER case data is excluded from MATPOWER's
  software license; see `MATPOWER-LICENSE` and the notice in the file.
- `five_bus.raw` and `five_bus_modified.raw` are synthetic PSS/E RAW v35 cases
  written for these tests. The modified case renumbers the spur bus (5 → 6),
  changes one load and one branch rating, so the comparison has added,
  removed and changed records in several sections.
- `five_bus_dc.raw` is `five_bus.raw` with a two-terminal DC line, "East
  link", from bus 1 to bus 3. `five_bus_dc_modified.raw` changes its
  scheduled power (SETVL) and adds a second DC line, "Spur link", from bus 5
  to bus 2.
- `five_bus_zip.raw` is `five_bus.raw` with voltage-dependent loads: bus 2's
  90 + j30 load split evenly into constant power (PL, QL), constant current
  (IP, IQ) and constant admittance (YP, YQ), and half of bus 4's load
  constant admittance. Both draw the original load at 1 p.u.
