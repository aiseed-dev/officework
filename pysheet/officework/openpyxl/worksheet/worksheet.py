"""`openpyxl.worksheet.worksheet` と同じ名前。`isinstance(ws, Worksheet)` の形を受けます。"""
from ...sheet import Sheet as Worksheet  # noqa: F401

__all__ = ["Worksheet"]
