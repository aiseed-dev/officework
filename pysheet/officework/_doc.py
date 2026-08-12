# -*- coding: utf-8 -*-
"""officework.doc — docx のエンジン(Rust)+ python-docx 互換層。

`officework.sheet` と同じ論法で docx を扱う。**原本を正として、変えた所だけ
書き戻す** ので、様式・ヘッダー・図形・変更履歴が壊れない。

    from officework import doc

    d = doc.Doc.open("報告書.docx")
    print(d.unsupported)          # 読めなかった物(空なら取りこぼしなし)
    d[3].text = "差し替え"
    d.replace("旧社名", "新社名")
    d.tables[0].cell(1, 2).text = "125000"   # python-docx の口も通る
    d.save("out.docx")

中身は Rust で、`officework._sheet` が組む1つの拡張の中に副モジュールとして
入っている — maturin が wheel に入れられる拡張は1つなので、`officework.sheet`
と `officework.doc` を**同じ .so に同居させる**ためにこうしてある(利用者に
2つ入れさせない)。この階はそれを包む純 Python の互換層で、python-docx の
口(cell / row_cells / clear / iter_inner_content 等)を足す
(台帳: docs/pysheet-gokan.ja.md)。エンジンには手を入れない。
"""

from . import _sheet as _engine

_doc = _engine.doc


class _Font(str):
    """Run.font の両対応。

    うちの口では font は**書体名の文字列**(`r.font == "MS明朝"`)。
    python-docx は font という物の下に置く(`r.font.name` / `r.font.size` /
    `r.font.bold`)。str の子にして両方を通す。
    """

    def __new__(cls, run):
        self = str.__new__(cls, run.font or "")
        self._run = run
        return self

    @property
    def name(self):
        return str(self) or None

    @property
    def size(self):
        return self._run.size_pt

    @property
    def bold(self):
        return self._run.bold

    @property
    def italic(self):
        return self._run.italic

    @property
    def underline(self):
        return self._run.underline

    @property
    def color(self):
        return self._run.color


class Run:
    """書式のまとまり。**写しであって handle ではない**(読むだけ)。
    字を替えるのは段落の `text` か `replace` から。
    (run 単位の書き — python-docx の add_text / clear — はエンジンに
    run の書き口が無いので、台帳の「足す」で待っている)"""

    __slots__ = ("_r",)

    def __init__(self, raw):
        self._r = raw

    @property
    def text(self):
        return self._r.text

    @property
    def size_pt(self):
        return self._r.size_pt

    @property
    def font(self):
        return _Font(self._r)

    @property
    def bold(self):
        return self._r.bold

    @property
    def italic(self):
        return self._r.italic

    @property
    def underline(self):
        return self._r.underline

    @property
    def color(self):
        return self._r.color

    def __repr__(self):
        return repr(self._r)


class Paragraph:
    """1つの段落。本文にも表のセルの中にもある。"""

    __slots__ = ("_p",)

    def __init__(self, raw):
        self._p = raw

    @property
    def text(self):
        return self._p.text

    @text.setter
    def text(self, value):
        self._p.text = value

    def replace(self, old, new):
        return self._p.replace(old, new)

    @property
    def runs(self):
        return [Run(r) for r in self._p.runs]

    @property
    def style(self):
        return self._p.style

    @property
    def align(self):
        return self._p.align

    @property
    def alignment(self):
        # python-docx の名前。中身は align と同じ字
        return self._p.align

    @property
    def in_table(self):
        return self._p.in_table

    # ── python-docx の口(互換層)───────────────────────────────

    def clear(self):
        """字を消す。段落の性質(見出し・寄せ)と先頭 run の書式は残る
        (python-docx の定義と同じ)。返りは自分。"""
        self._p.text = ""
        return self

    def iter_inner_content(self):
        """段落の中身を順に。いまは run だけ(リンクの読みはエンジンの
        「足す」待ち — 台帳の hyperlinks)。"""
        for r in self.runs:
            yield r

    def __repr__(self):
        return repr(self._p)


class Cell:
    """表のセル。中には段落が並んでいる。"""

    __slots__ = ("_c",)

    def __init__(self, raw):
        self._c = raw

    @property
    def text(self):
        return self._c.text

    @text.setter
    def text(self, value):
        self._c.text = value

    @property
    def paragraphs(self):
        return [Paragraph(p) for p in self._c.paragraphs]

    def __repr__(self):
        return repr(self._c)


class Row:
    """表の1行。"""

    __slots__ = ("_row",)

    def __init__(self, raw):
        self._row = raw

    def __len__(self):
        return len(self._row)

    def __getitem__(self, i):
        return Cell(self._row[i])

    @property
    def cells(self):
        return [Cell(c) for c in self._row.cells]

    def __repr__(self):
        return repr(self._row)


class _Column:
    """表の1列。python-docx の columns[j].cells の形だけを持つ。"""

    __slots__ = ("_t", "_col")

    def __init__(self, table, col):
        self._t = table
        self._col = col

    @property
    def cells(self):
        return self._t.column_cells(self._col)

    def __repr__(self):
        return "<officework.doc 列 {}>".format(self._col)


class Table:
    """表。`t[行][列]` でセルに届く。"""

    __slots__ = ("_t",)

    def __init__(self, raw):
        self._t = raw

    def __len__(self):
        return len(self._t)

    def __getitem__(self, i):
        return Row(self._t[i])

    @property
    def rows(self):
        return [Row(r) for r in self._t.rows]

    @property
    def shape(self):
        return self._t.shape

    def values(self):
        return self._t.values()

    # ── python-docx の口(互換層)───────────────────────────────

    def add_row(self):
        """行を1つ足す(末尾)。明細行の継ぎ足し。返りは新しい行。"""
        return Row(self._t.add_row())

    def add_column(self, width=None):
        """列を1つ足す(右端)。width は python-docx と同じ EMU
        (docx.shared.Mm(25) 等がそのまま通る)。省略なら等分のまま。
        返りは新しい列。"""
        width_mm = None if width is None else width / 36000
        self._t.add_column(width_mm)
        return self.columns[-1]

    def cell(self, row_idx, col_idx):
        return Cell(self._t[row_idx][col_idx])

    def row_cells(self, row_idx):
        return Row(self._t[row_idx]).cells

    def column_cells(self, col_idx):
        # 結合のある帳票では行によって列数が違う。無い行は飛ばす
        # (python-docx は長方形しか持てないので、この場合の定義が向こうに無い)
        out = []
        for row in self._t.rows:
            if col_idx < len(row):
                out.append(Cell(row[col_idx]))
        return out

    @property
    def columns(self):
        _, cols = self._t.shape
        return [_Column(self, j) for j in range(cols)]

    def __repr__(self):
        return repr(self._t)


class Doc:
    """docx の文書。エンジンの Doc を包み、python-docx の口を足す。"""

    def __init__(self):
        self._d = _doc.Doc()

    @staticmethod
    def open(path):
        d = Doc.__new__(Doc)
        d._d = _doc.Doc.open(path)
        return d

    def save(self, path):
        self._d.save(path)

    @property
    def unsupported(self):
        return self._d.unsupported

    @property
    def paragraphs(self):
        return [Paragraph(p) for p in self._d.paragraphs]

    @property
    def tables(self):
        return [Table(t) for t in self._d.tables]

    def __getitem__(self, i):
        return Paragraph(self._d[i])

    def __len__(self):
        return len(self._d)

    @property
    def text(self):
        return self._d.text

    @property
    def header(self):
        return self._d.header

    @property
    def footer(self):
        return self._d.footer

    def find(self, needle):
        return [Paragraph(p) for p in self._d.find(needle)]

    def replace(self, old, new):
        return self._d.replace(old, new)

    def add_paragraph(self, text=""):
        return Paragraph(self._d.add_paragraph(text))

    def add_table(self, rows, cols, style=None):
        """表を新しく組む(明細の帳票づくり)。各セルは空の段落を1つ持つ。

        style はまだ持てない(台帳の「足す(書式)」)— 黙って無視しない。
        """
        if style is not None:
            raise NotImplementedError(
                "表のスタイルはまだ持てない(台帳: docs/pysheet-gokan.ja.md の「足す(書式)」)"
            )
        return Table(self._d.add_table(rows, cols))

    def __getattr__(self, name):
        # エンジンに後から生えた口は、包み直しを待たずにそのまま通す
        if name.startswith("_"):  # 自分の畑(_d 等)で再帰しない
            raise AttributeError(name)
        return getattr(self._d, name)

    def __repr__(self):
        return repr(self._d)


__all__ = ["Doc", "Paragraph", "Run", "Table", "Row", "Cell"]
