# -*- coding: utf-8 -*-
"""表の揃え。本家の `docx.enum.table` と同じ名前です。"""

from ._moji import Hyou, Ne

WD_TABLE_ALIGNMENT = Hyou(
    "WD_TABLE_ALIGNMENT",
    LEFT=Ne("left", 0),
    CENTER=Ne("center", 1),
    RIGHT=Ne("right", 2),
)

WD_ALIGN_VERTICAL = Hyou(
    "WD_ALIGN_VERTICAL",
    TOP=Ne("top", 0),
    CENTER=Ne("center", 1),
    BOTTOM=Ne("bottom", 3),
)
WD_CELL_VERTICAL_ALIGNMENT = WD_ALIGN_VERTICAL

__all__ = ["WD_TABLE_ALIGNMENT", "WD_ALIGN_VERTICAL", "WD_CELL_VERTICAL_ALIGNMENT"]
