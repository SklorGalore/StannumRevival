"""Drive the running Stannum app from a script: :func:`stannum.attach`.

The app listens on a local port (127.0.0.1 only) and leaves a *ticket* with
the port and a random token in a folder only you can read. ``attach()``
reads the newest ticket, connects and shows the token. Commands then run on
the app's own workspace: the case on screen, its study and its results.
See stannum-core's ``attach.rs`` for the protocol.
"""

from __future__ import annotations

import json as _json
import os
import socket
import sys
import threading
from pathlib import Path
from typing import Any

from . import _native

#: Commands that stop long work. Each is harmless when nothing runs.
_CANCEL = (
    "cancel_solve",
    "cancel_contingencies",
    "cancel_transfer_limits",
    "cancel_pv_curve",
    "cancel_qv_curve",
    "cancel_secure_transfer",
    "cancel_run_file",
)


def ticket_dir() -> Path | None:
    """The folder the app leaves its tickets in (the same rule as the app):
    ``STANNUM_ATTACH_DIR`` when set; ``%LOCALAPPDATA%\\Stannum\\attach`` on
    Windows; ``~/Library/Application Support/Stannum/attach`` on macOS;
    ``$XDG_RUNTIME_DIR/stannum/attach`` or ``~/.local/state/stannum/attach``
    elsewhere."""

    def env(key: str) -> Path | None:
        value = os.environ.get(key)
        return Path(value) if value else None

    given = env("STANNUM_ATTACH_DIR")
    if given:
        return given
    if sys.platform == "win32":
        local = env("LOCALAPPDATA")
        return local / "Stannum" / "attach" if local else None
    home = env("HOME")
    if sys.platform == "darwin":
        return home / "Library" / "Application Support" / "Stannum" / "attach" if home else None
    run = env("XDG_RUNTIME_DIR")
    if run:
        return run / "stannum" / "attach"
    return home / ".local" / "state" / "stannum" / "attach" if home else None


def tickets(folder: Path | None = None) -> list[dict[str, Any]]:
    """Every app that left a ticket, newest first. A ticket can outlive its
    app (after a crash); :func:`stannum.attach` skips those."""
    folder = folder or ticket_dir()
    found = []
    if folder and folder.is_dir():
        for path in folder.glob("*.json"):
            try:
                ticket = _json.loads(path.read_text(encoding="utf-8"))
                ticket["_path"] = path
                found.append(ticket)
            except (OSError, ValueError):
                continue
    found.sort(key=lambda t: t.get("started", 0), reverse=True)
    return found


class _Line:
    """One connection to the app: a JSON object per line, both ways."""

    def __init__(self, ticket: dict[str, Any], timeout: float) -> None:
        self.ticket = ticket
        self._sock = socket.create_connection(("127.0.0.1", int(ticket["port"])), timeout=timeout)
        self._buffer = bytearray()
        hello = {"hello": {"token": ticket["token"], "client": f"stannum {_native.__version__} (Python)"}}
        answer = self.exchange(hello)
        # Long solves take as long as they take.
        self._sock.settimeout(None)
        if "error" in answer:
            self.close()
            kind, message = answer["error"]["kind"], answer["error"]["message"]
            raise _native.CommandError(kind, message)
        self.info: dict[str, Any] = answer["ok"]

    def send(self, message: Any) -> None:
        self._sock.sendall(_json.dumps(message).encode() + b"\n")

    def receive(self) -> Any:
        while b"\n" not in self._buffer:
            chunk = self._sock.recv(1 << 16)
            if not chunk:
                raise ConnectionError("The app closed the connection. Was it closed?")
            self._buffer += chunk
        line, _, rest = bytes(self._buffer).partition(b"\n")
        self._buffer = bytearray(rest)
        return _json.loads(line)

    def exchange(self, message: Any) -> Any:
        self.send(message)
        return self.receive()

    def close(self) -> None:
        try:
            self._sock.close()
        except OSError:
            pass


class Remote:
    """Runs registry commands on the app's workspace, as
    ``_native.Session`` does on a workspace of its own: ``invoke(command,
    args_json) -> answer_json``, failures as ``_native.CommandError``.
    Ctrl+C stops the app's solve, contingency run or run file, waits for
    its answer and raises ``KeyboardInterrupt``."""

    def __init__(self, ticket: dict[str, Any], timeout: float) -> None:
        self._ticket = ticket
        self._timeout = timeout
        self._line = _Line(ticket, timeout)
        self._lock = threading.Lock()
        self._next = 0

    @property
    def info(self) -> dict[str, Any]:
        return self._line.info

    def invoke(self, command: str, args: str = "{}") -> str:
        with self._lock:
            self._next += 1
            ident = self._next
            self._line.send({"id": ident, "command": command, "args": _json.loads(args)})
            interrupted: BaseException | None = None
            while True:
                try:
                    answer = self._line.receive()
                    break
                except KeyboardInterrupt as e:
                    if interrupted is None:
                        interrupted = e
                        self._cancel()
            if interrupted is not None:
                raise interrupted
        if answer.get("id") != ident:
            raise _native.CommandError("internal", f"{command}: the app answered another request.")
        if "error" in answer:
            raise _native.CommandError(answer["error"]["kind"], answer["error"]["message"])
        return _json.dumps(answer["ok"])

    def _cancel(self) -> None:
        """Ask the app to stop long work, on a connection of its own (this
        one is waiting for the answer)."""
        try:
            line = _Line(self._ticket, self._timeout)
        except (OSError, _native.CommandError):
            return
        try:
            for i, command in enumerate(_CANCEL):
                line.exchange({"id": i, "command": command, "args": {}})
        except (OSError, ValueError):
            pass
        finally:
            line.close()

    def close(self) -> None:
        self._line.close()


def connect(pid: int | None = None, timeout: float = 5.0, folder: Path | None = None) -> Remote:
    """Connect to the newest running app, or the one with process id
    ``pid``. Tickets whose app has gone are removed."""
    found = tickets(folder)
    if pid is not None:
        found = [t for t in found if t.get("pid") == pid]
    where = folder or ticket_dir()
    for ticket in found:
        try:
            return Remote(ticket, timeout)
        except ConnectionRefusedError:
            # The app is gone (it crashed or was killed): its ticket is stale.
            try:
                ticket["_path"].unlink()
            except OSError:
                pass
        except (OSError, ValueError):
            continue
    which = f"with process id {pid}" if pid is not None else "running"
    raise _native.CommandError(
        "not_found",
        f"No Stannum app {which} to attach to (looked for tickets in {where}). Start the app first.",
    )
