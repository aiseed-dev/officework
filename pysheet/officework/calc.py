# -*- coding: utf-8 -*-
"""officework.calc — 動いている calc を Jupyter/Python から操る(xlwings 流)。

使い方(xlwings の import を1行差し替えるだけ):

    from officework import calc as xw
    import pandas as pd

    wb = xw.Book()                 # 新しいブック(未保存が無ければ)
    xw.Range('A1').value = df      # DataFrame をセルへ(見出し・index つき)
    df2 = xw.Range('A1').options(pd.DataFrame, expand='table').value
    wb.sheets['Sheet1'].range('B2').value = [[1, 2], [3, 4]]
    wb.save('集計.xlsx')

未対応(正直に): @xw.func / @xw.sub / Book.caller() — Excel のアドイン機構の
話なので、calc では「AI タブ → マクロを書く」(plugins/*.py)が同じ役目。
"""

import os

from . import OfficeworkError, call as _shared_call

JoofficeError = OfficeworkError  # 旧名との互換
JocalcError = OfficeworkError


def _call(cmd, **kw):
    return _shared_call("calc", cmd, **kw)

def _col_name(n):
    # 0 → A, 25 → Z, 26 → AA
    s = ""
    n += 1
    while n > 0:
        n, r = divmod(n - 1, 26)
        s = chr(65 + r) + s
    return s


def _parse_a1(a1):
    # "B2" → (row0, col0)
    col = 0
    i = 0
    for ch in a1:
        if ch.isalpha():
            col = col * 26 + (ord(ch.upper()) - 64)
            i += 1
        else:
            break
    return int(a1[i:]) - 1, col - 1


class Options:
    def __init__(self, rng, convert=None, expand=None, index=True, header=True):
        self._rng = rng
        self._convert = convert
        self._expand = expand
        self._index = index
        self._header = header

    @property
    def value(self):
        rng = self._rng
        if self._expand == "table":
            rng = rng.expand("table")
        grid = rng._get()
        conv = self._convert
        if conv is not None and conv.__name__ == "DataFrame":
            return _grid_to_frame(grid, conv, index=self._index, header=self._header)
        return rng._plain(grid)

    @value.setter
    def value(self, v):
        self._rng.value = v


class Range:
    def __init__(self, a1, sheet=None):
        if ":" in a1:
            left, right = a1.split(":", 1)
            self._r0, self._c0 = _parse_a1(left)
            self._r1, self._c1 = _parse_a1(right)
        else:
            self._r0, self._c0 = _parse_a1(a1)
            self._r1, self._c1 = self._r0, self._c0
        self._sheet = sheet  # None = いまのシート

    def _a1(self):
        a = "{}{}".format(_col_name(self._c0), self._r0 + 1)
        if (self._r0, self._c0) == (self._r1, self._c1):
            return a
        return "{}:{}{}".format(a, _col_name(self._c1), self._r1 + 1)

    def _kw(self):
        kw = {"a1": self._a1()}
        if self._sheet is not None:
            kw["sheet"] = self._sheet
        return kw

    def _make(self, r0, c0, r1, c1):
        out = Range("{}{}".format(_col_name(c0), r0 + 1), sheet=self._sheet)
        out._r1, out._c1 = r1, c1
        return out

    def expand(self, mode="table"):
        if mode != "table":
            raise OfficeworkError("expand は 'table' だけに対応しています")
        r = _call("expand", **self._kw())
        return self._make(
            self._r0, self._c0,
            self._r0 + r["rows"] - 1, self._c0 + r["cols"] - 1,
        )

    def options(self, convert=None, **kw):
        return Options(self, convert=convert, **kw)

    def _get(self):
        return _call("get", **self._kw())["values"]

    def _plain(self, grid):
        # 1×1 はそのまま、1行/1列は1次元、他は2次元(xlwings と同じ)
        if len(grid) == 1 and len(grid[0]) == 1:
            return grid[0][0]
        if len(grid) == 1:
            return grid[0]
        if all(len(r) == 1 for r in grid):
            return [r[0] for r in grid]
        return grid

    @property
    def value(self):
        return self._plain(self._get())

    @value.setter
    def value(self, v):
        grid = _to_grid(v)
        _call("set", values=grid, **self._kw())

    @property
    def formula(self):
        f = _call("get_formula", **self._kw())["formulas"]
        return self._plain(f)

    @formula.setter
    def formula(self, v):
        # 式も set と同じ道(= から始まる文字列は式になる)
        self.value = v

    # ── xlwings 互換層(参照の算術。橋には出ない)────────────────

    @property
    def formula2(self):
        # xlwings では動的配列の式の別名。うちは式は1種類
        return self.formula

    @formula2.setter
    def formula2(self, v):
        self.formula = v

    @property
    def raw_value(self):
        # 変換(options)を通さない素の値。うちは value と同じ物
        return self._plain(self._get())

    @raw_value.setter
    def raw_value(self, v):
        self.value = v

    def get_value(self):
        # xlwings では「その場で取りに行く」手。中身は value と同じ
        return self.value

    @property
    def row(self):
        return self._r0 + 1

    @property
    def column(self):
        return self._c0 + 1

    @property
    def shape(self):
        return (self._r1 - self._r0 + 1, self._c1 - self._c0 + 1)

    @property
    def size(self):
        r, c = self.shape
        return r * c

    @property
    def count(self):
        return self.size

    def get_address(self, row_absolute=True, column_absolute=True,
                    include_sheetname=False, external=False):
        def one(r, c):
            return "{}{}{}{}".format(
                "$" if column_absolute else "", _col_name(c),
                "$" if row_absolute else "", r + 1,
            )

        a = one(self._r0, self._c0)
        if (self._r0, self._c0) != (self._r1, self._c1):
            a = "{}:{}".format(a, one(self._r1, self._c1))
        if include_sheetname or external:
            a = "{}!{}".format(self.sheet.name, a)
        if external:
            a = "[{}]{}".format(Book.attach().name, a)
        return a

    @property
    def address(self):
        return self.get_address()

    @property
    def sheet(self):
        if self._sheet is not None:
            return Sheet(self._sheet)
        info = _call("book_info")
        return Sheet(info["sheets"][info["active"]])

    @property
    def rows(self):
        return [self._make(r, self._c0, r, self._c1)
                for r in range(self._r0, self._r1 + 1)]

    @property
    def columns(self):
        return [self._make(self._r0, c, self._r1, c)
                for c in range(self._c0, self._c1 + 1)]

    def offset(self, row_offset=0, column_offset=0):
        return self._make(
            self._r0 + row_offset, self._c0 + column_offset,
            self._r1 + row_offset, self._c1 + column_offset,
        )

    def resize(self, row_size=None, column_size=None):
        r, c = self.shape
        if row_size is None:
            row_size = r
        if column_size is None:
            column_size = c
        return self._make(
            self._r0, self._c0,
            self._r0 + row_size - 1, self._c0 + column_size - 1,
        )

    @property
    def last_cell(self):
        return self._make(self._r1, self._c1, self._r1, self._c1)

    @property
    def current_region(self):
        # xlwings の current_region。うちは expand("table") と同族(台帳のとおり)
        return self.expand("table")

    # ── ここから下は橋(rpc)に出る ──────────────────────────────

    def clear(self):
        """中身も書式も消す(結合はそのまま — 解くのは unmerge)。"""
        _call("clear", **self._kw())

    def clear_contents(self):
        """値と式だけ消す(書式は据え置き)。"""
        _call("clear_contents", **self._kw())

    def merge(self, across=False):
        """結合する。across=True は xlwings と同じく行ごとに結合する。
        作法はアプリの結合と同じ(左上以外の中身は消える・空の左上へは
        最初の中身が書式ごと移る)。"""
        if across:
            for r in self.rows:
                _call("merge", **r._kw())
        else:
            _call("merge", **self._kw())

    def unmerge(self):
        """範囲に掛かる結合を全部解く(xlwings と同じ)。"""
        _call("unmerge", **self._kw())

    @property
    def merge_area(self):
        """左上のセルを含む結合の範囲。結合が無ければセル自身。"""
        kw = {"a1": "{}{}".format(_col_name(self._c0), self._r0 + 1)}
        if self._sheet is not None:
            kw["sheet"] = self._sheet
        return Range(_call("merge_area", **kw)["a1"], sheet=self._sheet)

    @property
    def merge_cells(self):
        """範囲に結合が掛かっているか(xlwings と同じ真偽)。"""
        kw = {}
        if self._sheet is not None:
            kw["sheet"] = self._sheet
        for a, b in _call("merges", **kw)["merges"]:
            r0, c0 = _parse_a1(a)
            r1, c1 = _parse_a1(b)
            if not (r1 < self._r0 or r0 > self._r1 or c1 < self._c0 or c0 > self._c1):
                return True
        return False

    def end(self, direction):
        """Ctrl+矢印相当。direction は "up" / "down" / "left" / "right"。
        端は使っている範囲まで(Excel の 1048576 行目には飛ばない)。"""
        d = str(direction).lower().lstrip("*")
        kw = {"a1": "{}{}".format(_col_name(self._c0), self._r0 + 1), "direction": d}
        if self._sheet is not None:
            kw["sheet"] = self._sheet
        return Range(_call("end", **kw)["a1"], sheet=self._sheet)

    def select(self):
        """画面の選択をこの範囲に動かして見せる。"""
        _call("select", **self._kw())

    @property
    def name(self):
        """この範囲をちょうど指す名前(xlwings と同じ)。無ければ None。"""
        me = self._a1()
        target_sheet = self.sheet.name
        for sheet, n, ref in _call("names")["names"]:
            if sheet == target_sheet and ref == me:
                return Name(n)
        return None

    @name.setter
    def name(self, value):
        kw = {"name": str(value), "a1": self._a1()}
        if self._sheet is not None:
            kw["sheet"] = self._sheet
        _call("define_name", **kw)

    # ── 書式(xlwings の形。合否は相手の定義どおり)──────────────

    @property
    def font(self):
        return _RangeFont(self)

    @property
    def color(self):
        """セルの塗り(背景)の色。xlwings と同じく (r, g, b) のタプル。
        塗りが無ければ None。読みは範囲の左上。"""
        d = _call("get_fmt", **self._kw())
        return _tuple_of(d["fill"]) if "fill" in d else None

    @color.setter
    def color(self, v):
        _call("set_fmt", fill=_hex_of(v), **self._kw())

    @property
    def number_format(self):
        return _call("get_fmt", **self._kw()).get("number_format", "General")

    @number_format.setter
    def number_format(self, v):
        _call("set_fmt",
              number_format=None if v in (None, "General") else v, **self._kw())

    @property
    def wrap_text(self):
        return bool(_call("get_fmt", **self._kw()).get("wrap"))

    @wrap_text.setter
    def wrap_text(self, v):
        _call("set_fmt", wrap=bool(v), **self._kw())

    def clear_formats(self):
        """範囲の書式を消す(値は残る)。"""
        _call("clear_formats", **self._kw())

    @property
    def column_width(self):
        """列幅(字数 — xlsx と同じ単位)。範囲の列がまちまちなら None。"""
        return _call("col_width", **self._kw())["value"]

    @column_width.setter
    def column_width(self, v):
        _call("col_width", value=float(v), **self._kw())

    @property
    def row_height(self):
        """行高(ポイント)。範囲の行がまちまちなら None。"""
        return _call("row_height", **self._kw())["value"]

    @row_height.setter
    def row_height(self, v):
        _call("row_height", value=float(v), **self._kw())

    def __len__(self):
        return self.size

    def __repr__(self):
        return "<officework.calc Range {}>".format(self._a1())


def _default_frame():
    # polars を第一に(無ければ pandas)— SEKKEI「Python 側の道具は polars を第一に」
    try:
        import polars as pl

        return pl.DataFrame
    except ImportError:
        import pandas as pd

        return pd.DataFrame


def _grid_to_frame(grid, convert, index=True, header=True):
    """セルの2次元(values)を DataFrame へ。**polars を第一に**、pandas は従来どおり。

    polars には index という物が無いので、polars のときは index は効かない
    (見出しの列もただの列として入る)。
    """
    if (getattr(convert, "__module__", "") or "").startswith("polars"):
        import polars as pl

        if not grid:
            return pl.DataFrame()
        if header:
            names = ["" if c is None else str(c) for c in grid[0]]
            return pl.DataFrame(grid[1:], schema=names, orient="row")
        return pl.DataFrame(grid, orient="row")
    import pandas as pd

    if not grid:
        return pd.DataFrame()
    if header:
        head, body = grid[0], grid[1:]
    else:
        head, body = None, grid
    df = pd.DataFrame(body, columns=head)
    if index and df.shape[1] >= 1:
        df = df.set_index(df.columns[0])
        # xlwings と同じく、見出しの欄が None なら index 名も無し
        if df.index.name is None or df.index.name == "":
            df.index.name = None
    return df


def _hex_of(v):
    # 色 → "RRGGBB"。(r, g, b) のタプル / "#RRGGBB" / "RRGGBB" を受ける
    if v is None:
        return None
    if isinstance(v, (tuple, list)) and len(v) == 3:
        return "%02X%02X%02X" % tuple(int(x) for x in v)
    s = str(v).lstrip("#")
    return (s[-6:] if len(s) == 8 else s).upper()


def _tuple_of(hex6):
    # "RRGGBB" → (r, g, b)。xlwings は色をタプルで見せる
    return tuple(int(hex6[i:i + 2], 16) for i in (0, 2, 4))


class _RangeFont:
    """Range.font の返り。xlwings と同じく、性質ごとに読み書きする
    (font.bold = True は太字だけを変える — openpyxl の「一式の置き換え」とは
    逆の作法。相手の定義に合わせてある)。"""

    __slots__ = ("_rng",)

    def __init__(self, rng):
        self._rng = rng

    def _get(self, key, default=None):
        return _call("get_fmt", **self._rng._kw()).get(key, default)

    def _set(self, **kw):
        kw.update(self._rng._kw())
        _call("set_fmt", **kw)

    @property
    def name(self):
        return self._get("font")

    @name.setter
    def name(self, v):
        self._set(font=v)

    @property
    def size(self):
        return self._get("size")

    @size.setter
    def size(self, v):
        self._set(size=None if v is None else float(v))

    @property
    def bold(self):
        return bool(self._get("bold"))

    @bold.setter
    def bold(self, v):
        self._set(bold=bool(v))

    @property
    def italic(self):
        return bool(self._get("italic"))

    @italic.setter
    def italic(self, v):
        self._set(italic=bool(v))

    @property
    def color(self):
        c = self._get("color")
        return _tuple_of(c) if c else None

    @color.setter
    def color(self, v):
        self._set(color=_hex_of(v))

    def __repr__(self):
        return "<officework.calc Font {}>".format(self._rng._a1())


def _to_grid(v):
    # DataFrame / ndarray / 2次元 / 1次元 / スカラー を 2次元へ
    name = type(v).__name__
    mod = type(v).__module__ or ""
    if mod.startswith("polars"):
        # polars には index が無い — 見出し1行+中身をそのまま
        if name == "DataFrame":
            return [[str(c) for c in v.columns]] + [
                [_scalar(x) for x in row] for row in v.rows()
            ]
        if name == "Series":
            return [[_scalar(x)] for x in v.to_list()]
    if name == "DataFrame":
        rows = [
            [v.index.name if v.index.name is not None else ""]
            + [str(c) for c in v.columns]
        ]
        for idx, row in zip(v.index, v.itertuples(index=False)):
            rows.append([_scalar(idx)] + [_scalar(x) for x in row])
        return rows
    if name == "Series":
        return [[_scalar(x)] for x in v]
    if name == "ndarray":
        v = v.tolist()
        name = type(v).__name__
    if isinstance(v, (list, tuple)):
        if v and isinstance(v[0], (list, tuple)):
            return [[_scalar(x) for x in row] for row in v]
        return [[_scalar(x) for x in v]]  # 1次元は横1行(xlwings と同じ)
    return [[_scalar(v)]]


def _scalar(x):
    if x is None:
        return None
    if isinstance(x, bool):
        return x
    if isinstance(x, (int, float)):
        # NaN は空欄に
        if x != x:
            return None
        return x
    if hasattr(x, "item"):  # numpy の数
        try:
            return x.item()
        except Exception:
            pass
    if hasattr(x, "isoformat"):  # 日付は文字で(v1 の割り切り)
        return x.isoformat()
    return str(x)


class _FreezePanes:
    """Sheet.freeze_panes の返り(xlwings と同じ形)。

    freeze_at には**固定する範囲そのもの**を渡す — "B2" なら上2行と左2列、
    "1:1" なら上1行だけ、"A:A" なら左1列だけが固定される。
    """

    def __init__(self, sheet):
        self._sheet = sheet

    def freeze_at(self, frozen_range):
        if isinstance(frozen_range, Range):
            if frozen_range._sheet not in (None, self._sheet.name):
                raise OfficeworkError("別のシートの範囲です")
            a1 = frozen_range._a1()
        else:
            a1 = str(frozen_range).replace("$", "")
        last = a1.split(":")[-1]
        if last.isalpha():  # "A:A" — 列だけ
            rows, cols = 0, _parse_a1(last + "1")[1] + 1
        elif last.isdigit():  # "1:1" — 行だけ
            rows, cols = int(last), 0
        else:  # "B2" / "A1:B2" — 右下までの行と列
            r, c = _parse_a1(last)
            rows, cols = r + 1, c + 1
        _call("freeze", sheet=self._sheet.name, rows=rows, cols=cols)

    def unfreeze(self):
        _call("freeze", sheet=self._sheet.name, rows=0, cols=0)

    def __repr__(self):
        r = _call("freeze", sheet=self._sheet.name)
        return "<officework.calc FreezePanes 行{} 列{}>".format(r["rows"], r["cols"])


class Sheet:
    def __init__(self, name):
        self.name = name

    def range(self, a1):
        return Range(a1, sheet=self.name)

    def __getitem__(self, a1):
        return self.range(a1)

    # ── xlwings 互換層 ──────────────────────────────────────────

    @property
    def book(self):
        return Book.attach()

    @property
    def index(self):
        # xlwings と同じ1起点
        return _call("book_info")["sheets"].index(self.name) + 1

    @property
    def cells(self):
        # 使っている所だけでなく、シートの全部のセル(xlwings と同じ定義)。
        # 読み書きするまでは参照の算術だけなので、大きさは害にならない
        return self.range("A1:XFD1048576")

    @property
    def used_range(self):
        # expand("table") が同じ役(台帳のとおり)
        return self.range("A1").expand("table")

    def clear(self):
        """シートの中身も書式も全部消す(結合はそのまま)。"""
        _call("clear", sheet=self.name)

    def clear_contents(self):
        """シートの値と式を全部消す(書式は据え置き)。"""
        _call("clear_contents", sheet=self.name)

    def clear_formats(self):
        """シートの書式を全部消す(値は残る)。"""
        _call("clear_formats", sheet=self.name)

    def activate(self):
        """画面のシートをこのシートに切り替える。"""
        _call("activate_sheet", sheet=self.name)

    @property
    def freeze_panes(self):
        """ウィンドウ枠の固定(xlwings と同じ形)。
        freeze_at("B2") で上2行・左2列、freeze_at("1:1") で上1行、
        unfreeze() で解除。保存で xlsx にも載る。"""
        return _FreezePanes(self)

    @property
    def visible(self):
        return _call("sheet_visible", sheet=self.name)["visible"]

    @visible.setter
    def visible(self, value):
        # 隠す作法はアプリの「非表示」と同じ — 最後の見えている1枚は断られる
        _call("sheet_visible", sheet=self.name, value=bool(value))

    def copy(self, name=None):
        """シートを複製する(アプリの「コピーを作成」と同じ作法 — 写しは
        自分の右隣に入り、画面はそこへ移る。名前は省略で「名前 (2)」)。
        返りは写しのシート。undo の束は消える(アプリの複製と同じ)。"""
        kw = {"sheet": self.name}
        if name is not None:
            kw["new_name"] = name
        return Sheet(_call("copy_sheet", **kw)["name"])

    def delete(self):
        """シートを削除する(最後の1枚は断られる。元に戻せない操作)。"""
        _call("delete_sheet", sheet=self.name)

    def select(self):
        # xlwings では activate と実質同じ(アプリは1つ)
        self.activate()

    def to_pdf(self, path):
        """このシートを PDF に(帳票の印刷設定に従う)。返りは保存先。
        効かせた設定はアプリの状態行と同じ文言で返事の note に載る。"""
        _call("to_pdf", path=os.path.abspath(path), sheet=self.name)
        return os.path.abspath(path)

    def load(self, convert=None, index=True, header=True):
        """使っている範囲を DataFrame に。**polars を第一に**
        (polars が無ければ pandas)。convert で選べる。"""
        rng = self.used_range
        return _grid_to_frame(rng._get(), convert or _default_frame(),
                              index=index, header=header)

    def __repr__(self):
        return "<officework.calc Sheet {}>".format(self.name)


class _Sheets:
    def __getitem__(self, key):
        info = _call("book_info")
        names = info["sheets"]
        if isinstance(key, int):
            return Sheet(names[key])
        if key in names:
            return Sheet(key)
        raise OfficeworkError("シート「{}」がありません".format(key))

    @property
    def active(self):
        info = _call("book_info")
        return Sheet(info["sheets"][info["active"]])

    def __iter__(self):
        return (Sheet(n) for n in _call("book_info")["sheets"])

    def __len__(self):
        return len(_call("book_info")["sheets"])


class Book:
    """動いている calc のブック。Book() = 新規(未保存が無ければ)、
    Book('path.xlsx') = そのファイルを開く。"""

    def __init__(self, path=None):
        if path is not None:
            info = _call("book_info")
            if info.get("path") != os.path.abspath(path):
                _call("open", path=os.path.abspath(path))
        else:
            _call("new")
        self.sheets = _Sheets()

    @staticmethod
    def attach():
        """新規も開くもせず、いま calc に出ているブックにそのまま付く。"""
        b = Book.__new__(Book)
        _call("ping")
        b.sheets = _Sheets()
        return b

    @staticmethod
    def caller():
        """呼び出し元のブック。calc が plugins の手続きを走らせているときも、
        Jupyter から触っているときも、**動いている calc のブック**を返す
        (officework には Excel のアドイン機構のような境目が無いので
        attach() と同じもの)。xlwings の書き方をそのまま持ち込めるように残す。"""
        return Book.attach()

    @property
    def name(self):
        p = _call("book_info").get("path")
        return os.path.basename(p) if p else "ブック1"

    @property
    def fullname(self):
        return _call("book_info").get("path")

    # ── xlwings 互換層 ──────────────────────────────────────────

    @property
    def sheet_names(self):
        return list(_call("book_info")["sheets"])

    @property
    def app(self):
        # App クラスは作らない(アプリは1つ、ブックも1つ — 選ぶ道具が要らない)。
        # wb.app.books のような書き方だけ通るように、小さな取っ手を返す
        return _app

    @property
    def selection(self):
        """いま画面で選んでいる範囲。「選んで、Jupyter で加工」の入り方。"""
        r = _call("selection")
        return Range(r["a1"], sheet=r["sheet"])

    @property
    def names(self):
        """名前付き範囲(xlwings の Names の役)。
        wb.names.add("単価", "=Sheet1!$A$1")・wb.names["単価"].refers_to。"""
        return _Names()

    def load(self, convert=None, index=True, header=True):
        """いま選んでいる範囲を DataFrame に(1マスだけなら表に広げる)。
        **polars を第一に**(polars が無ければ pandas)。"""
        rng = self.selection
        if rng.shape == (1, 1):
            rng = rng.expand("table")
        return _grid_to_frame(rng._get(), convert or _default_frame(),
                              index=index, header=header)

    def save(self, path=None):
        if path is not None:
            _call("save", path=os.path.abspath(path))
        else:
            _call("save")

    def __repr__(self):
        return "<officework.calc Book {}>".format(self.name)


class _Books:
    @property
    def active(self):
        return Book.attach()

    def open(self, path):
        return Book(path)

    def add(self):
        return Book()


books = _Books()


class Name:
    """名前付き範囲の1件(xlwings の Name の役)。refers_to は "=シート!$A$1"。"""

    __slots__ = ("name",)

    def __init__(self, name):
        self.name = name

    @property
    def refers_to(self):
        for sheet, n, ref in _call("names")["names"]:
            if n == self.name:
                def abs1(cell):
                    # "A1" → "$A$1"(xlwings と同じ絶対参照の形)
                    i = 0
                    while i < len(cell) and cell[i].isalpha():
                        i += 1
                    return "$" + cell[:i] + "$" + cell[i:]
                return "={}!{}".format(
                    sheet, ":".join(abs1(p) for p in ref.split(":")))
        raise OfficeworkError("名前「{}」がありません".format(self.name))

    @refers_to.setter
    def refers_to(self, v):
        sheet, a1 = _parse_refers(v)
        kw = {"name": self.name, "a1": a1}
        if sheet:
            kw["sheet"] = sheet
        _call("define_name", **kw)

    def delete(self):
        _call("delete_name", name=self.name)

    def __repr__(self):
        return "<officework.calc Name {}>".format(self.name)


def _parse_refers(v):
    # "=Sheet!$A$1:$B$2" / "Sheet!A1" / "A1" → (シート名か None, "A1:B2")
    s = str(v).lstrip("=").replace("$", "")
    if "!" in s:
        sheet, ref = s.rsplit("!", 1)
        return sheet.strip("'"), ref
    return None, s


class _Names:
    """Book.names の返り(xlwings の Names の役)。"""

    def add(self, name, refers_to):
        sheet, a1 = _parse_refers(refers_to)
        kw = {"name": name, "a1": a1}
        if sheet:
            kw["sheet"] = sheet
        _call("define_name", **kw)
        return Name(name)

    def _list(self):
        return [n for _, n, _ in _call("names")["names"]]

    def __getitem__(self, key):
        names = self._list()
        if isinstance(key, int):
            return Name(names[key])
        if key in names:
            return Name(key)
        raise OfficeworkError("名前「{}」がありません".format(key))

    def __contains__(self, key):
        return key in self._list()

    def __iter__(self):
        return (Name(n) for n in self._list())

    def __len__(self):
        return len(self._list())

    def __repr__(self):
        return "<officework.calc Names {}>".format(self._list())


class _App:
    """Book.app の返り。App クラスは作らない(台帳)ので、名前は出さない。"""

    _status_bar = None

    @property
    def books(self):
        return books

    def calculate(self):
        """全再計算(xlwings の App.calculate)。"""
        _call("calculate")

    @property
    def version(self):
        return _call("ping").get("version", "")

    @property
    def selection(self):
        return Book.attach().selection

    @property
    def status_bar(self):
        # アプリの状態行は読み戻せない — こちらから出した最後の文言を覚えて返す
        return self._status_bar

    @status_bar.setter
    def status_bar(self, text):
        _call("status", text=str(text))
        self._status_bar = str(text)

    def __repr__(self):
        return "<officework.calc app>"


_app = _App()


def load(convert=None, index=True, header=True):
    """いま選んでいる範囲を DataFrame に(xlwings の xw.load と同じ入り方)。"""
    return Book.attach().load(convert=convert, index=index, header=header)


def ping():
    """calc が応じるかの確かめ。"""
    return _call("ping")
