"""`openpyxl.styles` と同じ名前。中身は `officework.styles` です。"""
from ..styles import (  # noqa: F401
    Alignment, Border, Color, Font, GradientFill, PatternFill, Protection, Side,
)

__all__ = [
    "Alignment", "Border", "Color", "Font", "GradientFill",
    "PatternFill", "Protection", "Side",
]
