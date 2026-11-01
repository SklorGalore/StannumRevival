"""The stannum package against the core's own test cases."""

from __future__ import annotations

import _thread
import threading
import time
from pathlib import Path

import numpy as np
import pytest

import stannum as sn

ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "crates" / "stannum-core" / "tests" / "fixtures"
CASE9 = FIXTURES / "case9.m"
FIVE_BUS = FIXTURES / "five_bus.raw"
FIVE_BUS_B = FIXTURES / "five_bus_modified.raw"
CASE300 = ROOT / "crates" / "rs-power" / "tests" / "fixtures" / "case300.m"


@pytest.fixture
def case9() -> sn.Case:
    with sn.open(CASE9) as case:
        yield case


def test_version_and_commands() -> None:
    assert sn.API_VERSION >= 2
    assert sn.__version__
    names = {c["name"] for c in sn.commands()}
    assert {"open_case", "solve_power_flow", "run_contingencies"} <= names
    schemas = sn.schemas()
    assert set(schemas["commands"]) == names


def test_open_reports_the_case(case9: sn.Case) -> None:
    assert case9.id >= 1
    assert case9.name == "case9.m"
    assert case9.path == CASE9.resolve()
    assert case9.summary["format"] == "matpower"


def test_solve_gives_numpy_columns(case9: sn.Case) -> None:
    report = case9.solve(start="flat", tolerance_mva=0.01)
    assert report.converged
    assert report.status == report["status"]
    buses = report.buses
    assert isinstance(buses, sn.Table)
    assert len(buses) == 9
    assert buses["vm"].dtype == np.float64
    assert buses["id"].dtype == np.int64
    assert buses["kind"].tolist().count("Ref") == 1
    # Bus 1 is the slack at 1.04 p.u. (MATPOWER case9).
    assert buses[0]["vm"] == pytest.approx(1.04)
    gen = report.generators
    assert gen["pg"].sum() == pytest.approx(report.total_generation_mw, rel=1e-6)
    assert report.branches["online"].dtype == bool
    low = buses[buses["vm"] < 1.0]
    assert isinstance(low, sn.Table) and 0 < len(low) < 9
    assert len(buses.rows()) == 9


def test_to_pandas(case9: sn.Case) -> None:
    pd = pytest.importorskip("pandas")
    frame = case9.solve().buses.to_pandas()
    assert isinstance(frame, pd.DataFrame)
    assert list(frame.columns)[:3] == ["id", "name", "kind"]


def test_study_edits_change_the_solve(case9: sn.Case) -> None:
    base = case9.solve()
    case9.edits = [sn.outage("line:5:6:1"), sn.bus_load(5, 100.0, 40.0)]
    assert case9.edits[0] == {"kind": "status", "element": "line:5:6:1", "online": False}
    after = case9.solve()
    assert after.converged
    assert after.total_losses_mw > base.total_losses_mw
    out = after.branches[after.branches["element"] == "line:5:6:1"]
    assert not out["online"][0]
    case9.edits = []
    again = case9.solve(start="flat")
    assert again.total_losses_mw == pytest.approx(base.total_losses_mw, rel=1e-2)


def test_typed_errors(case9: sn.Case) -> None:
    with pytest.raises(sn.NotFound) as e:
        case9.element("bus:999")
    assert e.value.kind == "not_found"
    assert "bus:999" in str(e.value)
    assert isinstance(e.value, LookupError)
    with pytest.raises(sn.InvalidArguments):
        case9.solve(start="sideways")
    with pytest.raises(sn.UnknownCommand):
        case9.call("no_such_command")
    with pytest.raises(sn.FileError):
        sn.open(FIXTURES / "missing.m")
    with pytest.raises(sn.StannumError):
        case9.edits = [sn.outage("line:77:78:1")]
        case9.solve()


def test_closed_case_says_so() -> None:
    case = sn.open(CASE9)
    case.close()
    with pytest.raises(sn.CaseNotOpen):
        case.solve()


def test_cases_are_independent() -> None:
    a = sn.open(CASE9)
    b = sn.open(CASE9)
    a.edits = [sn.outage("line:5:6:1")]
    assert b.edits == []
    assert a.solve().total_losses_mw != pytest.approx(b.solve().total_losses_mw)


def test_contingencies_rank_every_outage(case9: sn.Case, tmp_path: Path) -> None:
    case9.solve()
    run = case9.contingencies(method="ac", limit_pct=90)
    rows = run.rows
    assert len(rows) == run.total
    assert rows["rank"].tolist() == list(range(1, run.total + 1))
    detail = case9.contingency(1)
    assert detail.row["element"] == rows["element"][0]
    assert len(case9.contingency_rows(limit=3)) == 3
    assert case9.contingency_rows(offset=1, limit=2)["rank"].tolist() == [2, 3]
    path = tmp_path / "run.stnr"
    written = case9.export_contingency_results(path)
    assert written["outages"] == run.total
    assert written["bytes"] == path.stat().st_size
    assert path.read_bytes()[:4] == b"\xff\xff\xff\xff"  # an Arrow IPC stream


def test_transfer_limits(case9: sn.Case) -> None:
    case9.solve()
    run = case9.transfer_limits({"buses": [2]}, {"buses": [3]}, contingencies={})
    limiting = run.summary.limiting
    assert run.summary.fcitc_mw == limiting["fcitc_mw"]
    assert len(run.limits) > 0
    assert run.limits["fcitc_mw"][0] == run.summary.fcitc_mw
    assert np.all(np.diff(run.limits["fcitc_mw"]) >= 0)
    # Moving the FCITC brings the limiting element to its limit.
    at = limiting["flow_mw"] + limiting["factor"] * limiting["fcitc_mw"]
    assert abs(abs(at) - limiting["limit_mw"]) < 1e-6
    assert run.summary.contingencies == 9
    with pytest.raises(sn.StannumError):
        case9.transfer_limits({"buses": [5]}, {"buses": [3]})


def test_transfer_limits_from_a_lists_file(case9: sn.Case, tmp_path) -> None:
    case9.solve()
    lists = tmp_path / "transfers.stnl"
    lists.write_text(
        "stannum-lists 0.1\n"
        "subsystem two buses=[2]\n"
        "subsystem three buses=[3]\n"
        "contingency_set n1 kind=n_1_branches\n"
        "transfer east source=two sink=three rating=2\n"
    )
    run = case9.transfer_limits(lists=str(lists))
    direct = case9.transfer_limits({"buses": [2]}, {"buses": [3]}, contingencies={}, rating="b")
    assert run.summary.fcitc_mw == direct.summary.fcitc_mw
    assert run.summary.contingencies == direct.summary.contingencies == 9
    with pytest.raises(sn.StannumError):
        case9.transfer_limits(lists=str(lists), transfer="west")


def test_pv_curve(case9: sn.Case) -> None:
    case9.solve()
    run = case9.pv_curve({"buses": [5]}, respect_generation_limits=False, max_curves=3)
    assert run.summary.limited_by == "collapse"
    assert run.summary.nose_mw > 100
    assert run.points["transfer_mw"][-1] == run.summary.nose_mw
    assert np.all(np.diff(run.points["transfer_mw"]) > 0)
    # Bus 5 falls furthest; its voltage column follows the points.
    assert run.curves["bus"][0] == 5
    assert len(run.voltages["5"]) == len(run.points)
    assert np.all(np.diff(run.voltages["5"]) <= 1e-6)
    with pytest.raises(sn.StannumError):
        case9.pv_curve({})


def test_qv_curve(case9: sn.Case) -> None:
    case9.solve()
    run = case9.qv_curve([5, 7], step_pu=0.02)
    assert list(run.curves["bus"]) == [5, 7]
    assert run.summary.weakest_bus in (5, 7)
    assert np.all(run.curves["margin_mvar"] > 0)
    five = run.points["bus"] == 5
    assert np.all(np.diff(run.points["v_pu"][five]) > 0)
    # At its own voltage the condenser gives (almost) nothing.
    base = run.curves["base_vm_pu"][0]
    at = np.argmin(np.abs(run.points["v_pu"][five] - base))
    assert abs(run.points["q_mvar"][five][at]) < 1
    with pytest.raises(sn.StannumError):
        case9.qv_curve([])


def test_secure_transfer(case9: sn.Case, tmp_path) -> None:
    case9.solve()
    out = tmp_path / "run.stnr"
    run = case9.secure_transfer(
        {"buses": [5]},
        major_step_mw=20,
        minor_step_mw=2,
        respect_generation_limits=False,
        channels=[{"quantity": "qg", "element": "generator:2:1"}],
        output=str(out),
    )
    assert run.summary.stopped_by == "violation"
    assert run.summary.limit_mw >= 0
    assert run.summary.resolution_mw == pytest.approx(2.0)
    assert list(run.channels["element"]) == ["bus:5", "generator:2:1"]
    assert out.exists()
    v = case9.secure_transfer_channel(0)
    assert v.columns[0] == "transfer_mw"
    assert v.columns[1] == "BASE CASE"
    assert len(v["transfer_mw"]) == len(run.points["transfer_mw"])
    only = case9.secure_transfer_channel(1, contingencies=[], path=str(out))
    assert only.columns == ["transfer_mw", "BASE CASE"]
    with pytest.raises(sn.StannumError):
        case9.secure_transfer({"buses": [5]}, major_step_mw=0)


def test_records_and_record_edits(case9: sn.Case) -> None:
    book = case9.records()
    assert [s["id"] for s in book.sections][:3] == ["bus", "gen", "branch"]
    buses = case9.records("bus")
    assert isinstance(buses, sn.RecordTable)
    assert len(buses) == 9
    assert "PD" in buses.editable
    assert len(buses.lines) == 9 and len(buses.elements) == 9
    assert buses.elements[0] == f"bus:{int(buses['BUS_I'][0])}"
    row = int(np.flatnonzero(buses["BUS_I"] == 5)[0])
    case9.edit_records([{"section": "bus", "row": row, "column": "PD", "value": 125}])
    assert case9.records("bus")["PD"][row] == 125
    with pytest.raises(sn.NotFound):
        case9.edit_records([{"section": "bus", "row": 0, "column": "NOPE", "value": 1}])


def test_raw_case_topology_and_diff() -> None:
    with sn.open(FIVE_BUS) as case:
        assert case.summary["format"] == "raw"
        topo = case.topology()
        assert "analysis" in topo
        path = case.shortest_path(1, 3)
        assert path is None or path["buses"][0] == 1
        raw = case.records("BUS")
        assert len(raw) == 5
    d = sn.diff(FIVE_BUS, FIVE_BUS_B)
    assert d.differing > 0
    changed = next(s for s in d.summary.sections if s["counts"]["changed"])
    records = d.records(changed["name"])
    assert len(records) > 0


def test_export_round_trip(case9: sn.Case, tmp_path: Path) -> None:
    case9.solve()
    out = tmp_path / "solved.m"
    case9.export_matpower(out, solved=True)
    with sn.open(out) as again:
        assert again.solve(start="case").iterations <= 1


def test_raw_export(tmp_path: Path) -> None:
    with sn.open(FIVE_BUS) as case:
        out = tmp_path / "five_bus_v33.raw"
        done = case.export_raw(out, revision=33)
        assert done["revision"] == 33
        assert done["source"] == "records"
    with sn.open(out) as again:
        assert again.solve().converged


def test_run_file(tmp_path: Path) -> None:
    run = tmp_path / "study.stnrun"
    run.write_text(
        f"open_case path={CASE9.as_posix()}\n"
        "solve_power_flow settings={\"start\": \"flat\"}\n"
        f"export_matpower path={(tmp_path / 'out.m').as_posix()} solved=true\n"
    )
    report = sn.run_file(run)
    assert report.failed == 0
    assert report.steps == 3
    assert (tmp_path / "out.m").exists()


def test_log_is_kept(case9: sn.Case) -> None:
    case9.solve()
    log = sn.log()
    assert len(log) > 0
    assert log["seq"].dtype == np.int64


def test_threads_run_side_by_side() -> None:
    cases = [sn.open(CASE300) for _ in range(2)]
    results: list[bool] = []

    def work(case: sn.Case) -> None:
        for _ in range(3):
            results.append(case.solve(start="flat").converged)

    threads = [threading.Thread(target=work, args=(c,)) for c in cases]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    assert results == [True] * 6


def test_ctrl_c_stops_a_long_run() -> None:
    case = sn.open(CASE300)
    case.solve()
    # Interrupt as Ctrl+C would, while the AC check of every outage runs.
    timer = threading.Timer(0.05, _thread.interrupt_main)
    started = time.monotonic()
    timer.start()
    try:
        case.contingencies(method="ac")
    except KeyboardInterrupt:
        pass
    else:
        timer.cancel()
        pytest.skip("the run finished before the interrupt")
    assert time.monotonic() - started < 10
    # The case is still usable afterwards.
    assert case.solve().converged
