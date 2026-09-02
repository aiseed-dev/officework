# -*- coding: utf-8 -*-
"""長さと色。**python-docx の `docx.shared` と同じ場所・同じ名前**です。

    from officework.shared import Pt, Cm, Inches, RGBColor

本家の書き方をそのまま持ってくるための口です。中身は
`officework.doc` にあるものと同じで、ここは別名を置いているだけです
(2026-09-01。本家の見本が `d.shared.Pt` で止まりました)。

長さは EMU の整数で、`.pt` `.mm` `.cm` `.inches` `.twips` `.emu` が読めます。
"""

from ._doc import Cm, Emu, Inches, Length, Mm, Pt, RGBColor, Twips

__all__ = ["Cm", "Emu", "Inches", "Length", "Mm", "Pt", "RGBColor", "Twips"]
