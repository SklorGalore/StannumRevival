---
title: Open a case
description: Supported file formats, what each module can do with them, and the case bar.
---

Open a case with **Open case…** in the top bar, from the
[palette](/StannumRevival/guide/command-palette/) (<kbd>Ctrl</kbd>+<kbd>K</kbd>,
then "Open"), or by dropping a `.raw` or `.m` file anywhere on the window.

## What each module can use

| Format | Power flow | Diagram | Topology | Case diff |
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

## The case bar

Once a case is open, the bar along the top of the window shows its name,
format, bus, generator and branch counts and system MVA base. It also holds:

- **Link selection**, which makes a selection in one module select the same
  element in every other module.
- The **selection chip**, naming the selected element with buttons to show it
  in another module or draw a one-line around it. <kbd>Esc</kbd> clears it.
- The **study badge** (for example "2 edits") when a
  [study](/StannumRevival/guide/studies/) is changing the case.
- **Side by side**, **Find in case** (<kbd>Ctrl</kbd>+<kbd>K</kbd>) and **Open
  another…**.

Close the case with the **×** at the right end of the bar.

## Import warnings

Converting a RAW file to the power flow model can approximate or skip data.
Those warnings are listed, with the line they come from, under the **Import
warnings** tab of [Power flow](/StannumRevival/guide/power-flow/), and in the
[log](/StannumRevival/guide/log/).
