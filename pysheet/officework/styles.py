# -*- coding: utf-8 -*-
"""セルの見た目。**openpyxl の `openpyxl.styles` と同じ場所・同じ名前**です。

    from officework.styles import Font, PatternFill, Border, Side, Alignment

中身は `officework.sheet` にあるものと同じで、ここは別名を置いている
だけです(2026-09-01。本家の見本が `openpyxl.styles` から取ります)。
"""

from .sheet import (  # noqa: F401
    Alignment, Border, Color, Font, GradientFill, PatternFill, Protection, Side,
)

__all__ = [
    "Alignment", "Border", "Color", "Font", "GradientFill",
    "PatternFill", "Protection", "Side",
]
