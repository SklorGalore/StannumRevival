---
title: Case diff
description: How the RAW comparison parses verified layouts, matches records by key and compares values exactly.
---

Case diff is a Rust port of the Python
[case-diff](https://github.com/SklorGalore/case-diff) tool. Its parser and
comparison are meant to agree with the Python tool record for record; the
`diff_dump` example prints a comparison as JSON in the same shape, to check
the two on new files.

## Parsing

- Only **verified layouts** are parsed: RAW revisions 33 and 35, with field
  layouts from the shared `raw.schema.json` (the same schema nodal uses).
- The revision comes from the case identification record. Other revisions are
  refused.
- Content in a layout that hasn't been verified (for example an unsupported
  populated section or setting) is kept as a **diagnostic**, not guessed at,
  and the comparison is marked **incomplete**.
- A duplicate record key within a section is a diagnostic too.

## Matching

Records are matched within each section by their **key fields**, from the
schema for that section and revision: a bus by its number, a branch by its
from bus, to bus and circuit, and so on. Keys are normalized (identifier
padding is trimmed), so reordering records or re-padding a circuit ID is not a
change.

The result keeps the baseline's order, followed by records only in the
comparison.

| Status | Meaning |
| --- | --- |
| Added | Key only in the comparison file |
| Removed | Key only in the baseline file |
| Changed | Key in both, at least one field differs |
| Unchanged | Key in both, every field equal |

## Exact values

Every numeric field is held as an **exact decimal**: a sign, its significant
digits and a base-10 exponent, never a binary float. Two numbers are equal
when their canonical forms are equal, after trailing zeros are stripped:

$$
1.0 = 1.000 = 1, \qquad 0.1 + 0.2 \text{ never enters into it}, \qquad 1.0 \ne 1.0000001 .
$$

There is no tolerance, so any change in a significant digit counts. Text
fields compare exactly, and are shown quoted so leading and trailing spaces
are visible. Display follows Python's `Decimal` and `int` rules, so values
read the same as in the original tool.

Comments, column alignment, delimiters and line endings are ignored, since
they are not record values.

## Large cases

The comparison is kept in memory in the core. The interface asks for a
summary, then one section's record list, then one record's fields, so cases
with tens of thousands of records stay responsive.
