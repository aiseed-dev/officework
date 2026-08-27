# officework

**xlsx and docx engines that do not destroy your forms**, plus a bridge that
drives a running office app from Python — the way `xlwings` drives Excel, but on
your own machine and without Excel.

Written in Rust (15,000+ lines, 240+ tests), exposed to Python through PyO3.

日本語の説明は GitHub にあります (Japanese documentation on GitHub):
[Python の手引き](https://github.com/aiseed-dev/officework/blob/main/docs/ja/python-manual.adoc)

## Install

```console
$ pip install officework
```

**0.5.0 is in beta.** It is published as `0.5.0b1`, so the line above still
gives you 0.4.0. To try the beta:

```console
$ pip install --pre officework
```

Wheels are abi3 (CPython 3.10+), so one wheel per platform covers every
version; Linux, macOS and Windows are published. The wheel is **just the
engines and the bridge** — a few MB, no GUI. `pandas` is imported only if you
ask for it (`pip install officework[pandas]`).

## Three ways in

```python
from officework import sheet             # the engine — no app needed
b = sheet.Book.open("form7.xlsx")
s = b["quote"]
s["A30"] = "Nihon Funen Co., Ltd."       # borders, merges, widths stay intact
s["C30"] = "=B30*100"                    # a formula; recalculated on the spot
s.insert_row(30)                         # remaining formulas follow the move
b.save("out.xlsx")                       # shapes and print setup carried over
```

```python
from officework import doc               # the engine — docx, no app needed
d = doc.Doc.open("report.docx")
print(d.unsupported)                     # anything it could not read, never dropped in silence
d.replace("Old Name Ltd.", "New Name Ltd.")   # per-run formatting is left alone
d[3].text = "replaced"                   # the paragraph stays a heading, stays aligned
print(d.tables[0][1][2].text)            # table, row, cell
d.save("out.docx")                       # styles, headers, shapes, tracked changes carried over
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

## Let an AI drive the app (MCP)

```console
$ pip install "officework[mcp]"
```

That installs `officework-mcp`, an MCP server speaking over stdin/stdout.
Register it with an MCP client (Claude Code, Claude Desktop, …) and the
assistant can read and write the workbook you have open — the same bridge the
Python API uses, so the same rules apply: your machine only, no TCP.

```jsonc
// claude_desktop_config.json
{ "mcpServers": { "officework": { "command": "officework-mcp" } } }
```

The tools it exposes are deliberately few: `book_info`, `used_range`,
`read_range`, `read_formulas`, `write_range`, `set_format`, `autofit`, `save`.
Reading a range gives values; `read_formulas` gives the formulas behind them.

## The app is a separate download

These engines are what **aiseed office** — a spreadsheet and a word processor
with a window — uses whenever it meets the Microsoft formats. The app is
downloaded on its own (`.deb`, `.tar.gz`, `.dmg`, `setup.exe`, Flatpak); this
wheel does not carry it. Nothing here needs it: the engines are complete
without a screen.

If the app is installed, `officework` starts it, and the bridge above drives it:

```console
$ officework report.xlsx        # opens it in aiseed office
```

Spreadsheets and documents open as tabs of one window. Passing a second file
adds a tab rather than opening another window. To point at a build of your own:

```console
$ OFFICEWORK_OFFICEWORK=/path/to/officework officework report.xlsx
```

## Your old vocabulary still works

Code written for openpyxl, xlwings or python-docx largely runs as-is:

```python
ws = wb.active                          # openpyxl: cell(), append, iter_rows,
ws.cell(2, 3).value                     #   dimensions, create_sheet,
ws.append(["Aug", "pens", 5000])        #   copy_worksheet, freeze_panes …
xw.Range("B2").offset(1, 2).address     # xlwings: '$D$3' — resize,
xw.Range("A1").current_region           #   last_cell, current_region …
d.tables[0].cell(0, 1).text             # python-docx: row_cells, columns,
d[3].runs[0].font.name                  #   runs, clear …
```

The inventory — all 324 core members of the three libraries, judged one
by one — is in the repo:
[docs/pysheet-gokan.ja.adoc](https://github.com/aiseed-dev/officework/blob/main/docs/pysheet-gokan.ja.adoc).
Interop is proven with the originals' own eyes: openpyxl reads what this
engine writes, **including the computed values** it cannot produce itself.
See the [Python manual](https://github.com/aiseed-dev/officework/blob/main/docs/en/python-manual.adoc)
for the details and the deliberate differences.

Since 0.3.0 the wheel also typesets **equations**: `officework.tex` takes
LaTeX and returns SVG or PNG. With TeX installed it typesets there (matrix
columns align); without it, matplotlib's mathtext does the job; with
neither, it refuses with the reason — never a silent empty picture.

New in 0.5.0, the engines **print**. `save()` looks at the extension, so a
workbook or a document becomes a PDF without an app, an office suite or a
print driver:

```python
b.save("quote.pdf")                      # the sheet, paginated, with headers
d.save("report.pdf")                     # the document, typeset
```

The fonts are subsetted, so a Japanese page is around 25 KB rather than the
20 MB a whole CJK font would cost. Neither `openpyxl` nor `python-docx` can
do this at all.

Charts are drawn the same way — as shapes, by this library, not as an
instruction for Excel to render later. So they appear in the PDF and on the
screen too, not only after you open the file in Excel:

```python
ws.add_chart("bar", data="B3:C8", categories="A4:A8", at="A10",
             title="Target and actual")
```

For finer control there is a small chart layer whose shape is borrowed from
d3 — build a scale, then place marks through it:

```python
from officework import chart
c = chart.Chart(340, 180, title="Attainment")
x = c.band(branches)
y = c.linear([0, 150])
c.axis_left(y, fmt=lambda v: f"{int(v)}%")
c.bars(x, y, rates, color="70AD47", labels=True)
c.place(ws, "A20")
```

What you give up is a live Excel chart: ours is fixed at the data it was drawn
from. Redraw it to update it.

New in 0.4.0, the bridge reaches the rest of a cell's formatting —
`align`, `valign`, `indent`, `rotation`, `shrink`, `locked`,
`underline`, `strike`, `superscript`, `subscript` — plus page setup and the
table-design commands, so a macro can finish a form rather than only fill it.

## Why the engines exist

`openpyxl` and `python-docx` rewrite the parts of the file they do not
understand. For a document used as a *printed form* — the way most Japanese
offices use one — that means the borders, merged cells, column widths, shapes,
styles and headers you spent an afternoon on come back wrong.

These engines keep the original as the source of truth and write back only what
changed. `b.unsupported` / `d.unsupported` list anything they could not read, so
nothing is dropped in silence.

The docx side is checked against an independent reader (genoffice's TypeScript
docx engine) over 51 real documents, 43 of which this project did not write:
46 survive an open-and-save untouched, and no document loses a single part of
its zip. The rest — footnote marks, second and later section breaks, equations —
are listed in `d.unsupported` rather than dropped quietly.

Measured on one machine, 1096 rows × 20 columns (21,920 cells):

| | |
|---|---|
| DataFrame → sheet | 44 ms |
| sheet → DataFrame | 65 ms |

## License

**AGPL-3.0-or-later.**

Using it inside your company — building forms, running ledgers, writing
scripts — carries **no obligations at all**. Obligations appear only if you
ship something built on it to third parties, or offer a modified version as a
network service.
