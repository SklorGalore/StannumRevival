---
title: Coming from PSS/E
description: Where each PSS/E activity lives in Stannum, which settings match, and what Stannum doesn't do.
---

Stannum is meant to sit beside PSS/E, not to replace it. It reads the RAW
files PSS/E writes, solves them to PSS/E's tolerance, and checks itself
against the voltages saved in the file. This page maps what you do in PSS/E
to where it is in Stannum, and says plainly what isn't there.

## Getting your case in

- **Save RAW, not SAV.** Stannum reads RAW revisions 23 to 35 (see
  [PSS/E RAW](/StannumRevival/reference/psse-raw/)). A `.sav` file is
  PSS/E's own binary format and can't be read; in PSS/E use **File → Save /
  Show**, or `psspy.rawd_2`, to write a RAW file.
- **Check the import.** Opening a case lists every record Stannum read
  differently from PSS/E or couldn't use, in the
  [Log](/StannumRevival/guide/log/). [What isn't modelled](/StannumRevival/reference/limitations/)
  has the full list.
- **Check the solution.** Solve, then press **Compare with the file**
  under **Against the voltages in the file** on the Power flow overview. A RAW file
  saved after a solve holds PSS/E's voltages, so this shows how close
  Stannum's solution is on your own case: the mean and largest difference,
  and the buses that differ most. The
  [Verification](/StannumRevival/reference/verification/) page has the same
  check on public cases.
- **Write RAW back.** **File → Export case → PSS/E RAW 35…/33…** writes the
  case, with any edits, for PSS/E to read.

## Where things are

| In PSS/E | In Stannum |
| --- | --- |
| Solve (FNSL, FDNS, NSOL) | [Power flow](/StannumRevival/guide/power-flow/): **Solve**, method Auto, Newton, FDNS or fast decoupled XB/BX |
| Bus, branch and machine data (spreadsheet view) | [Records](/StannumRevival/guide/records/), cell by cell, and the Power flow tables |
| Slider diagrams | [Diagram](/StannumRevival/guide/diagram/), drawn around any bus or by zone or owner |
| ACCC with `.sub`, `.mon`, `.con` | [Contingencies](/StannumRevival/guide/contingencies/), with subsystems, monitors and contingencies in a [lists file](/StannumRevival/reference/lists-file/) (`.stnl`) |
| TPL-001 planning events | **Planning events** in the Contingencies setup (P0, P1, P2.1, P7.2) |
| TLTG, DFAX (`.dfx`) | [Transfer limits](/StannumRevival/guide/transfers/), with transfers kept in a lists file |
| PV and QV analysis | [PV curves](/StannumRevival/guide/pv-curves/) and [QV curves](/StannumRevival/guide/qv-curves/) |
| SCAL | [Scaling load and generation](/StannumRevival/guide/scaling/) |
| Case comparison | [Case diff](/StannumRevival/guide/case-diff/), record by record |
| Subsystems (BSYS) | Areas, zones, owners, buses and a kV range, chosen in each setup |
| psspy and IDV files | The [stannum Python package](/StannumRevival/guide/python/) and [run files](/StannumRevival/guide/run-files/); see [Coming from psspy](/StannumRevival/guide/coming-from-psspy/) |

## Settings that match

| PSS/E | Stannum |
| --- | --- |
| Mismatch tolerance (TOLN), 0.1 MVA | **Mismatch tolerance (MVA)**, 0.1 by default |
| Flat start | **Flat start** |
| Var limits | **Enforce generator Q limits**, on by default |
| Tap adjustment | **Move transformer taps** |
| Phase shift adjustment | **Move phase shifters** |
| DC tap adjustment | **Adjust DC converter taps** |
| Switched shunt adjustment | **Move switched shunts** |
| Area interchange control: disabled, tie lines only, tie lines and loads | **Area interchange**: Off, Tie lines, Tie lines and loads |
| Zero-impedance line threshold (THRSHZ), 0.0001 p.u. | Same value, under **Zero-impedance lines** in the Contingencies setup |
| Rating sets A, B, C | **Base case rating** and **Contingency rating**: A, B or C |
| Non-conforming loads left out of SCAL | **Also scale non-conforming loads**, off by default |

Contingency names follow PSS/E's style (`ALPHA 101 BETA 102 1`), and the
default contingency method solves every outage in AC when a run has at most
2,000 of them, as ACCC does.

## What isn't there

These are the things a PSS/E user is most likely to look for. The full list
is in [What isn't modelled](/StannumRevival/reference/limitations/).

- Dynamics, short circuit and optimal power flow.
- Reading `.sav`, `.seq`, `.dyr`, `.con`, `.mon`, `.sub` and `.dfx` files.
  Contingencies, monitors, subsystems and transfers are written in a
  [lists file](/StannumRevival/reference/lists-file/) instead.
- Corrective actions in contingency analysis, and planning events that need
  breaker-level detail, two stages or tower data (P2.2 to P2.4, P3 to P6,
  P7.1).
- RAW 35 ratings 4 to 12; only rates A, B and C are read.
