"""Tables as NumPy columns."""

from __future__ import annotations

from collections.abc import Iterable, Iterator, Mapping, Sequence
from typing import Any

import numpy as np


def _column(values: Sequence[Any]) -> np.ndarray:
    """One column as the NumPy array that fits it best.

    Whole numbers become int64 and other numbers float64 (a missing value
    makes the column float with NaN in its place), true/false becomes bool,
    text becomes a string array, and anything else (mixed, nested) stays as
    objects.
    """
    present = [v for v in values if v is not None]
    if not present:
        return np.full(len(values), np.nan)
    if all(isinstance(v, bool) for v in present):
        if len(present) == len(values):
            return np.array(values, dtype=bool)
        return np.array(values, dtype=object)
    if all(isinstance(v, (int, float)) and not isinstance(v, bool) for v in present):
        if len(present) == len(values) and all(isinstance(v, int) for v in present):
            return np.array(values, dtype=np.int64)
        return np.array([np.nan if v is None else v for v in values], dtype=np.float64)
    if len(present) == len(values) and all(isinstance(v, str) for v in present):
        return np.array(values, dtype=str)
    out = np.empty(len(values), dtype=object)
    out[:] = list(values)
    return out


class Table(Mapping[str, np.ndarray]):
    """Rows of one kind, held as one NumPy array per field.

    ``table["vm"]`` is a column, ``table[3]`` a row as a dict, and
    ``table[mask]`` or ``table[2:5]`` a smaller table. ``len(table)`` is the
    number of rows. ``to_pandas()`` gives a DataFrame when pandas is
    installed.
    """

    def __init__(self, columns: Mapping[str, Any], count: int | None = None) -> None:
        self._columns: dict[str, np.ndarray] = {}
        for name, values in columns.items():
            if isinstance(values, np.ndarray):
                self._columns[name] = values
            else:
                self._columns[name] = _column(list(values))
        lengths = {len(c) for c in self._columns.values()}
        if count is None:
            count = lengths.pop() if lengths else 0
            if lengths:
                raise ValueError("Every column of a table must be the same length.")
        elif lengths - {count}:
            raise ValueError(f"Every column of a table must be {count} long.")
        self._count = count

    @classmethod
    def from_rows(cls, rows: Iterable[Mapping[str, Any]], fields: Sequence[str] | None = None) -> Table:
        """A table from row dicts. Fields are the first row's unless given."""
        rows = list(rows)
        if fields is None:
            fields = list(rows[0]) if rows else []
        return cls({f: [r.get(f) for r in rows] for f in fields}, count=len(rows))

    @property
    def columns(self) -> list[str]:
        """The field names, in order."""
        return list(self._columns)

    def __len__(self) -> int:
        return self._count

    def __iter__(self) -> Iterator[str]:
        return iter(self._columns)

    def __contains__(self, name: object) -> bool:
        return name in self._columns

    def __getitem__(self, key: Any) -> Any:
        if isinstance(key, str):
            return self._columns[key]
        if isinstance(key, (int, np.integer)):
            i = int(key)
            if i < 0:
                i += self._count
            if not 0 <= i < self._count:
                raise IndexError(f"Row {key} is outside a table of {self._count} rows.")
            return {name: _plain(col[i]) for name, col in self._columns.items()}
        picked = {name: col[key] for name, col in self._columns.items()}
        return Table(picked)

    def rows(self) -> list[dict[str, Any]]:
        """Every row as a dict of plain Python values."""
        return [self[i] for i in range(self._count)]

    def to_pandas(self) -> Any:
        """The table as a pandas DataFrame (pandas must be installed)."""
        try:
            import pandas as pd
        except ImportError as e:  # pragma: no cover - depends on the install
            raise ImportError("to_pandas() needs pandas: pip install pandas") from e
        return pd.DataFrame(self._columns)

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Table):
            return NotImplemented
        return self.columns == other.columns and all(
            np.array_equal(self[c], other[c]) for c in self.columns
        )

    __hash__ = None  # type: ignore[assignment]

    def __repr__(self) -> str:
        shown = ", ".join(self.columns[:8])
        more = f", … {len(self.columns) - 8} more" if len(self.columns) > 8 else ""
        return f"<Table {self._count} rows: {shown}{more}>"


def _plain(value: Any) -> Any:
    """A NumPy scalar as the Python value it stands for."""
    if isinstance(value, np.generic):
        value = value.item()
    if isinstance(value, float) and value != value:
        return None
    return value


def is_columns(value: Any) -> bool:
    """Whether a JSON object is a table sent as columns: ``count`` and at
    least one list, every list ``count`` long."""
    if not isinstance(value, dict) or not isinstance(value.get("count"), int):
        return False
    count = value["count"]
    others = [v for k, v in value.items() if k != "count"]
    return bool(others) and all(isinstance(v, list) and len(v) == count for v in others)


def tables_in(value: Any) -> Any:
    """The JSON answer with every column table turned into a :class:`Table`."""
    if is_columns(value):
        return Table({k: v for k, v in value.items() if k != "count"}, count=value["count"])
    if isinstance(value, dict):
        return Answer({k: tables_in(v) for k, v in value.items()})
    if isinstance(value, list):
        return [tables_in(v) for v in value]
    return value


class Answer(dict):
    """A command's answer: a dict whose keys also read as attributes
    (``report.converged`` is ``report["converged"]``)."""

    def __getattr__(self, name: str) -> Any:
        try:
            return self[name]
        except KeyError:
            raise AttributeError(name) from None

    def __dir__(self) -> list[str]:
        return [*super().__dir__(), *(k for k in self if isinstance(k, str))]
