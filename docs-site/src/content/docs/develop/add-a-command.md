---
title: Add a command
description: A worked example of adding a core command, from the Rust body to the schema, the tests, the interface and Python.
---

This walks through adding a small read-only command, `bus_voltage`, which
returns one bus's solved voltage. The same steps apply to any command. The
code below was built and tested against the current core.

## 1. Write the logic where it belongs

Put the work in the module that owns the data, not in `commands.rs`. A
solve report lives on the `OpenCase`, so this goes in `power.rs`:

```rust
/// One bus's solved voltage.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct BusVoltage {
    pub bus: u32,
    /// Magnitude, per unit.
    pub vm: f64,
    /// Angle, degrees.
    pub va: f64,
}

impl crate::OpenCase {
    /// One bus's voltage from the latest solve, or null if the case has no
    /// bus by that number.
    pub fn bus_voltage(&self, bus: u32) -> Result<Option<BusVoltage>, crate::Error> {
        let report = self
            .report
            .as_ref()
            .ok_or_else(|| crate::Error::not_ready("Solve the power flow first."))?;
        Ok(report.buses.iter().find(|b| b.id == bus).map(|b| BusVoltage {
            bus,
            vm: b.vm,
            va: b.va,
        }))
    }
}
```

The result type derives `Serialize` and `JsonSchema`; doc comments on its
fields become comments in the generated TypeScript. The error is a
`not_ready` with a message written for the person at the window.

## 2. Declare it

Add an entry to the `commands!` block in
`crates/stannum-core/src/commands.rs`, near related commands:

```rust
    /// One bus's voltage from the latest solve, or null if there is no such bus.
    bus_voltage { case: Option<CaseId>, bus: u32 } -> Option<crate::power::BusVoltage> =>
        |s, a| s.workspace()?.case(a.case)?.bus_voltage(a.bus);
```

- The doc comment is required. It is what `list_commands`, the Python help
  and `stannum-cli commands` show.
- `case: Option<CaseId>` makes it case-scoped. Always include it for a
  command that reads or changes the open case.
- Arguments are camelCase in JSON. A field `limit_pct` is sent as
  `limitPct`.
- This body holds the workspace lock for its whole run, which is fine for a
  lookup. For anything that can take more than a moment, use the job
  pattern in [Long work and the workspace lock](/StannumRevival/develop/commands/#long-work-and-the-workspace-lock).
- If the command changes the case or writes a file, add its name to
  `ACTIONS` in `run_file.rs` so **Record** keeps it in run files.

## 3. Exercise it in the schema test

`crates/stannum-core/tests/schemas.rs` runs every command and checks each
answer against its schema. It fails with `not exercised: ["bus_voltage"]`
until you call the new command there. Add a call after the five-bus case is
solved:

```rust
    let v = c.ok("bus_voltage", json!({ "case": case, "bus": 1 }));
    assert_eq!(v["vm"], 1.04);
```

The unit test `every_case_command_checks_the_case_id` in `commands.rs`
sends every case-scoped command a wrong case id. It invents a value for
each argument by name; if yours has a new name that must not be a number,
add it to the `value` match there.

## 4. Write the schemas again

```powershell
pwsh scripts/update-schemas.ps1
```

This rewrites `crates/stannum-core/schema/commands.schema.json` and
`src/lib/analysis/commands.gen.ts`. Commit both. The new command now
appears in `CoreCommands`:

```ts
  /** One bus's voltage from the latest solve, or null if there is no such bus. */
  bus_voltage: {
    args: { bus: number; case?: CaseId | null };
    result: BusVoltage | null;
  };
```

Then run the core's tests:

```sh
cargo test -p stannum-core
```

## 5. Call it from the interface

`invokeCore` is typed from the generated file, so the call is checked by
`tsc`:

```ts
import { invokeCore } from "../lib/analysis/api";

const v = await invokeCore("bus_voltage", { bus: 101 }); // BusVoltage | null
```

For a command the interface uses in more than one place, add a wrapper to
the `api` object in `src/lib/analysis/api.ts` built on `call(...)`, which
also writes the step in the log strip. Give it a line in `LOG_LABELS` there
("Reading a bus voltage", `quiet: true` for quick reads that should only
show up when they fail). Stores in `src/store/` call `api`, and components
read the stores.

## 6. Python, run files and the CLI

Nothing is needed for the command to work everywhere else:

```python
case.call("bus_voltage", bus=101)
```

```text
bus_voltage bus=101
```

works in Python, in a run file and in `stannum-cli`. Add a method to `Case`
in `crates/stannum-py/python/stannum/__init__.py` (and a test in
`crates/stannum-py/tests/`) only if it's something scripts will use often.

## 7. Document it

Change the docs in the same pull request: the user guide page for anything
visible in the app, the [Python](/StannumRevival/guide/python/) page for a
new method, and the technical manual for new analysis. Add a line under
**Unreleased** in `CHANGELOG.md` if users would notice.

## Checklist

- [ ] Logic in its own module, returning `Result<T, Error>`.
- [ ] `commands!` entry with a doc comment and `case: Option<CaseId>`.
- [ ] In `ACTIONS` if it changes the case or writes a file.
- [ ] Called in `tests/schemas.rs`.
- [ ] `pwsh scripts/update-schemas.ps1`, both files committed.
- [ ] `API_VERSION` bumped if an existing command's arguments or result
      changed incompatibly.
- [ ] Interface wrapper and log label, if the app uses it.
- [ ] Docs and changelog.
