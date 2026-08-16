# writer manual

*日本語版(secondary): [writer-manual.ja.md](writer-manual.ja.md)*

A word processor that opens, edits, and saves docx. It exports PDF and can read
JavaScript-free HTML as a document. **Ribbon: 118/118 — every button works
(zero grayed out).**

Three promises:

- **Formatting is preserved.** Styles, shapes, and parts we don't understand
  survive a save untouched (carried over from the original file)
- **Every operation is one undo away.** Keystrokes, IME commits, pastes, a single
  ink stroke, a color-scheme change — all come back with one Ctrl+Z
- **Nothing is dropped silently.** Anything we can't read, or that would be lost
  on save, is listed in the report and mentioned in the status bar

Try the samples in [sample/](../sample/README.md) — especially
`sample/writer/01〜05` (Japanese typesetting, vertical writing, an application
form, a monthly report, a feature tour).

## Starting

```bash
./target/release/writer                          # opens empty
./target/release/writer sample/報告書.docx
./target/release/writer sample/writer/02_縦書きの手紙.docx
```

If the machine has no Japanese font, the app stops and says so (instead of showing
tofu boxes). Install `fonts-noto-cjk` or `fonts-ipaexfont`, or set `OFFICE_FONT=…`.

## The screen

The window frame follows the desktop-app convention. **Row 1**: save, print, undo,
redo, and the document name (unsaved changes show a `*`; this row is also the
window drag handle). **Row 2**: tabs on a white strip (the current tab is
underlined; the 🔍 at the right edge is find & replace). **Bottom edge**: the
status bar — page n/m, character count (character, not word, count — matching
how Japanese documents are measured), status messages, proofreading, zoom.

- **The File tab is a full-page view** — a menu on the left (New, Open, Open URL,
  12 recent files, Save, Save As, Print, Protect, Properties, Open file location,
  Quit) and "Document info" on the right (statistics plus docx properties =
  author, title, and so on; click a field and type — it goes into docProps and
  is visible in Word)
- **Left panel** (View tab): headings / comments / search — click to jump
- **Right panel** (View tab): settings board for character, paragraph, and page —
  the buttons are the same actions as the ribbon; active toggles are tinted

Tab order:
**File / Home / Insert / Draw / Layout / References / Forms /
Header & Footer / Collaboration / Protection / View / Plugins**.

## Basic keys

| Action | Keys |
|---|---|
| Move | ↑ ↓ ← → (visual lines). By word: Ctrl+←→ |
| Line start / end | Home / End (Home twice = paragraph start) |
| Document start / end | Ctrl+Home / Ctrl+End |
| Select | Shift+arrows, Ctrl+A |
| Undo / redo | Ctrl+Z / Ctrl+Shift+Z (Ctrl+Y too) |
| Copy / cut / paste | Ctrl+C / Ctrl+X / Ctrl+V |
| Find / replace | Ctrl+F / Ctrl+H |
| Open / save | Ctrl+O / Ctrl+S |
| Indent / list level | Tab / Shift+Tab (inside a table: next cell) |
| Context menu | Menu key / Shift+F10 |
| Close dialog, put tool away | Esc |
| Quit | Ctrl+Q (asks if there are unsaved changes) |

Uncommitted IME text appears underlined in the body and becomes one undo step
when committed. New features (ruby, vertical writing, …) are all reachable from
buttons — there are no new shortcuts to memorize.

### All keys

Every live binding is listed below (generated from the binding table).
You can rebind in settings.toml: `key.bold = "alt-b"` replaces the
default keys for that action (separate several with commas, unbind with
an empty string `""`; unreadable lines are reported in the status bar
at startup; action names are the English names below).

<!-- keys:gen:start (rewritten by tools/keys_doc.py --write) -->
| Key | Action |
|---|---|
| Backspace | Delete backwards (on a cell: clear it) |
| Delete | Delete forwards (on a cell: clear it) |
| ← | Move left |
| → | Move right |
| Shift+← | Extend selection left |
| Shift+→ | Extend selection right |
| Ctrl+← | Word left (calc: to the data edge) |
| Ctrl+→ | Word right (calc: to the data edge) |
| Ctrl+Shift+← | Extend selection a word left (calc: to the edge) |
| Ctrl+Shift+→ | Extend selection a word right (calc: to the edge) |
| PageUp | Page up |
| PageDown | Page down |
| Ctrl+F / Ctrl+H | Find and replace |
| Ctrl+B | Bold |
| Ctrl+I | Italic |
| Ctrl+U | Underline |
| Ctrl+5 | Strikethrough |
| Ctrl+P | Print (to PDF) |
| F11 | Toggle full screen |
| Ctrl+Shift+S / F12 | Save as |
| Ctrl+0 | Zoom to 100% |
| F1 | Help |
| Ctrl+; | Insert today's date |
| Ctrl+: | Insert the current time |
| Ctrl+Home | To the beginning |
| Ctrl+End | To the end |
| Shift+↑ | Extend selection up |
| Shift+↓ | Extend selection down |
| Ctrl+A | Select all |
| Home | To the start of the line/row |
| End | To the end of the line/row |
| Enter | Confirm and move down / new line |
| ↑ | Move up |
| ↓ | Move down |
| Tab | Move right / indent |
| Shift+Tab | Move left / outdent |
| Ctrl+Z | Undo |
| Ctrl+Shift+Z / Ctrl+Y | Redo |
| Ctrl+S | Save |
| Ctrl+O | Open |
| Ctrl+C | Copy |
| Ctrl+X | Cut |
| Ctrl+V | Paste |
| Ctrl+Q | Quit |
| Menu / Shift+F10 | Context menu |
| Ctrl+= / Ctrl+Shift+= | Bigger UI text |
| Ctrl+- | Smaller UI text |
| Ctrl+K | Hyperlink |
| Esc | Cancel / close |
| Ctrl+E | Align centre |
| Ctrl+L | Align left |
| Ctrl+R | Align right |
| Ctrl+J | Justify |
| Ctrl+Enter | Page break |
| Ctrl+] | Bigger font |
| Ctrl+[ | Smaller font |
<!-- keys:gen:end -->

## Character formatting (Home)

**Applies to the selected text only** (with no selection, to the whole paragraph).
Bold, italic, underline, strikethrough, super/subscript, text color, highlight,
font (from this machine's font list), size, change case, clear formatting.

**Ruby (furigana)**: select text, Home > Ruby, type the reading in the dialog
(empty removes it). It is set at half size above the base text; if the reading is
longer than the base, spacing is distributed. Stored as `w:ruby` in docx and
visible in Word / LibreOffice. Denden-markdown `{漢字|かんじ}` in .md files is
also read.

## Paragraph formatting (Home)

- Alignment: left, center, right, justified, and **distributed** — unlike
  justified, it spreads spacing **through the last line** (the shape used for
  labels that must fill the column in Japanese forms; stored as `w:jc distribute`)
- Bulleted and numbered lists (levels via Tab / Shift+Tab), indent, line spacing
  (1.0→1.5→2.0), page-break-before, background color, borders, drop caps
- Toggle display of hidden characters

## Vertical writing (Home > Text Direction)

Each press toggles horizontal/vertical. Text flows top-to-bottom, columns
right-to-left; punctuation, brackets, and long-vowel marks switch to their
vertical forms, and ruby works vertically too. Stored as `textDirection tbRl`
in sectPr; verified to open in LibreOffice.

Honest first-version limits: tables and multi-column layout stay horizontal;
Latin letters and digits are stacked one per line (no rotated run yet); explicit
page breaks fold into column breaks. Not combined with facing-pages view.
Sample: `sample/writer/02_縦書きの手紙.docx`.

## Headings, TOC, bookmarks, cross-references (References)

- **Headings**: Home > paragraph styles (or References > Add Text to cycle
  body → heading 1 → 2 → 3)
- **TOC / table of figures**: page numbers are computed with **the same layout
  as print (PDF)**, so they can't disagree with the paper. After edits, press
  Update to rebuild. Stored in docx as static text
- **Captions**: "図 N" below the paragraph, numbered automatically
- **Bookmarks**: add/remove/jump from a dialog. **Cross-references** insert a
  bookmark's text or page number into the body (shown with a light shade =
  computed value; stored as REF / PAGEREF fields, so Word can recalculate them)

## Forms (form fields / content controls)

Ten kinds on the Forms tab: **text, combo box, dropdown, checkbox, radio,
picture, e-mail, phone, composite, signature**.

- Fields appear as light boxes; typing inside them works normally (the content
  stays part of the field)
- Checkboxes toggle ☐⇄☑ with the same button (or the space key). Combo/dropdown
  choices are set comma-separated in a dialog; the same button cycles candidates
- Stored as `w:sdt` (content controls) and opens in Word. Our own kinds
  (e-mail, phone, composite, signature) are tagged `jo:*`
- **Combined with Protection > Protect this becomes a distributable form** —
  hand out a read-only document where only the fields accept input. Sample:
  `sample/writer/03_申込書.docx`

## Tables, images, insert

- **Tables**: Insert > Table (3×3). Edit inside cells, Tab to the next cell.
  Tables with merged cells are read, displayed, saved, and printed (no UI to
  create new merges yet)
- **Images**: PNG, JPEG, SVG (SVG is converted to a high-resolution PNG).
  Saved as proper docx parts, visible in Word
- **Shapes, SmartArt (9 layouts), charts, text art**:
  behind the buttons Python (matplotlib etc.) draws the picture and pastes it
  as an image
- **Equations**: Insert > Equation opens a small panel — **type LaTeX and
  press Enter** (e.g. `\frac{a+b}{2}`). Python typesets it: with TeX
  installed it typesets there (matrix columns align), otherwise matplotlib.
  **The picture and the LaTeX source go into the docx as a pair**, so Word
  shows the picture and writer reopens it as an editable formula. A formula
  it cannot set is refused with the reason
- **Text box**: a 1×1 table (text inside a frame)
- **Text from file**: inserts .txt / .md / .docx

## Drawing (pen, highlighter, eraser)

Drag on the page. Strokes are **anchored to the page** (editing the body doesn't
move them); one stroke is one undo step. In docx they become freeform shapes,
visible in Word.

## Layout

- Paper (A4→B5→A3), orientation, margins (20/12/30mm), columns (1→2→3),
  line numbers, page color, watermark (light diagonal text, visible in Word),
  hyphenation (Latin text broken at syllables)
- **Color scheme**: cycles through six (standard, indigo, green, dark red,
  indigo+unbleached paper, ink+gray paper). Heading color and page color change
  as a pair, and the **actual color values are written**, so Word shows the same
  colors without needing theme parts. One Ctrl+Z restores

## Header & footer

Edited in a dialog (shared by all pages). Page number and page count go in as
placeholders and become real numbers on paper (PDF). The date is inserted as
**fixed text** at insertion time (not a field that changes every time the file
is opened — a classic source of clerical accidents).

## HTML — reading and filling in, without JavaScript

**Read, fill in, and print web documents in writer.**

- **Open**: an .html file, or File > **Open URL** (http/https). Headings,
  paragraphs, lists, tables, images, ruby, and forms are typeset onto the page —
  unreadable elements go to the report. Links are followed **only within the
  origin host**
- **JavaScript is never executed** (it is skipped and noted in the report).
  Pages that require JS won't work — and the app says so
- **Filling and submitting**: form fields (input / select / textarea) become
  form fields; submit is a plain GET / POST round trip. The response becomes
  a page again
- **In-page Python** (`<script type="py">`) is **never run on open**. It runs
  only on explicit command, inside the same sandbox as Python in Calc
- Opened HTML can be saved as docx and printed as PDF (we don't save HTML —
  compatibility stops at the format boundary)
- Acceptance samples: `sample/html/` (01〜10); a live counterpart is the catalog
  and order form from `python3 sample/catalog_server.py`

## Collaboration tab — no server, everything through files

Tools that work on a document in a shared folder:

- **Collaboration mode**: checks who holds the lock (`.~lock.document.docx#`),
  by name; if the previous holder has left, takes over editing rights. While
  someone else holds it, overwrite-save is blocked (first come, first served;
  later arrivals read — an honest arrangement)
- **Comments**: add/remove/toggle per paragraph (docx comments.xml; Word shows
  them as balloons)
- **Track changes**: start/stop recording. While recording, changed paragraphs
  get an orange bar in the margin. **On save** they become real Word tracked
  changes (insertions/deletions with author). Saving finalizes the recording
  (the app tells you so)
- **Chat**: appends named messages to a file next to the document
  (`document.docx.chat.txt`). Not live — messages passed through files
- **Version history**: every overwrite-save keeps a copy under `.jo-history/`
  (9 generations). Selecting one opens it as an **untitled copy** — to restore,
  save it yourself under the same name (nothing is written back silently)

## Protection tab

- **Protect**: read-only toggle (round-trips docx documentProtection — Word
  shows it as protected too). **No password, and no pretend-password.** The
  effect is real: typing and every document-changing button are blocked;
  viewing, printing, and searching still work
- **Encrypt**: set a password and the next save is wrapped in AES
  (**Agile = AES-256, the Word 2013+ default**; opening also accepts the older
  AES-128). Opens in Word / LibreOffice. Clear the field and press Enter to
  remove. Cross-verified against msoffcrypto-tool
- **Digital signature**: a signature file next to the document
  (`document.docx.sig`, Ed25519; the key is auto-generated at
  `~/.config/officework/sign.key`). Tamper detection plus a name — it is not the
  scheme that fills Word's signature line (and says so)

## View tab

Navigation (left panel), fit page / fit width / 100% / zoom in / out,
**print layout (sheets stacked one page at a time — the view where a
document whose sections change paper size shows the difference; the editing
view stays one continuous scroll)**,
**multiple pages (spread — screen only; printing stays one page per sheet)**,
dark mode (surroundings darken, paper stays white), ruler (mm),
toolbar / status bar / side panel toggles.

## The side panels and their icon rails

A rail of icons runs down the outer edge of each panel; pressing one switches
the face:

- **Left**: headings / comments / search / AI
- **Right**: settings (where you are) / page (whole-document settings)

Hover an icon to see its name. The panels **take space** — they never cover the
page.

## Right panel, "Settings — fix where you are"

**It follows where you are.** If the cursor is inside a table, the table face
comes first; if the paragraph holds an equation or a picture, that face does
(the Text / Paragraph / Page faces stay below — you still want bold inside a
table).

- **Table** (when inside one): shows rows × columns and where you are, plus
  **row above / row below / delete row / column left / column right / delete
  column**. **Added 2026-08-15** — until then writer could only drop a 3×3
  table at the end, with no way to add a row. The last row and column are never
  deleted (deleting the whole table is a separate action)
- **Equation** (when the paragraph holds one): shows the LaTeX source; "Edit
  the equation" loads that source into the box. **You never retype it**
- **Picture** (when the paragraph holds one): its size in mm, and smaller /
  bigger
- **Text**: bold, italic, underline, strike, size, font, colour, clear
- **Paragraph**: alignment, line spacing, indent, bullets, numbering,
  background, box
- **Page**: orientation, paper, margins, columns, vertical writing

The panel **takes space** (it never covers the page) and scrolls if its
contents grow.

## AI — talk to it in the left panel

**On 2026-08-15 the AI tab was removed.** Its eleven buttons (summarize,
rewrite, make polite, …) were replaced by **a conversation in the left panel** —
you can just type "make this paragraph polite", and you can ask for things no
button could express.

Open it with **View tab > Navigation** (left panel), then the **AI** ear.

- **The selection is what you are asking about** (with nothing selected, the
  whole document)
- **Revised text is shown before it goes in.** Press Apply and it replaces the
  selection, or is inserted after the cursor if nothing is selected. Either way
  **one Ctrl+Z** undoes it
- Requests that don't change the text (advice on wording) are simply answered

**Two things stayed on the ribbon** — work the conversation cannot do:

- **Home > Furigana** — what goes in is ruby *formatting*, not plain text
- **Plugins > Write a macro** — it puts a .py in the plugins folder
  (never run automatically; read it, then run it yourself)

**Choosing the destination** lives in File > Advanced settings, "AI
destination". Unavailable destinations **say why on the spot**.

## Proofreading (review)

English spelling via the machine's dictionary; Japanese misconversions
(以外/意外) and inconsistent spellings via a **local model** (OpenAI-compatible
endpoint, `OFFICE_HOST` / `OFFICE_PORT` / `OFFICE_MODEL`; nothing leaves your
network). Model hallucinations are filtered, and **if the model can't be
reached the app says "can't proofread"** — never a silent "no issues".
Automatic furigana uses the same model assets (readings chosen in context,
based on measurements of 561 ambiguous-reading words).

Headless tools: `office-spell document.txt` (exit code 0=clean 1=findings
2=unreadable 3=incomplete), `--furigana`, `--washi`.

## Macros tab

- **List**: the .py files in `~/.config/officework/plugins`; pick one to run it
  (in the sandbox). Extensions like Aozora annotations or EPUB go here
  Picking one runs **Python in a sandbox** (bubblewrap) on a copy of the
  document; the result lands as one undo step. The script gets `d` = the
  python-docx Document. **Code is never stored in the document** (no executable
  content in docx — the policy differs from xlsx because python-docx already
  exists as public infrastructure)

**Only .py files in the folder can be launched** (2026-08-16). The button that
ran a .py from anywhere is gone. A file you were given goes into the folder
first, then you pick it from the list — **that step is the gate**.
- **Writing macros** — named fields (`fill`/`extract`), templates (`render`),
  speed rules, and the AI button: see the
  [writer macro manual](writer-macro-manual.md)

## Two kinds of document — meaning only, and the docx you were sent (2026-08-16)

writer handles two kinds. **Which one you have open changes what you can do.**

| | Native (.adoc) | Compatible (.docx) |
|---|---|---|
| Content | **meaning only** (headings, emphasis, quotes, tables…) | the look is in the text |
| Look | a **template** (a .toml next to it) | the text itself |
| Direct formatting | **blocked** → name it and it becomes a style | works as before |
| Saving | .adoc (meaning only) | .docx (original parts kept) |

**A docx you were sent behaves exactly as before.** Nothing changes for it.

### Why the split

Word had styles too, but direct formatting (make *this* line bold 12pt) was
just as easy, so nobody used styles and documents sank into a swamp of
formatting. **The design prevents it**: in a native document, naming a look is
*easier* than applying it directly.

## Native documents (.adoc)

File > Open, pick a `.adoc`. It is plain text:

```asciidoc
= Monthly report
:template: house-style

== Summary

Sales *rose* against last month.footnote:[Tokyo and Osaka combined]

* Tokyo
* Osaka
```

- `= Title` and `:template: name` at the top
- Headings `==`–`====`, emphasis `*bold*` `_italic_`, quotes `____`
- `footnote:[]`, bookmarks `[[name]]`, references `<<name>>`, ruby
  `ruby:漢字[かんじ]`, maths `stem:[LaTeX]`, images `image::path[]`,
  page break `<<<`, tables `|===`
- **Not one bit of look goes in.** Size, colour and typeface live in the template

### The template (where the look lives)

`:template: house-style` looks for `house-style.toml` **next to the document →
`~/.config/officework/templates/` → the built-in default**.

```toml
[文書]
大きさ = 11

[ページ]
用紙 = "A4"
余白 = 25

[スタイル.見出し1]
大きさ = 20
太字 = true
色 = "1B6E3C"
```

Keys are accepted in Japanese or English (`size`, `bold`, `color`, `[style.…]`).

**The easiest way to get one is to ask the AI** ("make it look like a government
form", "like an internal report"). It is text, so the AI can write it and you can
read the diff.

A broken template **says what is wrong** and falls back to the built-in default —
falling back silently would leave you no way to fix it.

### When you want to change the look

Pressing size, typeface, colour, highlight, underline or strikethrough does not
apply anything. Instead you get

> **New style — give this look a name**
> What it applies: size 11pt
> `[ Large text ]` [Confirm (Enter)]

Name it and the style goes **into the template**, so every place with that name
changes at once. The AI suggests the name (without it you get a draft name).

- **With text selected it applies to the text; without, to the paragraph**
- A character style is saved as `[.name]#text#`
- Emphasis, superscript and subscript are **meaning, so they are not blocked**

### The Styles face in the right panel

View > Right panel → the third icon on the rail. It shows the style this
paragraph wears and the ones you can pick. Picking changes it; the size buttons
edit **the template**, so everywhere with that style changes together.

### Laying out for the web

One extra section in the template lays the text out as **one continuous flow**
instead of paper:

```toml
[組み方]
横幅 = "可変"     # follow the window, not the paper
区切り = "なし"   # do not fold into pages
```

Slides (one section per sheet, text never crossing a sheet) do not exist yet —
so there is no switch for them either.

## Reducing a docx to meaning (distilling)

File > **Reduce to meaning (distil)**. It pulls the direct formatting out of a
docx, groups similar looks into styles and moves them into a template.

> Distilled — 1 style, 15 paragraphs. The look moved into the template

- **It is lossy, so it is an explicit step.** Until you press it, the docx stays
  a docx (original parts included)
- Anything that does not fit a paragraph's look (one word in a different colour)
  is dropped, and **the count is reported** — no pretending nothing was lost.
  Emphasis, footnotes, links and ruby survive
- Afterwards the document is native and saves as .adoc

## Searching a folder (2026-08-17)

File > **Search a folder** walks a folder and searches many files at once.

- **It reads inside .docx too** — no need to convert to txt first
- Plain text (.txt / .md / .adoc / .py / .csv …) as well
- Narrow by name with `*.txt` (`;` for several)
- The folder defaults to the one holding the open document (or the last one you
  chose)
- Hidden folders are skipped; if there is too much it stops early **and says so**

Hits are grouped per file with line numbers. Picking one **shows its
surroundings below without opening anything**. It opens only when you press
**Load** below — so you look first, and a misclick never swaps your document.

## Printing (PDF) and saving

- File > Print writes a PDF. **Screen and paper are the same page** — they
  cannot disagree. Real header/footer numbers, watermark, page color, ink,
  and vertical writing all carry over
- Ctrl+S saves docx (blocked if someone else holds the lock). Body, tables,
  images, and headers are written back; parts we don't understand are carried
  over from the original file. Elements we couldn't read are noted as
  "preserved on save"

## No more gray

All 118 ribbon commands work. The gray of "don't make it look usable when it
isn't" reached **zero** in writer. The remaining honest limits are
noted in each section above (first-version vertical writing, per-change
accept/reject for tracked changes, no UI yet for creating cell merges).
