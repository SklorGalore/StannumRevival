---
title: Open a case
description: Supported file formats, what each module can do with them, and the menu bar and toolbar.
---

Open a case with **Open case…** in the top bar, from the
[palette](/StannumRevival/guide/command-palette/) (<kbd>Ctrl</kbd>+<kbd>K</kbd>,
then "Open"), or by dropping a `.raw` or `.m` file anywhere on the window.

## Sample cases

With no case open, Power flow and Records list a few small cases built into
Stannum under **Try a sample case**. They are also in the palette as "Open
sample: …".

| Sample | Format | Buses | Map |
| --- | --- | --- | --- |
| Nine-bus case | MATPOWER (`case9` from MATPOWER 8.1) | 9 | No |
| Five-bus case | PSS/E RAW 35 | 5 | Yes |
| Five-bus case with devices | PSS/E RAW 35, with a three-winding transformer, a breaker and switched shunts | 7 (with the transformer's star bus) | Yes |
| Synthetic 200-bus grid | MATPOWER, from the [synthetic case generator](/StannumRevival/manual/synthetic-cases/) | 201 | Yes |

Opening a sample writes a fresh copy into a `samples` folder in Stannum's
local data folder and opens that copy, with its bus coordinates attached when
it has some. Edit it as much as you like: the next time you open the sample
it starts from the original again, so use **Save case as…** to keep changes.
Scripts open them with the `sample_cases` and `open_sample` commands.

The five-bus places are made up, and the synthetic grid describes no real
system. `case9` is MATPOWER's case data, which keeps its own provenance (see
`THIRD-PARTY-NOTICES.md`).

## What each module can use

| Format | Power flow | Diagram | Topology (network, layers, tables) | Case diff |
| --- | --- | --- | --- | --- |
| PSS/E RAW revision 35 | Yes | Yes | Yes, from the RAW records | Yes |
| PSS/E RAW revision 33 | Yes | Yes | Yes, built from the power flow model | Yes |
| PSS/E RAW revisions 23–32, 34 | Yes | Yes | Yes, built from the power flow model | No |
| MATPOWER `.m` (version 2) | Yes | Yes | Yes, built from the power flow model | No |

Topology reads revision 35 RAW files itself, with every switching device and
original record. For other files it builds the network from the power flow
model instead, and says so in its diagnostics: switching devices and the
original records are not available then, and loads are summed per bus.

If a module can't use the open case it says why, and the other modules keep
working. See [PSS/E RAW](/StannumRevival/reference/psse-raw/) and
[MATPOWER](/StannumRevival/reference/matpower/) for what is read from each
format.

## The menu bar and toolbar

Once a case is open, the menu bar along the top of the window shows its name,
format, bus, generator and branch counts and system MVA base. The toolbar
under it holds:

- **Open** and **Save as run file**.
- **Solve**, **Sensitivities** and **Contingencies**, which run in
  [Power flow](/StannumRevival/guide/power-flow/) with its current settings.
- **Side by side** and **Find in case** (<kbd>Ctrl</kbd>+<kbd>K</kbd>).

Under the toolbar, the **context bar** shows:

- **Showing**: the result every module shows, "Not solved yet" until you
  solve.
- The **study badge** (for example "2 edits") when a
  [study](/StannumRevival/guide/studies/) is changing the case.
- **Link selection**, which makes a selection in one module select the same
  element in every other module.
- The **selection chip**, naming the selected element with buttons to show it
  in another module or draw a one-line around it. <kbd>Esc</kbd> clears it.

**File → Close case** closes the case. See [The
window](/StannumRevival/guide/shell/#menu-bar-and-toolbar) for every menu.

## Import warnings

Converting a RAW file to the power flow model can approximate or skip data.
Those warnings are listed, with the line they come from, under **Import
warnings** in [Power flow](/StannumRevival/guide/power-flow/) (a note under the
Overview opens it), and in the
[log](/StannumRevival/guide/log/).
