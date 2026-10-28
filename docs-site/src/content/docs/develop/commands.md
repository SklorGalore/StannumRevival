---
title: The command registry
description: How commands are declared in stannum-core, how every client reaches them, and how their schemas keep the frontend's types honest.
---

Every operation a client asks of the core is a **command**: a name and a
JSON object of arguments in, JSON text out. They are all declared in one
place, `crates/stannum-core/src/commands.rs`, and every client goes through
the same entry point:

```rust
let session = stannum_core::commands::Session::new();
let summary: String = session.invoke("open_case", json!({ "path": "case9.m" }))?;
```

| Client | How it reaches `Session::invoke` |
| --- | --- |
| Desktop window | `invokeCore` in `src/lib/analysis/api.ts` → Tauri command `stannum_command` (`src-tauri/src/analysis.rs`). |
| Browser tests | `dev_bridge` example: `POST /invoke/<command>` on 127.0.0.1:1421. |
| Python | `stannum._native.Session.invoke` (`crates/stannum-py/src/lib.rs`), in-process. |
| Python attached to the app | One JSON line per command over the attach socket (`stannum_core::attach`), on the window's own `Session`. |
| Run files | `Session::run_steps` (`run_file` command, `stannum-cli run`). |

Because the list exists once, a new command is available to all of these
as soon as it's declared.

## Declaring a command

Commands are written inside the `commands!` macro. Each entry has a doc
comment, a name, its arguments, its result type and a body:

```rust
commands! {
    /// Solve the power flow with the study's edits. `cancel_solve` stops it.
    /// Settings left out take their defaults.
    solve_power_flow { case: Option<CaseId>, settings: Option<SolveSettings> } -> crate::power::SolveReport =>
        |s, a| s.solve(a.case, a.settings.unwrap_or_default());

    /// The shortest path between two buses, or null.
    topology_shortest_path { case: Option<CaseId>, from: i32, to: i32 } -> Option<nodal::topology::Path> =>
        |s, a| s.workspace()?.case(a.case)?.shortest_path(a.from, a.to);
}
```

From each entry the macro generates:

- **An argument struct** `args::<name>` deriving `Deserialize` and
  `JsonSchema`, with `rename_all = "camelCase"` and `deny_unknown_fields`.
  A misspelt argument is refused with `invalid_arguments`; an `Option`
  argument may be left out.
- **A dispatch arm.** The body gets `s: &Session` and `a: args::<name>` and
  returns `Result<ReturnType, Error>`; the result is serialised to JSON.
- **An entry in `COMMANDS`** (`CommandInfo`): the name, the doc comment
  (which `list_commands` serves and the Python and CLI help show),
  `takes_case` and `action`.
- **Its schema** in `schemas()`: the argument and result types as JSON
  Schema 2020-12.

The doc comment matters: it is the command's only description, and a test
fails if one is missing.

### The `case` argument

A command whose arguments include a field named `case` is *case-scoped*.
Declare it as `case: Option<CaseId>` and pass it to `ws.case(a.case)` or
`ws.case_mut(a.case)`. Left out, the command uses the open case; given, it
must be the open case's id, or the command fails with `case_not_open`. A
test sends every case-scoped command a wrong id and checks it is refused
before anything else happens.

### Actions

`run_file.rs` keeps a list, `ACTIONS`, of commands that change the case or
write a file. Those are the ones **Record** keeps in a run file, and
`CommandInfo::action` reports it. Add a new command there if it changes
state; leave read-only commands out.

### Results

A result type must implement `Serialize` and `JsonSchema`. Large row
tables use `table::Table<R>`, which sends one array per field instead of one
object per row; give such rows a matching decoder on the TypeScript side
(`src/lib/analysis/wire.ts`). When a result borrows from the workspace,
build it into `Json<T>` (`Json::of(&value)`) while the lock is held: it
keeps `T`'s schema but owns its JSON, so the lock can be let go before the
answer is written.

## Errors

Every command returns `stannum_core::Error`, which serialises as
`{"kind": "...", "message": "..."}`. Use the constructors in `error.rs`
(`Error::not_ready`, `Error::not_found`, `Error::unsupported`,
`Error::invalid_arguments`, `Error::file`, `Error::internal` and so on) and
write the message for the person reading it in the app. Clients branch on
the kind, never the message. The kinds are listed in
[Architecture](/StannumRevival/manual/architecture/#commands); adding one is
an API change.

## Schemas and generated types

The schemas are checked in twice:

- `crates/stannum-core/schema/commands.schema.json`: every command's
  arguments and result, the error, and the element kinds.
- `src/lib/analysis/commands.gen.ts`: TypeScript generated from it
  (`schema.rs`). It gives `CoreCommand`, `CoreArgs<C>` and `CoreResult<C>`,
  which type `invokeCore`.

The `schemas` test in stannum-core fails when either copy is out of date,
and also runs **every** command on real cases, checking each answer and
error against its schema. After changing a command or anything it returns,
write both again:

```powershell
pwsh scripts/update-schemas.ps1
```

`src/lib/analysis/types.ts` re-exports the generated types under the names
the interface uses, and adds by hand only the shapes the interface rebuilds
from columns (solve reports, record tables, the topology snapshot).

## Long work and the workspace lock

The `Session` keeps the `Workspace` behind a mutex. A command that reads or
changes a little can lock it for its whole body. Long work must not: a
solve, sensitivities, contingency runs, "Trip and compare", the first read
of a case's records and Records edits all follow the same pattern:

1. Lock, take a *job* from the open case (shared `Arc`s, not copies) and
   note the case's `version`. Unlock.
2. Run the job.
3. Lock again and hand the result back to the case. If the case's version
   changed meanwhile (new case, study edits, Records edits), the result is
   set aside with a message (`case_changed`), or for reads, redone up to
   `RETRIES` times.

`Session::solve` and `Session::run_contingencies` are the examples to copy.
Cancellation goes through handles the `Session` keeps outside the lock
(`solving`, `contingencies`, `running_file`), so `cancel_solve`,
`cancel_contingencies`, `cancel_run_file` and `contingency_progress` answer
while the work runs. `tests/lock_free_work.rs` checks this.

## Versioning

`API_VERSION` goes up when a command is removed, or an argument or result
changes in a way existing callers would notice. Adding a command or an
optional argument doesn't need a bump. `list_commands` reports it, and
`stannum.attach()` refuses to drive an app whose version differs from the
package's.
