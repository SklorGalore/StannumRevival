---
title: Topology analysis
description: How the topology graph is built, and the algorithms for islands, bridges, articulation buses and fewest-hop paths.
---

Topology analysis works on connectivity alone: which buses are joined by
in-service equipment, with no impedances or flows. It runs in the nodal crate
on a graph built with [rustworkx-core](https://github.com/Qiskit/rustworkx)
(the Rust core of rustworkx, on petgraph).

## Building the graph

From a revision 35 RAW file:

- **Nodes** are buses with `IDE` ≠ 4 (isolated buses are excluded).
- **Edges** are in-service equipment terminals: lines, two-winding
  transformers and switching devices. Open switches and out-of-service
  connections do not connect buses.
- **Three-winding transformers** get a synthetic **star junction** node, with
  one edge per in-service winding.
- **Parallel circuits** stay distinct: the graph keeps one edge per pair of
  nodes, carrying every connection between them, and each connection adds to
  the buses' **degree**.

### Merging zero-impedance branches

With **Merge zero-impedance branches**, lines whose impedance is below the
case's zero-impedance threshold (`THRSHZ`, 0.0001 p.u. unless the file sets it) are absorbed: the buses
they join become one node, as PSS/E treats them in power flow. Absorbed
equipment is counted separately.

### Files nodal can't read

For MATPOWER files and RAW revisions other than 35, Stannum builds a
topology case from the power flow model instead: buses, branches (each
three-winding transformer becomes one piece of equipment again), generators
and per-bus load. It has no switching devices or original records, and the
diagnostics say so.

## Islands

Islands are the **connected components** of the graph. They are numbered
from 1 in order of size, largest first (ties by lowest bus number), and every
bus is tagged with its island.

## Bridges

A **bridge** is an edge whose removal splits its island. Bridges are found
with Tarjan's linear-time algorithm (rustworkx-core `bridges`), which
tracks each node's DFS discovery time and the earliest node reachable from its
subtree. Only an edge carrying a **single** connection is reported: parallel
circuits are never bridges, since losing one leaves the other.

A bridge on a three-winding transformer is one specific winding connection,
not an outage of the whole transformer.

## Articulation buses

An **articulation point** is a node whose removal splits its component, found
with the same DFS low-link method (rustworkx-core `articulation_points`).
Articulation buses are reported in the **Critical connections** table and the
inspector. Synthetic star junctions that are articulation points are listed
separately rather than counted as buses.

## Fewest-hop paths

**Path** runs a **breadth-first search** over bus adjacency, where each piece
of equipment crossed is one hop. A three-winding transformer is one hop
between any two of its windings. The first path found to the target is a
shortest one, and ties are broken the same way every time. The
result lists the buses and equipment along the way, and the star
representation is rebuilt only for drawing.

A missing or out-of-service endpoint is an error; unreachable buses return
"no path".

## Complexity

All of the above are linear in the size of the graph,
$O(\lvert V\rvert + \lvert E\rvert)$.
