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

## Versions

A coordinates file may start with a comment line that says which version of
the format it follows, before the column names:

```csv
# stannum-coordinates version 1
bus,latitude,longitude,substation
1,32.7767,-96.7970,Dallas North
```

A file without it is version 1, and a Stannum from before 0.5 skips it like
any other comment. The version goes up only when an older Stannum would read
the file wrongly, such as a column changing its meaning or units; new
optional columns don't change it, since older versions ignore columns they
don't know. A Stannum refuses a file with a higher version than it reads
("this coordinates file is version 2, from a newer Stannum") rather than put
buses in the wrong place.

| Version | Read by | Changes |
| --- | --- | --- |
| 1 | 0.4 and later | The first version. |

A coordinates file from 0.4.1 is kept in the repository under
[`tests/compat`](https://github.com/SklorGalore/StannumRevival/tree/development/tests/compat)
and attached by the tests on every change.

## What Stannum does with it

- **Bad rows.** A row whose bus number, latitude or longitude can't be read is
  skipped, and the log says which line and why. The rest of the file still
  attaches. A file without the three required columns isn't attached.
- **Buses the case doesn't have** are counted and reported, then ignored.
- **Buses the file leaves out** are estimated from the branches that reach
  them, working outward from the buses the file places. The inspector marks
  them **Placed: estimated**. Buses with nothing placed anywhere in their
  island stay without a place (**Placed: nowhere**). See
  [How missing buses are estimated](#how-missing-buses-are-estimated).
- **Record edits** that add, remove or reconnect buses are matched against the
  file again.

A bus's place, and where it came from, shows in the element inspector's
**Location** section.

## Places from the case itself

PSS/E RAW revision 34 and 35 files can carry substation data: each
substation has a latitude and longitude, and its node records name the buses
inside it. When a case has it, the map draws without a coordinates file:

- Each bus with a node in a substation that has a place goes there, drawn as
  that substation (**Placed: from case substation**).
- A substation at latitude 0 and longitude 0, PSS/E's "not set", has no place.
  Its buses are estimated from their neighbours like any other.
- A substation without a name is called "Substation N" by its number, and a
  name two substations share gets the number added.
- The rest of the substation data (switching devices and terminals, the
  node-breaker detail) isn't used; the case's warnings say how many records
  were skipped.

Attaching a coordinates file as well puts the file first: a bus the file
places goes where the file says, with the file's substation name, and the
case's own places fill in the buses the file leaves out. Detaching the file
goes back to the case's places.


## How missing buses are estimated

A bus no source places is put where its branches say it should be:

- **Transformers and switching devices** keep both their buses at one place,
  so a bus tied that way to a placed bus sits exactly on it, in the same
  site. Two buses at different base voltages joined by any branch count as
  a transformer.
- **Lines** say how far apart their buses are: from the `LEN` a RAW branch
  record gives, or, where a case has no lengths (MATPOWER cases, and many
  RAW exports that leave `LEN` at 0), from the line's reactance in ohms,
  which grows with length.
- Lengths are turned into kilometres by the lines whose two ends are
  already placed: the median ratio of straight distance to length. So the
  case's length units, and the detours real lines take, don't matter.
  Reactance gets its own ratio for each voltage level with at least five
  placed lines.
- Each missing bus then moves to best fit the lengths of all its lines at
  once, the placed buses held still. A bus hanging off one placed bus is
  put one line length away from it, on the side away from that bus's other
  lines.
- With fewer than three placed lines to scale lengths by, a missing bus
  goes at the average of its placed neighbours instead.

The coordinates summary says which was used (`estimated_from`: `lengths`,
`reactance` or `neighbours`), and so does the inspector's Map section.

On a synthetic 2,000-bus case, hiding the places of half its substations,
the estimates land 5.5 km from the true places on average, against 17.8 km
for neighbour averages. Synthetic lines have reactance in proportion to
length, so expect real cases to do somewhat worse.
