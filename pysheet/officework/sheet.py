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
        return self.parent[self.coordinate]

    @value.setter
    def value(self, v):
        self.parent[self.coordinate] = v

    @property
    def data_type(self):
        # openpyxl と同じ札: 'f' 式・'s' 文字・'b' 真偽・'n' 数と空
        if self.parent.formula(self.coordinate) is not None:
            return "f"
        v = self.parent[self.coordinate]
        if isinstance(v, bool):
            return "b"
        if isinstance(v, str):
            return "s"
        return "n"

    def offset(self, row=0, column=0):
        return Cell(self.parent, self.row + row, self.column + column)

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

    # ── うちの口(エンジンそのまま)──────────────────────────────

    @property
    def name(self):
        return self._s.name

    def __getitem__(self, key):
        return self._s[key]

    def __setitem__(self, key, value):
        self._s[key] = value

    def formula(self, key):
        return self._s.formula(key)

    def display(self, key):
        return self._s.display(key)

    @property
    def shape(self):
        return self._s.shape

    def values(self):
        return self._s.values()

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

    def append(self, iterable):
        """使われている範囲の次の行に1行置く(openpyxl と同じ定義)。

        list/tuple なら A から順に、dict なら {列番号か列の字: 値} で。
        空のシートでは1行目に入る。
        """
        rows, _ = self._s.shape
        at = rows + 1  # 空なら shape=(0,0) で1行目
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

    def __repr__(self):
        return '<officework.sheet.Sheet "{}">'.format(self.name)


class Book:
    """1冊のブック。エンジンの Book を包み、openpyxl の Workbook の口を足す。"""

    def __init__(self):
        self._b = _engine.Book()
        self._path = None

    @staticmethod
    def open(path):
        b = Book.__new__(Book)
        b._b = _engine.Book.open(path)
        b._path = path
        return b

    # ── うちの口(エンジンそのまま)──────────────────────────────

    def save(self, path):
        self._b.save(path)

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

    def index(self, worksheet):
        return self._b.sheet_names.index(worksheet.title)

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


__all__ = ["Book", "Sheet", "Cell"]
