---
title: Verification
description: How Stannum's solver and parsers are tested against MATPOWER and real cases.
---

## Against MATPOWER 8.1

rs-power's numerical behaviour targets the **MATPOWER 8.1 legacy core**
(AC power/polar Newton and DC power flow, constant-power loads). Reference
results were generated with MATPOWER 8.1 at a $10^{-10}$ mismatch tolerance
and are checked in as fixtures, so ordinary tests need no MATLAB, Octave or
MATPOWER installation.

| Quantity | Agreement required |
| --- | --- |
| Voltage magnitude | $10^{-6}$ p.u. |
| Voltage angle | $10^{-5}$ degrees |
| Power | $10^{-4}$ MW or Mvar |

Fixtures cover case9, case14, case30, case57, case118 and case300, plus
synthetic case9 variants for reactive limits, slack reassignment, shared
generators, infinite and zero-width reactive ranges, taps, phase shifts,
shunts, offline equipment, parallel branches and nonconsecutive bus numbers.

## RAW conversion

Conversion tests compare every numeric bus, generator, branch and DC line
field, and bus names, against MATPOWER 8.1's importer. They cover all
supported revision layouts, every transformer `CW`/`CZ` combination, partial
winding statuses, DC control modes and three upstream RAW fixtures.

## Numerical checks

- Analytic Jacobians against finite differences.
- Nodal power balance of solved cases.
- Singular systems, invalid inputs and nonconvergence.

## Meshed VSC grids

The station model reproduces every converter in Tables II and III of Beerten,
Cole and Belmans (2012) from its system-bus values. The DC network, droop
control and converter limits are checked against hand solutions, and a demo
case with three asynchronous AC grids and two meshed DC grids is checked for
power balance and set-points. See [Meshed VSC
grids](/StannumRevival/manual/hvdc/vsc-grids/#verification).

## Large cases

Optional tests (run with `--ignored`) solve large public cases that are not
stored in the repository; `scripts/fetch-test-cases.ps1` downloads them and
checks their hashes.

| Case | Size | Checks |
| --- | --- | --- |
| Texas A&M SyntheticUSA (RAW 33) | 82,000 buses | Q limits, remote regulation, 9 LCC lines, 150 continuous shunts, 291 taps; compared with the saved PowerWorld solution |
| ACTIVSg70k (MATPOWER) | 70,000 buses | Solve speed |
| Synthetic East–West US (RAW 34) | 80,000 buses | Revision 34 import |
| Memphis 2026 (RAW 35) | 993 buses | Shunt control, remote regulation, nodal parsing |

## Interface

A browser smoke test drives the real interface in Chromium against the real
Rust core on every pull request: opening MATPOWER and RAW cases, solving,
sensitivities, studies, every module, side by side, the palette, the log, DC
lines and VSC grids. The screenshots in this documentation come from that test.
