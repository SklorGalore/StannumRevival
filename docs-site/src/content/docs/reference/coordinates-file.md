---
title: Bus coordinates file
description: The CSV file that places a case's buses on the map.
---

A bus coordinates file is a CSV that gives each bus a latitude and
longitude, for the map. Like the DC grid file, it sits beside the case and
names buses by number, so one file serves every variant of a case. Attach it
with **Ctrl+K → Attach bus coordinates…**; a study saves which file was
attached.

## Example

```csv
bus,latitude,longitude,substation
1,32.7767,-96.7970,Dallas North
2,31.5493,-97.1467,Waco
3,32.3513,-95.3011,Tyler East
```

## Columns

The first line names the columns. They can come in any order, upper or lower
case, with spaces, dashes or underscores; other columns are ignored.

| Column | Also accepted | Required | Meaning |
| --- | --- | --- | --- |
| `bus` | `bus number`, `busnum`, `number`, `bus id`, `i` | Yes | Bus number in the case. |
| `latitude` | `lat` | Yes | Decimal degrees, WGS84, −90 to 90. |
| `longitude` | `lon`, `long`, `lng` | Yes | Decimal degrees, WGS84, −180 to 180. |
| `substation` | `substation name`, `sub`, `sub name`, `site` | No | Buses with the same name are drawn as one substation. |

Fields can be quoted (`"North, Main"`). Blank lines and lines starting with
`#` are skipped. A bus listed twice keeps its last row.

## What Stannum does with it

- **Bad rows.** A row whose bus number, latitude or longitude can't be read is
  skipped, and the log says which line and why. The rest of the file still
  attaches. A file without the three required columns isn't attached.
- **Buses the case doesn't have** are counted and reported, then ignored.
- **Buses the file leaves out** are placed at the average of their neighbours
  that have a place, working outward from the buses the file places. The
  inspector marks them **Placed: estimated**. Buses with nothing placed
  anywhere in their island stay without a place (**Placed: nowhere**).
- **Record edits** that add, remove or reconnect buses are matched against the
  file again.

A bus's place, and whether it came from the file, shows in the element
inspector's **Location** section.
