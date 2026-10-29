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

:func:`attach` drives the running app instead: its case, study and results,
shown in the window as the script goes.

.. code-block:: python

    app = sn.attach()
    case = app.case               # the case on screen
    case.solve()                  # the window shows the results
    app.show("bus:5")             # select it in every module

What's public: the names in ``__all__`` and the documented methods and
attributes of the classes among them. Modules and names that start with an
underscore, and the classes' constructors (use :func:`open`, :func:`diff`
and :func:`attach`), are not, and can change in any release. See
"Versions and compatibility" in the docs for what stays the same across
1.x.
"""


from . import _native
from ._api import (
    API_VERSION,
    App,
    Case,
    Diff,
    RecordTable,
    attach,
    bus_load,
    commands,
    diff,
    generation,
    log,
    open,
    outage,
    restore,
    run_file,
    schemas,
)
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
)
from ._table import Answer, Table

__version__: str = _native.__version__

#: The public API. tests/test_public_api.py holds the same list, with every
#: public method and attribute, so a change to it is made on purpose.
__all__ = [
    "API_VERSION",
    "Answer",
    "App",
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
    "attach",
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

# help(), reprs and tracebacks name them as stannum.Case and so on, where
# scripts import them from.
for _name in __all__:
    _obj = globals()[_name]
    if getattr(_obj, "__module__", "").startswith("stannum."):
        _obj.__module__ = "stannum"
del _name, _obj
