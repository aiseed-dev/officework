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

## The office_sheet (pysheet) API

```python
import office_sheet                     # inside calc it's pre-imported; b and s arrive bound
b = office_sheet.Book.open("form.xlsx")
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
  installed yourself, so files and the network work normally. The `@name net`
  distinction is gone (typing it says so)
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

- [templates/](../templates/README.md) — an inquiry ledger (CSV intake with
  `@取り込み net`, status aggregation with =PY) and more
- [sample/注文書.xlsx](../sample/README.md) — swapping in a product master
  (`@更新 net`) and sending JSON (`@送信 net`)
- [sample/受注台帳.xlsx](../sample/README.md) — incremental intake that
  avoids duplicates with a watermark cell (K2)
