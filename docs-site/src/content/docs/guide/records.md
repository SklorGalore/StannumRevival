---
title: Records
description: Every record in the open case as a table, with every field, to sort, search and filter.
---

![Records: the generators of an 82,000-bus case sorted by output, with the picked generator in the inspector](../../../assets/screens/records.png)

**Records** in the module rail shows the open case as a spreadsheet: one
table per record type, with every field as a column.

## Record types

The list on the left has each record type in the file, with its count:
buses, loads, fixed shunts, generators, branches, switching devices,
transformers, areas, DC lines, FACTS devices, switched shunts and so on, and
the case details (identification, titles and solution settings) last. Types
with no records fold under **Empty**. In a narrow pane the list becomes a menu
above the table.

Where the records come from depends on the file:

- **PSS/E RAW revisions 33 and 35** are read record by record, so the columns
  are the RAW field names (`I`, `NAME`, `BASKV`, `IDE`, …) and the values are
  the file's own. Three-winding transformers, and sections whose layout Case
  diff hasn't verified (fixed shunts in revision 33, for example), are read
  with the layout Stannum knows and say so above the table.
- **MATPOWER files, and other RAW revisions**, show the power flow model's
  tables in MATPOWER's columns (`BUS_I`, `TYPE`, `PD`, …): buses, generators,
  branches, DC lines and generator costs. For a RAW file this sums loads and
  fixed shunts into each bus, and a note says so.

The tables show the file as opened. A study's edits aren't applied here.

## Sort, search and filter

- Click a column heading to sort by it; click again to reverse, and a third
  time to return to file order.
- **Search** matches any field.
- **Add filter** works as in Power flow's tables: pick a field, an operator
  and a value. Fields with few distinct values (bus type, area, status, IDs)
  offer their values to pick from; numbers take `=`, `>`, `between` and so on.
  Each record type keeps its own filters.
- **CSV** saves the rows shown, with the RAW line each record starts on.

## Pick a record

Picking a record that is a bus, load, generator, branch, transformer,
switching device or DC line opens it in the element inspector, with its
results, limits, connections and **Draw around**. Shunts and induction
machines open their bus. While selections are linked, a pick here selects the
element in every module, and an element picked elsewhere is found here.
