# -*- coding: utf-8 -*-
"""officework.sheet — xlsx のエンジン(Rust)+ openpyxl 互換層。

    from officework import sheet

    b = sheet.Book.open("見積.xlsx")
    s = b["quote"]
    s["A30"] = "日本不燃株式会社"   # うちの口(値の直の読み書き)
    s.cell(row=30, column=3, value=125000)   # openpyxl の口も通る
    for row in s.iter_rows(values_only=True):
        ...
    b.save("out.xlsx")

中身は Rust(officework._sheet)。**原本を正として、変えた所だけ書き戻す**ので
罫線・結合・列幅・図形が壊れず、式は**その場で再計算される** — この2点が
openpyxl に無い上位分。この階は純 Python の互換層で、エンジンには手を入れない
(台帳: docs/pysheet-gokan.ja.md)。

openpyxl との違いをはっきり書いておく:

- `s["A1"]` は **値そのもの**を返す(openpyxl は Cell を返す)。Cell が
  欲しいときは `s.cell(row=1, column=1)`。ここはうちの口を正とする
- `Workbook.path` は**開いた元のファイルの径路**(無ければ None)。
  openpyxl の path("/xl/workbook.xml" という内部の定数)は真似しない
"""

import os as _os

from . import _sheet as _engine


def _col_letter(n):
    # 1 → A, 26 → Z, 27 → AA(1起点)
    s = ""
    while n > 0:
        n, r = divmod(n - 1, 26)
        s = chr(65 + r) + s
    return s


def _col_index(letters):
    # A → 1, AA → 27
    n = 0
    for ch in letters:
        n = n * 26 + (ord(ch.upper()) - 64)
    return n


def _coord(row, col):
    return "{}{}".format(_col_letter(col), row)


def _cell_rc(ref):
    """"B12" を (12, 2) に。$ は落とす(絶対参照でも同じ場所)"""
    t = ref.replace("$", "").strip()
    i = 0
    while i < len(t) and t[i].isalpha():
        i += 1
    if i == 0 or i == len(t):
        raise ValueError("セル参照として読めない: {!r}".format(ref))
    return int(t[i:]), _col_index(t[:i])


# ── 書式の入れ物(openpyxl の形。台帳「足す(書式)」2026-08-12)──────
#
# openpyxl の実物を渡されても動くように、**読みは属性名の一致だけ**に頼る
# (isinstance を見ない)。うちの入れ物は座標を持たない値 — セルに代入して
# 初めて効く、という openpyxl と同じ使い方。


class Color:
    """色。openpyxl の Color の役(rgb だけ持つ。"RRGGBB")。"""

    __slots__ = ("rgb",)

    def __init__(self, rgb=None):
        self.rgb = rgb

    def __repr__(self):
        return "<Color {}>".format(self.rgb)


def _rgb6(v):
    # 色 → "RRGGBB"。文字列 / Color / openpyxl の Color(aRGB 8桁)を受ける
    if v is None:
        return None
    rgb = getattr(v, "rgb", v)
    if not isinstance(rgb, str):
        return None
    return (rgb[-6:] if len(rgb) == 8 else rgb).upper()


class Side:
    """罫線の1辺。style は xlsx の線種("thin" "medium" "double" …)。"""

    __slots__ = ("style", "color")

    def __init__(self, style=None, color=None):
        self.style = style
        self.color = color


class Border:
    def __init__(self, left=None, right=None, top=None, bottom=None,
                 diagonal=None, **_rest):
        if getattr(diagonal, "style", None) is not None:
            raise NotImplementedError(
                "斜めの罫線はまだエンジンに無い(台帳: docs/pysheet-gokan.ja.md)"
            )
        self.left = left
        self.right = right
        self.top = top
        self.bottom = bottom


class Font:
    def __init__(self, name=None, size=None, bold=None, italic=None,
                 underline=None, strike=None, color=None, **_rest):
        self.name = name
        self.size = size
        self.bold = bold
        self.italic = italic
        self.underline = underline
        self.strike = strike
        self.color = color


class PatternFill:
    """塗りつぶし。効くのは solid だけ(模様はエンジンに無い — 正直に断る)。"""

    def __init__(self, patternType=None, fgColor=None, start_color=None,
                 end_color=None, fill_type=None, **_rest):
        self.patternType = patternType if patternType is not None else fill_type
        self.fgColor = fgColor if fgColor is not None else start_color


class GradientFill:
    """階調の塗り。openpyxl の GradientFill の形。

    `GradientFill(stop=("1B6E3C", "63BE7B"))` のように色を並べます。
    位置は等間隔に配ります。`degree` は角度(0 = 左から右)です。
    """

    def __init__(self, type="linear", degree=0, stop=(), **_rest):
        self.type = type
        self.degree = degree
        self.stop = list(stop)

    @property
    def _kumi(self):
        """(角度, [(位置, 色), …])。エンジンへ渡す形"""
        n = max(1, len(self.stop) - 1)
        return (float(self.degree),
                [(i / n, _rgb_of(c)) for i, c in enumerate(self.stop)])


class Alignment:
    def __init__(self, horizontal=None, vertical=None, wrap_text=None,
                 shrink_to_fit=None, text_rotation=0, indent=0, **_rest):
        self.horizontal = horizontal
        self.vertical = vertical
        self.wrap_text = wrap_text
        self.shrink_to_fit = shrink_to_fit
        self.text_rotation = text_rotation
        self.indent = indent


class _HFPart:
    """ヘッダー/フッターの1区分(openpyxl の HeaderFooterItem の役)。
    `.text` の読み書きが本体 — 書くと元の三分割に組み直して置く。"""

    __slots__ = ("_hf", "_which")

    def __init__(self, hf, which):
        self._hf = hf
        self._which = which  # 0=左 1=中 2=右

    @property
    def text(self):
        return self._hf._parts()[self._which]

    @text.setter
    def text(self, v):
        parts = list(self._hf._parts())
        parts[self._which] = "" if v is None else str(v)
        self._hf._set(parts)

    def __repr__(self):
        return "<HeaderFooterItem {!r}>".format(self.text)


class _HeaderFooter:
    """印刷のヘッダー(かフッター)。openpyxl と同じく left / center / right
    の三分割で触る。中身は xlsx の原文(&L 左 &C 中 &R 右)。

    **奇数・偶数・先頭頁の別は模型に無い**(1つだけ持つ)ので、
    evenHeader / firstHeader は正直に断る — 黙って同じ物を返さない。"""

    def __init__(self, sheet, footer, part="odd"):
        self._s = sheet
        self._footer = footer
        self._part = part  # "odd" / "even" / "first"

    def _attr(self):
        # エンジン側の畑の名前(print_header / print_footer_even など)
        base = "print_footer" if self._footer else "print_header"
        return base if self._part == "odd" else base + "_" + self._part

    def _raw(self):
        return getattr(self._s._s, self._attr()) or ""

    def _parts(self):
        # "&L左&C中&R右" → (左, 中, 右)。印より前の字は中(xlsx の慣わし)
        left = center = right = ""
        cur = 1
        i = 0
        raw = self._raw()
        while i < len(raw):
            if raw[i] == "&" and i + 1 < len(raw) and raw[i + 1] in "LCR":
                cur = {"L": 0, "C": 1, "R": 2}[raw[i + 1]]
                i += 2
                continue
            if cur == 0:
                left += raw[i]
            elif cur == 1:
                center += raw[i]
            else:
                right += raw[i]
            i += 1
        return (left, center, right)

    def _set(self, parts):
        out = ""
        for tag, v in zip("LCR", parts):
            if v:
                out += "&" + tag + v
        setattr(self._s._s, self._attr(), out or None)

    @property
    def left(self):
        return _HFPart(self, 0)

    @property
    def center(self):
        return _HFPart(self, 1)

    @property
    def right(self):
        return _HFPart(self, 2)

    @property
    def text(self):
        """原文のまま(&L…&C…&R…)。うちの口。"""
        return self._raw() or None

    @text.setter
    def text(self, v):
        setattr(self._s._s, self._attr(), v)

    def __repr__(self):
        return "<{}{} {!r}>".format(
            "" if self._part == "odd" else self._part,
            "Footer" if self._footer else "Header",
            self._raw())


class _Dim:
    """1本の行(列)の寸法。openpyxl の RowDimension / ColumnDimension の役。

    **持ち物ではなく窓** — width を書けばその場でシートに載る(openpyxl も
    同じで、後から一括で流し込む作りにはなっていない)。
    """

    __slots__ = ("_sheet", "_rows", "_key")

    def __init__(self, sheet, rows, key):
        self._sheet = sheet
        self._rows = rows
        self._key = key

    @property
    def width(self):
        """列の幅(字数)。行では openpyxl も持たないので None のまま。"""
        if self._rows:
            return None
        return self._sheet._s.col_width(self._key)

    @width.setter
    def width(self, value):
        if self._rows:
            raise AttributeError("行に幅は無い(高さは height)")
        self._sheet._s.set_col_width(self._key, None if value is None else float(value))

    @property
    def height(self):
        """行の高さ(ポイント)。列では None。"""
        if not self._rows:
            return None
        return self._sheet._s.row_height(self._key)

    @height.setter
    def height(self, value):
        if not self._rows:
            raise AttributeError("列に高さは無い(幅は width)")
        self._sheet._s.set_row_height(self._key, None if value is None else float(value))

    @property
    def hidden(self):
        """隠してあるか。**絞り込みと違って保存に残る**。"""
        if self._rows:
            return self._sheet._s.row_hidden(self._key)
        return self._sheet._s.col_hidden(self._key)

    @hidden.setter
    def hidden(self, value):
        if self._rows:
            self._sheet._s.set_row_hidden(self._key, bool(value))
        else:
            self._sheet._s.set_col_hidden(self._key, bool(value))

    def __repr__(self):
        if self._rows:
            return "<row {} 高さ{}>".format(self._key, self.height)
        return "<column {} 幅{}>".format(self._key, self.width)


class _Dimensions:
    """row_dimensions / column_dimensions の返り(openpyxl の役)。
    添字は openpyxl と同じで、行は番号(1起点)・列は字("A")。"""

    def __init__(self, sheet, rows):
        self._sheet = sheet
        self._rows = rows

    def __getitem__(self, key):
        return _Dim(self._sheet, self._rows, int(key) if self._rows else str(key))

    def group(self, start, end=None, outline_level=1, hidden=False):
        """行(列)をグループにする。openpyxl と同じ定義。
        畳んだ状態(hidden)は**保存に残る** — 畳んだ台帳は畳んだまま渡る。"""
        if self._rows:
            self._sheet._s.group_rows(int(start), None if end is None else int(end),
                                      outline_level, hidden)
        else:
            self._sheet._s.group_cols(str(start), None if end is None else str(end),
                                      outline_level, hidden)

    def __repr__(self):
        return "<{} dimensions>".format("row" if self._rows else "column")


class Table:
    """表(テーブル)。openpyxl の Table の形(displayName / ref / tableStyleInfo)。
    名前は式から使える識別子 — `=SUM(明細[金額])` の「明細」。"""

    def __init__(self, displayName=None, ref=None, name=None,
                 tableStyleInfo=None, headerRowCount=1, totalsRowCount=0, **_rest):
        self.displayName = displayName if displayName is not None else name
        self.ref = ref
        self.tableStyleInfo = tableStyleInfo
        self.headerRowCount = headerRowCount
        self.totalsRowCount = totalsRowCount

    @property
    def name(self):
        return self.displayName

    def __repr__(self):
        return "<Table {!r} {}>".format(self.displayName, self.ref)


class TableStyleInfo:
    """表の様式(openpyxl と同じ形)。name は "TableStyleMedium2" 等。"""

    def __init__(self, name=None, showRowStripes=True, showColumnStripes=False,
                 showFirstColumn=False, showLastColumn=False, **_rest):
        self.name = name
        self.showRowStripes = showRowStripes
        self.showColumnStripes = showColumnStripes
        self.showFirstColumn = showFirstColumn
        self.showLastColumn = showLastColumn


class _Tables:
    """Sheet.tables の返り。openpyxl と同じく名前で引ける dict 風。"""

    def __init__(self, sheet):
        self._s = sheet

    def _all(self):
        out = {}
        for name, ref, style, header, totals in self._s._s.tables:
            t = Table(displayName=name, ref=ref,
                      headerRowCount=1 if header else 0,
                      totalsRowCount=1 if totals else 0)
            if style:
                t.tableStyleInfo = TableStyleInfo(name=style)
            out[name] = t
        return out

    def __getitem__(self, name):
        try:
            return self._all()[name]
        except KeyError:
            raise KeyError("表が無い: {!r}".format(name)) from None

    def __contains__(self, name):
        return name in self._all()

    def __iter__(self):
        return iter(self._all())

    def __len__(self):
        return len(self._all())

    def keys(self):
        return self._all().keys()

    def items(self):
        return self._all().items()

    def values(self):
        return self._all().values()

    def __repr__(self):
        return repr(self._all())


class _SheetView:
    """`ws.sheet_view` の役。**中身はシートが持ちます** — ここは
    openpyxl の呼び名で読み書きするための薄い札です。"""

    __slots__ = ("_s",)

    def __init__(self, sheet):
        self._s = sheet

    @property
    def showGridLines(self):
        v = self._s.show_gridlines
        return True if v is None else v

    @showGridLines.setter
    def showGridLines(self, v):
        self._s.show_gridlines = bool(v)

    @property
    def zoomScale(self):
        return self._s.zoom_scale

    @zoomScale.setter
    def zoomScale(self, v):
        self._s.zoom_scale = None if v is None else int(v)

    @property
    def rightToLeft(self):
        v = self._s.rtl
        return False if v is None else v

    @rightToLeft.setter
    def rightToLeft(self, v):
        self._s.rtl = bool(v)

    def __repr__(self):
        return "<SheetView gridlines={}>".format(self.showGridLines)


class _Values(tuple):
    """`ws.values` の返り。**属性としても呼び出しとしても使えます。**

    本家は属性なので `for row in ws.values` と書きます。こちらは前まで
    呼び出しだったので、`ws.values()` と書いた台本も世に出ています。
    どちらも通しておきます(2026-08-28)。
    """

    def __call__(self):
        return self


class DataValidation:
    """入力規則。openpyxl の DataValidation の形(type / formula1 / add)。
    list はエンジンが効かせる(規則に合わない入力を堰き止める)。
    他の種類も落とさず持ち越す — 判定は分かる物だけ(模型の注のとおり)。"""

    def __init__(self, type=None, formula1=None, formula2=None, operator=None,
                 allow_blank=True, **_rest):
        self.type = type
        self.formula1 = formula1
        self.formula2 = formula2
        self.operator = operator
        self.allow_blank = allow_blank
        self.sqref = []

    def add(self, cell_range):
        self.sqref.append(str(cell_range))


class DefinedName:
    """名前付き範囲の1件。openpyxl の DefinedName の形(attr_text が参照)。"""

    __slots__ = ("name", "attr_text")

    def __init__(self, name, attr_text=None):
        self.name = name
        self.attr_text = attr_text

    @property
    def value(self):  # openpyxl の別名
        return self.attr_text

    def __repr__(self):
        return "<DefinedName {!r}={!r}>".format(self.name, self.attr_text)


def _split_ref(value, default_sheet):
    # "Sheet!$A$1:$B$2" / "$A$1" / "A1" → (シート名, "A1:B2")
    v = str(value).replace("$", "")
    if "!" in v:
        sheet, ref = v.rsplit("!", 1)
        return sheet.strip("'"), ref
    return default_sheet, v


class _DefinedNames:
    """Book.defined_names の返り。openpyxl と同じ dict 風の読み書き —
    wb.defined_names["単価"] = DefinedName("単価", attr_text="Sheet1!$A$1")。
    中身はシートの持ち物(names)— 名前はどこかのシートに属する。"""

    def __init__(self, book):
        self._b = book

    def _all(self):
        out = {}
        for ws in self._b.worksheets:
            for name, ref in ws.names:
                out[name] = DefinedName(
                    name, attr_text="{}!{}".format(ws.title, ref))
        return out

    def __getitem__(self, name):
        try:
            return self._all()[name]
        except KeyError:
            raise KeyError("名前が無い: {!r}".format(name)) from None

    def __setitem__(self, name, dn):
        sheet, ref = _split_ref(
            getattr(dn, "attr_text", dn), self._b.sheetnames[0])
        for ws in self._b.worksheets:  # 同じ名前は置き換え(どのシートでも)
            ws.delete_name(name)
        self._b[sheet].define_name(name, ref)

    def __delitem__(self, name):
        if not any(ws.delete_name(name) for ws in self._b.worksheets):
            raise KeyError("名前が無い: {!r}".format(name))

    def __contains__(self, name):
        return name in self._all()

    def __iter__(self):
        return iter(self._all())

    def __len__(self):
        return len(self._all())

    def keys(self):
        return self._all().keys()

    def items(self):
        return self._all().items()

    def values(self):
        return self._all().values()

    def __repr__(self):
        return repr(self._all())


class Comment:
    """セルのコメント。openpyxl の Comment(text, author) の形。
    模型は文だけを持つ — author は読みでは空になる(黙って落とさず、ここに書く)。"""

    __slots__ = ("text", "author")

    def __init__(self, text, author=""):
        self.text = text
        self.author = author

    def __repr__(self):
        return "<Comment {!r}>".format(self.text)


class Hyperlink:
    """セルのリンク。openpyxl の Hyperlink の役(target だけ持つ)。"""

    __slots__ = ("target",)

    def __init__(self, target):
        self.target = target

    def __repr__(self):
        return "<Hyperlink {!r}>".format(self.target)


def _rgb_of(v):
    """openpyxl の色を RRGGBB の字にする。

    `Color(rgb="FF9C0006")` でも `"9C0006"` でも `"#9C0006"` でも受けます。
    xlsx は頭2桁が透明度なので、8桁なら落とします。
    """
    if v is None:
        return None
    t = str(getattr(v, "rgb", None) or v).strip().lstrip("#")
    if len(t) == 8:
        t = t[2:]
    return t or None


class SheetProtection:
    """シート全体の保護。openpyxl の `ws.protection` の役。

    `ws.protection.sheet = True` で保護が掛かります。**パスワードは
    掛けません**(掛けた振りもしません)。許す操作は openpyxl と同じ
    「禁じる」向きの名前(`formatCells` など)でも読み書きできますが、
    模型は「許す」向きなので、ここで裏返します。
    """

    # openpyxl の名前 → エンジンの名前。openpyxl は「禁じる」向き
    KINSHI = {
        "formatCells": "format_cells",
        "formatColumns": "format_cols",
        "formatRows": "format_rows",
        "insertColumns": "insert_cols",
        "insertRows": "insert_rows",
        "insertHyperlinks": "insert_links",
        "deleteColumns": "delete_cols",
        "deleteRows": "delete_rows",
        "sort": "sort",
        "autoFilter": "autofilter",
        "pivotTables": "pivot",
        "objects": "objects",
    }
    # openpyxl の「許す」向きの名前
    YURUSU = {
        "selectLockedCells": "select_locked",
        "selectUnlockedCells": "select_unlocked",
    }

    def __init__(self, sheet):
        object.__setattr__(self, "_s", sheet)

    @property
    def sheet(self):
        return self._s._s.protected

    @sheet.setter
    def sheet(self, v):
        self._s._s.protected = bool(v)

    # openpyxl は enable() / disable() も持つ
    def enable(self):
        self.sheet = True

    def disable(self):
        self.sheet = False

    def __getattr__(self, name):
        allow = set(self._s._s.protect_allow)
        if name in self.KINSHI:
            return self.KINSHI[name] not in allow      # 禁じる向き
        if name in self.YURUSU:
            return self.YURUSU[name] in allow
        raise AttributeError(name)

    def __setattr__(self, name, value):
        if name in self.KINSHI or name in self.YURUSU:
            uchi = self.KINSHI.get(name) or self.YURUSU[name]
            hoshii = (not value) if name in self.KINSHI else bool(value)
            allow = set(self._s._s.protect_allow)
            allow.add(uchi) if hoshii else allow.discard(uchi)
            self._s._s.protect_allow = sorted(allow)
            return
        object.__setattr__(self, name, value)

    def __repr__(self):
        return "<SheetProtection sheet={}>".format(self.sheet)


class Protection:
    """セルの保護。openpyxl の Protection(locked) の形。
    シートを保護したとき、locked=False のセルだけが書ける(記入欄を開ける作法)。
    hidden(式を隠す)は模型に無い — True を渡されたら正直に断る。"""

    __slots__ = ("locked", "hidden")

    def __init__(self, locked=True, hidden=False):
        self.locked = locked
        self.hidden = hidden

    def __repr__(self):
        return "<Protection locked={}>".format(self.locked)


def _is_date_fmt(nf):
    # 表示形式が日付か。引用("...")と条件([...])の中を除いて
    # y / m / d / h / s が残るか — openpyxl と同じ考え方の簡易版
    if not nf or nf == "General":
        return False
    return any(t in _fmt_body(nf) for t in "ymdhs")


def _serial_to_datetime(v, nf, epoch):
    """Excel の通し番号を datetime に直す。openpyxl と同じ返し分けです。

    日付だけの表示形式なら `date`、時刻だけなら `time`、両方あれば
    `datetime` を返します。1900年うるう年の穴(通し番号 60 = 存在しない
    1900-02-29)は openpyxl と同じく前へずらして扱います。
    """
    import datetime

    body = _fmt_body(nf)
    hi = any(t in body for t in "hs") or "m" in body and ":" in body
    hizuke = any(t in body for t in "yd")
    # 1904 起点でないブックは、通し番号 60 までが1日ずれています
    days = int(v)
    frac = float(v) - days
    if epoch.year == 1899 and days < 60:
        days += 1
    try:
        at = epoch + datetime.timedelta(days=days, seconds=round(frac * 86400))
    except OverflowError:
        return v
    if hi and not hizuke:
        return at.time()
    if hizuke and not hi:
        return at.date()
    return at


def _fmt_body(nf):
    """表示形式から、引用と条件の中を除いた本体を小文字で返す"""
    out = []
    inq = inb = False
    for ch in nf or "":
        if ch == '"':
            inq = not inq
        elif inq:
            pass
        elif ch == "[":
            inb = True
        elif ch == "]":
            inb = False
        elif not inb:
            out.append(ch)
    return "".join(out).lower()


class Cell:
    """参照だけ持つセル(座標+シートの札)。openpyxl の Cell の役。

    エンジンはセルの物を持たず値を直に読み書きするので、これは
    「読むとき・書くときに座標で引き直す」薄い札。
    """

    __slots__ = ("parent", "row", "column")

    def __init__(self, worksheet, row, column):
        self.parent = worksheet
        self.row = row
        self.column = column

    @property
    def col_idx(self):
        return self.column

    @property
    def column_letter(self):
        return _col_letter(self.column)

    @property
    def coordinate(self):
        return _coord(self.row, self.column)

    @property
    def value(self):
        """セルの値。**表示形式が日付なら datetime で返します。**

        Excel の中では日付も数(起点からの通し番号)です。openpyxl は
        表示形式を見て datetime に直してから返すので、こちらも同じに
        します。書く側は先に済んでいて、読む側だけが数のままでした
        (2026-08-27、test/basic_xlsx.py の唯一の赤)。
        """
        v = self.parent.nama(self.coordinate)
        if isinstance(v, bool) or not isinstance(v, (int, float)):
            return v
        nf = self._fmt().get("number_format")
        if not _is_date_fmt(nf):
            return v
        return _serial_to_datetime(v, nf, self.parent.parent.epoch)

    @value.setter
    def value(self, v):
        self.parent[self.coordinate] = v

    @property
    def data_type(self):
        # openpyxl と同じ札: 'f' 式・'s' 文字・'b' 真偽・'n' 数と空
        if self.parent.formula(self.coordinate) is not None:
            return "f"
        v = self.parent.nama(self.coordinate)
        if isinstance(v, bool):
            return "b"
        if isinstance(v, str):
            return "s"
        return "n"

    def offset(self, row=0, column=0):
        return Cell(self.parent, self.row + row, self.column + column)

    # ── 書式(openpyxl の形で読み書き。合否は定義どおり動作するか)──

    def _fmt(self):
        return self.parent.fmt(self.coordinate)

    @property
    def font(self):
        d = self._fmt()
        return Font(
            name=d.get("font"),
            size=d.get("size"),
            bold=d.get("bold", False),
            italic=d.get("italic", False),
            underline="single" if d.get("underline") else None,
            strike=d.get("strike", False),
            color=Color(d["color"]) if "color" in d else None,
        )

    @font.setter
    def font(self, f):
        # openpyxl と同じく、代入は font 一式の置き換え(bold を書かない
        # Font を入れると太字は消える)
        u = getattr(f, "underline", None)
        size = getattr(f, "size", None)
        self.parent.set_fmt(
            self.coordinate,
            font=getattr(f, "name", None),
            size=None if size is None else float(size),
            bold=bool(getattr(f, "bold", None)),
            italic=bool(getattr(f, "italic", None)),
            underline=bool(u) and u != "none",
            strike=bool(getattr(f, "strike", None)),
            color=_rgb6(getattr(f, "color", None)),
        )

    @property
    def border(self):
        d = self._fmt()

        def side(k):
            v = d.get(k)
            if v is None:
                return Side()
            style, color = v
            return Side(style=style, color=Color(color) if color else None)

        return Border(
            left=side("border_left"),
            right=side("border_right"),
            top=side("border_top"),
            bottom=side("border_bottom"),
        )

    @border.setter
    def border(self, b):
        def edge(s):
            if s is None or getattr(s, "style", None) is None:
                return None  # エンジン側で「線なし」
            return (s.style, _rgb6(getattr(s, "color", None)))

        self.parent.set_fmt(
            self.coordinate,
            border_left=edge(getattr(b, "left", None)),
            border_right=edge(getattr(b, "right", None)),
            border_top=edge(getattr(b, "top", None)),
            border_bottom=edge(getattr(b, "bottom", None)),
        )

    @property
    def fill(self):
        d = self._fmt()
        if "fill" not in d:
            return PatternFill(patternType=None)
        return PatternFill(patternType="solid", fgColor=Color(d["fill"]))

    @fill.setter
    def fill(self, v):
        # 階調の塗りは別の鍵で渡します(模型が別に持っています)
        if isinstance(v, GradientFill):
            if len(v.stop) < 2:
                raise ValueError("階調には色が2つ以上要ります")
            self.parent.set_fmt(self.coordinate, gradient=v._kumi)
            return
        pt = getattr(v, "patternType", None)
        if pt is None:
            pt = getattr(v, "fill_type", None)
        if pt is None:
            self.parent.set_fmt(self.coordinate, fill=None)
            return
        if pt != "solid":
            raise NotImplementedError(
                "塗りは solid だけ(模様の塗りはエンジンに無い — 台帳)"
            )
        fg = getattr(v, "fgColor", None)
        if fg is None:
            fg = getattr(v, "start_color", None)
        self.parent.set_fmt(self.coordinate, fill=_rgb6(fg) or "000000")

    @property
    def alignment(self):
        d = self._fmt()
        return Alignment(
            horizontal=d.get("horizontal"),
            vertical=d.get("vertical"),
            wrap_text=d.get("wrap", False),
            shrink_to_fit=d.get("shrink", False),
            text_rotation=d.get("rotation", 0),
            indent=d.get("indent", 0),
        )

    @alignment.setter
    def alignment(self, a):
        rot = getattr(a, "text_rotation", 0) or 0
        ind = getattr(a, "indent", 0) or 0
        self.parent.set_fmt(
            self.coordinate,
            horizontal=getattr(a, "horizontal", None),
            vertical=getattr(a, "vertical", None),
            wrap=bool(getattr(a, "wrap_text", None)),
            shrink=bool(getattr(a, "shrink_to_fit", None)),
            rotation=int(rot) if rot else None,
            indent=int(ind),
        )

    @property
    def number_format(self):
        return self._fmt().get("number_format", "General")

    @number_format.setter
    def number_format(self, v):
        self.parent.set_fmt(
            self.coordinate, number_format=None if v in (None, "General") else v
        )

    @property
    def array_formula(self):
        """このセルが起点の配列数式(CSE)。無ければ None"""
        for at, f, _, _ in self.parent.array_formulae:
            if at == self.coordinate:
                return f
        return None

    @array_formula.setter
    def array_formula(self, v):
        """配列数式を入れる。**1つのセルに入れると 1×1 の配列**です。
        広い範囲に入れたいときは `ws.set_array_formula("D9:D12", …)` を。"""
        self.parent.set_array_formula(self.coordinate, str(v))

    @property
    def quotePrefix(self):
        """**先頭のクォート**(数に見える字を字として持つ印)。

        こちらは値の型で持ちます(`"0001"` は最初から字)ので、印は
        要りません。本家の台本が読んでも落ちないよう False を返します。
        """
        return False

    @property
    def pivotButton(self):
        """ピボットの ▼ が付いているか。**画面の持ち物**なので False"""
        return False

    def check_error(self, value=None):
        """エラー値か(openpyxl の内部の判定)。`#N/A` などなら True"""
        v = self.value if value is None else value
        return isinstance(v, str) and v.startswith("#")

    def check_string(self, value):
        """字として置ける形に直す(openpyxl の内部の判定)"""
        if value is None:
            return None
        t = str(value)
        # xlsx のセルは 32,767 字まで(本家と同じ切り方)
        return t[:32767]

    @property
    def internal_value(self):
        """**中に入っている値そのもの。** 日付の表示形式でも通し番号のまま
        返ります(`value` は datetime に直します)。openpyxl と同じ役です。"""
        return self.parent.nama(self.coordinate)

    @property
    def has_style(self):
        """既定でない書式を持っているか(openpyxl と同じ)"""
        return bool(self._fmt())

    @property
    def style_id(self):
        """書式の索引。**こちらは索引を人に見せません** — 原本の索引は
        保存のときだけ使い、模型は書式そのものを持ちます。本家の台本が
        読んでも落ちないよう 0 を返します。"""
        return 0

    @property
    def base_date(self):
        """日付の起点(ブックの `epoch` と同じ)"""
        return self.parent.parent.epoch

    @property
    def is_date(self):
        # 表示形式が日付で、中身が数(日付の通し番号)なら True。
        # **`value` ではなく生の値を見ます** — `value` は日付の表示形式が
        # あれば datetime に直して返すので、ここで使うと必ず False に
        # なります(2026-08-27 に踏みました)
        if not _is_date_fmt(self._fmt().get("number_format")):
            return False
        v = self.parent.nama(self.coordinate)
        return isinstance(v, (int, float)) and not isinstance(v, bool)

    @property
    def style(self):
        """名前付き様式の名前。**貼った名前はセルに持たない**(貼るのは
        書式そのもの)ので、読みは openpyxl と同じ既定の "Normal" を返す —
        原本から開いたセルの名前は保存で原文のまま残る(触らなければ)。"""
        return "Normal"

    @style.setter
    def style(self, name):
        """名前付き様式を貼る。**その様式の書式をこのセルに写す** —
        見た目は同じになるが、名前の帳簿はセルに持たない(模型の作り)。
        無い名前は KeyError(黙って何もしない、はしない)。"""
        n = str(getattr(name, "name", name))
        d = self.parent.parent.named_style_fmt(n)  # 無ければ KeyError
        self.parent.set_fmt(self.coordinate, **dict(d))

    @property
    def comment(self):
        t = self.parent.comment(self.coordinate)
        return None if t is None else Comment(t)

    @comment.setter
    def comment(self, v):
        # openpyxl の Comment(.text)も、ただの文字も、None(消す)も受ける
        self.parent.set_comment(
            self.coordinate, None if v is None else str(getattr(v, "text", v))
        )

    @property
    def hyperlink(self):
        t = self.parent.hyperlink(self.coordinate)
        return None if t is None else Hyperlink(t)

    @hyperlink.setter
    def hyperlink(self, v):
        self.parent.set_hyperlink(
            self.coordinate, None if v is None else str(getattr(v, "target", v))
        )

    @property
    def protection(self):
        return Protection(locked=self._fmt().get("locked", True))

    @protection.setter
    def protection(self, v):
        if getattr(v, "hidden", False):
            raise NotImplementedError("式を隠す(hidden)は模型に無い(台帳)")
        self.parent.set_fmt(self.coordinate, locked=bool(getattr(v, "locked", True)))

    def __repr__(self):
        return "<Cell {!r}.{}>".format(self.parent.name, self.coordinate)


class Sheet:
    """1枚のシート。エンジンの Sheet を包み、openpyxl の Worksheet の口を足す。"""

    # openpyxl の Worksheet が持つ定数(値は openpyxl 3.1.5 の実物)
    BREAK_NONE = 0
    BREAK_ROW = 1
    BREAK_COLUMN = 2
    ORIENTATION_PORTRAIT = "portrait"
    ORIENTATION_LANDSCAPE = "landscape"
    PAPERSIZE_LETTER = "1"
    PAPERSIZE_LETTER_SMALL = "2"
    PAPERSIZE_TABLOID = "3"
    PAPERSIZE_LEDGER = "4"
    PAPERSIZE_LEGAL = "5"
    PAPERSIZE_STATEMENT = "6"
    PAPERSIZE_EXECUTIVE = "7"
    PAPERSIZE_A3 = "8"
    PAPERSIZE_A4 = "9"
    PAPERSIZE_A4_SMALL = "10"
    PAPERSIZE_A5 = "11"
    SHEETSTATE_VISIBLE = "visible"
    SHEETSTATE_HIDDEN = "hidden"
    SHEETSTATE_VERYHIDDEN = "veryHidden"

    def __init__(self, raw, book):
        self._s = raw
        self._book = book
        # append が置いた最後の行。**shape から数え直さない** —
        # 中身が全部空の行を置いても shape は伸びないので、数え直すと
        # 同じ行に上書きし続ける(表題・空行・見出しの用紙が1行ずつずれた。
        # 2026-08-15)。openpyxl も内部で行を数えている
        self._append_row = 0

    # ── うちの口(エンジンそのまま)──────────────────────────────

    @property
    def name(self):
        return self._s.name

    def __getitem__(self, key):
        """openpyxl と同じ形で返します。

        - `ws["A1"]` — **セル**(Cell)。値ではありません。空の席でも
          セルを返します(本家の約束。結合の左上に書く形がこれに拠ります)
        - `ws["A1:F1"]` — Cell の組の組。1行なら `((c, c, …),)`、
          1列なら `((c,), (c,), …)`
        - `ws["A"]` / `ws["A:C"]` — その列の Cell
        - `ws[1]` / `ws[1:3]` — その行の Cell
        - `ws["A1":"C3"]` — スライスの書き方も範囲と同じ

        **2026-08-28 に値からセルへ変えました。** 前は `ws["A1"]` が値を
        返していて、`ws["A1"].value` と書く本家の台本が全部止まりました
        (実物の連載のサンプル 24 本のうち 5 本がここで落ちました)。
        値が欲しいときは `.value` を付けてください。
        """
        # ws["A1":"C3"] — スライス
        if isinstance(key, slice):
            if key.start is None or key.stop is None:
                raise ValueError("スライスは端を両方書いてください")
            # `ws["A":"B"]` も `ws[1:3]` も、`:` で繋いだ書き方と同じに扱います
            return self["{}:{}".format(key.start, key.stop)]
        # ws[1] / ws[1:3] — 行番号
        if isinstance(key, int):
            return self._gyou(key)
        if not isinstance(key, str):
            raise TypeError("セルの指し方が分かりません: {!r}".format(key))
        t = key.replace("$", "").strip()
        if ":" in t:
            a, _, b = t.partition(":")
            # 行番号だけの範囲("2:5")と列文字だけの範囲("A:C")
            if a.isdigit() and b.isdigit():
                return tuple(self._gyou(r) for r in range(int(a), int(b) + 1))
            if a.isalpha() and b.isalpha():
                return tuple(
                    self._retsu(_col_letter(c))
                    for c in range(_col_index(a), _col_index(b) + 1)
                )
            return self._hani(t)
        if t.isdigit():
            return self._gyou(int(t))
        if t.isalpha():
            return self._retsu(t)
        r, c = _cell_rc(t)
        return Cell(self, r, c)

    def _hani(self, ref):
        """"A1:C3" を Cell の組の組に"""
        a, _, b = ref.partition(":")
        (r0, c0), (r1, c1) = _cell_rc(a), _cell_rc(b)
        if r0 > r1:
            r0, r1 = r1, r0
        if c0 > c1:
            c0, c1 = c1, c0
        return tuple(
            tuple(Cell(self, r, c) for c in range(c0, c1 + 1))
            for r in range(r0, r1 + 1)
        )

    def _gyou(self, row):
        """その行の Cell。**使っている列の幅まで**(本家と同じ)"""
        if row < 1:
            raise ValueError("行は1から数えます: {}".format(row))
        haba = max(1, self.max_column)
        return tuple(Cell(self, row, c) for c in range(1, haba + 1))

    def _retsu(self, letters):
        """その列の Cell。**使っている行の高さまで**(本家と同じ)"""
        c = _col_index(letters)
        takasa = max(1, self.max_row)
        return tuple(Cell(self, r, c) for r in range(1, takasa + 1))

    def __setitem__(self, key, value):
        self._s[key] = value

    def nama(self, key):
        """**セルの生の値。** `ws["A1"]` は Cell を返すので、値そのものが
        要るときはこちらです(`Cell.value` が使います)。"""
        return self._s[key]

    def formula(self, key):
        return self._s.formula(key)

    def display(self, key):
        return self._s.display(key)

    @property
    def shape(self):
        return self._s.shape

    @property
    def values(self):
        """使っている範囲の値を、行ごとの組で返します。

        **本家は属性です**(`for row in ws.values`)。こちらは呼び出しに
        していたので、本家の台本が「method は回せない」で止まりました
        (2026-08-28)。`ws.values()` と書いていた分も動くよう、返る物は
        呼んでも自分を返します。
        """
        return _Values(self._s.values())

    @property
    def merges(self):
        return self._s.merges

    def insert_row(self, at):
        self._s.insert_row(at)

    def remove_row(self, at):
        self._s.remove_row(at)

    def insert_col(self, at):
        self._s.insert_col(at)

    def remove_col(self, at):
        self._s.remove_col(at)

    def __getattr__(self, name):
        # エンジンに後から生えた口は、包み直しを待たずにそのまま通す
        if name.startswith("_"):  # 自分の畑(_s 等)で再帰しない
            raise AttributeError(name)
        return getattr(self._s, name)

    # ── openpyxl の口(互換層)──────────────────────────────────

    @property
    def title(self):
        return self._s.name

    @title.setter
    def title(self, value):
        # 改名。式の参照(古い名前!A1)と名前の定義も追随する(エンジンの作法)
        self._s.name = value

    def merge_cells(self, range_string=None, start_row=None, start_column=None,
                    end_row=None, end_column=None):
        """セルを結合する。"A1:B2" でも openpyxl の数字指定でも。

        家の作法(アプリの「結合だけ」と同じ): 左上が空なら最初の中身が
        左上へ移り、左上以外の中身は消える(書式は残る)。
        """
        if range_string is None:
            range_string = "{}:{}".format(
                _coord(start_row, start_column), _coord(end_row, end_column)
            )
        self._s.merge_cells(range_string)

    def unmerge_cells(self, range_string=None, start_row=None, start_column=None,
                      end_row=None, end_column=None):
        """結合を解く。openpyxl と同じ定義 — **その範囲そのものが結合で
        なければ ValueError**(黙って近くの結合を解かない)。範囲に掛かる
        結合をまとめて解きたいときはエンジンの口(`s._s.unmerge_cells`)で。
        """
        if range_string is None:
            range_string = "{}:{}".format(
                _coord(start_row, start_column), _coord(end_row, end_column)
            )
        rng = range_string.replace(" ", "").upper()
        if rng not in self.merged_cell_ranges:
            raise ValueError("結合されていない範囲は解けない: {}".format(range_string))
        self._s.unmerge_cells(rng)

    @property
    def freeze_panes(self):
        # openpyxl と同じ A1 形式("B2" = 上1行・左1列)。無ければ None
        return self._s.freeze_panes

    @freeze_panes.setter
    def freeze_panes(self, value):
        if isinstance(value, Cell):  # openpyxl は Cell も受ける
            value = value.coordinate
        self._s.freeze_panes = value

    @property
    def parent(self):
        return self._book

    def cell(self, row, column, value=None):
        c = Cell(self, row, column)
        if value is not None:
            c.value = value
        return c

    @property
    def max_row(self):
        return max(self._s.shape[0], 1)

    @property
    def max_column(self):
        return max(self._s.shape[1], 1)

    def _min_used(self):
        # 値の入った最初の行・列(1起点)。何も無ければ (1, 1)。
        # openpyxl は様式だけのセルも数える — その正確さが要る事例が出たら
        # エンジンに API を足す(台帳の注のとおり)
        grid = self._s.values()
        r0 = c0 = None
        for i, row in enumerate(grid):
            for j, v in enumerate(row):
                if v is None:
                    continue
                if r0 is None or i < r0:
                    r0 = i
                if c0 is None or j < c0:
                    c0 = j
            if r0 is not None and c0 == 0:
                break
        if r0 is None:
            return (1, 1)
        return (r0 + 1, c0 + 1)

    @property
    def min_row(self):
        return self._min_used()[0]

    @property
    def min_column(self):
        return self._min_used()[1]

    def calculate_dimension(self):
        r0, c0 = self._min_used()
        return "{}:{}".format(_coord(r0, c0), _coord(self.max_row, self.max_column))

    @property
    def dimensions(self):
        return self.calculate_dimension()

    def add_chart(self, kind, *, data, categories=None, at="A1", title=None,
                  width=320.0, height=200.0, color=None, **kw):
        """**図を置く。** データはセルの範囲("B3:C7")か、数の列で渡します。

        図は図形の集まりとして**こちらで描きます**(2026-08-27 発注者
        「チャートは python による独自描画でいいのでは」)。画面にも紙にも
        xlsx にも同じ物が出ます。細かく組みたいときは
        `officework.chart.Chart` を直に使ってください。

        - `kind` — "bar"(縦棒)/ "line"(折れ線)/ "pie"(円)/
          "doughnut"(ドーナツ)/ "area"(面)/ "scatter"(散布)/
          "bubble"(バブル)/ "radar"(レーダー)/ "stock"(高安終値)/
          "surface"(等高線)/ "projected_pie"(補助縦棒つきの円)
        - `data` — 値の範囲。**1行目が系列の名前**なら見出しとして外します
        - `categories` — 区分の名前の範囲("A4:A7")
        """
        from . import chart as _chart

        atai = self._hani_no_atai(data)
        名 = self._hani_no_atai(categories, moji=True) if categories else None
        if 名 and 名 and isinstance(名[0], list):
            名 = [r[0] for r in 名]
        yaku = {"bar": _chart.bar, "line": _chart.line,
                "pie": _chart.pie, "doughnut": _chart.pie,
                "area": _chart.area, "radar": _chart.radar,
                "scatter": _chart.scatter, "bubble": _chart.scatter,
                "stock": _chart.stock, "surface": _chart.surface,
                "projected_pie": _chart.projected_pie}
        if kind not in yaku:
            raise ValueError(
                "図の種類に「{}」はありません。使えるのは {}".format(
                    kind, " / ".join(sorted(yaku))))
        if kind in ("scatter", "bubble"):
            # **散布は (x, y) の組**が要ります。範囲を渡したときは、
            # 1列目を x、2列目を y と見ます(openpyxl の Series と同じ)
            kumi = atai
            if kumi and not isinstance(kumi[0], (list, tuple)):
                kumi = [[float(i + 1), float(v)] for i, v in enumerate(kumi)]
            elif len(kumi) >= 2 and isinstance(kumi[0], list):
                kumi = list(zip(kumi[0], kumi[1]))
            if kind == "bubble" and "size" not in kw:
                kw["size"] = [abs(float(p[1])) ** 0.5 * 2.0 for p in kumi]
            return yaku[kind](self, at, list(kumi), title=title, width=width,
                              height=height, color=color, **kw)
        if kind == "stock":
            # **高値・安値・終値の3つの列**が要ります(あれば4つ目が始値)
            hashira = atai if atai and isinstance(atai[0], list) else [atai]
            if len(hashira) < 3:
                raise ValueError("高安終値には高値・安値・終値の3列が要ります")
            return yaku[kind](self, at, hashira[0], hashira[1], hashira[2],
                              hashira[3] if len(hashira) > 3 else None, 名,
                              title=title, width=width, height=height,
                              color=color, **kw)
        if kind == "surface":
            # 等高線は数の格子。範囲は行の並びのまま渡します
            koushi = atai if atai and isinstance(atai[0], list) else [atai]
            return yaku[kind](self, at, koushi, title=title, width=width,
                              height=height, color=color, **kw)
        if kind == "projected_pie":
            hira = [v for r in atai for v in (r if isinstance(r, list) else [r])]
            return yaku[kind](self, at, hira, 名, title=title, width=width,
                              height=height, color=color, **kw)
        if kind == "radar":
            return yaku[kind](self, at, atai, 名, title=title, width=width,
                              height=height, color=color, **kw)
        if kind in ("pie", "doughnut"):
            hira = [v for r in atai for v in (r if isinstance(r, list) else [r])]
            return yaku[kind](self, at, hira, 名, title=title, width=width,
                              height=height, color=color,
                              hole=0.5 if kind == "doughnut" else 0.0, **kw)
        return yaku[kind](self, at, atai, 名, title=title, width=width,
                          height=height, color=color, **kw)

    def _hani_no_atai(self, hani, *, moji=False):
        """範囲("B3:C7")を値の列にする。**列ごとに1つの系列**です。

        1行目が字なら系列の名前とみて外します(openpyxl の
        `from_rows=False` と同じ見方)。数の列をそのまま渡してもかまいません。
        """
        if not isinstance(hani, str):
            return list(hani)
        cells = self[hani] if ":" in hani else ((self[hani],),)
        hyou = [[c.value for c in row] for row in cells]
        if not hyou:
            return []
        if moji:
            return [str(r[0]) if r[0] is not None else "" for r in hyou]
        # 1行目が全部字なら見出し
        if len(hyou) > 1 and all(isinstance(v, str) for v in hyou[0]):
            hyou = hyou[1:]
        # 列ごとに縦に読み替えます
        keiretsu = [[r[c] for r in hyou] for c in range(len(hyou[0]))]
        kazu = [[float(v) for v in k if isinstance(v, (int, float))] for k in keiretsu]
        kazu = [k for k in kazu if k]
        return kazu if len(kazu) > 1 else (kazu[0] if kazu else [])

    # ── 条件付き書式 — openpyxl の Font / PatternFill をそのまま受ける ──
    #
    # エンジンの口は色を字で取ります。openpyxl の台本は `font=Font(...)`
    # `fill=PatternFill(...)` と書くので、ここでほどいて渡します。
    # **両方の書き方が通る** — 色を字で渡しても構いません

    @staticmethod
    def _mitame(font=None, fill=None, **kw):
        """openpyxl の Font / PatternFill を、色と飾りの鍵にほどく"""
        out = dict(kw)
        if font is not None:
            for uchi, honke in [("color", "color"), ("bold", "bold"),
                                ("italic", "italic"), ("strike", "strike")]:
                v = getattr(font, honke, None)
                if v is not None and out.get(uchi) is None:
                    out[uchi] = _rgb_of(v) if uchi == "color" else bool(v)
            u = getattr(font, "underline", None)
            if u is not None and out.get("underline") is None:
                out["underline"] = u not in (None, "none", False)
        if fill is not None and out.get("fill") is None:
            out["fill"] = _rgb_of(getattr(fill, "fgColor", None) or getattr(fill, "start_color", None))
        return {k: v for k, v in out.items() if v is not None}

    def conditional_formatting_cellis(self, range, op, value, value2=None, **kw):
        self._s.conditional_formatting_cellis(
            range, op, str(value), None if value2 is None else str(value2),
            **self._mitame(**kw))

    def conditional_formatting_formula(self, range, formula, **kw):
        self._s.conditional_formatting_formula(range, formula, **self._mitame(**kw))

    def conditional_formatting_duplicates(self, range, unique=False, **kw):
        self._s.conditional_formatting_duplicates(range, unique=unique, **self._mitame(**kw))

    def add_pivot(self, src, at, rows, value, cols=None, agg="sum", totals=True,
                  subtotals=False, grand_label=None, subtotal_label=None):
        """**ピボットテーブルを置く。** 中では polars(Rust)が集計します。

        `src` は元の表("A1:C7"。1行目が見出し)、`at` は置く左上。
        `rows` は行に並べる見出し、`cols` は列に広げる見出しです。
        `agg` は sum / count / mean / min / max / median。

        **札は既定が英語です。** 日本語の帳票なら
        `grand_label="総計"`、`subtotal_label="{} 小計"` を渡します。

        返りは置いた広さ (行数, 列数)。列に広げると見出しが2行になります。
        """
        return self._s.add_pivot(src, at, list(rows), str(value),
                                 None if cols is None else list(cols),
                                 str(agg), bool(totals), bool(subtotals),
                                 grand_label, subtotal_label)

    @property
    def active_cell(self):
        """いま選んでいるセル。**ファイルの側には持ちません** — 画面の
        持ち物なので、開いたときは常に左上です(openpyxl は sheetView の
        値を返しますが、こちらは画面が持ちます)。"""
        return "A1"

    @property
    def selected_cell(self):
        """選んでいる範囲。`active_cell` と同じ理由で左上を返します"""
        return "A1"

    @property
    def sheet_view(self):
        """画面の設定(目盛線・固定枠・拡大)。openpyxl は入れ物を返しますが、
        こちらは**シートが直に持ちます** — `ws.show_gridlines` などです。"""
        return _SheetView(self)

    def set_printer_settings(self, paper_size=None, orientation=None):
        """紙の設定(openpyxl と同じ口)。`ws.paper_size` などと同じ所です"""
        if paper_size is not None:
            self.paper_size = int(paper_size)
        if orientation is not None:
            self.orientation = str(orientation)

    def add_image(self, img, anchor=None, width_px=None, height_px=None):
        """**シートに画像を置く**(openpyxl と同じ口)。

        `img` は径路でも bytes でも、openpyxl の `Image` でも受けます。
        `anchor` は左上を留めるセル("B2")。`Image` が `anchor` を持って
        いればそちらを使います。大きさは絵の実寸(96dpi)が既定で、
        `Image` に `width` / `height`(px)があればそれを使います。
        """
        # **openpyxl の `Image` は `path` に部品の名前を持ちます**
        # (`/xl/media/image1.png`)。実体は `ref` の側です — 径路のことも
        # あれば、開いたファイルや PIL の絵のこともあります
        moto = getattr(img, "ref", None) or getattr(img, "path", None) or img
        if hasattr(moto, "read"):          # 開いたファイル
            moto = moto.read()
        elif hasattr(moto, "save") and not isinstance(moto, (str, bytes, bytearray)):
            # PIL の絵。PNG にして渡します
            import io
            buf = io.BytesIO()
            moto.save(buf, format="PNG")
            moto = buf.getvalue()
        at = anchor or getattr(img, "anchor", None) or "A1"
        at = str(getattr(at, "_from", at) or "A1")
        if not isinstance(at, str) or not at[:1].isalpha():
            at = "A1"
        # 大きさは名指しが勝ちます(`width_px` はこちらの前からの呼び方)
        w = width_px if width_px is not None else getattr(img, "width", None)
        h = height_px if height_px is not None else getattr(img, "height", None)
        self._s.add_image(
            moto if isinstance(moto, (bytes, bytearray)) else str(moto),
            at,
            None if w is None else float(w),
            None if h is None else float(h),
        )

    @property
    def protection(self):
        """シートの保護。`ws.protection.sheet = True` で掛かります"""
        return SheetProtection(self)

    def protect(self, *, allow=None):
        """**シートを保護する。** `ws.protection.sheet = True` の短い書き方。

        `allow` に許す操作の名前を渡すと、そこだけ開けます
        (`ws.protect(allow=["sort", "autofilter"])`)。渡さなければ
        エンジンの既定(ロックの有無にかかわらずセルは選べる)です。
        """
        self._s.protected = True
        if allow is not None:
            self._s.protect_allow = sorted(allow)

    def unprotect(self):
        """保護を外す。許す操作の設定はそのまま残します"""
        self._s.protected = False

    def append(self, iterable):
        """使われている範囲の次の行に1行置く(openpyxl と同じ定義)。

        list/tuple なら A から順に、dict なら {列番号か列の字: 値} で。
        空のシートでは1行目に入る。
        """
        rows, _ = self._s.shape
        # 直に書かれて伸びた分にも追いつく(どちらか大きい方の次へ)
        at = max(rows, self._append_row) + 1
        self._append_row = at
        if isinstance(iterable, dict):
            for k, v in iterable.items():
                col = k if isinstance(k, int) else _col_index(k)
                self._s[_coord(at, col)] = v
        else:
            for j, v in enumerate(iterable, start=1):
                self._s[_coord(at, j)] = v

    def iter_rows(self, min_row=None, max_row=None, min_col=None, max_col=None,
                  values_only=False):
        r0 = min_row or 1
        r1 = max_row or self.max_row
        c0 = min_col or 1
        c1 = max_col or self.max_column
        for i in range(r0, r1 + 1):
            if values_only:
                yield tuple(self._s[_coord(i, j)] for j in range(c0, c1 + 1))
            else:
                yield tuple(Cell(self, i, j) for j in range(c0, c1 + 1))

    def iter_cols(self, min_col=None, max_col=None, min_row=None, max_row=None,
                  values_only=False):
        r0 = min_row or 1
        r1 = max_row or self.max_row
        c0 = min_col or 1
        c1 = max_col or self.max_column
        for j in range(c0, c1 + 1):
            if values_only:
                yield tuple(self._s[_coord(i, j)] for i in range(r0, r1 + 1))
            else:
                yield tuple(Cell(self, i, j) for i in range(r0, r1 + 1))

    @property
    def rows(self):
        return self.iter_rows()

    @property
    def columns(self):
        return self.iter_cols()

    def insert_rows(self, idx, amount=1):
        for _ in range(amount):
            self._s.insert_row(idx)

    def delete_rows(self, idx, amount=1):
        for _ in range(amount):
            self._s.remove_row(idx)

    def insert_cols(self, idx, amount=1):
        for _ in range(amount):
            self._s.insert_col(_col_letter(idx))

    def delete_cols(self, idx, amount=1):
        for _ in range(amount):
            self._s.remove_col(_col_letter(idx))

    @property
    def merged_cell_ranges(self):
        # openpyxl と同じ「"B2:C3" の一覧」の形で返す
        return ["{}:{}".format(a, b) for a, b in self._s.merges]

    @property
    def print_area(self):
        return self._s.print_area

    @print_area.setter
    def print_area(self, value):
        # openpyxl と同じく、文字でも一覧でも受ける
        if isinstance(value, (list, tuple)):
            value = ",".join(str(v) for v in value)
        self._s.print_area = value

    def move_range(self, cell_range, rows=0, cols=0, translate=False):
        """範囲を動かす(openpyxl と同じ呼び方)。移った先は上書き。

        **openpyxl との違い(上位分)**: 外から動かした範囲を指していた式は
        **付いて動く**(`=B1+1` は B1 が B6 へ動けば `=B6+1`)。openpyxl は
        古びたまま(空のセルを指す)にする。範囲の中の式はそのままで、
        translate=True なら中の相対参照もずれる(本家と同じ定義)。"""
        return self._s.move_range(str(cell_range).replace("$", ""),
                                  int(rows), int(cols), bool(translate))

    @property
    def row_dimensions(self):
        """行の寸法(openpyxl の役)。row_dimensions[1].height / .hidden、
        まとめて畳むなら .group。"""
        return _Dimensions(self, rows=True)

    @property
    def column_dimensions(self):
        """列の寸法(openpyxl の役)。column_dimensions["A"].width / .hidden。"""
        return _Dimensions(self, rows=False)

    @property
    def column_groups(self):
        """列のグループ [(列の字, 深さ, 畳んで隠れているか)]。"""
        return self._s.col_groups

    @property
    def row_groups(self):
        """行のグループ [(行, 深さ, 畳んで隠れているか)]。"""
        return self._s.row_groups

    @property
    def array_formulae(self):
        """配列式(スピル)。openpyxl と同じく {左上のセル: 式} の形。
        **うちは値まで計算されている**(openpyxl は式を持つだけ)。"""
        return {a1: f for a1, f, _r, _c in self._s.array_formulae}

    @property
    def tables(self):
        """表(テーブル)。openpyxl と同じ名前で引ける形。
        名前は式から使える — `=SUM(明細[金額])`(**構造化参照は計算まで効く** —
        openpyxl は式を計算しないので、ここは上位分)。"""
        return _Tables(self)

    def add_table(self, table):
        """表を足す(openpyxl と同じ口 — Table を渡す)。
        本家の実物でもうちの Table でもよい(属性名で受ける)。"""
        name = getattr(table, "displayName", None) or getattr(table, "name", None)
        ref = getattr(table, "ref", None)
        if not name or not ref:
            raise ValueError("表には displayName と ref が要ります")
        si = getattr(table, "tableStyleInfo", None)
        header = getattr(table, "headerRowCount", 1)
        totals = getattr(table, "totalsRowCount", 0)
        self._s.add_table(
            str(ref).replace("$", ""),
            str(name),
            style=getattr(si, "name", None) if si is not None else None,
            header=bool(header),
            totals=bool(totals),
            banded_rows=bool(getattr(si, "showRowStripes", True)) if si else True,
            banded_cols=bool(getattr(si, "showColumnStripes", False)) if si else False,
        )

    def remove_table(self, name):
        """表を外す(中身と書式は残る — Excel と同じ)。"""
        if not self._s.remove_table(str(getattr(name, "displayName", name))):
            raise KeyError("表が無い: {!r}".format(name))

    @property
    def show_gridlines(self):
        """画面の枠線を出すか(openpyxl の sheet_view.showGridLines と同じ役)。
        原本に指定が無ければ None(= 出す、が既定)。"""
        return self._s.show_gridlines

    @show_gridlines.setter
    def show_gridlines(self, v):
        self._s.show_gridlines = None if v is None else bool(v)

    @property
    def print_gridlines(self):
        """**印刷**の枠線(openpyxl の print_options.gridLines)。画面とは別。"""
        return self._s.print_gridlines

    @print_gridlines.setter
    def print_gridlines(self, v):
        self._s.print_gridlines = bool(v)

    @property
    def oddHeader(self):
        """印刷のヘッダー(openpyxl と同じ left / center / right)。
        奇数・偶数・先頭頁の別は模型に無いので、これが唯一のヘッダー。"""
        return _HeaderFooter(self, footer=False)

    @property
    def oddFooter(self):
        return _HeaderFooter(self, footer=True)

    @property
    def evenHeader(self):
        """偶数頁だけのヘッダー(**左右で綴じる帳票**はこれを別に組む)。
        置くと「奇数偶数で分ける」旗が立つ(付け忘れると効かないため)。"""
        return _HeaderFooter(self, footer=False, part="even")

    @property
    def evenFooter(self):
        return _HeaderFooter(self, footer=True, part="even")

    @property
    def firstHeader(self):
        """先頭頁だけのヘッダー(表紙の扱い)。"""
        return _HeaderFooter(self, footer=False, part="first")

    @property
    def firstFooter(self):
        return _HeaderFooter(self, footer=True, part="first")

    @property
    def print_title_rows(self):
        """頁ごとに繰り返す見出し行("1:2" の形。openpyxl と同じ)。
        PDF と印刷が実際に繰り返す — 複数頁の明細の定番。"""
        return self._s.print_title_rows

    @print_title_rows.setter
    def print_title_rows(self, value):
        self._s.print_title_rows = value

    @property
    def print_title_cols(self):
        """頁ごとに左で繰り返す見出し列("A:B" の形。openpyxl と同じ)。
        横に長い台帳で品名の列を毎ページ出すための物。

        **注**: ファイル(xlsx)には正しく入り、Excel は繰り返す。
        こちらの PDF はまだ列を繰り返さない — 描く側が列を「連番」で
        持っており、そこを一覧に変える仕事が残っている(台帳)。"""
        return self._s.print_title_cols

    @print_title_cols.setter
    def print_title_cols(self, value):
        self._s.print_title_cols = value

    @property
    def print_titles(self):
        """openpyxl と同じ「'シート'!$1:$2」の形(無ければ None)。"""
        r = self._s.print_title_rows
        if not r:
            return None
        a, b = r.split(":")
        return "'{}'!${}:${}".format(self.title, a, b)

    def add_data_validation(self, dv):
        """入力規則を足す(openpyxl と同じ口 — DataValidation を渡す)。
        本家の実物でもうちの DataValidation でもよい(属性名で受ける)。"""
        sqref = getattr(dv, "sqref", None)
        ranges = ([str(r) for r in sqref] if isinstance(sqref, (list, tuple))
                  else str(sqref).split())
        if not ranges:
            raise ValueError("先に dv.add(範囲) で掛ける範囲を決めてください")
        for r in ranges:
            self._s.add_validation(
                r.replace("$", ""),
                str(getattr(dv, "formula1", "") or ""),
                kind=str(getattr(dv, "type", "") or ""),
                operator=str(getattr(dv, "operator", "") or ""),
                formula2=str(getattr(dv, "formula2", "") or ""),
                allow_blank=bool(getattr(dv, "allow_blank", True)),
            )

    def __repr__(self):
        return '<officework.sheet.Sheet "{}">'.format(self.name)


class DocumentProperties:
    """ブックの情報。openpyxl の `wb.properties` の役。

    `wb.properties.title = "御見積書"` のように書きます。読める欄は
    creator / title / subject / keywords / description の5つで、
    openpyxl の別名(`author`)も受けます。
    """

    ALIAS = {"author": "creator"}
    FIELDS = ("creator", "title", "subject", "keywords", "description")

    def __init__(self, book):
        object.__setattr__(self, "_b", book)

    def __getattr__(self, name):
        k = self.ALIAS.get(name, name)
        if k in self.FIELDS:
            return self._b._b.props().get(k) or None
        raise AttributeError(name)

    def __setattr__(self, name, value):
        k = self.ALIAS.get(name, name)
        if k in self.FIELDS:
            self._b._b.set_props(**{k: "" if value is None else str(value)})
            return
        object.__setattr__(self, name, value)

    def __repr__(self):
        return "<DocumentProperties title={!r}>".format(self.title)


class Book:
    """1冊のブック。エンジンの Book を包み、openpyxl の Workbook の口を足す。"""

    def __init__(self):
        self._b = _engine.Book()
        self._path = None

    @staticmethod
    def open(path):
        # **pathlib.Path も受ける**(openpyxl と同じ。2026-08-15)。
        # 芯は文字しか取らないので、ここで径路の形に直してから渡す
        path = _os.fspath(path)
        b = Book.__new__(Book)
        b._b = _engine.Book.open(path)
        b._path = path
        return b

    # ── うちの口(エンジンそのまま)──────────────────────────────

    def save(self, path, dpi=None):
        """保存する。拡張子で行き先が決まります。

        ``.xlsx`` はブック、``.pdf`` は紙、``.png`` は絵です。
        ``dpi`` は絵の細かさで、既定は 150 です(``.png`` のときだけ効きます)。
        頁が複数あるときは、2枚目から名前に ``-2``・``-3`` が付きます。
        """
        # pathlib.Path も受ける(上の open と同じ理由)
        self._b.save(_os.fspath(path), dpi)

    def recalc(self):
        self._b.recalc()

    @property
    def sheet_names(self):
        return self._b.sheet_names

    @property
    def unsupported(self):
        return self._b.unsupported

    def add_sheet(self, name):
        return Sheet(self._b.add_sheet(name), self)

    def __getitem__(self, key):
        return Sheet(self._b[key], self)

    def __len__(self):
        return len(self._b)

    @property
    def properties(self):
        """ブックの情報(題・著者など)。openpyxl と同じ書き方です"""
        return DocumentProperties(self)

    def __getattr__(self, name):
        if name.startswith("_"):  # 自分の畑(_b 等)で再帰しない
            raise AttributeError(name)
        return getattr(self._b, name)

    # ── openpyxl の口(互換層)──────────────────────────────────

    @property
    def sheetnames(self):
        return self._b.sheet_names

    @property
    def worksheets(self):
        return [self[i] for i in range(len(self))]

    @property
    def active(self):
        # 台帳のとおり先頭のシート(xlsx の activeTab を読むならエンジンに小さな API)
        return self[0]

    @property
    def path(self):
        # 開いた元のファイル(無ければ None)。openpyxl の内部定数は真似しない
        return self._path

    def create_sheet(self, title=None, index=None):
        if title is None:
            # openpyxl と同じ流儀で空いている名前を探す
            title = "Sheet"
            n = 0
            while title in self._b.sheet_names:
                n += 1
                title = "Sheet{}".format(n)
        self.add_sheet(title)
        if index is not None:
            self._b.move_sheet(title, index)
        # 札は位置で指すので、並べた後の位置で引き直して返す
        return self[title]

    def copy_worksheet(self, worksheet):
        # openpyxl と同じ「〜 Copy」の名前。塞がっていれば番号を継ぐ
        base = "{} Copy".format(worksheet.title)
        name, n = base, 0
        while name in self._b.sheet_names:
            n += 1
            name = "{}{}".format(base, n)
        self._b.copy_sheet(worksheet.title, name)
        return self[name]

    def remove(self, worksheet):
        # 最後の1枚は抜けない(エンジンが正直に断る)
        self._b.remove_sheet(worksheet.title)

    def move_sheet(self, sheet, offset=0):
        # openpyxl と同じ「相対のずらし」。sheet は Sheet でも名前でも
        name = sheet if isinstance(sheet, str) else sheet.title
        names = self._b.sheet_names
        cur = names.index(name)
        to = max(0, min(len(names) - 1, cur + offset))
        self._b.move_sheet(name, to)

    # ── openpyxl の古い呼び名 ────────────────────────────────────
    #
    # openpyxl 2.x の名前です。本家では非推奨ですが、世に出ている台本
    # (連載・社内のマクロ)はこの名前で書かれています。**動かないと
    # 移ってこられない**ので受けます(2026-08-29)。

    @property
    def data_only(self):
        """**値だけで開いたか。** openpyxl は開くときに選ばせますが、
        こちらは**式も値も常に両方持ちます**(自分で計算できるため)。
        選ぶ必要が無いので False を返します。"""
        return False

    @property
    def read_only(self):
        """**読むだけで開いたか。** こちらはその形を持ちません
        (原本を壊さない作りなので、開いて保存しても元は残ります)"""
        return False

    @property
    def chartsheets(self):
        """グラフだけのシート。**まだ持ちません** — 空を返します。
        図はシートの上に置く形(`ws.add_chart`)で作れます。"""
        return []

    def create_chartsheet(self, title=None, index=None):
        """グラフだけのシートを足す。**まだ持ちません**ので、正直に断ります"""
        raise NotImplementedError(
            "グラフだけのシートはまだ持ちません。"
            "普通のシートに ws.add_chart(...) で置いてください"
        )

    def get_sheet_by_name(self, name):
        """シートを名前で。いまの書き方は `wb[name]`"""
        return self[name]

    def get_sheet_names(self):
        """シートの名前の並び。いまの書き方は `wb.sheetnames`"""
        return list(self.sheetnames)

    def remove_sheet(self, worksheet):
        """シートを消す。いまの書き方は `wb.remove(ws)`"""
        return self.remove(worksheet)

    def index(self, worksheet):
        return self._b.sheet_names.index(worksheet.title)

    @property
    def named_styles(self):
        """名前付きセル様式の一覧(openpyxl と同じく**名前の並び**)。
        定義は原本の styles.xml が持ち、保存でそのまま持ち越される。"""
        return [n for n, _b in self._b.named_styles]

    @property
    def style_names(self):
        # openpyxl の別名
        return self.named_styles

    def add_named_style(self, style):
        """名前付き様式を作る(openpyxl と同じ口 — NamedStyle を渡す)。
        保存で styles.xml の cellStyleXfs / cellStyles に**追記**する
        (原本の索引は動かさないので、触っていないセルの書式は無傷)。
        本家の NamedStyle でもうちの Font / Border / PatternFill / Alignment
        を持つ物でもよい(属性名で受ける)。"""
        name = getattr(style, "name", None)
        if not name:
            raise ValueError("様式には name が要ります")
        kw = {}
        f = getattr(style, "font", None)
        if f is not None:
            size = getattr(f, "size", None)
            kw.update(
                font=getattr(f, "name", None),
                size=None if size is None else float(size),
                bold=bool(getattr(f, "bold", None)),
                italic=bool(getattr(f, "italic", None)),
                strike=bool(getattr(f, "strike", None)),
                color=_rgb6(getattr(f, "color", None)),
            )
            u = getattr(f, "underline", None)
            kw["underline"] = bool(u) and u != "none"
        b = getattr(style, "border", None)
        if b is not None:
            for side, key in (("left", "border_left"), ("right", "border_right"),
                              ("top", "border_top"), ("bottom", "border_bottom")):
                sd = getattr(b, side, None)
                st = getattr(sd, "style", None) if sd is not None else None
                kw[key] = None if st is None else (
                    st, _rgb6(getattr(sd, "color", None)))
        fill = getattr(style, "fill", None)
        if fill is not None:
            pt = getattr(fill, "patternType", None) or getattr(fill, "fill_type", None)
            if pt == "solid":
                fg = getattr(fill, "fgColor", None) or getattr(fill, "start_color", None)
                kw["fill"] = _rgb6(fg) or "000000"
        al = getattr(style, "alignment", None)
        if al is not None:
            kw.update(
                horizontal=getattr(al, "horizontal", None),
                vertical=getattr(al, "vertical", None),
                wrap=bool(getattr(al, "wrap_text", None)),
                indent=int(getattr(al, "indent", 0) or 0),
            )
        nf = getattr(style, "number_format", None)
        if nf and nf != "General":
            kw["number_format"] = nf
        self._b.add_named_style(str(name), **kw)

    @property
    def epoch(self):
        """日付の起点(openpyxl と同じ datetime)。1899-12-30 か、
        1904 起点のブックは 1904-01-01。"""
        import datetime

        return (datetime.datetime(1904, 1, 1) if self._b.date1904
                else datetime.datetime(1899, 12, 30))

    @epoch.setter
    def epoch(self, value):
        # openpyxl と同じ2つの定数だけを受ける。通し番号はそのままなので、
        # 既にある日付の意味が4年動く(Excel の設定切り替えと同じ)
        y = getattr(value, "year", None)
        if y == 1904:
            self._b.date1904 = True
        elif y in (1899, 1900):
            self._b.date1904 = False
        else:
            raise ValueError("起点は 1899-12-30 か 1904-01-01: {!r}".format(value))

    @property
    def excel_base_date(self):
        # openpyxl の別名
        return self.epoch

    @property
    def defined_names(self):
        """名前付き範囲(openpyxl と同じ dict 風)。名前は式(=単価*2)で使える。"""
        return _DefinedNames(self)

    def create_named_range(self, name, worksheet=None, value=None, scope=None):
        """名前を定義する(openpyxl と同じ口)。value は "$A$1:$B$2" か
        "Sheet!$A$1"。scope(ブック/シートの別)は持たない — 名前は
        属するシートの物(それで式も保存も足りている)。"""
        if scope is not None:
            raise NotImplementedError("scope は持たない(名前はシートの物)")
        sheet, ref = _split_ref(
            value, worksheet.title if worksheet is not None else self.sheetnames[0])
        self[sheet].define_name(name, ref)

    def get_index(self, worksheet):
        # openpyxl の古い別名(本家では index を使えと言う)
        return self.index(worksheet)

    def get_index(self, worksheet):
        # openpyxl でも廃止予定の旧名(index と同じ物)
        return self.index(worksheet)

    def close(self):
        # openpyxl の close は read_only/write_only モードの後始末。
        # うちは開きっぱなしの資源が無いので、何もしないで良い
        pass

    def __iter__(self):
        return iter(self.worksheets)

    def __contains__(self, name):
        return name in self._b.sheet_names

    def __repr__(self):
        return "<officework.sheet.Book {}>".format(self._b.sheet_names)


__all__ = [
    "Book", "Sheet", "Cell",
    "Font", "Border", "Side", "PatternFill", "Alignment", "Color",
    "Comment", "Hyperlink", "Protection", "DefinedName", "DataValidation",
    "Table", "TableStyleInfo",
]
