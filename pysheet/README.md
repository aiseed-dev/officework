# officework

**xlsx and docx engines built not to destroy your forms — and that print.**
Read a spreadsheet or a document, change it, write back only what you changed,
and turn it into a PDF. No office suite, no headless browser, no print driver.

Written in Rust (about 88,000 lines across the crates in this wheel, 1,158 tests), exposed to Python through PyO3.

日本語の説明は GitHub にあります (Japanese documentation on GitHub):
[Python の手引き](https://github.com/aiseed-dev/officework/blob/main/docs/ja/python-manual.adoc)

## Install

```console
$ pip install officework
```

**0.5.0 is out, and plenty in it is still broken.** The
[release notes](https://github.com/aiseed-dev/officework/blob/main/RELEASE.adoc)
list the gaps we know about. Please report what breaks at
[the issues](https://github.com/aiseed-dev/officework/issues).

Wheels are abi3 (CPython 3.10+), so one wheel per platform covers every
version; Linux, macOS and Windows are published. The wheel is **just the
engines** — no GUI, and nothing to install alongside: the equation
typesetter is inside it, so there is no TeX to set up. That makes it a
30 MB download, 78 MB installed. `pandas` is imported only if you ask for
it (`pip install officework[pandas]`).

## Spreadsheets

```python
from officework import sheet
b = sheet.Book.open("form7.xlsx")
s = b["quote"]
s.insert_row(30)                         # make room; the formulas below follow the move
s["A30"] = "Sample Trading Co., Ltd."    # only the value is rewritten
s["C30"] = "=B30*100"                    # a formula; recalculated on the spot
print(s["C30"].value)                    # the computed value, as in openpyxl
b.save("out.xlsx")                       # the xlsx to send on
b.save("quote.pdf")                      # the same sheet, straight to paper
```

Indexing follows openpyxl exactly: `ws["A1"]` is a **cell**, `ws["A1:C3"]`
a tuple of rows, `ws[1]` a row, `ws["A"]` a column. Reading an untouched
address gives you a cell whose `.value` is `None`, not `None` itself — so
writing into a merged region's top-left works the way the articles show.

## Documents

```python
from officework import doc
d = doc.Doc.open("report.docx")
d.replace("Old Name Ltd.", "New Name Ltd.")   # replaces the text, run by run
d.fill("customer", "Sample Trading K.K.")     # fill a named form field
d.save("out.docx")                       # the docx to send on
d.save("report.pdf")                     # the same document, typeset for paper
```

## PDF and PNG — the part the others cannot do

`save()` looks at the extension. The same book or document you just edited
becomes a PDF, laid out by the same typesetting engine that drives the desktop
app, so the paper matches the screen:

```python
b.save("quote.pdf")                      # the sheet, paginated, repeating header rows
d.save("report.pdf")                     # the document, typeset
```

`.png` gives you the same page as an image — for a thumbnail, a preview in a web
page, or a picture to drop into a chat message:

```python
b.save("quote.png")                      # 150 dpi by default; A4 comes out 1240x1754
d.save("report.png", dpi=300)            # print resolution
```

One file per page. The first page keeps the name you gave it, and later pages
get `-2`, `-3` appended, so a three-page document writes `report.png`,
`report-2.png` and `report-3.png`. Both formats come off the same laid-out page,
so the image and the PDF agree.

This runs on a server with nothing else installed. There is no LibreOffice to
launch, no Chromium to drive, no `wkhtmltopdf`, no temporary HTML. It is one
library call, and it is fast enough to sit inside a request handler.

Fonts are subsetted, so only the glyphs you used are embedded. A Japanese page
comes out under 30 KB — measured here at 8 KB for plain text and 25 KB for a
page with a table, colour and shading — where embedding a whole CJK font costs
20 MB. Line breaking follows JIS X 4051, so Japanese text does not break before
a closing bracket or after an opening one.

Neither `openpyxl` nor `python-docx` can produce a PDF at all. The commercial
libraries that can are priced accordingly.

### Shapes in a document

`python-docx` can only place pictures. This engine can place real shapes —
rectangles, rounded rectangles, ellipses, arrows, diamonds and lines — pinned
to the page, with fill, outline, text inside, rotation, opacity and a drop
shadow:

```python
d.add_shape("roundRect", 25, 80, 40, 25, fill="DDE7F0", line="2E5A87",
            text="Approved", shadow=True)
```

Coordinates and sizes are millimetres from the top-left of the page. They are
written as DrawingML, so Word and LibreOffice open them as shapes you can
select and edit — not as a flattened picture. They also come out in the PDF,
they are read back when you reopen the file, and `page=` puts a shape on any
page, not just the first.

### Charts

Charts are **drawn by this library**, as shapes, rather than written as an
instruction for Excel to render later. So they appear in the PDF, not only
after someone opens the file in Excel:

```python
ws.add_chart("bar", data="B3:C8", categories="A4:A8", at="A10",
             title="Target and actual")
```

All eleven chart types `openpyxl` can write are built in: bar, line, area, pie,
doughnut, projected pie, radar, scatter, bubble, stock (high-low-close) and
surface. For finer control there is a small
chart layer whose shape is borrowed from d3 — build a scale, then place marks
through it:

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

### Equations

`officework.tex` takes LaTeX and returns SVG or PNG. With TeX installed it
typesets there (matrix columns align); without it, matplotlib's mathtext does
the job; with neither, it refuses with the reason — never a silent empty
picture.

## Your old vocabulary still works

Code written for openpyxl or python-docx largely runs as-is:

```python
ws = wb.active                          # openpyxl: cell(), append, iter_rows,
ws.cell(2, 3).value                     #   dimensions, create_sheet,
ws.append(["Aug", "pens", 5000])        #   copy_worksheet, freeze_panes …
d.tables[0].cell(0, 1).text             # python-docx: row_cells, columns,
d[3].runs[0].font.name                  #   runs, clear …
```

What is the same as Word and Excel, and what is deliberately not, is set
out in
[How this differs from docx and xlsx](https://github.com/aiseed-dev/officework/blob/main/docs/en/docx-xlsx-tono-chigai.adoc).
Interop is proven with the originals' own eyes: openpyxl reads what this
engine writes, **including the computed values** it cannot produce itself.
See the [Python manual](https://github.com/aiseed-dev/officework/blob/main/docs/en/python-manual.adoc)
for the details and the deliberate differences.

## Why the engines exist

`openpyxl` and `python-docx` rewrite the parts of the file they do not
understand. For a document used as a *printed form* — the way most Japanese
offices use one — that means the borders, merged cells, column widths, shapes,
styles and headers you spent an afternoon on come back wrong.

When these engines open an xlsx or docx and write it back, they rewrite only
what changed. `b.unsupported` / `d.unsupported` list what they recognised as
unreadable, as far as they can tell.

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
