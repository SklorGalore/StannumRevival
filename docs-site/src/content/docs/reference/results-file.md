---
title: Results file format (.stnr)
description: "The .stnr compact results file: a contingency run as one Arrow IPC stream, zstd compressed, readable by pyarrow, polars, pandas and DuckDB."
---

:::caution[Preview]
The results format is a **preview** (format version 0.1). Columns may be
added before 1.0; existing ones keep their names and meaning.
:::

A results file (`.stnr`) holds a whole contingency run: every outage
studied and every violation it caused. It is made to be kept and shared:
small, typed, and readable without Stannum.

- **Compact.** Values are columns, compressed with zstd, and repeated text
  (contingency and element names) is stored once per batch as a dictionary.
  On Texas 2k a run with 73,000 violations is 13 MB as CSV, 780 KB as gzipped
  CSV, and 450 KB as `.stnr`.
- **Streamable.** The file is an [Arrow IPC stream](https://arrow.apache.org/docs/format/Columnar.html#ipc-streaming-format)
  of record batches of up to 65,536 rows. A reader takes one batch at a
  time, so a run of any size never has to fit in memory.
- **Open.** Any Arrow reader opens it. Nothing about it is Stannum's own
  except the column names and the metadata below.

Export it from the Contingencies tab (**Export → Compact results (.stnr)**),
from Python with `case.export_contingency_results(path)`, or with the
`export_contingency_results` command.

## Reading it

```python
import pyarrow as pa
table = pa.ipc.open_stream("run.stnr").read_all()
df = table.to_pandas()          # pandas
overloads = df[df.type == "branch"]

import polars as pl
df = pl.read_ipc_stream("run.stnr")

# One batch at a time
with pa.ipc.open_stream("run.stnr") as reader:
    for batch in reader:
        ...

# The run's summary and settings
import json
meta = pa.ipc.open_stream("run.stnr").schema.metadata
summary = json.loads(meta[b"stannum.summary"])
```

DuckDB queries the pyarrow table above by name
(`duckdb.sql("select type, count(*) from table group by type")`), or reads
the file itself with its community `arrow` extension:

```sql
INSTALL arrow FROM community; LOAD arrow;
SELECT contingency, element, value_pct FROM read_arrow('run.stnr')
WHERE type = 'branch' ORDER BY value_pct DESC LIMIT 20;
```

## Rows

One row per finding, in rank order (worst outage first):

| `type` | One row for |
| --- | --- |
| `branch` | each branch over its limit after the outage |
| `voltage` | each bus outside its voltage limits |
| `interface` | each interface over its limit |
| `not_converged` | an outage whose AC solve failed (its findings aren't listed) |
| `ok` | an outage with nothing to report, so the file says every outage studied |

Which violations are reported follows the run's settings (**Worse by** and
**Include existing**), as in the CSV export. Filter `type != "ok"` for the
violations alone.

## Columns

Text columns are dictionary encoded (`dictionary<int32, utf8>`); numbers are
`uint32` or `float32` (about seven significant digits). Empty cells are null.

| Column | Type | Meaning |
| --- | --- | --- |
| `rank` | uint32 | The outage's rank in the run, from 1 |
| `category` | text | Planning category or contingency set name |
| `contingency` | text | The outage's name, PSS/E style (`NORTH 101 SOUTH 102 1`) or as a lists file names it; `BASE CASE` for P0 |
| `outage` | text | Element ids of what goes out, joined by `&`; null for the base case |
| `description` | text | A planning event's description |
| `solved` | text | `ac`, `linear` or `ac_not_converged` |
| `type` | text | `branch`, `voltage`, `interface`, `not_converged` or `ok` |
| `element` | text | The violating element by name: bus names and numbers then the circuit, a bus as `NAME NUMBER`, an interface's name |
| `element_id` | text | Its id: `line:1:2:1`, `bus:5`, `interface:North` |
| `from_bus`, `to_bus` | uint32 | A branch's end buses |
| `bus` | uint32 | A voltage problem's bus |
| `value_pct` | float32 | Loading after the outage, % of the limit's rating |
| `before_pct` | float32 | Loading before the outage |
| `limit_pct` | float32 | The limit, % of the rating |
| `mw`, `before_mw` | float32 | Flow after and before the outage, MW |
| `vm_pu`, `before_vm_pu` | float32 | Voltage after and before, p.u. |
| `vmin_pu`, `vmax_pu` | float32 | The bus's voltage limits |
| `note` | text | Why an AC solve failed, or another note on the outage |

## Metadata

The stream's schema carries:

| Key | Value |
| --- | --- |
| `stannum.format` | `stannum-results 0.1` |
| `stannum.kind` | `contingency` |
| `stannum.case` | The case's file name |
| `stannum.summary` | The run's summary, JSON (outages, violations, time) |
| `stannum.settings` | The run's settings, JSON, enough to run it again |
| `stannum.created_by` | The writer and its version |
