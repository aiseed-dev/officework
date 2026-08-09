# calc manual

*日本語版(secondary): [calc-manual.ja.md](calc-manual.ja.md)*

A spreadsheet app that opens, edits, and saves xlsx — and calculates formulas.

This manual describes **only what works today**. Commands shown grayed out on the
ribbon are unimplemented and cannot be pressed (we never make something look
usable when it isn't). Grayed-out items and their Python alternatives are listed
[at the end](#not-implemented-gray-and-alternatives).

Three promises:

- **Formatting is preserved.** Borders, merged cells, column widths, shapes, and
  themes of an opened form survive a save (parts we don't understand are carried
  over from the original file)
- **Every operation is one undo away.** When in doubt, Ctrl+Z
- **Nothing is dropped silently.** Anything we can't read, or that would be lost
  on save, is mentioned in the status bar

## Starting

```bash
./target/release/calc              # opens empty
./target/release/calc 帳票.xlsx    # open a file
```

Without a Japanese font the app won't start (install `fonts-noto-cjk` or
`fonts-ipaexfont`, or set `OFFICE_FONT=/path/to/font.ttf`).

## The screen

The window frame follows the desktop-app convention. **Row 1**: save, print,
undo, redo, and the workbook name (unsaved changes show `*`; drag this row to
move the window). **Row 2**: tabs on a white strip (current tab has a green
underline; 🔍 at the right edge is find & replace). The button band is a single
row of icon buttons like the original (major buttons are labeled; hovering shows
the name in the status bar; only Home has two rows). **The File tab is a
full-page view** — a menu on the left (New, Open, Recent, Save, Save As, Print,
Protect, Properties, Open file location, Quit) and "Workbook info" on the right
(statistics plus xlsx properties = author, title, etc. **Click a field, type,
Enter to record** — saved into docProps, visible in Excel). "Recent" keeps 12
entries. Tab order matches Euro-Office:
**File / Home / Insert / Draw / Layout / Formulas / Data / Pivot Table /
Table Design / Collaboration / Protection / View / Plugins** (all the original
tabs). **Interface theme** (View tab) darkens the surroundings —
**cells stay white** (screen and paper must agree).
**Bottom edge**: the status bar — sheet tabs, results/warnings/errors, and when
a range is selected its **sum, average, and count** update live (like Excel's
status bar). Protected sheets get a 🔒 on their tab.

**View tab** (matching the original desktop app): zoom (50–200%; cells and text
scale together — paper is unaffected), formula bar on/off, gridlines, headings
(row numbers and column letters; hide them for a clean table look), show zeros
(hides zero values — **display only; the value stays 0** and formulas still see
it). Freeze panes, sheet visibility (unhide/hide sheets), and the interface
theme also live here.

## Basics

### Keys

| Action | Keys |
|---|---|
| Move | ↑ ↓ ← → / Enter (down) / Tab (right) / Shift+Tab (left) |
| Select range | Shift+arrows |
| First / last cell | Ctrl+Home / Ctrl+End |
| Page up / down | PageUp / PageDown |
| Select all | Ctrl+A |
| Undo / redo | Ctrl+Z / Ctrl+Shift+Z (Ctrl+Y too) |
| Copy / cut / paste | Ctrl+C / Ctrl+X / Ctrl+V |
| **Paste values only** | Ctrl+Shift+V (formulas become their computed values) |
| Find / replace | Ctrl+F / Ctrl+H |
| Open / save | Ctrl+O / Ctrl+S |
| Context menu | Menu key / Shift+F10 |
| Cancel / close | Esc |
| Quit | Ctrl+Q |

### Mouse

- Click to select, drag to select a range, Shift+click to extend from the anchor
- Click a row number or column letter to select the whole row/column
- Drag the boundary between headings to resize columns/rows
- Ctrl+click opens hyperlinks
- Drag shapes to move them; the bottom-right corner resizes; **drag the circle
  above the frame to rotate** (Shift snaps to 15°). Selecting a shape opens a
  settings panel on the right (fill/line color, line width, opacity,
  rotate/flip, shadow). **Ctrl+click selects several shapes** → right-click →
  Align (2+) / Distribute (3+). The right-click menu also has cut/copy/paste,
  arrange (front/back) and save-as-SVG; Del deletes the whole selection

## Formulas and functions

Start with `=` for a formula. Arithmetic (`+ - * /`), comparisons
(`= <> > >= < <=`), ranges (`A1:B3`), absolute references (`$A$1`), and defined
names all work. Editing triggers recalculation; circular references are detected
and reported (never silent).

**Cross-sheet references**: write `=Sheet2!B2` directly (ranges too:
`=SUM(Sheet2!B1:B5)`; quote names containing spaces or symbols:
`='Q1 actuals'!B2`). An unknown sheet name gives **#REF!** — it is never
silently read as the current sheet. Cross-sheet values are **copied as of that
moment**, so edit the source and recalculate (F9) to catch up. Renaming a sheet
rewrites `oldname!` inside formulas (but **not inside strings** such as
`INDIRECT("oldname!A1")` — quoted text is left alone).

**About 185 functions** are implemented (including modern aliases). By category:

| Category | Functions |
|---|---|
| Aggregation | SUM, AVERAGE, COUNT, COUNTA, COUNTBLANK, MIN, MAX, SUMIF, **SUMIFS**, COUNTIF, **COUNTIFS**, AVERAGEIF(S), MINIFS, MAXIFS, **SUBTOTAL**, AGGREGATE, SUMPRODUCT, SUMSQ, AVERAGEA, MAXA, MINA |
| Math | ABS, MOD, QUOTIENT, POWER, SQRT, INT, ROUND(UP/DOWN), TRUNC, CEILING(.MATH), FLOOR(.MATH), MROUND, EVEN, ODD, SIGN, PRODUCT, FACT, COMBIN, PERMUT, GCD, LCM, PI, SIN…ATAN2, SINH…TANH, EXP, LN, LOG, LOG10, DEGREES, RADIANS, RAND, RANDBETWEEN |
| Statistics | MEDIAN, MODE, STDEV(P), VAR(P) (and .S/.P names), PERCENTILE, QUARTILE, LARGE, SMALL, RANK(.EQ/.AVG), CORREL, SLOPE, INTERCEPT, FORECAST |
| Finance | PMT, PV, FV, NPER, **NPV, IRR, RATE** (IRR and RATE are iterative) |
| Date & time | TODAY, NOW, DATE, DATEVALUE, YEAR, MONTH, DAY, WEEKDAY, TIME, HOUR, MINUTE, SECOND, EDATE, EOMONTH, DATEDIF, WORKDAY, NETWORKDAYS, DAYS, DAYS360, YEARFRAC, WEEKNUM, ISOWEEKNUM |
| Text | LEN, LEFT, RIGHT, MID, TRIM, UPPER, LOWER, PROPER, EXACT, CLEAN, CONCATENATE/CONCAT, **TEXT**, SUBSTITUTE, FIND, SEARCH, VALUE, NUMBERVALUE, TEXTJOIN, REPT, FIXED, YEN, CHAR/UNICHAR, CODE/UNICODE |
| Japanese | **LENB, LEFTB, RIGHTB, MIDB** (full-width = 2, matching Excel's Japanese locale), **ASC, JIS** (full/half-width conversion), **DATESTRING** (Japanese era dates), **PHONETIC** (furigana — reads the rPh data in the xlsx) |
| Logic & info | IF, IFS, SWITCH, CHOOSE, AND, OR, NOT, IFERROR, IFNA, NA, IS… (BLANK/ERROR/ERR/NA/NUMBER/TEXT/NONTEXT/LOGICAL/EVEN/ODD), T, N, TYPE |
| Lookup | VLOOKUP, HLOOKUP, **XLOOKUP**, LOOKUP, INDEX, MATCH, ROW(S), COLUMN(S), ADDRESS, HYPERLINK, **OFFSET, INDIRECT** (including `'Sheet name'!A1` across sheets) |
| Dynamic arrays | **FILTER, SORT, UNIQUE, SEQUENCE, TRANSPOSE** — results spill into neighboring cells; blocked cells give `#SPILL!`. They also combine with operators (`=SEQUENCE(3)+1`) and nest into aggregates (`=SUM(FILTER(…))`) |

Errors (`#N/A` etc.) propagate from arguments to the formula. IFERROR catches an
error in its first argument, and IF never trips over the error in the branch it
didn't take (`=IFERROR(VLOOKUP(…),"")` turns "not found" into a blank).
Unknown function names return **#NAME?** — the app never silently computes 0.

On top of these, **`=PY(…)` lets you write your own functions in Python** —
see [the Python section](#writing-cell-functions-in-python-py-udf).

Formula-tab tools: AutoSum, **Insert Function** (category → function), function
lists by category, **calculation mode** (automatic ⇔ manual; manual avoids
waiting on big sheets), **watch window** (pins formula cells and shows their
values in the bottom band), name manager, trace precedents/dependents (the cells
light up; "remove arrows" clears), show formulas.

**Defined names**: select a range, then "Define Name" (Home or Formulas).
Names are alphanumeric plus `_` (must not look like a cell reference) and can be
used in formulas.

## Formatting

- Text: bold, italic, underline, strikethrough, **subscript**, color, font, size
  (the font list shows **this machine's fonts**)
- Cells: fill, borders (each side independently), horizontal and vertical
  alignment (including **justified**), wrap, **text orientation** (each press:
  vertical → stacked → back; for the vertical headings of Japanese forms)
- **Right-to-left text** (Home): characters in a cell run right to left —
  for Japanese right-to-left horizontal writing (old signboards); Latin bidi is
  out of scope
- **Sheet direction** (Layout): columns run right to left
- **Cell styles** (Home): heading, title, good/bad/neutral, note, calculation,
  currency, percent — one click applies the set (Ctrl+Z undoes)
- **Theme colors**: modern Excel stores colors as "theme slot + tint". We read
  and resolve them, so **colors don't disappear**. Switching the **color scheme**
  (Layout: Office / warm / cool / ink) updates every cell that uses theme colors
- Number formats: thousands separator, more/fewer decimals, %, currency
- Merged cells (hidden values are not destroyed)
- **Conditional formatting**: greater/less/equal rules changing text color or
  fill (cellIs type; round-trips through xlsx)
- **Data validation** (Data tab): list type. Choices are inline (`甲,乙,丙`) or
  a range reference (`=D2:D5` — edit the referenced cells and the choices
  follow). Invalid input doesn't enter the cell; Esc restores. Rules we can't
  resolve (references to another sheet, etc.) don't block input (we don't warn
  on what we can't check). Non-list rule types are reported as unreadable
- **Table design** (Table Design tab): tools applied to the selection one step
  at a time — header-row band, banded rows/columns, first/last column bold,
  **total row** (adds a =SUM row below; it's a formula, so it follows edits;
  refuses if something is already below), filter buttons. Tables are stored as
  named **table objects**, so "Convert to range" (removes the table; band
  formatting and formulas stay — like Excel) and "Resize table" (type a new
  range like A1:C9) also work. Saved as real xlsx tables, visible in Excel.
  To apply everything at once, use Insert > Table

## Sheets and data

- **Multiple sheets**: add, delete, rename, switch
- **Freeze panes** (View): freezes rows above and columns left of the cursor;
  press again to release
- **Filter**: filter by cell value (display only; data unchanged). One step to clear
- **Sort**: by column, ascending/descending. The header row stays put
- **Remove duplicates** (Data): shows the count before deleting
- **Text to data** (Data): pours CSV/TSV in at the cursor. Encoding (including
  CP932) and delimiter are auto-detected
- **External links** (Data): imports another workbook's values **as values**.
  No live links — we don't create forms that break when a link dies
- **Goal seek** (Data)
- **Solver** (Data): the same "Solver parameters" dialog as ONLYOFFICE —
  target cell (max/min/value), variable cells (a range or comma-separated, up to
  64), constraints (cell / <= = >= / number or cell), then "Solve". The method
  is **simplex LP** (backed by scipy). Coefficients are measured on a copy of
  the sheet, and **nonlinear problems are refused** (as in the original).
  The solution lands in the variable cells; one Ctrl+Z restores
- **Subtotals** (Data): select a table with headings and it asks for the
  grouping column (e.g. department) → columns to total (empty Enter = every
  numeric column). Inserts "〜 小計" (=SUM) rows per group and a grand total at
  the end, and groups **only the detail rows**. **Collapse to show just the
  subtotals and grand total.** They're formulas, so edits flow through. Sort
  first to bring groups together (as in Excel). All of it is one Ctrl+Z
- **Grouping** (Data): drag across row headings, then "Group". **Hide detail**
  collapses, **Show detail** expands (with no selection, the group containing
  the cursor). Works on columns too. Grouping again deepens the nesting (up to
  7). Depth and collapsed state round-trip through xlsx outlineLevel / hidden,
  so Excel shows a normal outline. Collapsed rows/columns don't print (PDF)
  either. Unlike filtering this **persists in the file** — a collapsed ledger
  stays collapsed for the next person. The heading of the row just after a
  group shows a **+/− button**; click to collapse or expand (like Excel's
  outline margin)
- **Pivot tables** (Pivot Table tab): select a table with headings and press
  Insert; it asks for row headings (multiple allowed) → column headings
  (optional) → value and aggregation (sum/average/count/max/min). polars does
  the aggregation and the result is placed **as values** in the empty space to
  the right of the source (further right if occupied — nothing is overwritten
  silently). The definition is stored in the workbook (custom part
  xl/joPivot.xml), so when the source changes, put the cursor on the pivot and
  press **Refresh** (Refresh All does the whole workbook). **Grand totals**
  (rows, and columns if you spread by column), **subtotals**, **blank rows**
  (with 2+ row headings), and **report layout** (tabular ⇔ compact) toggle and
  re-place on each press. **Select** selects the whole pivot. Every operation
  re-aggregates from the source data with polars, so even averages in the grand
  total are correct (not averages of averages). Each is one Ctrl+Z.
  If Excel re-saves the file the definition part disappears — the pivot then
  can't refresh and becomes a plain table of values (honest degradation)
- **Comments**: one per cell. Empty + Enter deletes. Saved into the xlsx

## Insert

- **Table**: creates a bordered table frame with a header row in one go
- **Images**: PNG / JPEG. Shapes (rectangle, rounded, ellipse, arrow, diamond,
  line — text inside works too), symbols, text boxes, sparklines
- **Chart / recommended chart**: matplotlib draws the selected range and floats
  it on the sheet as an image (first column = labels, remaining columns =
  series; a heading row becomes series names). Japanese fonts are registered
  from the machine so no tofu. If Python is missing, the status bar explains
  what to install
- **SmartArt**: pick a category (list / process / cycle / hierarchy /
  relationship / matrix / pyramid) then a layout. Categories, order, and names
  match Euro-Office. What's inserted is **a group of shapes**, so each shape
  can be selected, Enter to edit text, drag to move, Del to delete — and the
  whole thing is one Ctrl+Z. Saved to xlsx as ordinary shapes (Excel shows
  them as shapes; they are not native SmartArt parts — in exchange, any tool
  can open and edit them). The list shows **only layouts this method can build**
- **Text art**: decorative text — drawn bold with an outline and floated as an
  image (matplotlib, same pipeline as equations)
- Draw tab **pen / highlighter / eraser**: press to arm the tool, drag over the
  grid to draw (pen: thin black; highlighter: wide translucent yellow). The
  eraser removes **one stroke at a time**. Press the button again or Esc to
  return to cell operations. Strokes are saved as **ordinary polyline shapes**,
  so Excel shows them, Ctrl+Z steps back stroke by stroke, and they print (PDF)
- **Checkbox**: placed on an empty cell it becomes ☑/☐ and **toggles with the
  space key** (the value is TRUE/FALSE — usable from formulas: `=E4`,
  `=COUNTIF(E:E,TRUE)`). Any TRUE/FALSE cell displays as ☑/☐. Occupied cells
  are refused. Excel shows the TRUE/FALSE values
- **Slicer**: a panel listing the values of the cursor's column as buttons;
  press one to filter to those rows (≡ multi-select, ✕ clear, Esc close).
  Like filtering, **display only** — the saved data doesn't change
- **Equation**: mathematical typesetting (separate from cell formulas — nothing
  is computed). Type TeX (`\frac{a}{b}`, `\sum_{i=1}^n i^2`) and matplotlib's
  mathtext renders it as an image. Unreadable input is refused with a message
- **Hyperlink**: right-click or Insert tab. Empty + Enter removes

## AI — transformations and generation by a model

Ten buttons on the AI tab. Because this is a spreadsheet, what's sent is **the
selected range** and what comes back is **a table (cells) or a formula**.
**Every response is one Ctrl+Z away.**

- **Destination**: each press cycles local model → Claude (subscription) →
  Claude (API). **Subscription** means calling the local `claude` command
  (Claude Code CLI), so no API key is needed. Unavailable destinations say why
  on the spot (default is the local model; **nothing leaves your network**)
- **Summarize**: the selected table (or the used area) in 2–4 sentences, into
  a **comment** at the cursor
- **Rewrite / polite / plain / translate**: rewrites **only text cells** in the
  selection (numbers and formulas are untouched)
- **Furigana**: select one column and readings go into the **column to the
  right** (the reading column of a name roster; refuses if it's occupied)
- **Continue**: extends the selected table's pattern into the rows below
  (**it's the model guessing — check the result**)
- **To table**: type prose, get a table poured in at the cursor
- **Ask**: type a request; answers starting with `=` are inserted **as a
  formula** at the cursor, anything else becomes a comment ("write me a sum
  formula" works)

If the destination can't be reached the app says so (it never silently returns
nothing). **Keys are never stored in the workbook.**

## Cell text is Markdown

Text cells are **drawn as Markdown** (settled 2026-08-09). The cell holds plain
text; the markup is interpreted only at draw time.

```
This is **bold**            → This is bold
*italic* ~~strike~~ `mono`
see [here](https://…)       → a blue underlined link
# Heading   ## Sub   ### Small
- bullet    1. numbered
```

- **No editor for formatting part of a cell is needed** — you get formatting by
  typing, and the formula bar and Python always see the same plain text
- **It is written to xlsx as plain text**. A round trip can never garble it
  (Excel shows the markers literally — honest degradation)
- **The selected cell shows the raw text** so you can retype it
- To stack headings and bullets vertically, turn on **wrap** for the cell
  (without wrap, newlines still collapse to spaces as before)
- Japanese typefaces have no italic face, so `*italic*` does not slant in
  Japanese (the markers are still removed). Latin text slants
- **Heading sizes come from the template** — the workbook's named cell styles
  "Heading 1 / 2 / 3". Define them in a .xltx and every workbook made from it
  follows (editing them in Excel carries over too). Without them the defaults
  are 1.6 / 1.35 / 1.15× and bold. **Typing a heading grows the row** (it never
  shrinks one)

**Where the line is drawn so tables survive**: if nothing reads as markup,
nothing happens. `2*3*4` (multiplication), `-100`, `1.5`, `2026-08-09`, and
`name*` (an unpaired marker) all render exactly as before. The rule is that a
delimiter pair whose content is only digits is not treated as markup.

## Python — instead of macros

The programmer's reference (ranges ⇄ arrays, the API, =PY arguments and return
values) is the [Python manual](python-manual.md). This section is the
operations side.

**There are no VBA-style macros** — and no mechanism for putting code inside
a document at all (settled 2026-08-09). Think of it as VS Code and Excel
merged: the sheet is an .xlsx, the code is a .py, and they are separate files.

- What people exchange is **data only**. A workbook you receive contains no code
- Both cell functions and procedures live in `~/.config/office/plugins/*.py`,
  installed once per machine
- So "opening a file runs someone else's code" cannot happen by construction
- Because the code is only ever your own, **no sandbox is applied** — the same
  treatment as running a script from VS Code

### Setup

- Python is found as `JO_PYTHON` → `.venv/bin/python` → `python3`
- Put `office_sheet.so` (pysheet) **next to the calc executable**: build with
  `cargo build -p pysheet --release --features extension-module` and rename
  `liboffice_sheet.so` to `office_sheet.so`
- Plugins live in `~/.config/office/plugins/` (create it if missing)

### Writing cell functions in Python

Just write a plain `def` in `~/.config/office/plugins/tools.py`:

```python
def double(x):
    return x * 2

def grid(table):
    return [[v * 10 for v in row] for row in table]
```

Call it from a cell like any other function. **No decorator, no registration**:

```
=double(A1)          → 42 (when A1 is 21)
=grid(A1:B2)         → spills 2×2 down-right
```

- Function names **may be Japanese** (`=集計(A1:B9)`)
- **When the arguments change, it recomputes in the background** — you do not
  press `@計算`. The work runs off the main thread in one batch and is written
  back as a single step once every answer is in
- Arguments are passed as **current values** (ranges as row×column 2-D lists)
- A 2-D return value **spills** down-right (1-D spills down). If the target
  area holds someone's data or formulas, it stops with `#SPILL!` — nothing is
  overwritten. When a result shrinks, the leftover area is cleared
- Built-in names (SUM and friends) win; a `def` with the same name is skipped
- Only when the same name exists in two .py files do you qualify it:
  `=tools.double(A1)`
- Edit the .py and calc notices, then recomputes
- A cell not yet computed shows `#PY?`. Excel shows `#NAME?` (saved values
  remain visible until Excel recalculates)
- The older form `=PY("double", A1)` still works

### Procedures (what replaces macros)

`~/.config/office/plugins/inbox.py`:

```python
from officework import calc as xw

def paste():
    s = xw.Book.caller().sheets.active
    s["A1"].value = [["received", "name"], ["2026-08-09", "Yamada"]]
```

Type `@inbox.paste` in the Data > Python field to run it. It drives **the
running workbook directly** (not a temporary copy). However many cells it
writes, **one Ctrl+Z** returns you to the state before the procedure.

### Running a line of code (Data > Python)

`b` = the book, `s` = the current sheet, are bound.

```python
s["A30"] = "Nihon Funen Co., Ltd."   # value only; formatting is left alone
```

- Press Enter on an empty field to pick a .py file instead
- This one runs **on a copy** — a failure leaves the sheet untouched, and a
  success is applied as **a single step** (one Ctrl+Z even across sheets)
- Everything installed on the machine (polars, scipy, …) is available

### @ commands

| What you type | What happens |
|---|---|
| `@module` | run `~/.config/office/plugins/module.py` top to bottom |
| `@module.func` | call just that function in that .py |
| `@list` (or `@`) | the .py files in plugins and the defs inside them |
| `@計算` | compute cell functions by hand (normally automatic, so rarely needed) |
| `@export name` | extract code embedded in an **old** workbook to a .py (never runs it) |
| `@del name` | drop code embedded in an old workbook |

`@name net` is gone (with no sandbox there is no network distinction). Typing
it says so.

### Opening an old workbook

If a workbook carries code in the old form (`xl/joPython.xml`), the report
says so. It **cannot be run, and disappears on save** — `@export name` writes
it to a .py so you can read it and install it into plugins yourself. That
review is the only gate.

### Time limits

60 seconds for a procedure, 30 for a cell function. Past that it is stopped
and says so, so a runaway loop never ties up the app.

## Shared folders and locking

On open, the app checks `.~lock.filename#` in the same place LibreOffice does.

- If someone holds it, **their name is shown** and **overwrite-save is
  blocked** (Save As still works). This prevents the "two people open it, last
  save wins" accident
- Otherwise our lock is placed, and removed when closing or moving to another
  file

## Collaboration tab and plugins

No server. Everything works **through files** (i.e. in a shared folder):

- **Collaboration mode**: shows who currently holds editing rights (the lock);
  if they've left, takes over
- **Add / remove / show comments**: per cell. Hiding doesn't delete
- **Chat**: appends named messages to the file next to the workbook
  (`name.xlsx.chat.txt`). Not live — messages passed through files
- **Version history**: every overwrite-save keeps a copy under
  `.jo-history/name/timestamp.xlsx` (9 generations). Picking one opens it as
  an **untitled copy** — to restore, save it under the original name yourself
  (nothing is written back silently)
- **Macros** (Plugins tab): choose a .py and run it
  (same machinery as Data > Python; b = workbook, s = sheet)
- **Manage plugins**: lists and runs .py files from `~/.config/office/plugins`

## Protection tab

- **Protect**: makes the current sheet read-only (edits are blocked; same
  button to release). **No password, and no pretend-password.** Round-trips
  xlsx sheetProtection, so Excel shows the sheet as protected too
- **Encrypt**: set a password and the next save is wrapped in **AES-256
  (Agile, the Excel 2013+ default)**. Opens in Excel and LibreOffice. Clear
  the field and press Enter to stop encrypting. Opening prompts for the
  password (masked). **Reading accepts both Agile and the older AES-128
  (2007 Standard)**
- **Add digital signature**: a signature file next to the workbook
  (`name.xlsx.sig`, Ed25519). Press once: if a valid signature exists it
  reports; if missing or the content changed, it (re-)signs. Not the scheme
  that appears in Excel's signature line — tamper detection plus a name

## Print and PDF

Configure on the Layout tab, then File > Print writes a PDF.

- Paper: A4 / A3 / B4 / B5 / A5. **B sizes are JIS** (matching Japanese
  office forms)
- Orientation, margins (default 20mm / narrow 10mm / wide 30mm), scale
  (100→90→80→70→50%)
- Print area (select and set; press again to clear), page breaks, repeated
  title rows, print gridlines, print headings (row numbers / column letters)
- These are saved into the xlsx (by patching the original attributes — ones we
  don't understand are not deleted), so Excel opens with the same print setup
- The PDF carries values, formatting, borders, fills, and the shapes/images
  inserted by this app. Columns that don't fit the paper are cut (the status
  bar says so)

## Saving

- Ctrl+S overwrites (blocked if someone else holds the lock). If unnamed, a
  dialog asks
- Original parts (charts, shapes, themes, XML we don't understand) are
  **carried over as-is**
- Unsaved changes show in the status bar, and quitting asks "save and quit?"

## Not implemented (gray) and alternatives

**Zero grayed-out ribbon commands (155/155 including the AI tab).** The
following work differently here, or are delegated to Python:

| Grayed elsewhere / different | Our way |
|---|---|
| Forecasting, statistics | polars, statsmodels |
| Macros (VBA) | **deliberately absent**; plugins written in Python take the role (code stays out of the workbook) |

We also don't create **reference links** to external workbooks (values are
imported instead) — no forms that break when a link dies.
