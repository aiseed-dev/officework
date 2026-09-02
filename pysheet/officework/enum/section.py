# -*- coding: utf-8 -*-
"""紙の向きなど。本家の `docx.enum.section` と同じ名前です。"""

from ._moji import Hyou, Ne

WD_ORIENT = Hyou(
    "WD_ORIENT",
    PORTRAIT=Ne("portrait", 0),
    LANDSCAPE=Ne("landscape", 1),
)
WD_ORIENTATION = WD_ORIENT

WD_SECTION = Hyou(
    "WD_SECTION",
    CONTINUOUS=Ne("continuous", 0),
    NEW_PAGE=Ne("new_page", 2),
)
WD_SECTION_START = WD_SECTION

__all__ = ["WD_ORIENT", "WD_ORIENTATION", "WD_SECTION", "WD_SECTION_START"]
