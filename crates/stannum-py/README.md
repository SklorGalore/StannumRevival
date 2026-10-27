# stannum (Python)

Stannum's analysis core as a Python package: open PSS/E RAW and MATPOWER
cases, solve power flow, run N-1 contingencies, read and edit records,
check topology and compare RAW files, from a script, a notebook or CI. No
app window needed.

```python
import stannum as sn

case = sn.open("case9.m")
report = case.solve(start="flat")
print(report.converged, report.iterations, report.total_losses_mw)

buses = report.buses              # a Table: one NumPy array per field
low = buses[buses["vm"] < 0.95]   # rows where the voltage is low
buses.to_pandas()                 # with pandas installed

case.edits = [sn.outage("line:4:5:1")]
after = case.solve()

ranked = case.contingencies(method="ac")
ranked.rows                       # every outage, worst first
```

Every call goes through the same command registry the desktop app uses
(`crates/stannum-core/src/commands.rs`), so a script and the app give the
same answers. `case.call(name, **args)` runs any registry command, and
`stannum.commands()` lists them.

- Each `Case` is independent: open several and use them from several
  threads. Long commands let the GIL go, and Ctrl+C stops them.
- Failures raise typed errors (`stannum.NotFound`, `stannum.NotReady`,
  `stannum.Cancelled`, …), all subclasses of `stannum.StannumError`.

The user guide page is <https://sklorgalore.github.io/StannumRevival/guide/python/>.

## Build

Wheels are built with [maturin](https://www.maturin.rs/) and are abi3: one
wheel per platform works on Python 3.10 and newer.

```powershell
pwsh scripts/ci/test-python.ps1        # build the wheel, install it in a venv, run the tests
```

or by hand:

```sh
cd crates/stannum-py
pip install maturin pytest
maturin develop --release
pytest
```
