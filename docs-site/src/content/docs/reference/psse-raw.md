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

## Writing RAW

**Export PSS/E RAW** writes revision 33 or 35. How depends on the open case.

### From a RAW 33 or 35 file

The working case (the file with its Records edits and study edits written
in, as [Case diff](/StannumRevival/guide/case-diff/) compares it) is read
record by record with case-diff's parser and written out again. Records
whose layout is the same in both revisions keep their lines exactly,
including spacing and number formats; records the parser keeps without
decoding (fixed shunts, VSC and multi-terminal DC, FACTS devices and the
like) are copied as they are. Revision 35 substation data is copied into a
revision 35 file. Writing the file's own revision changes nothing: Case diff
finds no differences between the file and its export.

Converting between the revisions:

| Data | 35 → 33 | 33 → 35 |
| --- | --- | --- |
| System-wide data (solution settings, rating names) | Dropped | None written |
| Loads | Distributed generation (`DGENP`, `DGENQ`, when `DGENM` is 1) netted from `PL` and `QL`; load type dropped | `DGEN*` zero, load type blank |
| Generators | `NREG` and `BASLOD` dropped | Both zero |
| Branches | `RATE1`–`RATE3` become `RATEA`–`RATEC`; ratings 4 to 12 and names dropped | The reverse; ratings 4 to 12 zero |
| System switching devices | Branches with an `@` (breaker, `STYPE` 2) or `*` (switch) circuit, keeping status, reactance and ratings | Branches with an `@` or `*` circuit become switching devices again |
| Transformers (two- and three-winding) | Winding ratings 1–3 become `RATA`–`RATC`; ratings 4 to 12 and `NOD` dropped | The reverse |
| Two-terminal DC | `NDR`, `NDI` dropped | Zero |
| Switched shunts | One per bus: the first keeps its control (`SWREG` → `SWREM`, blocks out of service left out); others become fixed shunts at `BINIT` | `ID` 1, `SWREG` from `SWREM`, every block in service |
| Impedance correction tables | Up to 11 real (T, F) points | (T, Re F, 0) points, six to a line, ended by 0, 0, 0 |
| Substation data | Dropped | None |

Every loss is listed after the export. The impedance correction and
three-winding transformer layouts follow the PSS/E manual; no example file
holds one to check them against.

### From anything else

MATPOWER cases, RAW revisions other than 33 and 35, and RAW files
case-diff's parser can't read (such as records with trailing fields left
out) are written from the power flow model, with the study's edits applied:
buses, one load per bus (with its constant-current and constant-impedance
parts), fixed shunts, generators, lines, two-winding transformers on the
system base, switched shunts and tap changers in voltage control, areas,
zones and one owner. Three-winding transformers are written as two-winding
transformers to their star buses, and transformer line charging as fixed
shunts at their buses (the same π model). DC lines, generator costs and
anything else the model doesn't hold are left out, with a note.
