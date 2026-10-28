---
title: PSS/E RAW
description: Which RAW revisions Stannum reads, how each engine reads them, and how RAW data converts to the power flow model.
---

PSS/E RAW is Siemens PTI's text format for power flow cases. Stannum reads
it with three parsers, one per engine, each with its own coverage.

| Engine | Revisions | Used by |
| --- | --- | --- |
| rs-power | 23 to 35, including 34 | Power flow, Diagram, Ctrl+K search, topology for non-35 files |
| nodal | 35 | Topology |
| case-diff | 33 and 35 | Case diff |

## Reading the file (rs-power)

- **Revision** comes from the case identification record or its PSS/E
  comment. An unidentified revision defaults to 23.
- **Delimiters**: commas or whitespace; quoted names; mixed line endings.
- **Sections** are assigned in the revision's defined order using their `0`
  terminators; `@!` comments and section headers are ignored. `Q` ends the
  input and is optional after a terminator; trailing empty sections may be
  left out.
- Malformed required data returns an error with its line number.
- Change-code files warn and are treated as standalone cases, never as
  incremental updates.

## Converting to the power flow model

The conversion follows **MATPOWER 8.1's RAW importer** (`psse2mpc`),
including its approximations, with the extensions marked below.

| Data | Conversion |
| --- | --- |
| Buses | Sorted by number; names kept. Type 4 is isolated. |
| Loads | Summed per bus. Constant-current (`IP`, `IQ`) and constant-admittance (`YP`, `YQ`) parts are kept, and the solve varies them with voltage. MATPOWER freezes them as constant power at the file's voltage. *Extension.* |
| Fixed shunts | Summed into the bus shunt. |
| Switched shunts | `BINIT` goes into the bus shunt, including offline records, as in MATPOWER. Those in voltage control (MODSW 1, 2) are also kept for [voltage control](/StannumRevival/manual/voltage-controls/). *Extension.* |
| Areas | `I`, `ISW`, `PDES`, `PTOL`, `ARNAME` kept for [area interchange control](/StannumRevival/manual/area-interchange/), with loads whose `AREA` differs from their bus's. *Extension.* |
| Generators | One generator per record. `IREG` and `RMPCT` kept for [remote regulation](/StannumRevival/manual/remote-regulation/). *Extension.* |
| Lines | π model; negative metered-end IDs normalized. Unknown endpoints kept with a warning. |
| Two-winding transformers | All `CW`/`CZ` combinations (1–3); impedance on the winding nominal voltage base (MATPOWER `use_winding_baseV = 1`), including its zero-`NOMV` behaviour. `COD` = 1 windings kept for [tap control](/StannumRevival/manual/voltage-controls/#tap-changers). *Extension.* |
| System switching devices (revisions 34, 35) | A lossless branch of reactance `X` (PSS/E's 0.0001 p.u. when blank or zero), in service when `STAT` is 1, with `RATE1` as its rating. Ids are `switch:I:J:CKT`. Skipped by MATPOWER. Default N-1 runs leave them out; list one to run it. *Extension.* |
| Three-winding transformers | A star bus with MATPOWER's numbering, three branches, and winding statuses. |
| Two-terminal DC | MATPOWER's lossless row with estimated reactive limits, plus every field for the [detailed LCC model](/StannumRevival/manual/hvdc/lcc/). LCC reactive limits stored as absorbed. *Extension.* |
| VSC DC | Skipped by MATPOWER; Stannum adds a row and the [detailed VSC model](/StannumRevival/manual/hvdc/vsc/). *Extension.* |
| Multi-terminal DC | Skipped by MATPOWER; Stannum adds rows and the [detailed MTDC model](/StannumRevival/manual/hvdc/mtdc/). *Extension.* |

### Ignored with a warning

Transformer magnetizing admittance, impedance correction tables, tap
controls other than COD 1, switched shunt modes 3–6, wind control, FACTS
devices, GNE devices, induction machines and other unsupported equipment
sections. Ownership, zone, interchange and grouping metadata are not kept.

Every warning is listed with its line under **Import warnings** in Power flow.
Warning wording and counts are not MATPOWER-compatible.

## Revision 34

Revision 34 shares revision 35's newer record layouts (system-wide data,
wider branch and transformer records). PowerWorld writes it for some large
synthetic cases, such as the Texas A&M East–West case.

## nodal (Topology)

nodal reads revision 35 only and keeps every record. Field positions and
section order come from `raw.schema.json`. UTF-8 is tried first, then
Windows-1252. Missing status fields default to in service, bus type to PQ,
circuit identifiers to `1`, injections to zero, and frequency to 60 Hz; bus
references, base voltage and series reactance are required.

RAWX, DC connectivity and substation node-breaker topology are outside nodal.

## case-diff

See [Case diff](/StannumRevival/manual/case-diff/#parsing): revisions 33 and 35,
verified layouts only.
