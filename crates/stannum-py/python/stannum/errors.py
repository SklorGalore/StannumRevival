"""The errors Stannum raises.

Every command failure is a :class:`StannumError`. Its subclass says what
went wrong in a form a script can catch, one per kind in Stannum's command
registry (``crates/stannum-core/src/error.rs``); ``kind`` holds the same
name as text, and ``message`` the sentence the app would show.

.. code-block:: python

    try:
        case.solve()
    except stannum.NotReady:
        ...
"""

from __future__ import annotations


class StannumError(Exception):
    """A Stannum command failed."""

    #: The error kind as the registry names it, such as ``"no_case"``.
    kind: str = "failed"

    def __init__(self, message: str, kind: str | None = None) -> None:
        super().__init__(message)
        self.message = message
        if kind is not None:
            self.kind = kind


class UnknownCommand(StannumError):
    """No command has this name."""

    kind = "unknown_command"


class InvalidArguments(StannumError, ValueError):
    """The arguments are missing, misspelled or the wrong type, or a value
    isn't one the command takes."""

    kind = "invalid_arguments"


class NoCase(StannumError):
    """The command works on a case and none is open."""

    kind = "no_case"


class CaseNotOpen(StannumError):
    """The case was closed (or replaced) before the command reached it."""

    kind = "case_not_open"


class NotReady(StannumError):
    """Another step has to come first, such as solving the case before
    asking for its results."""

    kind = "not_ready"


class NotFound(StannumError, LookupError):
    """The element, record type, result or row asked for isn't there."""

    kind = "not_found"


class Unsupported(StannumError):
    """The command can't work on this case's format or revision."""

    kind = "unsupported"


class CaseChanged(StannumError):
    """The case changed while the work ran, so its results were set aside.
    Running it again works."""

    kind = "case_changed"


class Cancelled(StannumError):
    """The work was stopped on request."""

    kind = "cancelled"


class FileError(StannumError, OSError):
    """A file couldn't be read or written."""

    kind = "file"


class Failed(StannumError):
    """The work itself failed: a file didn't parse, a solve couldn't start,
    an edit was refused. The message says why."""

    kind = "failed"


class InternalError(StannumError):
    """Something inside Stannum went wrong. Please report it."""

    kind = "internal"


#: Each registry error kind and the exception raised for it.
BY_KIND: dict[str, type[StannumError]] = {
    cls.kind: cls
    for cls in (
        UnknownCommand,
        InvalidArguments,
        NoCase,
        CaseNotOpen,
        NotReady,
        NotFound,
        Unsupported,
        CaseChanged,
        Cancelled,
        FileError,
        Failed,
        InternalError,
    )
}


def from_kind(kind: str, message: str) -> StannumError:
    """The exception for an error kind; unknown kinds read as failures."""
    cls = BY_KIND.get(kind, StannumError)
    return cls(message, kind)


# Tracebacks name them as stannum.NotFound, where scripts import them from.
for _cls in (StannumError, *BY_KIND.values()):
    _cls.__module__ = "stannum"
del _cls
