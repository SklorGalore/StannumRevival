---
title: What isn't modelled
description: Known gaps in Stannum's models, controls, file formats and drawings.
---

Stannum aims to say plainly what it does not do. Import warnings name any of
these that a case uses, and every RAW export lists what the file couldn't
keep.

## Studies

- Optimal power flow, continuation power flow, short circuit, dynamics and
  three-phase models.
- [Contingency analysis](/StannumRevival/manual/contingencies/) takes out one
  branch or generator at a time (a three-winding transformer counts as one),
  two at a time (N-2, up to 250,000 pairs), each bus on its own, or the
  NERC TPL-001 planning events P0, P1, P2.1 and P7.2. N-3 and up, planning
  events that need breakers, two stages or tower data (P2.2 to P2.4, P3 to
  P6, P7.1) and remedial actions are not run. A bus outage opens the
  branches and DC lines at the bus; FACTS devices and VSC grid converters
  there stay in. Loading is checked against rate A, B or C
  as chosen; RAW 35 ratings 4 to 12 are not read.
- The linear contingency screen leaves out resistance and phase shifts, and
  finds overloads only; voltage problems and reactive limits come from the
  AC checks.
- [Sensitivities](/StannumRevival/manual/sensitivities/) linearize the solved
  case with every control and DC line held where the solve left them.
  Sensitivities with meshed VSC grids are not available.

## Power flow

- The AC methods are Newton and the decoupled family (see
  [AC solution methods](/StannumRevival/manual/ac-methods/)). Current
  injection and Cartesian formulations and radial (backward/forward sweep)
  methods are not included.
- Voltage, branch rating, angle-difference and generator active power
  (`PMAX`, `PMIN`) limits are checked in the results, not enforced. That
  includes generation moved by [area interchange
  control](/StannumRevival/manual/area-interchange/).
- PSS/E's low-voltage load conversion (`PQBRAK`): constant-power and
  constant-current loads keep their characteristic at any voltage.
- Area interchange holds each area's total `PDES`. The RAW inter-area transfer
  data (scheduled transfers between pairs of areas) is skipped, and an area
  that holds an island's reference bus is not held.
- Switched shunts, transformer taps, phase shifters, FACTS devices and area
  interchange act in AC solves. In a DC (linear) solve they stay put; only a
  FACTS series element's active power is applied.

## Controls

- Transformer magnetizing admittance given as no-load loss and exciting
  current (`CM` 2) uses `SBASE1-2` but not `NOMV1`: the admittance is taken
  at the bus base voltage.
- DC line transformer taps (`COD` 4) on three-winding transformers.
- FACTS devices in modes 3 to 8 (constant series impedance or voltage,
  IPFC). The series element's voltage and current limits (`VSMX`, `IMX`,
  `TRMX`, `VTMN`, `VTMX`) and reactance (`LINX`) are not used.
- A revision 35 MODSW 5 switched shunt reads `RMIDNT` as the ID of the shunt
  it watches; check one against PSS/E before relying on it.

## HVDC

- LCC: capacitor-commutated converters (`XCAP`), separate commutating buses
  (`IC`), `DCVMIN`.
- VSC: remote voltage regulation (`REMOT`, `RMPCT`).
- Multi-terminal: `DCPF`/`MARG` order sharing, `RGRND`.
- Meshed VSC grids: grid-forming converters for islands with no generator,
  enforced limits on slack and droop converters, DC breakers, and a unified
  AC/DC Newton.

## File formats

- RAWX (the JSON form of RAW).
- RAW import skips GNE devices, induction machines, wind control, and zone
  and owner names. Multi-section line groupings are read only for
  contingencies. See [PSS/E RAW](/StannumRevival/reference/psse-raw/).
- RAW export writes revisions 33 and 35 only. From a RAW 33 or 35 file it
  keeps every record; the three-winding transformer layout follows the
  PSS/E manual but has not been checked against a file that holds one.
- RAW written from the power flow model (MATPOWER cases and RAW revisions
  other than 33 and 35) leaves out multi-terminal DC, MATPOWER's DC line
  rows, loads assigned to another area than their bus's, and generator
  costs. Three-winding transformers become two-winding transformers to their
  star buses.
- Case diff compares RAW revisions 33 and 35, in layouts it has verified.
  Content in other layouts is listed as unsupported.

## Topology and drawings

- Topology reads revision 35 RAW files itself. For other files it is built
  from the power flow model: revision 34 switching devices connect their
  buses but count as lines, loads are summed per bus, and the original
  records aren't shown.
- Substation node-breaker topology and DC connectivity are not analysed.
- Generated one-lines don't draw FACTS devices. Generic switching devices
  (`STYPE` 1) are drawn as plain wires.
- Substation data is not shown in Records or Case diff.

## Large cases

Cases of 80,000 buses and more open, solve and page their tables quickly
(see [Verification](/StannumRevival/reference/verification/)). Some views
still read the whole solve report at once (the Power flow overview, the
diagram's results and Compare), and a full N-1 run of such a case takes
several minutes.
