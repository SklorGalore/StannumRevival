---
title: Glossary
description: Terms used in Stannum and this documentation.
---

### Articulation bus

A bus whose loss would split its island in two.

### Bridge

A single piece of equipment whose loss would split its island.

### Compounding point

The point on an LCC DC line, `RCOMP` ohms from the inverter, where the
scheduled DC voltage is held.

### Island

A set of buses connected to each other by in-service equipment and to no
other bus.

### LCC

Line-commutated converter: a thyristor converter whose commutation depends
on the AC voltage. Absorbs reactive power.

### LODF

Line outage distribution factor: the change in flow on one branch per MW
of pre-outage flow on another branch that is taken out.

### Loose pass

A Newton solve stopped at 0.01 p.u. while generator reactive limits may
still switch.

### MTDC

Multi-terminal DC: a DC network with more than two converters.

### Pass

One Newton solve within a power flow. Controls act between passes.

### PTDF

Power transfer distribution factor: the change in a branch's flow per MW
injected at a bus and withdrawn at the reference.

### PV, PQ, reference bus

Bus types by what is held: active power and voltage magnitude; active and
reactive power; voltage magnitude and angle.

### Remote regulation

A generator holding the voltage of a bus other than its terminal.

### Shared identity

The one name an element has in every module, so selecting it in one module
finds it in the others. Bus numbers in a branch's name are sorted; the last
part is the circuit, machine id or name. Scripts and run files use the same
names (`element_kinds` lists them):

| Element | Name | From |
|---|---|---|
| Bus | `bus:101` | power flow model |
| Line | `line:101:102:1` | power flow model |
| Transformer | `transformer:101:102:1`, `transformer:101:102:103:1` | power flow model |
| Switch | `switch:101:102:1` | topology (RAW 35) |
| Generator | `generator:101:1` | power flow model |
| Load | `load:101:1` | power flow model |
| Fixed shunt | `shunt:101:1` | its RAW record |
| Switched shunt | `switched-shunt:101:1` | its RAW record |
| Induction machine | `machine:101:1` | its RAW record |
| DC line (two-terminal or VSC) | `dcline:101:205:DC1` | power flow model |
| FACTS device | `facts:SVC_1` | its RAW record |

Elements read from their RAW record show every field as filed in the
inspector, and appear among a bus's connections once Records has read the
case.

### Study

A list of edits (outages, generator output, load) that every solve uses
without changing the file.

### VSC

Voltage source converter: a self-commutated converter that can control its
AC voltage or power factor independently of its active power.
