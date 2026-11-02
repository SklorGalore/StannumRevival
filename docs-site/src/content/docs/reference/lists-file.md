---
title: Lists file format (.stnl)
description: "The .stnl text format: subsystems, contingencies, monitored elements, interfaces and transfers, kept apart from the case so they run against any case."
---

:::note
Format version 1.0, frozen with Stannum 2.0 like the
[case format](/StannumRevival/reference/case-file/); files from the 0.x
preview still read. The contingency module
runs a file's `contingency` records (`open` and `trip_bus` events) and its
`contingency_set` rules, and checks what its `monitor` records name,
including interfaces. Transfer limits run its `transfer` records. `close`, `set_p_mw` and `set_q_mvar` events, and
rating slots above 3, are read and checked but not applied yet.
:::

A lists file (`.stnl`) holds the study definitions that PSS/E keeps in
`.sub`, `.mon` and `.con` files (and the transfers of a `.dfx` setup):
subsystems, contingencies, what to monitor, interfaces and transfers. They are kept out of the case on purpose, so one set of
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

subsystem south areas=[2, 3]
transfer north_south name="North to South" source=north sink=south scaling=reserve rating=2
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

## Transfers

A `transfer` record names a transfer to find the
[limit](/StannumRevival/manual/transfer-limits/) of: power moved from its
`source` subsystem to its `sink`, shared out by `scaling` (`generation`,
`reserve` or `load`), with its own `cutoff_pct`, `base_rating`, `rating`
(slots 1 to 3, A to C) and `limit_pct`. Run against a case, it studies the
file's contingencies and contingency sets and watches its flow and
interface monitors, when the file has them; a monitor's own rating and
limit give way to the transfer's. A file can hold several transfers, and a
run names the one it wants.

```text
stannum-lists 0.1
subsystem far_west name="Far West" areas=[1]
subsystem coast name="Coast" areas=[7]
subsystem hv name="100 kV and up" areas=[1, 2, 3, 4, 5, 6, 7, 8] kv_min=100
contingency_set n1 kind=n_1_branches kv_min=100
monitor hv_flows subsystem=hv
transfer far_west_coast name="Far West to Coast" source=far_west sink=coast
```
