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
    `r.font.bold`)— 読みも書きも。str の子にして両方を通す。
    書き(`r.font.name = "明朝"`)は元の run(手)に効く。
    """

    def __new__(cls, run):
        self = str.__new__(cls, run.font or "")
        self._run = run
        return self

    @property
    def name(self):
        return str(self) or None

    @name.setter
    def name(self, v):
        self._run.font = v

    @property
    def size(self):
        return self._run.size_pt

    @size.setter
    def size(self, v):
        self._run.size_pt = float(v)

    @property
    def bold(self):
        return self._run.bold

    @bold.setter
    def bold(self, v):
        self._run.bold = bool(v)

    @property
    def italic(self):
        return self._run.italic

    @italic.setter
    def italic(self, v):
        self._run.italic = bool(v)

    @property
    def underline(self):
        return self._run.underline

    @underline.setter
    def underline(self, v):
        self._run.underline = bool(v) and v != "none"

    @property
    def color(self):
        return self._run.color

    @color.setter
    def color(self, v):
        self._run.color = v


class Run:
    """書式のまとまり。**位置で引き直す手**(python-docx の run と同じ使い方)。
    `r.bold = True` も `r.add_text("続き")` も効く。段落の text の代入や
    replace で run の並びが変わった後は、`runs` から引き直すこと。"""

    __slots__ = ("_r",)

    def __init__(self, raw):
        self._r = raw

    @property
    def text(self):
        return self._r.text

    @text.setter
    def text(self, v):
        self._r.text = v

    @property
    def size_pt(self):
        return self._r.size_pt

    @property
    def font(self):
        return _Font(self._r)

    @property
    def bold(self):
        return self._r.bold

    @bold.setter
    def bold(self, v):
        self._r.bold = bool(v)

    @property
    def italic(self):
        return self._r.italic

    @italic.setter
    def italic(self, v):
        self._r.italic = bool(v)

    @property
    def underline(self):
        return self._r.underline

    @underline.setter
    def underline(self, v):
        self._r.underline = bool(v) and v != "none"

    @property
    def strike(self):
        return self._r.strike

    @strike.setter
    def strike(self, v):
        self._r.strike = bool(v)

    @property
    def color(self):
        return self._r.color

    @color.setter
    def color(self, v):
        self._r.color = v

    def add_text(self, text):
        """字を後ろに継ぎ足す(書式はこの run のまま — 本家と同じ定義)。"""
        self._r.add_text(text)

    def clear(self):
        """字を消す(書式は残る)。返りは自分(本家と同じ)。"""
        self._r.clear()
        return self

    def __repr__(self):
        return repr(self._r)


def _align_word(v):
    # 寄せの受け皿: "center" / python-docx の WD_ALIGN_PARAGRAPH(.name)/ None
    if v is None:
        return None
    return str(getattr(v, "name", v)).lower()


class ParagraphFormat:
    """python-docx の paragraph_format の役。模型が持つ物だけ —
    alignment・line_spacing・page_break_before。余白(space_before /
    space_after)と字下げ(left_indent)は模型に無いので、読みは None・
    書きは正直に断る(黙って捨てない)。"""

    __slots__ = ("_p",)

    def __init__(self, raw):
        self._p = raw

    @property
    def alignment(self):
        return self._p.align

    @alignment.setter
    def alignment(self, v):
        self._p.align = _align_word(v) or "left"

    @property
    def line_spacing(self):
        return self._p.line_spacing

    @line_spacing.setter
    def line_spacing(self, v):
        self._p.line_spacing = float(v)

    @property
    def page_break_before(self):
        return self._p.page_break_before

    @page_break_before.setter
    def page_break_before(self, v):
        self._p.page_break_before = bool(v)

    @property
    def space_before(self):
        return None

    @space_before.setter
    def space_before(self, v):
        raise NotImplementedError("段落の前後の余白はまだ模型に無い(台帳)")

    @property
    def space_after(self):
        return None

    @space_after.setter
    def space_after(self, v):
        raise NotImplementedError("段落の前後の余白はまだ模型に無い(台帳)")

    @property
    def left_indent(self):
        return None

    @left_indent.setter
    def left_indent(self, v):
        raise NotImplementedError(
            "字下げは模型では段数(1段=全角2字)— python-docx の Length との"
            "対応はまだ決めていない(台帳)"
        )


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

    @style.setter
    def style(self, value):
        # python-docx のスタイルの物("Heading 1" を .name に持つ)も、
        # ただの文字("heading1")も受ける
        self._p.style = str(getattr(value, "name", value))

    @property
    def align(self):
        return self._p.align

    @align.setter
    def align(self, value):
        self._p.align = value

    @property
    def alignment(self):
        # python-docx の名前。中身は align と同じ字
        return self._p.align

    @alignment.setter
    def alignment(self, value):
        self._p.align = _align_word(value) or "left"

    @property
    def paragraph_format(self):
        return ParagraphFormat(self._p)

    @property
    def in_table(self):
        return self._p.in_table

    # ── python-docx の口(互換層)───────────────────────────────

    def clear(self):
        """字を消す。段落の性質(見出し・寄せ)と先頭 run の書式は残る
        (python-docx の定義と同じ)。返りは自分。"""
        self._p.text = ""
        return self

    def add_run(self, text="", style=None):
        """段落の末尾に run を継ぎ足す(python-docx と同じ口)。
        書式は末尾の run のものを継ぐ。style(文字スタイル)は
        スタイル定義を持たない主義と衝突するので、渡されたら正直に断る。"""
        if style is not None:
            raise NotImplementedError(
                "文字スタイルはスタイル定義を持たない主義と衝突(台帳 — 発注者判断待ち)"
            )
        return Run(self._p.add_run(text))

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

    @property
    def style(self):
        """表のスタイルの名前(styleId)。定義は持たない — 名前を運ぶだけ。"""
        return self._t.style

    @style.setter
    def style(self, v):
        if v is None:
            self._t.style = None
            return
        # python-docx のスタイルの物(.style_id)も、名前の文字も受ける。
        # 名前("Table Grid")は Word の流儀で styleId("TableGrid")に寄せる
        sid = getattr(v, "style_id", None)
        if sid is None:
            sid = str(getattr(v, "name", v)).replace(" ", "")
        self._t.style = sid

    @property
    def alignment(self):
        return self._t.alignment

    @alignment.setter
    def alignment(self, v):
        self._t.alignment = _align_word(v)

    @property
    def autofit(self):
        return self._t.autofit

    @autofit.setter
    def autofit(self, v):
        self._t.autofit = bool(v)

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

    def add_heading(self, text="", level=1):
        """見出しを足す(python-docx と同じ口)。level は 1〜3 —
        模型の見出しは3段まで。0(Title)は持たないので正直に断る。"""
        return Paragraph(self._d.add_heading(text, level))

    def add_page_break(self):
        """改ページを足す(python-docx と同じ口)。本家は「改ページの run」を
        足すが、うちは**段落の性質(page_break_before)**で持つ — 紙の上の
        意味は同じで、本家の paragraph_format.page_break_before でも読める。"""
        return Paragraph(self._d.add_page_break())

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
