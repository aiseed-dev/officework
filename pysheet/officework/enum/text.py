# -*- coding: utf-8 -*-
"""段落の揃えなど。本家の `docx.enum.text` と同じ名前です。"""

from ._moji import Hyou, Ne

WD_ALIGN_PARAGRAPH = Hyou(
    "WD_ALIGN_PARAGRAPH",
    LEFT=Ne("left", 0),
    CENTER=Ne("center", 1),
    RIGHT=Ne("right", 2),
    JUSTIFY=Ne("justify", 3),
    DISTRIBUTE=Ne("distribute", 4),
)

# 本家は同じ物に2つの名前を持ちます
WD_PARAGRAPH_ALIGNMENT = WD_ALIGN_PARAGRAPH

WD_BREAK = Hyou("WD_BREAK", LINE=Ne("line", 6), PAGE=Ne("page", 7))
WD_BREAK_TYPE = WD_BREAK

WD_UNDERLINE = Hyou(
    "WD_UNDERLINE",
    NONE=Ne("none", 0),
    SINGLE=Ne("single", 1),
    DOUBLE=Ne("double", 3),
)

WD_LINE_SPACING = Hyou(
    "WD_LINE_SPACING",
    SINGLE=Ne("single", 0),
    ONE_POINT_FIVE=Ne("one_point_five", 1),
    DOUBLE=Ne("double", 2),
    AT_LEAST=Ne("at_least", 3),
    EXACTLY=Ne("exactly", 4),
    MULTIPLE=Ne("multiple", 5),
)

__all__ = [
    "WD_ALIGN_PARAGRAPH", "WD_PARAGRAPH_ALIGNMENT", "WD_BREAK",
    "WD_BREAK_TYPE", "WD_UNDERLINE", "WD_LINE_SPACING",
]
