# officework

An **xlsx engine that does not destroy your forms**, plus a bridge that drives a
running office app from Python — the way `xlwings` drives Excel, but on your own
machine and without Excel.

Written in Rust (15,000+ lines, 240+ tests), exposed to Python through PyO3.

日本語は下にあります。

## Two ways in

```python
import office_sheet                      # the engine — no app needed
b = office_sheet.Book.open("form7.xlsx")
s = b["quote"]
s["A30"] = "Nihon Funen Co., Ltd."       # borders, merges, widths stay intact
s["C30"] = "=B30*100"                    # a formula; recalculated on the spot
s.insert_row(30)                         # remaining formulas follow the move
b.save("out.xlsx")                       # shapes and print setup carried over
```

```python
from officework import calc as xw        # the bridge — drives the running app
import pandas as pd

wb = xw.Book()                           # a blank workbook comes up
wb.sheets.active["A1"].value = df        # the DataFrame lands in the sheet
df2 = wb.sheets.active["A1"].options(pd.DataFrame, expand="table").value
```

The bridge talks over a unix socket on **this machine only** — no TCP is opened.
It needs [officework](https://github.com/aiseed-dev/officework) running.

## Why the engine exists

`openpyxl` rewrites the parts of the file it does not understand. For a
spreadsheet used as a *printed form* — the way most Japanese offices use one —
that means the borders, merged cells, column widths and shapes you spent an
afternoon on come back wrong.

This engine keeps the original as the source of truth and writes back only what
changed. `b.unsupported` lists anything it could not read, so nothing is dropped
in silence.

Measured on one machine, 1096 rows × 20 columns (21,920 cells):

| | |
|---|---|
| DataFrame → sheet | 44 ms |
| sheet → DataFrame | 65 ms |

## Install

```
pip install officework
```

Wheels are abi3 (Python 3.10+), so one wheel per platform covers every version.
`pandas` is only imported when you ask for it (`pip install officework[pandas]`).

## License

**AGPL-3.0-or-later.**

Using it inside your company — building forms, running ledgers, writing
scripts — carries **no obligations at all**. Obligations appear only if you
ship something built on it to third parties, or offer a modified version as a
network service.

---

## 日本語

**帳票を壊さない xlsx エンジン**と、**動いているオフィスソフトを Python から
操る橋**です。`xlwings` の使い勝手を、Excel なしで、手元だけで。

`openpyxl` は理解できない部品を書き直すので、罫線・結合・列幅・図形が戻って
きません。日本の事務の表計算は「印刷する様式」なので、そこが壊れると使えない。
このエンジンは**原本を正として、変えた所だけ書き戻します**。読めなかった物は
`b.unsupported` に出るので、黙って落ちることはありません。

```python
import office_sheet
b = office_sheet.Book.open("様式7.xlsx")
b["提案見積書"]["A30"] = "日本フネン株式会社"   # 書式は据え置き
b.save("out.xlsx")
```

ライセンスは **AGPL-3.0-or-later**。**社内で使う分に義務はありません**
(帳票を作る・台帳を回す・スクリプトを書く・社内に配る、いずれも自由)。
義務が出るのは、これを組み込んだ物を社外へ配るときと、改造版をネットワーク
越しの役務として外部に提供するときだけです。
