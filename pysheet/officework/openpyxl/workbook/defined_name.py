"""`openpyxl.workbook.defined_name` と同じ名前。"""
from ...sheet import DefinedName  # noqa: F401
from ...sheet import _DefinedNames as DefinedNameList  # noqa: F401

__all__ = ["DefinedName", "DefinedNameList"]
