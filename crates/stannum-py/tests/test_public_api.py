"""The stannum package's public API, frozen.

Every name scripts may rely on is listed here: the names in ``__all__``
and the public methods, properties and attributes of its classes. From 1.0
on these stay compatible across 1.x (see "Versions and compatibility" in
the docs, docs-site/src/content/docs/reference/versioning.md).

If this test fails, the public API changed. Adding a name is fine in a
minor release: add it here too. Removing or renaming one has to wait for
the next major release, after a deprecation.
"""

from __future__ import annotations

import inspect

import stannum as sn

FUNCTIONS = {
    "attach": "(pid=None, timeout=5.0)",
    "bus_load": "(bus, mw, mvar)",
    "commands": "()",
    "diff": "(baseline, comparison)",
    "generation": "(element, mw)",
    "log": "(since=0)",
    "open": "(path)",
    "outage": "(element)",
    "restore": "(element)",
    "run_file": "(path, keep_going=False, results=False)",
    "schemas": "()",
}

ERRORS = [
    "CaseChanged",
    "CaseNotOpen",
    "Cancelled",
    "Failed",
    "FileError",
    "InternalError",
    "InvalidArguments",
    "NoCase",
    "NotFound",
    "NotReady",
    "StannumError",
    "UnknownCommand",
    "Unsupported",
]

VALUES = ["API_VERSION", "__version__"]

#: Each public class's public members: methods with their parameters,
#: properties as "property".
CLASSES: dict[str, dict[str, str]] = {
    "Case": {
        "app": "property",
        "bus_coordinates": "(self)",
        "call": "(self, command, **args)",
        "close": "(self)",
        "compare_working": "(self)",
        "contingencies": "(self, settings=None, **options)",
        "contingency": "(self, rank)",
        "contingency_rows": "(self, offset=0, limit=None)",
        "edit_records": "(self, edits)",
        "edits": "property",
        "element": "(self, element)",
        "export_contingency_results": "(self, path)",
        "export_matpower": "(self, path, solved=True)",
        "export_raw": "(self, path, revision=35)",
        "id": "property",
        "load_coordinates": "(self, path)",
        "load_dc_grids": "(self, path)",
        "name": "property",
        "path": "property",
        "pv_curve": "(self, sink, source=None, settings=None, **options)",
        "qv_curve": "(self, buses, settings=None, **options)",
        "records": "(self, section=None)",
        "responses": "(self, column, quantity, engineering=True)",
        "result": "(self, name)",
        "save_as": "(self, path)",
        "sensitivities": "(self, settings=None, **options)",
        "shortest_path": "(self, from_bus, to_bus)",
        "solve": "(self, settings=None, **options)",
        "summary": "property",
        "topology": "(self)",
        "transfer_limits": "(self, source=None, sink=None, settings=None, **options)",
        "trip": "(self, element, limit_pct=100.0, rating='a', base_rating='a')",
    },
    "App": {
        "call": "(self, command, **args)",
        "case": "property",
        "clear_selection": "(self)",
        "commands": "(self)",
        "detach": "(self)",
        "draw_around": "(self, element)",
        "go": "(self, module)",
        "open": "(self, path)",
        "pid": "property",
        "run": "(self, command)",
        "say": "(self, message, level='info')",
        "show": "(self, element)",
        "state": "property",
        "version": "property",
    },
    "Diff": {
        "detail": "(self, section, index)",
        "differing": "property",
        "records": "(self, section)",
    },
    "Table": {
        "columns": "property",
        "rows": "(self)",
        "to_pandas": "(self)",
    },
    # Inherits Table's.
    "RecordTable": {},
    # A dict whose keys read as attributes too.
    "Answer": {},
}

#: Public attributes set on instances, which inspecting the class can't see.
INSTANCE_ATTRIBUTES = {
    "Diff": ["summary"],
    "RecordTable": ["editable", "elements", "id", "kinds", "label", "lines", "note", "skipped"],
    "StannumError": ["kind", "message"],
}

#: Methods the package defines but doesn't promise: alternate constructors.
NOT_PUBLIC = {"Table": {"from_rows"}}


def test_all_lists_exactly_the_public_names():
    expected = {*FUNCTIONS, *ERRORS, *VALUES, *CLASSES}
    assert set(sn.__all__) == expected


def test_nothing_else_is_public_at_the_top():
    # Modules and helpers the package uses inside shouldn't look public.
    public = {n for n in dir(sn) if not n.startswith("_")}
    assert public == {n for n in sn.__all__ if not n.startswith("_")}


def test_functions_keep_their_parameters():
    for name, params in FUNCTIONS.items():
        assert _params(getattr(sn, name)) == params, name


def _params(fn) -> str:
    """A signature without annotations, as it reads in the lists above."""
    sig = inspect.signature(fn)
    parts = []
    for p in sig.parameters.values():
        text = ("**" if p.kind is p.VAR_KEYWORD else "*" if p.kind is p.VAR_POSITIONAL else "") + p.name
        if p.default is not p.empty:
            text += f"={p.default!r}"
        parts.append(text)
    return f"({', '.join(parts)})"


def _members(cls: type) -> dict[str, str]:
    """A class's own public members, beyond what its library bases give."""
    bases = (dict, Exception, object)
    inherited = {n for b in bases for n in dir(b)}
    out = {}
    for name in dir(cls):
        if name.startswith("_") or name in inherited:
            continue
        attr = inspect.getattr_static(cls, name)
        if isinstance(attr, property):
            out[name] = "property"
        elif callable(attr):
            out[name] = _params(getattr(cls, name))
    return out


def test_classes_keep_their_members():
    for name, members in CLASSES.items():
        cls = getattr(sn, name)
        got = _members(cls)
        for skip in NOT_PUBLIC.get(name, ()):
            got.pop(skip, None)
        if name == "RecordTable":
            got = {k: v for k, v in got.items() if k not in CLASSES["Table"]}
        assert got == members, name


def test_record_table_is_a_table():
    assert issubclass(sn.RecordTable, sn.Table)
    assert issubclass(sn.Answer, dict)


def test_errors_are_stannum_errors_with_kinds():
    kinds = set()
    for name in ERRORS:
        cls = getattr(sn, name)
        assert issubclass(cls, sn.StannumError), name
        kinds.add(cls.kind)
    assert kinds == {
        "case_changed",
        "case_not_open",
        "cancelled",
        "failed",
        "file",
        "internal",
        "invalid_arguments",
        "no_case",
        "not_found",
        "not_ready",
        "unknown_command",
        "unsupported",
    }
    # Catchable as the built-in errors they resemble.
    assert issubclass(sn.InvalidArguments, ValueError)
    assert issubclass(sn.NotFound, LookupError)
    assert issubclass(sn.FileError, OSError)


def test_instance_attributes(tmp_path):
    case_file = tmp_path / "two.m"
    case_file.write_text(TWO_BUS)
    with sn.open(case_file) as case:
        table = case.records("bus")
        for attr in INSTANCE_ATTRIBUTES["RecordTable"]:
            assert hasattr(table, attr), attr
    error = sn.NotFound("gone")
    for attr in INSTANCE_ATTRIBUTES["StannumError"]:
        assert hasattr(error, attr), attr


def test_public_names_read_as_stannum():
    for name in [*CLASSES, *ERRORS, *FUNCTIONS]:
        assert getattr(sn, name).__module__ == "stannum", name


def test_values():
    assert isinstance(sn.API_VERSION, int)
    assert isinstance(sn.__version__, str)


TWO_BUS = """function mpc = two
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
  1 3 0 0 0 0 1 1 0 230 1 1.1 0.9;
  2 1 50 10 0 0 1 1 0 230 1 1.1 0.9;
];
mpc.gen = [
  1 0 0 100 -100 1 100 1 200 0;
];
mpc.branch = [
  1 2 0.01 0.1 0 0 0 0 0 0 1 -360 360;
];
"""
