"""The package's code. Everything public is re-exported from ``stannum``
(see ``__init__.py``); nothing here is public under its own name."""

from __future__ import annotations

import json as _json
import os
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path
from typing import Any, Union

from . import _attach, _native
from ._table import Answer, Table, _plain, tables_in
from ._errors import (
    CaseChanged,
    CaseNotOpen,
    Cancelled,
    Failed,
    FileError,
    InternalError,
    InvalidArguments,
    NoCase,
    NotFound,
    NotReady,
    StannumError,
    UnknownCommand,
    Unsupported,
    from_kind,
)

#: Version of the command set this package was built with. It goes up when
#: a command is removed or changes in a way existing scripts would notice.
API_VERSION: int = _native.API_VERSION

_PathLike = Union[str, "os.PathLike[str]"]

#: What commands run on: a workspace of the package's own
#: (``_native.Session``) or the running app's (``_attach.Remote``).
_Link = Union[_native.Session, _attach.Remote]

# --- calling the registry -------------------------------------------------


def _camel(name: str) -> str:
    """``keep_going`` -> ``keepGoing``: the registry's argument names."""
    head, *rest = name.split("_")
    return head + "".join(part[:1].upper() + part[1:] for part in rest)


def _encode(value: Any) -> Any:
    """Paths as text, and any other value JSON can't hold as an error."""
    if isinstance(value, os.PathLike):
        return os.fspath(value)
    if isinstance(value, Mapping):
        return {k: _encode(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [_encode(v) for v in value]
    if hasattr(value, "item") and callable(value.item):  # NumPy scalars
        return value.item()
    return value


def _call(session: _Link, command: str, args: Mapping[str, Any]) -> Any:
    """Run one registry command and decode its answer (tables included)."""
    payload = {_camel(k): _encode(v) for k, v in args.items() if v is not None}
    try:
        text = session.invoke(command, _json.dumps(payload))
    except _native.CommandError as e:
        kind, message = e.args
        raise from_kind(kind, message) from None
    return tables_in(_json.loads(text))


def _settings(given: Mapping[str, Any] | None, extra: Mapping[str, Any]) -> dict[str, Any] | None:
    """Settings from a dict and/or keyword arguments; None when neither."""
    merged = {**(given or {}), **extra}
    return merged or None


# --- study edits ----------------------------------------------------------


def outage(element: str) -> dict[str, Any]:
    """A study edit that takes a branch or generator out of service, by its
    shared id such as ``"line:4:5:1"`` or ``"generator:2:1"``."""
    return {"kind": "status", "element": element, "online": False}


def restore(element: str) -> dict[str, Any]:
    """A study edit that puts a branch or generator back in service."""
    return {"kind": "status", "element": element, "online": True}


def generation(element: str, mw: float) -> dict[str, Any]:
    """A study edit that sets a generator's active power output, in MW."""
    return {"kind": "generation", "element": element, "mw": float(mw)}


def bus_load(bus: int, mw: float, mvar: float) -> dict[str, Any]:
    """A study edit that sets a bus's total load, in MW and Mvar."""
    return {"kind": "load", "bus": int(bus), "mw": float(mw), "mvar": float(mvar)}


# --- cases ----------------------------------------------------------------


class RecordTable(Table):
    """One record type of a case, as the Records module shows it: one
    column per field, plus where each record came from.

    ``lines`` is the file line each record starts on (0 when not from the
    file) and ``elements`` the shared element id of each record, or None.
    """

    def __init__(self, raw: Mapping[str, Any]) -> None:
        names = [c["name"] for c in raw["columns"]]
        rows: Table = raw["rows"]  # columns: line, element, cells (one list per record)
        count = len(rows)
        cells = list(rows["cells"])
        super().__init__({n: [r[i] for r in cells] for i, n in enumerate(names)}, count=count)
        self.id: str = raw["id"]
        self.label: str = raw["label"]
        self.kinds: dict[str, str] = {c["name"]: c["kind"] for c in raw["columns"]}
        self.editable: list[str] = [c["name"] for c in raw["columns"] if c["editable"]]
        self.lines = rows["line"]
        self.elements: list[str | None] = [_plain(e) for e in rows["element"]]
        self.skipped: int = raw.get("skipped", 0)
        self.note: str | None = raw.get("note")

    def __repr__(self) -> str:
        return f"<RecordTable {self.id} ({self.label}): {len(self)} records, {len(self.columns)} fields>"


class Case:
    """An open case. Open one with :func:`open`.

    Each case has its own copy of Stannum's workspace, so cases never share
    results, edits or a lock. Methods that run long (solve, contingencies,
    sensitivities) let other Python threads run meanwhile, and stop on
    Ctrl+C.

    Use it in a ``with`` block, or call :meth:`close`, to free its memory
    early.
    """

    def __init__(self, path: _PathLike) -> None:
        self._session: _Link = _native.Session()
        self._summary: Answer = _call(self._session, "open_case", {"path": Path(path).resolve()})
        self._id: int = self._summary["id"]
        self._app: App | None = None

    @classmethod
    def _in_app(cls, app: App, summary: Answer) -> Case:
        """The case open in an attached app."""
        case = cls.__new__(cls)
        case._session = app._remote
        case._summary = summary
        case._id = summary["id"]
        case._app = app
        return case

    @property
    def app(self) -> App | None:
        """The app this case is open in, when it came from :func:`attach`."""
        return self._app

    # The basics

    @property
    def id(self) -> int:
        """The handle every command names this case by."""
        return self._id

    @property
    def path(self) -> Path:
        """The case file."""
        return Path(self._summary["path"])

    @property
    def name(self) -> str:
        return self._summary["name"]

    @property
    def summary(self) -> Answer:
        """What the case holds: format, element counts, attached files."""
        self._summary = self.call("case_summary")
        return self._summary

    def call(self, command: str, **args: Any) -> Any:
        """Run any registry command on this case. Arguments are keyword
        arguments in snake_case; the case is filled in when the command
        takes one. See :func:`commands` for the list."""
        takes_case = self._app._takes_case(command) if self._app else _takes_case(command)
        if takes_case:
            args.setdefault("case", self._id)
        return _call(self._session, command, args)

    def close(self) -> None:
        """Close the case. Calls after this raise :class:`CaseNotOpen`. In
        an attached app this closes the case in the window."""
        try:
            self.call("close_case")
        except (CaseNotOpen, NoCase):
            pass

    def __enter__(self) -> Case:
        return self

    def __exit__(self, *exc: object) -> None:
        # A case in the app stays open for the person at the window.
        if self._app is None:
            self.close()

    def __repr__(self) -> str:
        where = " in the app" if self._app else ""
        return f"<stannum.Case {self.name!r} ({self._summary.get('format')}), id {self._id}{where}>"

    # Power flow

    def solve(self, settings: Mapping[str, Any] | None = None, **options: Any) -> Answer:
        """Solve the power flow with the study's edits and return the
        report. ``buses``, ``generators`` and ``branches`` are
        :class:`Table` s.

        Options (all optional): ``model`` ("ac" or "dc"), ``method``
        ("auto", the default, "newton", "fixed_slope",
        "fast_decoupled_xb" or "fast_decoupled_bx"), ``start`` ("last",
        "flat" or "case"), ``tolerance_mva`` (2 by default),
        ``max_iterations`` (Newton, 10), ``decoupled_iterations`` (30),
        ``enforce_q_limits``, ``dc_model`` ("detailed" or "simplified"),
        ``adjust_dc_taps``, ``switch_shunts``, ``adjust_transformer_taps``,
        ``adjust_phase_shifters``. Setting one of the last four to False
        locks those controls where they are instead of moving them.
        ``area_interchange`` ("off", the default, "tie_lines" or
        "tie_lines_and_loads") holds each area's net interchange at its RAW
        ``PDES`` with the area slack generation.
        """
        return self.call("solve_power_flow", settings=_settings(settings, options))

    @property
    def edits(self) -> list[dict[str, Any]]:
        """The study's edits, which every later solve uses. Set it to a list
        built from :func:`outage`, :func:`restore`, :func:`generation` and
        :func:`bus_load`."""
        return [dict(e) for e in self.call("study_edits")]

    @edits.setter
    def edits(self, edits: Iterable[Mapping[str, Any]]) -> None:
        self.call("set_study_edits", edits=[dict(e) for e in edits])

    def result(self, name: str) -> Answer:
        """A result by name, as a full solve report: "base", "study",
        "contingency 3", "trip line:1:2:1"."""
        return self.call("result_report", name=name)

    def sensitivities(self, settings: Mapping[str, Any] | None = None, **options: Any) -> Answer:
        """Sensitivities at the solved operating point (solve first). Takes
        the same options as :meth:`solve`. Read a column's responses with
        :meth:`responses`."""
        return self.call("calculate_sensitivities", settings=_settings(settings, options))

    def responses(self, column: int, quantity: str, engineering: bool = True) -> Table:
        """One sensitivity column's responses at every bus, as a table.
        ``quantity`` is one of the quantities :meth:`sensitivities` lists."""
        answer = self.call("sensitivity_responses", column=column, quantity=quantity, engineering=engineering)
        table = Table.from_rows(answer["rows"]) if answer["rows"] else Table({})
        table.units = answer["units"]  # type: ignore[attr-defined]
        table.title = answer["title"]  # type: ignore[attr-defined]
        return table

    # Contingencies

    def contingencies(self, settings: Mapping[str, Any] | None = None, **options: Any) -> Answer:
        """Run N-1 contingencies and return the summary, with every ranked
        outage as a :class:`Table` under ``rows``.

        Options (all optional): ``elements`` (ids to take out; every
        branch and generator by default), ``branches``, ``generators``,
        ``method`` ("screen" or "ac"), ``ac_count``, ``limit_pct``,
        ``solve`` (settings as for :meth:`solve`).
        """
        summary = self.call("run_contingencies", settings=_settings(settings, options))
        summary["rows"] = self.contingency_rows()
        return summary

    def contingency_rows(self, offset: int = 0, limit: int | None = None) -> Table:
        """The last run's ranked outages, as a table."""
        pages: list[Table] = []
        got_rows = 0
        page = 2000
        while limit is None or got_rows < limit:
            want = page if limit is None else min(page, limit - got_rows)
            got: Table = self.call("contingency_rows", offset=offset + got_rows, limit=want)
            pages.append(got)
            got_rows += len(got)
            if len(got) < want:
                break
        if len(pages) == 1:
            return pages[0]
        return Table({c: [_plain(v) for p in pages for v in p[c]] for c in pages[0].columns}, count=got_rows)

    def contingency(self, rank: int) -> Answer:
        """One outage's flow and voltage changes, by rank (from 1)."""
        return self.call("contingency_detail", rank=rank)

    def trip(self, element: str, limit_pct: float = 100.0) -> Answer:
        """Take one element out of the last AC solve, solve again and
        compare."""
        return self.call("trip_and_compare", element=element, limit_pct=limit_pct)

    # Records and elements

    def records(self, section: str | None = None) -> Any:
        """Without a section, the case's record types and their counts.
        With one (such as "BUS", or "bus" for MATPOWER), every record of
        that type as a :class:`RecordTable`."""
        if section is None:
            return self.call("case_records")
        raw = _call(self._session, "record_table", {"case": self._id, "section": section})
        return RecordTable(raw)

    def edit_records(self, edits: Iterable[Mapping[str, Any]]) -> Answer:
        """Write cell edits into the case and reopen it in place. Each edit
        is a dict of ``section``, ``row`` (position in the table),
        ``column`` (position or field name) and ``value``. Returns the
        case's new summary."""
        tables: dict[str, RecordTable] = {}
        out = []
        for edit in edits:
            section = edit["section"]
            column = edit["column"]
            if isinstance(column, str):
                if section not in tables:
                    tables[section] = self.records(section)
                names = tables[section].columns
                if column not in names:
                    raise NotFound(f"{section} has no field called {column}.")
                column = names.index(column)
            out.append({"section": section, "row": int(edit["row"]), "column": int(column), "value": str(edit["value"])})
        self._summary = self.call("apply_record_edits", edits=out)
        return self._summary

    def element(self, element: str) -> Answer:
        """One element's data and limits from the file, by shared id such as
        ``"bus:5"`` or ``"line:4:5:1"``."""
        return self.call("element_detail", element=element)

    # Topology

    def topology(self) -> Answer:
        """Islands, bridges and articulation buses, with the graph, in the
        compact form the app reads (see ``topology_wire``)."""
        return self.call("topology_snapshot")

    def shortest_path(self, from_bus: int, to_bus: int) -> Answer | None:
        """The shortest path between two buses, or None."""
        return self.call("topology_shortest_path", **{"from": from_bus, "to": to_bus})

    # Files

    def export_matpower(self, path: _PathLike, solved: bool = True) -> None:
        """Write the case as a MATPOWER file, solved or as filed."""
        self.call("export_matpower", path=Path(path).resolve(), solved=solved)

    def export_raw(self, path: _PathLike, revision: int = 35) -> Answer:
        """Write the case, with its record and study edits, as a complete
        PSS/E RAW file of revision 33 or 35. ``notes`` in the answer lists
        anything the file couldn't keep."""
        return self.call("export_raw", path=Path(path).resolve(), revision=revision)

    def save_as(self, path: _PathLike) -> Answer:
        """Save the case, with its record edits, to a new file."""
        self._summary = self.call("save_case_as", path=Path(path).resolve())
        return self._summary

    def load_dc_grids(self, path: _PathLike) -> Answer:
        """Attach a Stannum DC grid file to the case."""
        return self.call("load_dc_grids", path=Path(path).resolve())

    def load_coordinates(self, path: _PathLike) -> Answer:
        """Attach a bus coordinates CSV (bus, latitude, longitude, optional
        substation)."""
        return self.call("load_coordinates", path=Path(path).resolve())

    def bus_coordinates(self) -> Answer:
        """Where every bus goes on the map, as columns in bus order."""
        return self.call("bus_coordinates")

    def compare_working(self) -> Diff:
        """Compare the case's file with the case as it is now (RAW only)."""
        return Diff(self._session, self.call("diff_compare_working"))


# --- comparing files ------------------------------------------------------


class Diff:
    """A record-by-record comparison of two RAW files. :attr:`summary`
    has the counts per record type; :meth:`records` and :meth:`detail`
    read the records themselves."""

    def __init__(self, session: _Link, summary: Answer) -> None:
        self._session = session
        self.summary = summary

    @property
    def differing(self) -> int:
        """Records added, removed or changed, across every record type."""
        return self.summary["differing"]

    def records(self, section: str) -> Table:
        """One record type's records and how each differs."""
        rows = _call(self._session, "diff_section_records", {"section": section})
        return Table.from_rows(rows) if rows else Table({})

    def detail(self, section: str, index: int) -> Answer | None:
        """One record, field by field."""
        return _call(self._session, "diff_record_detail", {"section": section, "index": index})

    def __repr__(self) -> str:
        return f"<stannum.Diff {self.differing} differing records>"


def diff(baseline: _PathLike | Case, comparison: _PathLike | Case) -> Diff:
    """Compare two RAW files record by record."""
    a = baseline.path if isinstance(baseline, Case) else Path(baseline).resolve()
    b = comparison.path if isinstance(comparison, Case) else Path(comparison).resolve()
    session = _native.Session()
    return Diff(session, _call(session, "diff_compare", {"baseline": a, "comparison": b}))


# --- the running app ------------------------------------------------------


class App:
    """The running Stannum app, from :func:`attach`.

    Commands run on the app's own workspace, so the window shows what the
    script does as it goes: a case it opens, edits it makes, results it
    solves. The ``ui`` methods move the window itself.

    Use it in a ``with`` block, or call :meth:`detach`, to let go of the
    app; the app and its case stay open.
    """

    def __init__(self, remote: _attach.Remote) -> None:
        self._remote = remote
        self._commands: list[Answer] | None = None
        info = remote.info
        if info.get("api_version") != API_VERSION:
            remote.close()
            raise Unsupported(
                f"The app runs command set {info.get('api_version')} and this package "
                f"{API_VERSION}. Use the stannum package from the app's release ({info.get('version')})."
            )

    @property
    def version(self) -> str:
        """The app's version."""
        return self._remote.info["version"]

    @property
    def pid(self) -> int:
        """The app's process id."""
        return self._remote.info["pid"]

    # Cases

    @property
    def case(self) -> Case | None:
        """The case open in the window, or None."""
        summary = self.call("case_summary")
        return Case._in_app(self, summary) if summary else None

    def open(self, path: _PathLike) -> Case:  # noqa: A003 - mirrors stannum.open
        """Open a case file in the window, in place of the one there."""
        summary = self.call("open_case", path=Path(path).resolve())
        return Case._in_app(self, summary)

    def call(self, command: str, **args: Any) -> Any:
        """Run any registry command on the app's workspace, or a ``ui.*``
        command in the window. Arguments are keyword arguments in
        snake_case."""
        return _call(self._remote, command, args)

    def commands(self) -> list[Answer]:
        """The Ctrl+K commands the window offers now: ``id`` and ``title``.
        Run one with :meth:`run`."""
        return self.call("ui.commands")

    def _registry(self) -> list[Answer]:
        if self._commands is None:
            self._commands = self.call("list_commands")["commands"]
        return self._commands

    def _takes_case(self, command: str) -> bool:
        return any(c["name"] == command and c["takes_case"] for c in self._registry())

    # The window

    @property
    def state(self) -> Answer:
        """What the window shows: ``module``, ``split``, ``selection``,
        ``case``, ``solved``, ``shown`` (the result on screen) and ``theme``."""
        return self.call("ui.state")

    def run(self, command: str) -> Answer:
        """Run a Ctrl+K command by id, such as ``"theme"`` or
        ``"go-diagram"``; :meth:`commands` lists them."""
        return self.call("ui.run", id=command)

    def go(self, module: str) -> Answer:
        """Switch the window to a module: ``"diagram"``, ``"power"``,
        ``"records"`` or ``"diff"``."""
        return self.call("ui.go", module=module)

    def show(self, element: str) -> Answer:
        """Select an element in every module, by shared id such as
        ``"bus:5"`` or ``"line:4:5:1"``."""
        return self.call("ui.show", element=element)

    def clear_selection(self) -> Answer:
        """Clear the selection in every module."""
        return self.call("ui.clear_selection")

    def draw_around(self, element: str) -> Answer:
        """Draw a one-line a few hops around an element, in the Diagram."""
        return self.call("ui.draw_around", element=element)

    def say(self, message: str, level: str = "info") -> None:
        """Write a line in the app's log, from "script". ``level`` is
        "info", "warn" or "error"."""
        self.call("ui.say", message=message, level=level)

    # Letting go

    def detach(self) -> None:
        """Let go of the app. The app and its case stay open."""
        self._remote.close()

    def __enter__(self) -> App:
        return self

    def __exit__(self, *exc: object) -> None:
        self.detach()

    def __repr__(self) -> str:
        return f"<stannum.App {self.version}, process {self.pid}>"


def attach(pid: int | None = None, timeout: float = 5.0) -> App:
    """Attach to the running Stannum app: the newest one, or the one with
    process id ``pid``. Raises :class:`NotFound` when no app is running.

    The app must come from the same release as this package."""
    try:
        remote = _attach.connect(pid=pid, timeout=timeout)
    except _native.CommandError as e:
        kind, message = e.args
        raise from_kind(kind, message) from None
    return App(remote)


# --- everything else ------------------------------------------------------


def open(path: _PathLike) -> Case:  # noqa: A001 - stannum.open reads well
    """Open a case file (PSS/E RAW or MATPOWER)."""
    return Case(path)


def run_file(path: _PathLike, keep_going: bool = False, results: bool = False) -> Answer:
    """Run a run file (``.stnrun``) on its own workspace and return the
    report. Every line is checked before the first runs. ``keep_going``
    runs the remaining lines after one fails; ``results`` keeps each
    command's answer in the report."""
    return _call(
        _native.Session(),
        "run_file",
        {"path": Path(path).resolve(), "keep_going": keep_going, "results": results},
    )


_COMMANDS: list[Answer] | None = None


def commands() -> list[Answer]:
    """Every registry command: ``name``, ``doc``, ``takes_case`` and
    ``action``. :meth:`Case.call` runs any of them."""
    global _COMMANDS
    if _COMMANDS is None:
        _COMMANDS = tables_in(_json.loads(_native.command_list()))["commands"]
    return list(_COMMANDS)


def _takes_case(command: str) -> bool:
    return any(c["name"] == command and c["takes_case"] for c in commands())


def schemas() -> Answer:
    """Every command's arguments and result as JSON Schema (2020-12)."""
    return _call(_native.Session(), "command_schemas", {})


def log(since: int = 0) -> Table:
    """The core's log lines numbered above ``since``: ``seq``, ``time``
    (ms since the epoch), ``level``, ``source`` and ``message``."""
    rows = _call(_native.Session(), "log_entries", {"since": since})
    fields: Sequence[str] = ("seq", "time", "level", "source", "message")
    return Table.from_rows(rows, fields)

