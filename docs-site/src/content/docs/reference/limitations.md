---
title: What isn't modelled
description: Known gaps in Stannum's models and data coverage.
---

Stannum aims to say plainly what it does not do. Import warnings name any of
these that a case uses.

## Power flow

- Optimal power flow, continuation power flow, contingency analysis (beyond
  manual [studies](/StannumRevival/guide/studies/)), short circuit and dynamics.
- PSS/E's low-voltage load conversion (`PQBRAK`): constant-power and
  constant-current loads keep their characteristic at any voltage.
- Alternative AC formulations (current injection, Cartesian), fast-decoupled
  and radial methods, three-phase models.
- Enforcement of voltage, branch rating, angle-difference and generator P
  limits (they are checked in results only).
- Inter-area transfer schedules (the RAW inter-area transfer data); area
  interchange holds each area's total `PDES` only.

## Controls

- Transformer controls other than voltage (`COD` 2–5): reactive power, active
  power, DC line and asymmetric controls.
- Impedance correction tables and magnetizing admittance.

## HVDC

- LCC: capacitor-commutated converters (`XCAP`), the AC transformer named by
  `IF`/`IT`/`ID`, separate commutating buses (`IC`), `DCVMIN`.
- VSC: remote voltage regulation (`REMOT`, `RMPCT`).
- Multi-terminal: `DCPF`/`MARG` order sharing, `RGRND`.
- Meshed VSC grids: grid-forming converters for islands with no generator,
  enforced limits on slack and droop converters, and a unified AC/DC Newton.
- Sensitivities with detailed DC models.

## Data

- FACTS devices in modes 3 to 8 (constant series impedance or voltage,
  IPFC); the series element's voltage and current limits and reactance.
- GNE devices, induction machines.
- RAWX (the JSON form of RAW). RAW export writes revisions 33 and 35 only;
  DC lines are left out of RAW files written from a MATPOWER case.
- Topology from RAW files other than revision 35 has no switching devices.
- Case diff for revisions other than 33 and 35.
