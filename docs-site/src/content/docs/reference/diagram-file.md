---
title: Diagram file format
description: "The .sld file: a one-line drawing as JSON, with each symbol's place, data and link to the case."
---

A diagram file (`.sld`) holds one one-line drawing: its symbols, the wires
between them and the view. Save one from the Diagram toolbar's **File** menu (**Save** or
**Save as…**, <kbd>Ctrl</kbd>+<kbd>S</kbd>). A
[study file](/StannumRevival/reference/study-file/) holds the same drawing in
its `diagram` field.

The file is UTF-8 JSON, written by the app. Stannum also opens a `.json` file
in the same shape.

## Example

One generator wired to one bus, shortened:

```json
{
  "metadata": {
    "title": "five_bus",
    "revision": "A",
    "company": "",
    "description": "One-line generated from five_bus.raw",
    "createdAt": "2026-10-28T20:35:19.010Z",
    "modifiedAt": "2026-10-28T20:35:23.583Z",
    "schemaVersion": "1.0",
    "caseName": "five_bus.raw"
  },
  "nodes": [
    {
      "id": "g0",
      "type": "generator",
      "position": { "x": 340, "y": 26 },
      "data": {
        "label": "G 1",
        "nominalMW": 300,
        "voltageKV": 138,
        "powerFactor": 0.85,
        "groundingType": "solidly",
        "caseRef": { "kind": "gen", "id": 0, "element": "generator:1:1" }
      }
    },
    {
      "id": "b1",
      "type": "bus",
      "position": { "x": 300, "y": 120 },
      "data": { "label": "Bus 1", "voltageKV": 138, "orientation": "horizontal", "length": 200,
                "caseRef": { "kind": "bus", "id": 1, "element": "bus:1" } }
    }
  ],
  "edges": [
    {
      "id": "e-g0",
      "type": "conductor",
      "source": "g0",
      "target": "b1",
      "targetHandle": "t0",
      "data": { "voltageKV": 138, "conductorType": "transmission" }
    }
  ],
  "viewport": { "x": 139.5, "y": 16.2, "zoom": 0.89 }
}
```

## Top level

| Field | Meaning |
| --- | --- |
| `metadata` | The title block and where the drawing came from; see below. |
| `nodes` | The symbols. |
| `edges` | The wires. |
| `viewport` | The pan (`x`, `y`, in screen pixels) and `zoom` when it was saved. Missing in some early files, which open at 1:1. |

## Metadata

| Field | Meaning |
| --- | --- |
| `title`, `revision`, `company`, `description` | The title block, as text. |
| `createdAt`, `modifiedAt` | ISO 8601 times. |
| `schemaVersion` | The diagram format version, `"major.minor"`. See [Versions](#versions). |
| `caseName` | For a generated drawing: the file name of the case it came from. |
| `around` | For a local drawing: `bus`, the bus it's centred on, `depth`, how many hops out it reaches, and `element`, the element asked for, if it was one. |

## Symbols

Each node has an `id` (unique in the file), a `type`, a `position` (its top
left corner, in drawing pixels, on a 20 px grid when generated) and `data`.
Other fields the drawing canvas keeps, such as `measured` or `selected`, may
be present and are ignored when the file is read.

| `type` | Symbol | Main `data` fields |
| --- | --- | --- |
| `bus` | Busbar | `voltageKV`, `orientation` (`"horizontal"` or `"vertical"`), `length` (px), `taps` |
| `generator` | Generator | `nominalMW`, `voltageKV`, `powerFactor`, `groundingType` |
| `load` | Load | `nominalMW`, `nominalMVAR`, `loadType` |
| `shunt` | Shunt | `nominalMVAR`, `nominalMW` |
| `transformer` | Two-winding transformer | `ratingMVA`, `primaryKV`, `secondaryKV`, `connection`, `impedancePct` |
| `transformer3` | Three-winding transformer | `legs` (one per winding: `winding`, `bus`, `kv`, `ratingMVA`), `star` |
| `circuitBreaker` | Breaker | `state` (`"open"`, `"closed"`, `"fault"`), `normallyOpen`, `ratingKV`, `ratingKA` |
| `disconnector` | Disconnector | `state`, `normallyOpen`, `ratingKV`, `type` |
| `fuse` | Fuse | `ratingKV`, `ratingA`, `type`, `blown` |
| `offPage` | Branch leaving the drawing | `bus`, `busName`, `voltageKV`, `links` (when one marker stands for several branches) |
| `dcLink` | DC line with its two converters | `scheduledMW`, `upperKV`, `lowerKV`, `dcKind` |

Every symbol's `data` also has a `label`, and may have:

- `rotation`: 0, 90, 180 or 270 degrees clockwise;
- `labelOffset`: `{ x, y }`, how far the label was dragged;
- `caseRef`: the case record the symbol stands for, so results can be drawn on
  it: `kind` (`"bus"`, `"gen"`, `"branch"`, `"load"`, `"shunt"` or
  `"dcline"`), `id` (bus number for buses, loads and shunts; row number in
  the case for the rest) and `element`, the shared element id such as
  `line:1:2:1`, which links the symbol to the same element in the other
  modules. Drawings saved before element ids existed have only `kind` and
  `id`;
- `reversed`, `outOfService`, `focus`: how a generated symbol is drawn.

A drawing saved before the one-symbol three-winding transformer draws one as
a small `bus` with `star: true` and three `transformer` symbols; it still
opens as it was drawn.

## Wires

Each edge has an `id`, `type` `"conductor"`, `source` and `target` (node ids),
and optionally `sourceHandle` and `targetHandle`: the terminal on each symbol
(on a bus, the `id` of one of its `taps`). `data` has:

| Field | Meaning |
| --- | --- |
| `voltageKV`, `conductorType` | `"transmission"`, `"distribution"` or `"buswork"`. |
| `route` | The wire's corners, ends included, in drawing pixels. |
| `pinnedRoute` | `true` once a segment was dragged by hand: the router keeps that shape. |
| `labelAt`, `labelOffset` | Where the flow text goes. |
| `caseRef`, `reversed`, `outOfService`, `focus` | As for symbols. A wire with a `caseRef` of kind `"branch"` carries that branch's flow. |

## Versions

`metadata.schemaVersion` is `"major.minor"`; Stannum 0.3 to 1.x write
`"1.0"`.

- The **minor** part goes up when fields are added that an older Stannum can
  ignore. An older Stannum opens the file and leaves the new fields out of
  the drawing.
- The **major** part goes up only when an older Stannum couldn't draw the file
  correctly. A Stannum refuses a diagram with a higher major version than its
  own ("This diagram was saved by a newer Stannum") rather than open it
  wrongly.
- A file without `schemaVersion` reads as `"1.0"`. Saving writes the current
  version.

| `schemaVersion` | Written by | Changes |
| --- | --- | --- |
| `"1.0"` | 0.3.0 and later (and StannumSLD before it) | The first version. Later releases added optional fields, such as `caseRef.element`, `route` and `around`, without changing it. |

Throughout 1.x, Stannum keeps opening every diagram a 1.x release wrote.
Diagrams saved by 0.3.0 and 0.4.1 are kept in the repository under
[`tests/compat`](https://github.com/SklorGalore/StannumRevival/tree/development/tests/compat)
and opened by the tests on every change.
