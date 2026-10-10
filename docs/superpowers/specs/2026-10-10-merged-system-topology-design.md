# Merged system topology

## Intent and scope

The user wants a checkbox available before loading a RAW case that treats buses joined by zero-impedance branches as one node throughout the application. When enabled, graphing, islands, degrees, bridges, articulation nodes, shortest paths, neighborhoods, tables, and analysis exports must use the merged topology. This supersedes the previous neighborhood-only grouping behavior.

## Grouping rules

Reuse the parsed GENERAL THRSHZ value and existing transitive groups of active buses connected by active nontransformer lines with R = 0 and |X| <= THRSHZ. THRSHZ = 0 disables merging. Transformers and switching devices do not create groups. Excluded buses cannot join active groups.

Each merged node uses the smallest member bus number as its stable representative and retains a sorted list of all original bus numbers. Singleton nodes retain their current bus number and identity. Original parsed RAW records remain available for inspection and in the source-case portion of JSON reports.

## Rust topology

Add an explicit topology construction option, defaulting to unmerged behavior for existing Rust callers. Map original bus numbers to representative nodes before creating connections. Remove connections internal to a merged node, including other equipment whose active terminals all resolve to the same node. Retain distinct external equipment circuits and their original equipment IDs.

Three-winding transformers continue to count as one equipment hop. Their active terminal mappings must handle repeated representatives without creating false connectivity or exposing an internal connection as a bridge. Preserve source winding information for external connections.

Run connected-component, degree, bridge, and articulation algorithms on the resulting graph. Report one degree and articulation entry per representative node. Islands list representative nodes; member lists provide access to original buses. Distinguish source active-bus counts from analysis-node counts and absorbed internal equipment from out-of-service equipment.

Resolve either original bus number supplied to a path request through the representative mapping. A path between members of the same node has zero hops. Path node IDs and returned representative bus sequence match the displayed graph. Paths outside a group use external equipment only.

## Worker and state

Pass the merge option through the worker into Rust topology construction. Support rebuilding the topology of the case already held in the worker without reopening the file or sending case contents elsewhere. Include the applied option in snapshots and JSON reports.

Use one global checkbox labeled "Treat zero-impedance branches as the same node", available on the import screen and in the loaded workspace. It is initially unchecked. Retain the user's choice when replacing a case. The inspector must reflect the same global option rather than maintain an independent distance-only mode.

Changing the option rebuilds analysis, clears stale paths, and refreshes the current graph and table selections against the new node mapping. Generation checks prevent an older path, topology response, or file load from replacing a newer selection. During rebuilding, prevent actions that would combine old analysis with the new option.

## Graphs and inspection

Render each merged group as one node, using its representative and member count in the label. Its inspector lists member bus numbers and names, aggregates connected equipment, loads, and generators, and lets users inspect individual source records. Searching for any member number or name finds the merged node.

Provide a "Show full network" action that uses the active topology, so full-system graphing explicitly follows the option. The existing 500-node display limit applies after merging; analysis and exports remain complete beyond that limit. Island, path, and neighborhood views all use the same active graph.

## Tables and exports

In merged mode, topology bus tables and neighborhood CSV exports contain one row per analysis node, with representative bus number and member bus numbers. Degree, island, and articulation values describe that merged node. Excluded source buses remain identifiable as excluded, without becoming active analysis nodes.

Equipment tables identify branches absorbed into a merged node separately from excluded equipment. Metrics distinguish original RAW bus and equipment totals from merged analysis totals. Islands and critical-connection exports use the same representative identities as their views.

JSON reports retain original source records and explicitly include the applied merge option, member mapping, active graph, analysis, and path. CSV formula neutralization and worker/WASM resource cleanup remain intact.

## Validation

Rust tests cover contraction, transitive membership, threshold boundaries, inactive exclusions, representative lookup, zero-hop paths within groups, external paths, changed articulation/bridge results, parallel circuits after contraction, and transformer terminal collisions.

Frontend and production browser tests cover choosing the option before import, full-network node counts, merged inspector membership, member search, node-based CSV and JSON exports, global toggling, choice persistence on case replacement, and stale response protection. Existing unmerged fixture acceptance tests remain passing. Run cargo test, cargo fmt --check, npm test, npm run build, and the complete production browser suite.
