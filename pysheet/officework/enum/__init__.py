# -*- coding: utf-8 -*-
"""揃えや向きの決まった値。**python-docx の `docx.enum` と同じ置き場**です。

    from officework.enum.text import WD_ALIGN_PARAGRAPH
    p.alignment = WD_ALIGN_PARAGRAPH.CENTER

こちらは揃えを `"center"` のような字で持ちます。ここの値は**字としても
数としても使える**ので、本家の書き方をそのまま持ってこられます
(2026-09-01。本家の見本が `d.enum.text.WD_ALIGN_PARAGRAPH` で止まりました)。
"""

from . import section, table, text  # noqa: F401

__all__ = ["section", "table", "text"]
