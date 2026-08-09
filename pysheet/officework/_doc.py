# -*- coding: utf-8 -*-
"""officework.doc — docx のエンジン(Rust)。

`officework.sheet` と同じ論法で docx を扱う。**原本を正として、変えた所だけ
書き戻す** ので、様式・ヘッダー・図形・変更履歴が壊れない。

    from officework import doc

    d = doc.Doc.open("報告書.docx")
    print(d.unsupported)          # 読めなかった物(空なら取りこぼしなし)
    d[3].text = "差し替え"
    d.replace("旧社名", "新社名")
    d.save("out.docx")

**この階は名前だけを受け持つ。** 中身は Rust で、`officework._sheet` が組む
1つの拡張の中に副モジュールとして入っている — maturin が wheel に入れられる
拡張は1つなので、`officework.sheet` と `officework.doc` を**同じ .so に
同居させる**ためにこうしてある(利用者に2つ入れさせない)。
"""

from . import _sheet as _engine

_doc = _engine.doc

Doc = _doc.Doc
Paragraph = _doc.Paragraph
Run = _doc.Run
Table = _doc.Table
Row = _doc.Row
Cell = _doc.Cell

__all__ = ["Doc", "Paragraph", "Run", "Table", "Row", "Cell"]
