"""stannum.attach() against a stand-in for the app: a real attach listener
on a fresh core session, with a window that echoes ui.* commands back."""

from __future__ import annotations

import json
import signal
import socket
import threading
import time
from pathlib import Path

import numpy as np
import pytest

import stannum as sn
from stannum import _attach, _native

ROOT = Path(__file__).resolve().parents[3]
CASE9 = ROOT / "crates" / "stannum-core" / "tests" / "fixtures" / "case9.m"
CASE300 = ROOT / "crates" / "rs-power" / "tests" / "fixtures" / "case300.m"


@pytest.fixture
def host(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> _native.AttachHost:
    monkeypatch.setenv("STANNUM_ATTACH_DIR", str(tmp_path))
    h = _native.AttachHost(str(tmp_path))
    yield h
    h.stop()


def test_attach_finds_the_app_and_shares_its_workspace(host: _native.AttachHost) -> None:
    with sn.attach() as app:
        assert app.pid == host.pid
        assert app.case is None
        case = app.open(CASE9)
        assert case.app is app
        assert "in the app" in repr(case)
        report = case.solve(start="flat")
        assert report.converged
        assert isinstance(report.buses["vm"], np.ndarray)
        # Another script attached to the same app sees the same case.
        with sn.attach() as other:
            seen = other.case
            assert seen is not None and seen.id == case.id
            assert other.call("result_report", name="base").converged
        case.edits = [sn.outage("line:4:5:1")]
        assert case.edits == [sn.outage("line:4:5:1")]
        assert case.solve().converged
    # The window heard about every change, and nothing else.
    assert host.changed() == ["open_case", "solve_power_flow", "set_study_edits", "solve_power_flow"]


def test_errors_keep_their_kind(host: _native.AttachHost) -> None:
    with sn.attach() as app:
        with pytest.raises(sn.NoCase):
            app.call("solve_power_flow")
        case = app.open(CASE9)
        with pytest.raises(sn.NotFound):
            case.element("bus:999")
        # A case replaced in the window can't be reached by its old handle.
        app.open(CASE9)
        with pytest.raises(sn.CaseNotOpen):
            case.solve()
        with pytest.raises(sn.NotFound):
            app.call("ui.fail")


def test_ui_commands_reach_the_window(host: _native.AttachHost) -> None:
    with sn.attach() as app:
        assert app.show("bus:5") == {"command": "ui.show", "args": {"element": "bus:5"}}
        assert app.go("diagram")["args"] == {"module": "diagram"}
        assert app.run("theme")["args"] == {"id": "theme"}
        assert app.draw_around("bus:5")["command"] == "ui.draw_around"
        assert app.state["command"] == "ui.state"


def test_a_wrong_token_is_refused(host: _native.AttachHost, tmp_path: Path) -> None:
    ticket = _attach.tickets(tmp_path)[0]
    with pytest.raises(_native.CommandError) as e:
        _attach.Remote({**ticket, "token": "0" * 64}, timeout=5)
    assert e.value.args[0] == "invalid_arguments"


def test_tickets_are_private_and_go_with_the_app(host: _native.AttachHost, tmp_path: Path) -> None:
    (ticket,) = _attach.tickets(tmp_path)
    assert ticket["pid"] == host.pid and len(ticket["token"]) == 64
    if hasattr(ticket["_path"], "stat") and _attach.sys.platform != "win32":
        assert ticket["_path"].stat().st_mode & 0o077 == 0
    host.stop()
    assert _attach.tickets(tmp_path) == []
    with pytest.raises(sn.NotFound):
        sn.attach()


def test_a_stale_ticket_is_skipped_and_removed(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("STANNUM_ATTACH_DIR", str(tmp_path))
    # A port nothing listens on: bind one, then let it go.
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
    stale = tmp_path / "1-1.json"
    stale.write_text(json.dumps({"pid": 1, "port": port, "token": "x", "started": 9e15}))
    host = _native.AttachHost(str(tmp_path))
    try:
        with sn.attach() as app:
            assert app.pid == host.pid
        assert not stale.exists()
    finally:
        host.stop()


def test_the_ticket_folder_follows_the_platform(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setenv("STANNUM_ATTACH_DIR", str(tmp_path))
    assert _attach.ticket_dir() == tmp_path
    monkeypatch.delenv("STANNUM_ATTACH_DIR")
    folder = _attach.ticket_dir()
    assert folder is not None and folder.name == "attach"


@pytest.mark.skipif(not hasattr(signal, "pthread_kill"), reason="needs a real SIGINT")
def test_ctrl_c_stops_the_apps_run(host: _native.AttachHost) -> None:
    with sn.attach() as app:
        case = app.open(CASE300)
        case.solve()
        # A real SIGINT, as Ctrl+C sends, while the app checks every outage.
        main = threading.main_thread().ident
        timer = threading.Timer(0.05, signal.pthread_kill, (main, signal.SIGINT))
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
        # The connection is still in step, and the case still works.
        assert case.solve().converged
