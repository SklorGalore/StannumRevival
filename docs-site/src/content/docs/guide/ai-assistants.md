---
title: AI assistants (MCP)
description: Let Claude and other AI assistants open cases, solve, run contingencies and read results through Stannum's MCP server.
---

`stannum-cli mcp` runs Stannum as a
[Model Context Protocol](https://modelcontextprotocol.io) (MCP) server. An AI
assistant that speaks MCP, such as Claude Desktop, Claude Code or any other
MCP client, can then use Stannum's analysis as tools: open a case, solve it,
find the violations, run N-1 or the NERC TPL-001 planning events, look at
topology, compare RAW files and export. You ask in plain words ("which
outage overloads the most lines?") and the assistant calls the tools.

The server runs on your computer and reads and writes files there. Nothing
about the case leaves your computer except what the assistant reads from the
tools' answers.

## Two ways to run it

| Command | What the assistant drives |
|---|---|
| `stannum-cli mcp` | A Stannum session of its own, with no window. Good for headless work and big batch questions. |
| `stannum-cli mcp --attach` | The Stannum app that is running now. Every case it opens, every edit and every result shows in the window, and five more tools let it point the window at things. Start the app first. `--pid <pid>` picks one app when several are open. |

Attach uses the same local connection as
[Python's `stannum.attach()`](/StannumRevival/guide/python/#driving-the-running-app).

## Set it up

You need `stannum-cli` from the release page (see
[Download](/StannumRevival/start/download/)). Use its full path below if it
isn't on your `PATH`.

**Claude Code:**

```sh
claude mcp add stannum -- stannum-cli mcp
# or, to drive the running app:
claude mcp add stannum -- stannum-cli mcp --attach
```

**Claude Desktop:** open **Settings > Developer > Edit Config** and add
Stannum to `claude_desktop_config.json`, then restart Claude Desktop:

```json
{
  "mcpServers": {
    "stannum": {
      "command": "C:\\Tools\\stannum-cli.exe",
      "args": ["mcp", "--attach"]
    }
  }
}
```

Leave out `"--attach"` for a session of its own. On a Mac or Linux the
command is a path such as `/usr/local/bin/stannum-cli`.

**Other clients** start `stannum-cli mcp` and talk to it over stdin and
stdout (the MCP stdio transport).

## Tools

| Area | Tools |
|---|---|
| Cases | `list_sample_cases`, `open_case` (a path or a built-in sample), `case_summary` |
| Power flow | `solve_power_flow` (the same settings as the Power flow module), `find_violations`, `query_results` (buses, branches or generators, filtered, sorted and paged), `element_detail` |
| What-if | `get_study_edits`, `set_study_edits` (outages, dispatch and loads applied on top of the case) |
| Contingencies | `run_contingencies` (N-1 or NERC planning events), `planning_event_counts`, `query_contingencies`, `contingency_detail`, `trip_and_compare` |
| Transfer limits | `transfer_limits` (DFAX and FCITC from a source to a sink, in the base case and after each contingency) |
| Topology | `topology_summary` (islands, bridges, articulation buses), `shortest_path` |
| Records | `list_records`, `query_records`, `edit_records` |
| Comparing | `compare_cases`, `diff_records` |
| Files | `export_case` (RAW 33/35, MATPOWER, `.stn`, `.stnb`), `run_run_file` |
| Anything else | `list_commands`, `call_command` (any command in the [command list](/StannumRevival/develop/commands/)) |
| The window, with `--attach` | `app_state`, `app_show`, `app_go`, `app_draw_around`, `app_say` |

Tables come back a page at a time (50 rows unless the assistant asks for
more, at most 500), filtered and sorted in Stannum, so an 80,000-bus case
doesn't flood the conversation. Solve and contingency answers are summaries;
the assistant asks for the rows it needs. A long contingency run reports its
progress, and stops when the assistant's client cancels the call.

Tools that only read are marked read-only, and `export_case`,
`run_run_file` and `call_command` are marked as able to overwrite files, so
a client that asks before risky steps asks before those.

## Things to ask

- "Open `C:\cases\summer_peak.raw`, solve it and list the ten most loaded
  branches."
- "Which buses are below 0.95 p.u.? Show me the worst one in the app."
- "Run the P1 planning events above 230 kV and summarise the overloads by
  category."
- "Take line 4-5 out, re-solve and tell me what changed."
- "Compare `before.raw` and `after.raw` and list the changed generators."

## Versions

The `mcp` subcommand is a preview: tool names, arguments and answers are
written for assistants, not scripts, and can change in a minor release. For
a stable interface use run files, the Python package or the commands
themselves (see [Versions and compatibility](/StannumRevival/reference/versioning/)).
