"""Stannum's analysis core from Python.

Open cases, solve power flow, run contingencies, read records and compare
RAW files without the app, in a script, a notebook or CI. Every call goes
through the same command registry the app uses, so a script and the app
always agree.

.. code-block:: python

    import stannum as sn

    case = sn.open("case9.m")
    report = case.solve(start="flat")
    print(report.converged, report.iterations)
    report.buses["vm"]            # NumPy array, one value per bus

    case.edits = [sn.outage("line:4:5:1")]
    worse = case.solve()

Each :class:`Case` is independent: open as many as you like and use them
from several threads. Ctrl+C stops a long solve or contingency run.
"""

from __future__ import annotations

import json as _json
import os
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path
from typing import Any, Union

from . import _native
from ._table import Answer, Table, tables_in
from .errors import (
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

__version__: str = _native.__version__
#: Version of the command set this package was built with. It goes up when
#: a command is removed or changes in a way existing scripts would notice.
API_VERSION: int = _native.API_VERSION

PathLike = Union[str, "os.PathLike[str]"]

__all__ = [
    "API_VERSION",
    "Answer",
    "Case",
    "CaseChanged",
    "CaseNotOpen",
    "Cancelled",
    "Diff",
    "Failed",
    "FileError",
    "InternalError",
    "InvalidArguments",
    "NoCase",
    "NotFound",
    "NotReady",
    "RecordTable",
    "StannumError",
    "Table",
    "UnknownCommand",
    "Unsupported",
    "__version__",
    "bus_load",
    "commands",
    "diff",
    "generation",
    "log",
    "open",
    "outage",
    "restore",
    "run_file",
    "schemas",
]


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


def _call(session: _native.Session, command: str, args: Mapping[str, Any]) -> Any:
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
        rows = raw["rows"]
        super().__init__({n: [r["cells"][i] for r in rows] for i, n in enumerate(names)}, count=len(rows))
        self.id: str = raw["id"]
        self.label: str = raw["label"]
        self.kinds: dict[str, str] = {c["name"]: c["kind"] for c in raw["columns"]}
        self.editable: list[str] = [c["name"] for c in raw["columns"] if c["editable"]]
        self.lines = Table({"line": [r["line"] for r in rows]}, count=len(rows))["line"]
        self.elements: list[str | None] = [r.get("element") for r in rows]
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

    def __init__(self, path: PathLike) -> None:
        self._session = _native.Session()
        self._summary: Answer = _call(self._session, "open_case", {"path": Path(path).resolve()})
        self._id: int = self._summary["id"]

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
        if _takes_case(command):
            args.setdefault("case", self._id)
        return _call(self._session, command, args)

    def close(self) -> None:
        """Close the case. Calls after this raise :class:`CaseNotOpen`."""
        try:
            self.call("close_case")
        except (CaseNotOpen, NoCase):
            pass

    def __enter__(self) -> Case:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    def __repr__(self) -> str:
        return f"<stannum.Case {self.name!r} ({self._summary.get('format')}), id {self._id}>"

    # Power flow

    def solve(self, settings: Mapping[str, Any] | None = None, **options: Any) -> Answer:
        """Solve the power flow with the study's edits and return the
        report. ``buses``, ``generators`` and ``branches`` are
        :class:`Table` s.

        Options (all optional): ``model`` ("ac" or "dc"), ``start`` ("last",
        "flat" or "case"), ``tolerance_mva`` (2 by default),
        ``max_iterations``, ``enforce_q_limits``, ``dc_model``
        ("detailed" or "simplified"), ``adjust_dc_taps``, ``switch_shunts``,
        ``adjust_transformer_taps``.
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
        rows: list[dict[str, Any]] = []
        page = 2000
        while limit is None or len(rows) < limit:
            want = page if limit is None else min(page, limit - len(rows))
            got = self.call("contingency_rows", offset=offset + len(rows), limit=want)
            rows.extend(got)
            if len(got) < want:
                break
        return Table.from_rows(rows) if rows else Table({})

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

    def export_matpower(self, path: PathLike, solved: bool = True) -> None:
        """Write the case as a MATPOWER file, solved or as filed."""
        self.call("export_matpower", path=Path(path).resolve(), solved=solved)

    def save_as(self, path: PathLike) -> Answer:
        """Save the case, with its record edits, to a new file."""
        self._summary = self.call("save_case_as", path=Path(path).resolve())
        return self._summary

    def load_dc_grids(self, path: PathLike) -> Answer:
        """Attach a Stannum DC grid file to the case."""
        return self.call("load_dc_grids", path=Path(path).resolve())

    def load_coordinates(self, path: PathLike) -> Answer:
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

    def __init__(self, session: _native.Session, summary: Answer) -> None:
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


def diff(baseline: PathLike | Case, comparison: PathLike | Case) -> Diff:
    """Compare two RAW files record by record."""
    a = baseline.path if isinstance(baseline, Case) else Path(baseline).resolve()
    b = comparison.path if isinstance(comparison, Case) else Path(comparison).resolve()
    session = _native.Session()
    return Diff(session, _call(session, "diff_compare", {"baseline": a, "comparison": b}))


# --- everything else ------------------------------------------------------


def open(path: PathLike) -> Case:  # noqa: A001 - stannum.open reads well
    """Open a case file (PSS/E RAW or MATPOWER)."""
    return Case(path)


def run_file(path: PathLike, keep_going: bool = False, results: bool = False) -> Answer:
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

