---
title: Area interchange control
description: Area slack generation holds each area's net interchange at its desired value (PSS/E area data PDES within PTOL), by tie lines or tie lines and loads.
---

A RAW file's area data gives each control area a desired net interchange
(`PDES`, MW, positive for export), a tolerance (`PTOL`) and an area slack bus
(`ISW`). With **Area interchange** set in the solve settings, Stannum moves
each area's slack generation until the area's net interchange is within
`PTOL` of `PDES`. It is off by default, as in PSS/E. In run files and Python
the setting is `area_interchange`: `"off"`, `"tie_lines"` or
`"tie_lines_and_loads"`.

## What counts as interchange

**Tie lines.** An area's net interchange is the active power leaving it on
branches whose ends are in different areas. Each tie is metered at its from
end (PSS/E's default metered end for lines, and winding 1 for transformers),
so what one area exports another imports and the tie's losses fall on the
receiving side. DC lines between areas count too, metered at the sending
converter, so the DC losses also fall on the receiving area. A multi-terminal
system shares its sent power among its receiving converters' areas in
proportion to what each delivers.

**Tie lines and loads.** As above, and a load that belongs to another area
than the bus it is on (the load record's `AREA`) counts as an export from the
bus's area to the load's area, at the power it draws at the solved voltage.

With control off, the **Areas** tab still shows every area's interchange by
tie lines.

## How the slack moves

Area interchange is an outer loop, like the reactive limit and voltage control
passes. Once the reactive limits have settled and the solve has met the full
tolerance, each **area interchange pass** compares every held area's
interchange with its `PDES`. Each area outside its tolerance moves its slack
generation by the difference, and the case is solved again. The change in
losses leaves a small error, so a second pass is sometimes needed. Shunts and
taps move after the areas are within tolerance.

- The generators at the area slack bus share each change in proportion to
  their machine base (`MBASE`), equally when the bases are missing.
- Generator active limits (`PMAX`, `PMIN`) are not enforced, as elsewhere in
  the power flow; check the generator table.
- An area that holds an island's reference bus is not held: the reference
  bus balances the island, so the area's interchange is whatever the other
  areas leave. Make the reference bus the area slack of that area, as PSS/E
  recommends. If the desired values do not add up to zero, that area takes the
  difference.
- Areas without a slack bus, or whose slack bus has no generator in service,
  are not held.
- If the solve after a pass fails, the pass is undone and tried again with
  half the step, up to four times. After 20 passes the area slack
  generation is held where it is.

The log names every area a pass moves, with its interchange, desired value
and the change at its slack bus.

## The Areas tab

The Power flow module shows an **Areas** tab when the case has more than one
area: each area's interchange, desired value and tolerance, how far it is off,
its slack bus, whether the solve held it, and how far its slack generation
moved. An error beyond the tolerance is highlighted.
