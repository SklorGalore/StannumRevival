"""Types for the native half of the package (see src/lib.rs)."""

API_VERSION: int
__version__: str

class CommandError(Exception):
    """args are (kind, message)."""

class Session:
    """One workspace of Stannum's core."""

    def __init__(self) -> None: ...
    def invoke(self, command: str, args: str = "{}") -> str: ...

def command_list() -> str: ...
