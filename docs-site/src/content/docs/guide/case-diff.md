---
title: Case diff
description: Compare two RAW cases record by record, exactly.
---

![Case diff: section counts, a changed bus record and its fields side by side](../../../assets/screens/case-diff.png)

Case diff compares two PSS/E RAW files (revisions 33 and 35) record by record.

## Comparing

1. Choose a **baseline** file. The open case is offered.
2. Choose a **comparison** file.
3. Click **Compare files**.

Each section lists how many records were **added**, **removed**, **changed**
and **unchanged**. Select a section to list its records, and a record to see
its fields side by side with the original lines from both files. Changed
fields are highlighted.

Selecting a record that is a bus, branch, generator or DC line also selects
that element in the other modules, and an element selected elsewhere is found
here.

## What counts as a change

- **Numbers compare exactly.** `1.0` equals `1.000`, and any other change in a
  significant digit counts. There is no tolerance.
- **Text compares exactly**, and padding inside quotes is shown in quotes so
  you can see it.
- **Formatting is ignored**: column alignment, comments and line endings.
- Records are matched by their key fields (for example a branch's from bus, to
  bus and circuit), so reordering a section is not a change.

Content in layouts that haven't been verified is listed as **unsupported** and
the result is marked incomplete, rather than guessed at.

See [Case diff](/StannumRevival/manual/case-diff/) in the technical manual for
how records are matched and values compared.
