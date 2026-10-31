---
title: Lists file format (.stnl)
description: "The .stnl text format: subsystems, contingencies, monitored elements and interfaces, kept apart from the case so they run against any case."
---

:::caution[Preview]
The lists format is a **preview** (format version 0.1), like the
[case format](/StannumRevival/reference/case-file/). The contingency module
runs a file's `contingency` records (`open` and `trip_bus` events) and its
`contingency_set` rules, and checks what its `monitor` records name,
including interfaces. `close`, `set_p_mw` and `set_q_mvar` events, and
rating slots above 3, are read and checked but not applied yet.
:::

A lists file (`.stnl`) holds the study definitions that PSS/E keeps in
`.sub`, `.mon` and `.con` files: subsystems, contingencies, what to monitor,
and interfaces. They are kept out of the case on purpose, so one set of
lists can run against every case of a study: this summer's peak, next
year's, or a case with a project added.

Every table and field is listed in
[Lists file tables](/StannumRevival/reference/lists-file-schema/), generated
from `crates/stannum-format/schema/stannum-lists.toml`.

## Example

```text
stannum-lists 0.1
# North area screening.

subsystem north name="North 138" areas=[1] kv_min=100

contingency "Line 1-2" category=P1 events=[[open, line:1:2:1]]
contingency "Gen 1 + line" category=P2 events=[[open, generator:1:1], [open, line:1:3:1]]
contingency_set n1 kind=n_1_branches subsystem=north

monitor flows subsystem=north limit_pct=95
monitor v2 kind=voltage element=bus:2 vmin_pu=0.95
interface flowgate elements=[[line:1:2:1, 1], [line:1:3:1, 1]] limit_mw=180
monitor gate kind=interface interface=flowgate
```

## Syntax

The same as the case file: the header `stannum-lists MAJOR.MINOR`, then one
record per line, `table id name=value ...`, with comments, quoting, lists
and canonical form as described in
[Case file format](/StannumRevival/reference/case-file/#lines). A record's
id is its name, quoted when it has spaces.

## Naming equipment

Elements are named by typed references with natural keys, `kind:id`, as in
the case file: `bus:205`, `line:205:310:1`, `generator:205:1`. Areas,
zones, owners and buses in subsystems are named by number. Nothing refers to
a particular case file, so a list works with any case that has the
equipment. When lists are applied to a case, an element the case doesn't
have is reported and skipped, not treated as an error.
