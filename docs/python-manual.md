# Python manual — arrays and the API

*日本語版(secondary): [python-manual.ja.md](python-manual.ja.md)*

For the buttons, see the [calc](calc-manual.md) / [writer](writer-manual.md)
manuals. This is **the one document for people writing code** — in particular
the range ⇄ array exchange, which is invisible from the UI, is specified here.
Everything was measured on a real machine.

## Code is a file, data is a file — never mixed

**What gets exchanged is data only** (settled 2026-08-09). There is no
mechanism, xlsm-style, for putting data and program in one file. Both cell
functions and procedures live in `~/.config/office/plugins/*.py`, and **a
workbook you receive contains no code at all**.

- You receive a sheet (data). **You do the processing with your own code**
- So `xl/joPython.xml` (workbook-borne code) is **gone**. Code in an old
  workbook is shown but never run, and disappears on save (the open report
  says so; `@export name` extracts it to a .py)
- Since the code is only ever your own, **no sandbox (bubblewrap) is applied**,
  and the `@name net` distinction is gone
- Decorators (`@xw.func`, `@xw.ret`) are unnecessary too — **write a plain
  `def`**

## Where Python runs, and what is bound

| Place | How you write it | Bindings |
|---|---|---|
| Cell function (UDF) | `=double(A1)` — calls `def double(x)` in a plugin | arguments passed as values (below) |
| Procedure | `@module` / `@module.func` | call `xw.Book.caller()` yourself |
| Outside (Jupyter, …) | `from officework import calc as xw` | same |
| calc: Data > Python (one-liner / .py) | typed into the panel | `b` = workbook, `s` = current sheet |
| calc / writer: macros, plugins | — | calc: `b`/`s`; **writer: `d` = python-docx Document** |
| writer: in-page Python (HTML) | — | `form` = dict of field name → value |

**Procedures and outside code drive the running calc directly** — not a
temporary copy (this is the article's "a library that drives Excel itself,
not files"). However many cells a procedure writes, **one Ctrl+Z** returns you
to the state before it ran.

The Data > Python one-liner and the writer side still run on a copy (a failure
leaves the sheet/document unharmed; a success lands as one undo step).

## The `officework.sheet` API

The engine ships on PyPI as **`officework`**; the xlsx engine is the `sheet`
submodule. No app is needed to use it.

```console
$ pip install officework
```

```python
from officework import sheet            # inside calc it's pre-imported; b and s arrive bound
b = sheet.Book.open("form.xlsx")
s = b["SheetName"]                      # or by index: b[0]
b.sheet_names                           # ['見積書', …]
b.add_sheet("NewSheet")                 # error if the name exists
b.recalc()                              # recalculate before reading values
b.save("out.xlsx")                      # original parts preserved
b.unsupported                           # list of parts we couldn't read (empty = everything read)
```

### Reading and writing cells

```python
s["A1"]            # read: numbers are float, text is str, ☑/☐ is bool, formula cells give the computed value
s.formula("E2")    # the formula itself ("=SUM(B2:D2)"; None if not a formula)
s.display("E2")    # display string ("238" — through the number format)
s["A1"] = 100      # write: number
s["A1"] = "text"   #        text
s["A1"] = True     #        bool (shows as ☑/☐ in calc)
s["A1"] = "=B1*C1" #        formula (string starting with "=")
s["A1"] = date(2026, 8, 5)  # datetime.date/datetime/time → Excel serial number
s["A1"] = None     #        clear
```

- **Formatting is preserved** — writing a value never touches borders, merges,
  or number formats
- Empty cells read back as **None or ""** (never-touched cells are None; cells
  where an empty string was stored are "". Both are falsy, so `if s["A1"]:`
  usually suffices; to be precise use `s["A1"] in (None, "")`)

### Ranges as arrays — the main topic

**There is no range subscript** (`s["A2:C3"]` raises) and **no 2-D bulk
assignment** (`s["A1"] = [[…]]` raises). Arrays work like this:

```python
# read: values() is the whole used area as a 2-D list (rows × columns, 0-based)
rows, cols = s.shape          # (10, 6) — shape is a property (no parentheses)
v = s.values()                # v[0] = first row (headings), v[1][1] = value of B2
tbl = [r[0:3] for r in v[1:6]]   # cut out A2:C6

# write: loop cell by cell (row numbers are 1-based in A1 notation!)
data = [["pen", 10, 150], ["notebook", 5, 180]]
for i, row in enumerate(data):
    n = 2 + i                              # starting at row 2
    s[f"A{n}"], s[f"B{n}"], s[f"C{n}"] = row
    s[f"D{n}"] = f"=B{n}*C{n}"             # formulas are strings too
b.recalc()
```

### Round-tripping with polars

```python
import polars as pl
# sheet → DataFrame (first row as headings)
v = s.values()
df = pl.DataFrame({h: [r[i] for r in v[1:]] for i, h in enumerate(v[0])})

# DataFrame → sheet (below the headings)
for i, row in enumerate(df.rows()):
    for j, val in enumerate(row):
        s[f"{chr(65 + j)}{2 + i}"] = val
```

Aggregation, joins, and filtering belong on the polars side — that's the
division of labor (the sheet is the form; computation is Python's job).

## The `officework.doc` API

Same wheel, same promise, for docx: the `doc` submodule. `Doc.open` keeps the
original bytes and `save` writes back only what changed, so styles, headers,
footers, shapes and tracked changes come through untouched — the thing
`python-docx` cannot promise.

```python
from officework import doc

d = doc.Doc.open("report.docx")
d.unsupported          # [(what, how many)] — anything it could not read. Look here first
d.paragraphs           # body paragraphs (paragraphs inside tables are not in this list)
d[3]                   # the 4th body paragraph; d[-1] works too
len(d)                 # how many body paragraphs
d.text                 # the body as one string, paragraphs joined with "\n"
d.header, d.footer     # read-only. Page numbers read as "#", page counts as "##"
d.tables               # tables, in document order
d.add_paragraph("...") # append to the body
d.save("out.docx")
```

**Read `d.unsupported` before you trust anything else.** An empty list means
everything was read. A non-empty one names what was not — and those parts are
still carried over from the original on save; "we could not read it" and "we
dropped it" are different statements.

### Paragraphs

```python
p = d[3]
p.text = "replacement"    # replace the text; the paragraph keeps its style and alignment
p.text                    # runs joined
p.replace("old", "new")   # -> how many were replaced. Keeps every run boundary
p.runs                    # [Run], readable *and* writable: .text .bold .italic
                          #   .underline .strike .color .size_pt .font .style .hyperlink
p.runs[0].bold = True     # formatting is set per run (since 2026-08-12)
p.add_run("more")         # append a run (inherits the last run's formatting)
p.style                   # "body", "heading1".."heading9", "toc1".., "tof"
p.align                   # "left" | "center" | "right" | "justify" | "distribute"
p.in_table                # True if this paragraph lives in a table cell
```

**Two ways to change text, and they are not the same tool.**

`p.text = "..."` replaces the whole paragraph and gives the new text the *first
run's* formatting. That is the same rule the writer app uses when you edit a
table cell, so Python and the app agree. But it is a blunt instrument: in a
paragraph reading `Bill to: ` in plain type and `ACME Ltd.` in bold, assigning
`.text` makes all of it plain.

`p.replace(old, new)` — or `d.replace(...)` for the whole document — edits inside
the runs, so every formatting boundary survives. It also finds text split across
runs, which matters because Word routinely splits a word into several runs for no
visible reason. **For mail merge into a form, use `replace`.**

```python
d.find("Old Name Ltd.")            # paragraphs containing it, body and table cells alike
d.replace("Old Name Ltd.", "New Name Ltd.")   # -> count
```

### Filling by name (form fields)

Surer than hunting for text: **named form fields** (content controls —
placed in writer via Insert > Form field, named via "Name the field";
the name is the docx `w:tag`). They resolve the same way in the body and
inside table cells:

```python
d.fields()                  # [(name, current value)]
d.fill("customer", "Nihon Funen K.K.")  # writes every field of that name -> count
                            # (0 means no such field — never a silent success)
d.extract("customer")       # value of the first one, or None
```

The field keeps its own formatting (a bold field stays bold). These are
**the same words as the writer macros' `fill` / `extract` / `fields`**, so
what you learned there carries over. All measured on this machine.

### Tables

```python
t = d.tables[0]
t.shape                # (rows, widest row's columns)
len(t), t.rows         # rows
t[1][2]                # table, row, cell
t[1][2].text = "..."   # newlines split the cell into paragraphs
t.values()             # list[list[str]] — hand it straight to polars
t[1][2].paragraphs     # the cell's paragraphs, as Paragraph objects
```

### What it does not do — and what is reported but never lost

Body reading covers paragraphs and tables (sections are separate under
`d.sections`, comments under `d.comments`, inline pictures under
`d.inline_shapes`).

Footnotes and equations are **listed in `d.unsupported` yet survive save** —
the report's own wording says so ("kept on save"). Read `unsupported` as the
ledger of what could not be *read in full*, not of what was thrown away.

Existing equations do not appear in the body text (they are carried through
verbatim). **To write a new equation, use `officework.tex`** (section below) —
it takes LaTeX, typesets a picture, and stores the source alongside it.

## Writing in vocabularies you already know — openpyxl, xlwings, python-docx

You don't have to throw away your existing code or the vocabulary in your
head (2026-08-12). The policy is **copy the API and the tests, never the
implementation** (docs/sekkei/python.ja.md); the inventory is the ledger
[docs/pysheet-gokan.ja.md](pysheet-gokan.ja.md) — all 324 core members of
the three libraries, judged one by one (what works, what we will build,
what we won't and why). Everything below works today and was measured on
this machine.

And the two things the originals can't give you — untouched formatting and
recalculation — come along whichever vocabulary you write in.

### The openpyxl vocabulary (officework.sheet)

```python
from officework import sheet
wb = sheet.Book.open("売上台帳.xlsx")
ws = wb.active                        # first sheet
ws.title, ws.max_row, ws.max_column   # ('売上台帳', 37, 6)
ws.dimensions                         # 'A1:F37'
ws.cell(2, 3).value                   # 'ボールペン(黒)' (row/column are 1-based)
ws.append(["8月", "筆記具", "万年筆", 1, 5000, 5000])   # one row at the end
for row in ws.iter_rows(min_row=2, max_row=3, values_only=True):
    print(row)                        # ('4月', '筆記具', 'ボールペン(黒)', 12.0, …)
ws2 = wb.create_sheet()               # names itself (Sheet, Sheet1, …)
wb.copy_worksheet(ws)                 # duplicate — contents, formats, merges, widths
wb.remove(ws2)                        # the last sheet can't be removed (it says so)
wb.save("out.xlsx")
```

- `ws.cell(50, 1)` returns a **reference-only Cell** even where nothing
  exists yet (value is None; assign and it lands there — the openpyxl feel)
- `insert_rows` / `delete_rows` / `insert_cols` / `delete_cols` take
  `amount=`; `merged_cell_ranges` and `freeze_panes` work too
- **our own idiom stays alive**: `ws["A1"]` still returns the **value**
  (openpyxl returns a Cell — that is the one deliberate difference;
  use `cell()` when you want a Cell)
- interop is proven with the original's own eyes: openpyxl reads what we
  write — **including the computed values** it cannot produce itself

Formatting and print setup also speak openpyxl (the 324-item ledger was
closed on 2026-08-12/13):

```python
from officework.sheet import Font, Border, Side, PatternFill, Alignment
ws.cell(1, 1).font = Font(bold=True, size=14) # openpyxl's own objects work too
                                              # (ws["A1"] returns the value — use cell())
ws.column_dimensions["A"].width = 20          # width (chars), height, hidden
ws.print_title_rows = "1:1"                   # repeat headings on every page ("A:A" for columns)
ws.freeze_panes = "B2"
ws.add_table(...)                             # tables — =SUM(Items[Amount]) computes
wb.add_named_style(...)                       # named cell styles are carried too
```

Data validation, defined names, outline groups, pictures (`add_image`),
headers/footers (down to odd/even/first pages), the 1904 epoch, and
`move_range` (references follow the move) all work the same way. **The
canonical list of what exists is the ledger**
([pysheet-gokan.ja.md](pysheet-gokan.ja.md) — 324 items, each with a verdict
and a reason).

### The xlwings vocabulary (officework.calc — a running calc)

Reference arithmetic is in. **The arithmetic works even without a
connection** (measured):

```python
from officework import calc as xw
xw.Range("B2").address                    # '$B$2'
xw.Range("B2").offset(1, 2).address       # '$D$3'
xw.Range("B2").resize(3, 2).address       # '$B$2:$C$4'
xw.Range("B2:D5").last_cell.address       # '$D$5'
xw.Range("A1").current_region             # the contiguous table (kin of expand)

b = xw.Book.attach()                      # attach to a running calc (caller() too)
b.sheets.active["A1"].value = 42
```

When no calc is running it never pretends — it says so:
`OfficeworkError: calc に繋がりません(…/officework/calc.sock: Connection refused)`

### The python-docx vocabulary (officework.doc)

```python
from officework import doc
d = doc.Doc.open("報告書.docx")
t = d.tables[0]
t.cell(0, 1).text                    # '件名'
[c.text for c in t.row_cells(1)]     # ['7月3日', '外壁塗装工事', '株式会社みほん商事', '640,200円']
len(t.columns)                       # 4
p = d[3]
p.runs[0].font == "MS明朝"           # font compares as a string, and
p.runs[0].font.name                  # answers .name too (None when the run names no font)
```

Paragraphs also carry `clear` / `iter_inner_content`. **A Run is a live
handle resolved by position** (same usage as python-docx — `r.bold = True`
and `r.add_text("more")` both work; changed from "frozen copy" on
2026-08-12). After `p.text = ...` or `replace` reshuffles the runs,
re-fetch from `p.runs`.

The writing side is complete too — `d.add_heading(text, level)` (1–3; we
do not carry level 0 = Title, and say so), `d.add_paragraph(text, style=)`,
`d.add_picture`, `d.add_section()`, `d.add_table(rows, cols)`,
`d.styles.add_style`, `p.add_comment(text)` (paragraph-level), and
`d.core_properties`. The full inventory with verdicts and reasons is the
ledger ([pysheet-gokan.ja.md](pysheet-gokan.ja.md)).

## Typesetting equations (officework.tex)

Equations are **taken as LaTeX and typeset into a picture** (2026-08-13).
We wrote no typesetter of our own: with TeX (pdflatex) installed it
typesets there; without it, matplotlib's mathtext does the job. **All you
need is matplotlib** — TeX, when present, raises the quality (matrix
columns align properly).

```python
from officework import tex
tex.kumi_kata()                       # "tex" | "mathtext" | None (what typesets today)
svg = tex.to_svg(r"\frac{a+b}{2}")    # bytes; glyphs become outlines, no font needed
png, w_mm, h_mm = tex.to_png(r"\sqrt{x^2+y^2}", size_pt=11)  # this one goes into documents
```

- A formula it cannot set raises **`tex.Muri`** with the reason — never a
  silent empty picture
- mathtext handles a **subset** of LaTeX. `\begin{matrix}`-style environments
  are bent into `\substack` (**columns do not align** — with TeX they do)
- `from_sympy()` builds LaTeX from a SymPy expression, but **SymPy rewrites
  the formula** (`(a+b)/2` → `a/2 + b/2`). If you need it verbatim, pass LaTeX
- Inserting an equation in writer (Insert > Equation) stores **the picture
  and the LaTeX source as a pair** in the docx — Word shows the picture,
  officework reopens it as an editable formula.

## Cell functions (UDFs) and arrays

Write a plain `def` in `~/.config/office/plugins/tools.py` and it is callable
from a cell by that name. **No decorators** (neither `@xw.func` nor
`@xw.ret(expand='table')` — the shape of the return value decides how it
spreads).

```
=aggregate(A1:B10, 100, "甲")
```

- Function names **may be Japanese** (`=集計(A1:B10)`)
- Range arguments arrive at your `def` as **row × column 2-D lists** of values
  (a single cell is a scalar)
- Return values: scalar → into the cell / **1-D list → spills downward** /
  **2-D list → spills down-right**. If the target area holds someone's data,
  it stops with `#SPILL!` (nothing is overwritten)
- **When the arguments change it recomputes in the background** (no need to
  press `@計算`); the work runs off the main thread in one batch and is
  written back as a single step
- Built-in names (SUM and friends) win; a `def` with the same name is skipped
- Only when the same name exists in two .py files do you qualify it:
  `=tools.aggregate(…)`
- The older form `=PY("aggregate", …)` still works

```python
def aggregate(r, limit, kind):   # r = [[r1c1, r1c2], [r2c1, …], …]
    hit = [row for row in r if row[0] == kind and row[1] <= limit]
    return [[row[0], row[1]] for row in hit]   # 2-D → spills
```

## writer macros (d = python-docx)

**Full manual: [writer-macro-manual.md](writer-macro-manual.md)** — named
fields (`fill` / `extract` / `fields`), templates (`render` / `tpl_fields`,
docxtpl), the sandbox, and letting the AI write the script.

```python
# d is a python-docx Document. The API is exactly python-docx's
d.paragraphs[12].runs[0].text = "商号 例示工務店"
for r in d.paragraphs[12].runs[1:]:
    r.text = ""                  # write to the first run, empty the rest (keeps formatting)
fill("代表・商号", "例示工務店")  # named fields beat label-hunting — see the manual
```

Saving is writer's job (don't call d.save in the script).

## In-page Python (HTML forms)

```python
# form = dict of field name → value. Values you set are written back to the page
qty = int(form.get("qty") or 0)
form["total"] = qty * 150
```

## The execution environment

- **No sandbox is applied** (removed 2026-08-09). Plugins are code you
  installed yourself, so files and the network work normally.
  The `@name net` distinction is gone (typing it says so)
- Time-limited (procedures 60 s, cell functions 30 s); overruns are killed
  and reported
- Libraries installed on the machine (polars, scipy, matplotlib, …) work
- `print` output appears in the status bar (report progress and counts there)
- The Data > Python one-liner and the writer side still run on a copy
  (sandboxed if a sandbox is available)

## Writing with an AI — a collaboration guide

You don't have to write macros yourself. **Ask an AI (Claude etc.), inspect,
run in the sandbox** — that is the intended workflow, including VBA
migrations. But AIs write for the common world (openpyxl, xlwings, VBA), so
**hand them this house's rules first**. Paste the block below as-is.

### Briefing for the AI (copy-paste)

```
Write Python for the following environment.

[calc macro] b (workbook) and s (current sheet) are pre-bound.
- read: s["A1"] (number=float, text=str, checkbox=bool; formula cells give
  the computed value. Empty is None or "". For the formula use
  s.formula("A1"), for the display string s.display("A1"))
- bulk read: s.values() (2-D list, rows × columns, 0-based); size is
  s.shape (a property — no parentheses)
- write: s["A1"] = value. Formulas are strings like "=B1*C1". None clears
- IMPORTANT: there is no range subscript (s["A2:C3"]) and no 2-D bulk
  assignment — write in a loop, one cell at a time. Row numbers in A1
  notation are 1-based
- after writing formulas call b.recalc() before reading values
- don't call b.save() (applying is the app's job). print goes to the app's
  status bar
- formatting (borders/merges/number formats) survives value writes — don't
  touch it

[writer macro] d (python-docx Document) is pre-bound.
Ordinary python-docx API. Don't call d.save().
When filling form fields: write to the first run and empty the rest
(p.runs[0].text = value; the remaining runs get "" — keeps paragraph
formatting)

[procedures — plugin .py files] put them in
~/.config/office/plugins/name.py and run them with `@name` or `@name.func`.
They drive the running calc directly:
  from officework import calc as xw
  def paste():
      s = xw.Book.caller().sheets.active
      s["A1"].value = [["received", "name"], ["2026-08-09", "Yamada"]]
No decorators — a plain def. However many cells it writes, one Ctrl+Z undoes it.

[execution] plain Python, no sandbox (plugins are code the user installed
themselves). Files and the network work normally; polars, scipy, and
matplotlib are available.

[when writing cell functions] a plain def in a plugin .py is callable as
`=name(A1:B9)` (names may be Japanese). Range arguments arrive as row ×
column 2-D lists of values. Return a scalar / 1-D list (spills down) / 2-D
list (spills down-right). It recomputes automatically when arguments change.
```

Then add **what you want, in plain language** (sheet name, heading row, what
should happen). If the table's shape matters, paste `s.values()[0]` (the
heading row) — it shortens the conversation.

### Inspecting the code you receive

**No sandbox is applied.** A .py you place in plugins is code you installed
yourself, treated exactly like a script you'd run from VS Code. So **reading
it before you install it** is the only gate:

1. **Where does it write** (does it touch columns/rows you must not lose?)
2. **What does it delete** (None assignments, row removals?)
3. **What does it do outside** (network, file reads/writes — is the
   destination the one you intended?)

The remaining safety net is undo: however many cells a procedure writes,
**one Ctrl+Z** returns you to the state before it ran. So the right way to try
things is **run it, look at the result, undo if you don't like it**. Once
reviewed, place it in `~/.config/office/plugins/name.py`.
**Code can never be embedded in a workbook** — data and program are separate
files (settled 2026-08-09).

### Migrating VBA

For a workplace .xlsm, extract the VBA (the standard tool is `olevba` from
`oletools`), paste it to the AI, and ask for "the same job in Python for the
environment above". Range/Cells loops map naturally to `s[f"A{n}"]` loops,
Worksheet to `b["name"]`. **After migrating, run both against the same input
and compare** — the comparison is part of the migration.

### An example request

> (after pasting the briefing)
> Sheet 受注台帳: row 1 is headings (受付・社名・品番・品名・数量・
> 単価・金額・発送済). Write a macro that totals 金額 per 社名 and writes
> "社名, total" starting at J5 downward.

The AI writes → you inspect (is column J free? no net needed) → run → look →
maybe ask "sort by total, descending" next. That loop is the basic form of
the collaboration.

## Worked examples (readable as-is)

**Engine only (pip install officework — no app needed).**
All six were run in that folder before being written down (sample/README.md
quotes the outputs):

- [sample/差し込み.py](../sample/差し込み.py) — fill the quote form; formats
  survive, formulas recalculate through to the total
- [sample/量産.py](../sample/量産.py) — three quotes from one template
- [sample/集計.py](../sample/集計.py) — bulk read with `values()`, totals by category
- [sample/差し替え.py](../sample/差し替え.py) — docx `replace()` that keeps run formatting
- [sample/表の吸い上げ.py](../sample/表の吸い上げ.py) — a document table → CSV
- [sample/点検.py](../sample/点検.py) — open a whole folder and count unsupported parts

**Working with the app (plugins procedures).**

- [templates/](../templates/README.md) — an inquiry ledger (CSV intake with
  `@取り込み`, status aggregation with =PY) and more
- [sample/注文書.xlsx](../sample/README.md) — swapping in a product master
  (`@更新`) and sending JSON (`@送信`)
- [sample/受注台帳.xlsx](../sample/README.md) — incremental intake that
  avoids duplicates with a watermark cell (K2)
