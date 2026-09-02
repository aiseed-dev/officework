# -*- coding: utf-8 -*-
"""列の名前と番号。**openpyxl の `openpyxl.utils` と同じ場所・同じ名前**です。

    from officework.utils import get_column_letter, column_index_from_string
"""

from .sheet import column_index_from_string, get_column_letter  # noqa: F401

__all__ = ["get_column_letter", "column_index_from_string"]
