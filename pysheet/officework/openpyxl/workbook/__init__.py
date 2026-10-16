"""`openpyxl.workbook` と同じ名前。`from openpyxl import workbook` の後に
`workbook.Workbook()` と書く形を受けます。"""
from ...sheet import Workbook  # noqa: F401
from . import defined_name  # noqa: F401

__all__ = ["Workbook", "defined_name"]
