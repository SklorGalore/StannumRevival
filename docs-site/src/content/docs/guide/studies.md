---
title: Studies
description: Outages, generator output and load edits that every solve uses, with saved solves to compare, kept in a run file.
---

A **study** is a list of edits to the open case that every solve uses, without
changing the file. Use one to take a line out, move generation or change a
load, and see what it does.

![The Study tab with two edits: a line outage and a generator redispatch](../../../assets/screens/study-edits.png)

## Making edits

Edits are made where the element is shown: in the
[element inspector](/StannumRevival/guide/shell/#element-inspector) in any
module, or in a Power flow table's selection bar when the pane is too narrow
for the inspector. The inspector's **Study** section lists the edits that
touch the element, each with a button to remove it.

| Edit | Applies to |
| --- | --- |
| **Take out** / **Put back** | Lines, transformers (every winding of a three-winding transformer goes together) and generators |
| **Set output** | A generator's active power in MW |
| **Set load** | A bus's total load in MW and Mvar |

**Set output** and **Set load** open a small form; **Apply to study** adds the edit.

The toolbar shows a badge ("2 edits") while a study is changing the case, and
the edits take effect on the next solve. Every edit names an element that
exists; an edit that names nothing in the case is refused rather than ignored.

## The Study tab

Power flow's **Study** tab lists the edits in order, each with what it was
before ("was 163.0 MW"). Remove one with its **×**, draw around its element,
or **Clear all**.

## Saving and comparing solves

**Save this solve** keeps the current result under a name you can edit. Each
saved solve shows its losses, violation count and how many edits it used, and
**Restore** goes back to its edits.

**Compare solves** puts any two saved solves (or the current one) side by
side: generation, load, losses and limit violations, then the branch flows and
bus voltages that moved, largest first.

![Comparing the case as filed with the study: branch flows and voltages that moved](../../../assets/screens/study-compare.png)

## Saving the work as a run file

**Save as run file** in the toolbar (or **File → Save as run file**) writes a
[run file](/StannumRevival/guide/run-files/) that puts the work back: it
opens the case, attaches its DC grid and bus coordinates files, sets the
study edits and solves with the current solve settings. Open it again, or run
it with `stannum-cli`, to get back to the same place.

```text
# stannum-run-file version 1
# Stannum run file, recorded in the app.
# Run it from Ctrl+K > Run a run file, or: stannum-cli run case9-outage.stnrun

open_case path=case9.m
set_study_edits edits=[{"element": "line:4:5:1", "kind": "status", "online": false}, {"element": "generator:2:1", "kind": "generation", "mw": 120}]
solve_power_flow
```

Once saved, the button shows the run file's name, with a dot while there are
edits or settings changes since the last save; click it to save again.
**Save run file as…** writes a copy.

The run file leaves out saved solves, which come back by solving again, and
the one-line drawing, which you save from the Diagram's **File** menu. For
everything you did, step by step, save part or all of the
[history](/StannumRevival/guide/run-files/#history) instead.

If the case has values edited in [Records](/StannumRevival/guide/records/)
that aren't in a file yet, Stannum asks you to save the edited case first, and
the run file then opens that file.

## Older study files

Stannum 0.3 to 1.0 saved studies as `.stannum` files. They still open (with
**Open**, **File → Open case or run file…**, **Open old study file…** in the
command palette, or by dropping one on the window), but are no longer
written. The case opens, the edits are applied, saved solves and the drawing
come back and the case is solved again with the saved settings. Save it as a
run file to keep it.

Stannum says so under the toolbar when:

- **the case moved.** It looks for the case where it was, then at the same
  place relative to the study, then beside the study. If none of those has it,
  it asks you to find it.
- **the case changed.** If the file's contents differ from when the study was
  saved, the study still opens, and the saved solves keep showing the results
  of the case as it was then.
- **an edit no longer fits.** An edit naming an element that is no longer in
  the case is left out, and the message lists it.

![An older study opened after its case moved and changed: the notice under the toolbar, the edits and the saved solve](../../../assets/screens/study-reopened.png)

Before a different case replaces one with study edits that aren't saved to a
run file, Stannum asks whether to discard them.

## Recent files

With no case open, Power flow and Records list the cases, run files and
studies opened most recently. Click one to open it, or hover and press **×** to drop it from
the list. The list is kept on this computer only.

![The empty Power flow screen listing recent studies and cases](../../../assets/screens/recent-files.png)

## Scope

Studies change what Power flow and the diagram's results show. The topology
layers and tables show the file as it is; Case diff shows the file unless
you pick **Compare with file** in the Study tab, which compares the case
with its edits against the file.
