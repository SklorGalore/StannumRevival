---
title: Case file format (.stn)
description: "The .stn text format: Stannum's own open case format, one record per line with named fields, stable ids and units in the names."
---

:::caution[Preview]
The Stannum case format is a **preview** (format version 0.1). Any Stannum
release may change it until format 1.0, which is planned to freeze with
Stannum 2.0. Keep the RAW or MATPOWER file you converted from, and convert
again after an update if a file stops opening. PSS/E RAW and MATPOWER stay
fully supported for reading and writing.
:::

A Stannum case file (`.stn`) holds a whole case, the network and its
operating state, as UTF-8 text. It is meant to be read, edited and compared
by people and by any tool:

- one record per line, so a version-control diff shows exactly which
  equipment changed;
- every field is named, so nothing depends on column positions;
- each record has a stable id (`205:310:1` for a line) next to its bus
  numbers;
- units are in the field names (`x_pu`, `p_mw`, `kv`), never in codes;
- comments are kept, and Stannum writes records in a fixed order.

Every table and field, with its type, unit and default, is listed in
[Case file tables](/StannumRevival/reference/case-file-schema/). That page
is generated from the schema in
`crates/stannum-format/schema/stannum-case.toml`, which is the source of
truth for the format.

Contingencies, monitored elements, subsystems and interfaces are **not** in
the case. They go in a separate [lists file](/StannumRevival/reference/lists-file/)
(`.stnl`), so the same lists can run against any case.

## Example

The five-bus test case with its devices, shortened:

```text
stannum-case 0.1
# Summer peak, base case.

# case
case main name="Stannum five-bus test case" description="Two areas, one transformer"

# area
area 1 number=1 name=North slack_bus=1 tolerance_mw=1
area 2 number=2 name=East slack_bus=3 tolerance_mw=1

# bus
bus 1 number=1 name=North_138 kv=138 type=ref vm_pu=1.04
bus 2 number=2 name=Central_138 kv=138 va_deg=-2
bus 3 number=3 name=East_138 kv=138 type=pv area=2 vm_pu=1.02 va_deg=-1
bus 4 number=4 name=Central_69 kv=69 va_deg=-4
bus 5 number=5 name=Spur_69 kv=69 area=2 vm_pu=0.99 va_deg=-5

# load
load 2:1 bus=2 p_mw=90 q_mvar=30

# generator
generator 1:1 bus=1 qmax_mvar=200 qmin_mvar=-100 vset_pu=1.04 pmax_mw=300 pmin_mw=0

# line
# Rebuilt 2024.
line 1:2:1 from=1 to=2 r_pu=0.01 x_pu=0.06 b_pu=0.03 rate1_mva=150 rate2_mva=150
line 4:5:1 from=4 to=5 r_pu=0.02 x_pu=0.05 rate1_mva=40

# transformer
transformer 2:4:1 from=2 to=4 r_pu=0.00382 x_pu=0.11262 w1_kv=138 w2_kv=69 rate1_mva=146.6

# switched_shunt
switched_shunt 5:1 bus=5 vmax_pu=1.05 vmin_pu=0.95 reg_bus=5 b_init_mvar=10 blocks=[[2, 10]]
```

## Lines

The first line is the header: `stannum-case` and the format version,
`MAJOR.MINOR`. After it, each line is one of:

- blank;
- a comment: `#` to the end of the line;
- a record: the table name, the record's id, then `name=value` fields
  separated by spaces, in any order. A field may appear once per record.

A comment on the lines just above a record, or at the end of its line,
belongs to that record and moves with it. Comments before the first record,
separated from it by a blank line, stay at the top of the file. Stannum
writes a `# table` comment above each table; it isn't kept as a comment of
the first record.

### Values

| Value | Written as | Examples |
|---|---|---|
| Number | Decimal or exponent form. `inf` and `-inf` where a limit has none. No `nan`; `-0` reads as `0`. | `138`, `-2.5`, `1.2e-7`, `inf` |
| Text | Bare when it is only letters, digits and `_ . : + - / & * @ ! ? ' ( ) < > $ % ^ ~ \|`; otherwise in double quotes, with `\"`, `\\`, `\n`, `\t` and `\r` escapes. | `North_138`, `"HILL 230"` |
| Choice | One of the field's words. | `type=ref`, `status=off` |
| True/false | `true` or `false`. | `dgen_on=true` |
| List | `[` values separated by commas `]`; a list of tuples is a list of lists. | `areas=[1, 2]`, `blocks=[[2, 10], [1, -15.5]]` |

A field left out has the default the schema gives it, so `bus 4 number=4`
is a 0 kV PQ bus in area 1 at 1.0 pu. A field with no default is absent
unless written, and absent means none (no limit, no regulated bus).

## Ids and references

Every record has an id, unique within its kind. Stannum makes it from the
record's natural key, the same key it uses for the element everywhere else
in the app:

| Table | Id | Example |
|---|---|---|
| `bus` | bus number | `205` |
| `load`, `shunt`, `generator`, `switched_shunt` | bus and id | `205:1` |
| `line`, `switch`, `transformer` | both buses, lower first, and circuit | `205:310:1` |
| `transformer3` | three buses and circuit | `205:310:400:1` |
| `area`, `zone`, `owner`, `impedance_table` | number | `3` |
| `lcc_line`, `vsc_line` | both converter buses, lower first, and name | `205:310:PDCI` |
| `facts` | name | `STATCOM_3` |

The id is only made once. When a bus is renumbered, its `number` changes and
its id stays, so other files that name it keep working. Fields that point at
another record (`bus=205`, `from=205`, `slack_bus=1`) hold that record's
id. Lists files and other places that can name any kind of element use a
typed reference, `kind:id`: `bus:205`, `line:205:310:1`, `generator:205:1`.

A reference to a missing bus is an error. Areas, zones and owners may be
named without a record of their own.

## Units

Units are part of each field's name and are fixed:

| Suffix | Unit |
|---|---|
| `_pu` | per unit: on the system MVA base, or for transformer impedances on the transformer's own `mva_base` |
| `_mw`, `_mvar`, `_mva` | MW, Mvar, MVA |
| `_kv` (and `kv`) | kV, line to line |
| `_deg` | degrees |
| `_pct` | percent |

There are no unit codes: RAW's CZ, CW and CM codes are converted on the way
in, and written back on the way out.

## Canonical form

Stannum writes every file the same way, so two files of the same case are
the same text, and a diff shows only real changes:

- tables in the schema's order, and records within a table sorted by number
  or bus, then id;
- fields in the schema's order, with defaults left out;
- numbers in the shortest form that reads back to the same value
  (`0.1`, not `0.10000000000000001`).

Reading a file and writing it again gives the canonical form; reading the
canonical form and writing it gives the same text.

## Extensions

A field or table whose name starts `x.` is an extension. Readers keep
extensions they don't know and write them back, so a tool can add its own
data (`x.planning.note="added 2027"`) without breaking other readers.

Stannum uses `x.psse` for PSS/E data that has no place in the format yet:

- `x.psse.lines` records hold RAW records that aren't mapped, such as GNE
  devices, multi-terminal DC and the system-wide data, with their RAW
  section and text. Writing RAW puts them back.
- `x.psse.orig` on a transformer keeps its original RAW numbers when the
  file used kV ratios, its own MVA base or loss-and-current magnetizing.
  RAW written from an unedited transformer then has the file's own numbers;
  once it is edited, it is written in the format's units.

## Versions

The header carries `MAJOR.MINOR`. While the format is in preview (0.x),
Stannum refuses a file with a newer version than it reads and says to
update. From format 1.0, a minor version only adds optional tables and
fields, which older readers skip, and a major version is a breaking change.
See [Versions and compatibility](/StannumRevival/reference/versioning/).

## Converting

- **In the app**: open a `.stn` file like any case. **Save case as** saves a
  RAW or Stannum case as either `.raw` or `.stn`, by the extension you
  choose. **Export Stannum case** (Power flow's side panel, or **Ctrl+K**)
  writes any open case, with its Records and study edits, as `.stn`.
- **From the command line**: `stannum-cli convert case.raw case.stn`, or the
  other way round. MATPOWER cases convert too.
- **In run files and Python**: the `export_stn` command, as
  `export_stn path=case.stn` or `case.call("export_stn", path="case.stn")`.

Saving or exporting a case that was opened from a `.stn` file keeps that
file's ids, comments and `x.` extensions for every record still in the
case, matched by natural key; new records get natural ids.

RAW revision 33 and 35 cases convert record by record and convert back
exactly. MATPOWER cases and other RAW revisions are written from the power
flow model, as RAW export does, and the export says so.

## Conformance suite

`crates/stannum-format/conformance` holds files any reader of the format
can test against:

- `valid/`: files that must read, and `canonical/`: the text a reader writes
  for each;
- `invalid/`: files that must be refused, each with the problems Stannum
  reports (line, column and message) in a `.expect` file;
- `raw/`: RAW and MATPOWER cases with the `.stn` file each converts to.

## Still to come

The binary form (`.stnb`, columnar and fast for very large cases), applying
lists files to the contingency module, and a reader in the Python package
are planned for the next preview.
