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
- `five_bus.coordinates.csv` places buses 1 to 3 of `five_bus.raw` at made-up
  Texas locations for the map. It also has a row for a bus the case doesn't
  have (9) and a row that can't be read (bus 4), so buses 4 and 5 are
  estimated from their neighbours.
- `five_bus_substations.raw` is `five_bus.raw` with RAW substation data: North
  (bus 1) and Central (buses 2 and 4) with made-up Texas latitudes and
  longitudes, and East (bus 3) at 0, 0, which means no place. Bus 5 is in no
  substation. North also has one terminal record, which isn't read.
- `five_bus_switches.raw` is `five_bus.raw` with Central 138 kV split into
  three buses joined by system switching devices: breaker "Central BT" (2-6,
  closed), disconnector "Central D1" (6-7, closed) and breaker "Central BC"
  (2-7, open and normally open). The line to East leaves from bus 6, and bus
  7 has a 15 MW load, so power flows through both closed devices.
