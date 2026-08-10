# The officework engine

*日本語版: [engine.ja.md](engine.ja.md)*

`writer` and `calc` are two applications over one engine. The engine is the part
that reads a document, understands it, and writes it back without damaging what
it did not touch. It has no window, so it runs anywhere: in the applications, in
`pip install officework`, and — as of 2026-08-10 — inside a spreadsheet written
by somebody else.

| | crate |
|---|---|
| xlsx: reading, writing, formulas, cell formats | `sheet` |
| docx: reading and writing | `ooxml` |
| line breaking, kinsoku, metrics, page coordinates | `engine` (kumihan) |
| putting a page onto paper | `paper` |

None of these depend on GPUI. That is the point of the split, not a
side-effect.

## Using it from Python

```console
$ pip install officework
```

See the [Python manual](python-manual.md). Nothing on this page is needed for
that; the sections below are about embedding the engine in another application.

## Running the engine inside genoffice

[genoffice](https://github.com/genspark-ai/genoffice) is an Electron office
suite whose spreadsheet talks to a Rust helper over stdin/stdout — one JSON line
each way, twelve commands. It reads the helper's path from `XLSX_SIDECAR_PATH`.

That environment variable is the whole story. Point it at officework's engine
and genoffice's spreadsheet runs on officework:

```bash
# Keep a copy of genoffice's own helper first — the engine forwards to it.
cp apps/sheets/native/xlsx-engine/target/release/xlsx-sidecar /tmp/genoffice-sidecar

XLSX_SIDECAR_PATH=/path/to/officework/target/release/xlsx-sidecar \
GENOFFICE_SIDECAR=/tmp/genoffice-sidecar \
  npm run dev -w @genoffice/sheets
```

**genoffice needs no patch.** Not one line, not one file. Remove the two
variables and it is back to its own helper. Build the engine with
`cargo build --release -p sidecar`.

The interesting part here is not our engine. It is that genoffice put a seam
where a seam belonged, and the seam holds: a second implementation of a
protocol, written by someone with no access to their build, drops in behind it.

## What is actually replaced

**Reading and calculating. Not writing.** The distinction matters, so here is
the whole protocol:

| Commands | Who does them |
|---|---|
| `open` `read_range` `read_formula_cells` `read_media` `recalc_cells` | **officework** |
| `close` `cancel` | officework (session bookkeeping) |
| `archive_manifest` `read_entries` `scan_entries` `save_archive` `convert_workbook` | **forwarded to genoffice's helper, byte for byte** |

genoffice does not write xlsx in Rust. Its TypeScript computes an XML patch and
the helper applies it to the ZIP, copying every untouched entry through still
compressed and checking the manifest by CRC32 and size before and after. We
measured what that path does to a workbook it was not asked to change: nothing.
So replacing it would buy nothing today, and the five commands are passed
straight through instead.

**This means the engine is not standalone in this configuration.** It spawns
genoffice's helper as a child for those five commands, so a copy of that binary
has to exist and `GENOFFICE_SIDECAR` has to point at it. Point it at the engine
itself and it will call itself forever.

## Which genoffice this was checked against

**`fd33934` (2026-08-10).** The protocol is theirs and they can change it
without telling anyone, because nobody here has asked them not to — this is a
second implementation of someone else's interface, not an arrangement with them.
So the commit is written down, and after pulling a newer genoffice the three
tiers below get run again before trusting it.

Their Rust helper was untouched by the seven commits that landed on 2026-08-10,
which is the usual case: the protocol moves far more slowly than the application
around it.

## How far it has been checked

- **Cross-check against genoffice's own helper** over 26 real workbooks (Bank of
  Japan flow-of-funds, Statistics Bureau household survey, and others): values,
  formulas, merges, extents and cell formats compared field by field.
- **genoffice's own test suite**, run against the engine: 18 of 21. The three
  are deliberate — genoffice reports pivot-table output ranges we do not model,
  numbers its style table by the original `cellXfs` index where we renumber, and
  has a test asserting that `CELL("filename")` *fails*, which ours answers
  correctly.
- **The application itself**, opened and driven by hand: text, borders, merges,
  cell formats, recalculation down three levels of dependency, and Save As.

Each tier caught defects the one above it could not. The cross-check cannot see
a simplification taught to both sides; a test suite whose schema is
`passthrough()` cannot see a field the running application rejects with
`strict()`. Five defects surfaced only when the real application was launched.

## Licence

officework is **AGPL-3.0-or-later**. genoffice is Apache-2.0, and nothing of
genoffice is redistributed here — the recipe above runs a copy the user built
themselves. Combining the two in a distributed work would make the combination
AGPL, which is a decision for whoever distributes it, not something this page
can grant.
