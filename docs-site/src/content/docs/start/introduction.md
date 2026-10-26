---
title: What Stannum is
description: The idea behind Stannum, its four modules and how they share one open case.
---

Stannum is a desktop application for power system engineers. It is built for
one job above all: **investigating a problem around one element** of a
transmission case. You open a case, find the bus, branch or generator you care
about, and look at it from every side: its solved voltages and flows, a
one-line drawn around it, its place in the network's topology, and how it
changed between two versions of the case.

Everything runs on your machine. Case files are parsed and solved by native
Rust code inside the app and never leave your computer.

## Four modules, one case

The rail on the left of the window switches between four modules. They all
work on the same open case.

| Module | What it does |
| --- | --- |
| [Diagram](/StannumRevival/guide/diagram/) | Generate one-lines from the case, with power flow results on them, or draw, edit and export your own. Draw the network graph, with [topology](/StannumRevival/guide/topology/) layers (islands, bridges, cut buses, shortest paths) and tables. |
| [Power flow](/StannumRevival/guide/power-flow/) | AC and DC power flow, operating-limit checks, result tables, sensitivities and studies. |
| [Case diff](/StannumRevival/guide/case-diff/) | Exact record-by-record comparison of two RAW cases. |

Selecting an element in one module selects it in the others, so a bus you
spot in a violation table can be shown in the one-line, the network graph and
the case diff with one click. Two modules can sit [side by
side](/StannumRevival/guide/side-by-side/) with their selections linked.

## Design principles

These ideas shape how Stannum behaves, and explain a few choices that may
surprise you at first.

- **Start from an element.** Most work begins with one bus or branch, so
  [Ctrl+K](/StannumRevival/guide/command-palette/) finds any element by number
  or name, and every module can be opened at it.
- **Generate, don't draw.** One-lines are drawn from the case around the
  element you are looking at, not maintained by hand.
- **Results in place.** Voltages, flows and loading appear on the diagram and
  in coloured table cells, not in a separate report.
- **Every finding is a link.** A limit violation, a log line or a changed
  record takes you to the element it is about.
- **Works on any case.** If a module can't use the open case it says why, and
  the other modules keep working.
- **Keyboard first.** The palette, shortcuts and the log are all reachable
  without the mouse.

## Engines

Stannum combines three engines that used to be separate projects and now live
in one repository:

- **rs-power**: AC/DC power flow, sensitivities, MATPOWER and PSS/E RAW loading.
- **nodal**: a RAW parser and topology analysis built on rustworkx-core.
- **case-diff**: a record-level RAW comparison, ported from Python to Rust.

The [Architecture](/StannumRevival/manual/architecture/) page explains how they
fit together.
