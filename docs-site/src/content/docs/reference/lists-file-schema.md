---
title: Lists file tables
description: "Every table and field of the stannum-lists 0.1 format: type, unit and default."
---

<!-- Written by the stannum-format tests from crates/stannum-format/schema/stannum-lists.toml; don't edit it here. -->

:::caution[Preview]
Format version 0.1 is a preview: it may change in any Stannum release until it reaches 1.0. See [Lists file (.stnl)](/StannumRevival/reference/lists-file/).
:::

Stannum lists: subsystems, contingencies, monitored elements and interfaces, reusable across cases. Elements are named by natural keys (bus numbers and circuit or machine ids).

A field left out has its default; one with no default is absent, which means none. Units are in the field names (`_pu`, `_mw`, `_kv` and so on) and in the Unit column. Fields starting `x.` are extensions any table may hold.

## subsystem

A named set of equipment. Listed members combine as a union; the kV filters then narrow it.

Id: `{name}` · Typed references: `subsystem:<id>`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `name` | string |  | `""` | Display name. |
| `areas` | list of int |  | `[]` | Areas whose buses are members, by number. |
| `zones` | list of int |  | `[]` | Zones whose buses are members, by number. |
| `owners` | list of int |  | `[]` | Owners whose buses are members, by number. |
| `buses` | list of int |  | `[]` | Buses that are members, by number. |
| `subsystems` | list of subsystem id |  | `[]` | Other subsystems whose members are members. |
| `kv_min` | float | kV | none | Only buses at or above this base voltage. |
| `kv_max` | float | kV | none | Only buses at or below this base voltage. |

## contingency

A contingency: events applied together.

Id: `{name}` · Typed references: `contingency:<id>`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `name` | string |  | `""` | Display name. |
| `category` | string |  | `""` | Free-text category, such as P1 or P2. |
| `events` | list of [open \| close \| set_p_mw \| set_q_mvar \| trip_bus, element, [float]] |  | `[]` | Events as `[action, element]` or `[action, element, value]`. Actions: `open`, `close`, `set_p_mw`, `set_q_mvar`, `trip_bus` (open everything at the bus). |

## contingency_set

A rule that generates contingencies when the lists are applied to a case.

Id: `{name}` · Typed references: `contingency-set:<id>`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `name` | string |  | `""` | Display name. |
| `kind` | `n_1_branches` \| `n_1_generators` \| `n_2_branches` \| `bus_outages` |  | required | What to take out: single branches, single generators, pairs of branches, or whole buses. |
| `subsystem` | [subsystem](#subsystem) id |  | none | Only equipment in this subsystem; the whole case when absent. |
| `kv_min` | float | kV | none | Only equipment at or above this base voltage. |

## monitor

What to check after each contingency.

Id: `{name}` · Typed references: `monitor:<id>`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `name` | string |  | `""` | Display name. |
| `element` | element |  | none | One element to monitor; use this or `subsystem`. |
| `subsystem` | [subsystem](#subsystem) id |  | none | Every element in a subsystem. |
| `kind` | `flow` \| `voltage` \| `interface` |  | `flow` | `flow`, `voltage` or `interface`. |
| `rating` | int |  | `1` | Rating slot that limits are taken from (1 to 12). |
| `limit_pct` | float | % | `100` | Percent of the rating that counts as a violation. |
| `vmax_pu` | float | pu | none | Voltage monitor: upper limit; the bus's own limit when absent. |
| `vmin_pu` | float | pu | none | Voltage monitor: lower limit; the bus's own limit when absent. |
| `interface` | [interface](#interface) id |  | none | The interface, for `kind=interface`. |

## interface

A flowgate: weighted branch flows summed.

Id: `{name}` · Typed references: `interface:<id>`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `name` | string |  | `""` | Display name. |
| `elements` | list of [element, float] |  | `[]` | Branches as `[element, weight]`, flow measured from the first bus of the element's id. |
| `limit_mw` | float | MW | none | Limit; none when absent. |

## transfer

A transfer to find the limit of (DFAX and FCITC): power moved from one subsystem to another. Its outages are the file's contingencies and contingency sets, and it watches the file's flow and interface monitors (with this record's ratings and limit); without them, the run's own settings apply.

Id: `{name}` · Typed references: `transfer:<id>`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `name` | string |  | `""` | Display name. |
| `source` | [subsystem](#subsystem) id |  | required | Where the power comes from. |
| `sink` | [subsystem](#subsystem) id |  | required | Where it goes. |
| `scaling` | `generation` \| `reserve` \| `load` |  | `generation` | How each side shares out the transfer: online generators by output, by room to their limits, or loads. |
| `cutoff_pct` | float | % | `5` | Elements whose distribution factor is smaller than this don't limit the transfer. |
| `base_rating` | int |  | `1` | Rating slot in the base case (1 to 3, A to C). |
| `rating` | int |  | `1` | Rating slot after an outage (1 to 3, A to C). |
| `limit_pct` | float | % | `100` | Percent of the rating that counts as the limit. |
