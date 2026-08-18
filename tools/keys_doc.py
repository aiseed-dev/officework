#!/usr/bin/env python3
"""キーの一覧を、束縛の表(ui/src/lib.rs の KEYS_*)から手引きへ起こす。

    python3 tools/keys_doc.py --write   # 手引きの自動生成の節を書き直す
    python3 tools/keys_doc.py           # 揃っているか見るだけ(CI の門番)

手で書いた表は必ずずれる(嘘は欠落より悪い)。手引きの「### キー」の
手書きの表は**読み物**として残し、網羅の一覧だけをこの道具が受け持つ。
書き込む場所は keys:gen の印の間 — 印の外は触らない。

限界: 操作の説明はこの表(DESC)にしか無い。操作を足したら説明も足す —
無ければこの道具が**名指しで止まる**(黙って空欄にしない)。
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# キーの表は face(gpui を持たない層)にある。2026-08-15 に ui から移った
LIB = ROOT / "face/src/keys.rs"

# 操作名 → (日本語, 英語)。**束縛の表に居る操作は全部ここに要る**
DESC = {
    "Backspace": ("1つ消す(セルの上では中身を消す)", "Delete backwards (on a cell: clear it)"),
    "Delete": ("1つ消す(セルの上では中身を消す)", "Delete forwards (on a cell: clear it)"),
    "Left": ("左へ", "Move left"),
    "Right": ("右へ", "Move right"),
    "Up": ("上へ", "Move up"),
    "Down": ("下へ", "Move down"),
    "SelectLeft": ("選択を左へ伸ばす", "Extend selection left"),
    "SelectRight": ("選択を右へ伸ばす", "Extend selection right"),
    "SelectUp": ("選択を上へ伸ばす", "Extend selection up"),
    "SelectDown": ("選択を下へ伸ばす", "Extend selection down"),
    "SelectAll": ("すべて選択", "Select all"),
    "WordLeft": ("単語の左へ(calc はデータの端へ)", "Word left (calc: to the data edge)"),
    "WordRight": ("単語の右へ(calc はデータの端へ)", "Word right (calc: to the data edge)"),
    "SelectWordLeft": ("単語ぶん選択を左へ(calc は端まで)", "Extend selection a word left (calc: to the edge)"),
    "SelectWordRight": ("単語ぶん選択を右へ(calc は端まで)", "Extend selection a word right (calc: to the edge)"),
    "EdgeUp": ("データの端へ(上)", "To the data edge (up)"),
    "EdgeDown": ("データの端へ(下)", "To the data edge (down)"),
    "SelectEdgeUp": ("端まで選択(上)", "Extend selection to the edge (up)"),
    "SelectEdgeDown": ("端まで選択(下)", "Extend selection to the edge (down)"),
    "Home": ("行頭へ", "To the start of the line/row"),
    "End": ("行末へ", "To the end of the line/row"),
    "DocHome": ("先頭へ", "To the beginning"),
    "DocEnd": ("末尾へ", "To the end"),
    "PageUp": ("ページ送り(上)", "Page up"),
    "PageDown": ("ページ送り(下)", "Page down"),
    "Enter": ("確定して下へ / 改行", "Confirm and move down / new line"),
    "Tab": ("右へ / 字下げ", "Move right / indent"),
    "ShiftTab": ("左へ / 字上げ", "Move left / outdent"),
    "Undo": ("元に戻す", "Undo"),
    "Redo": ("やり直し", "Redo"),
    "Save": ("保存", "Save"),
    "SaveAs": ("名前を付けて保存", "Save as"),
    "Open": ("開く", "Open"),
    "Copy": ("コピー", "Copy"),
    "Cut": ("切り取り", "Cut"),
    "Paste": ("貼り付け", "Paste"),
    "PasteValues": ("値だけ貼り付け", "Paste values only"),
    "Quit": ("終了", "Quit"),
    "ContextMenu": ("右クリックメニュー", "Context menu"),
    "Cancel": ("取り消し・閉じる", "Cancel / close"),
    "Find": ("検索と置換", "Find and replace"),
    "EditCell": ("セルの編集", "Edit the cell"),
    "Recalc": ("再計算(ブック全体)", "Recalculate the whole book"),
    "RecalcSheet": ("再計算(このシート)", "Recalculate this sheet"),
    "NewLine": ("セルの中の改行", "New line inside the cell"),
    "UiBigger": ("画面の文字を大きく", "Bigger UI text"),
    "UiSmaller": ("画面の文字を小さく", "Smaller UI text"),
    "InsLink": ("ハイパーリンク", "Hyperlink"),
    "Bold": ("太字", "Bold"),
    "Italic": ("斜体", "Italic"),
    "Underline": ("下線", "Underline"),
    "Strikeout": ("取り消し線", "Strikethrough"),
    "ArrayEnter": ("昔ながらの配列数式(CSE)", "Legacy array formula (CSE)"),
    "InsertFn": ("関数の挿入", "Insert function"),
    "PercentFmt": ("パーセント書式", "Percent format"),
    "Print": ("印刷(PDF に出す)", "Print (to PDF)"),
    "FullScreen": ("全画面の切り替え", "Toggle full screen"),
    "FlashFill": ("フラッシュフィル", "Flash fill"),
    "ZoomReset": ("ズームを 100% に", "Zoom to 100%"),
    "Help": ("ヘルプ", "Help"),
    "InsDate": ("今日の日付を挿入", "Insert today's date"),
    "InsTime": ("今の時刻を挿入", "Insert the current time"),
    "PrevSheet": ("前のシートへ", "Previous sheet"),
    "NextSheet": ("次のシートへ", "Next sheet"),
    "CycleRef": ("参照の $ 回し", "Cycle $ in the reference"),
    "SlicerMulti": ("スライサーの複数選択(開いている間)", "Slicer multi-select (while open)"),
    "SlicerClear": ("スライサーの絞り解除(開いている間)", "Clear the slicer filter (while open)"),
    "CellFormat": ("セルの書式", "Cell format"),
    "SelectCol": ("列の選択", "Select the column"),
    "SelectRow": ("行の選択", "Select the row"),
    "AutoSum": ("オートSUM", "AutoSum"),
    "FillDown": ("下へコピー", "Fill down"),
    "FillRight": ("右へコピー", "Fill right"),
    "Jump": ("ジャンプ(名前ボックスへ)", "Go to (focus the name box)"),
    "ToggleFilter": ("フィルタの付け外し", "Toggle the filter"),
    "MakeTable": ("表にする", "Make a table"),
    "AddComment": ("コメント", "Comment"),
    "AlignLeft": ("左揃え", "Align left"),
    "AlignCenter": ("中央揃え", "Align centre"),
    "AlignRight": ("右揃え", "Align right"),
    "AlignJustify": ("両端揃え", "Justify"),
    "PageBreak": ("改ページ", "Page break"),
    "FontBigger": ("文字を大きく", "Bigger font"),
    "FontSmaller": ("文字を小さく", "Smaller font"),
}

# 見せる鍵の書き方(GPUI の綴りのまま出さない)
def pretty(key: str) -> str:
    parts = key.split("-")
    # "ctrl-shift-=" のような末尾の記号は split で空になる — 繋ぎ直す
    while "" in parts:
        i = parts.index("")
        if i + 1 < len(parts):
            parts = parts[:i] + ["-" + parts[i + 1]] + parts[i + 2:]
        else:
            parts[i - 1] += "-"
            parts.pop(i)
    name = {
        "ctrl": "Ctrl", "alt": "Alt", "shift": "Shift",
        "pageup": "PageUp", "pagedown": "PageDown", "escape": "Esc",
        "menu": "Menu", "enter": "Enter", "space": "Space", "tab": "Tab",
        "backspace": "Backspace", "delete": "Delete", "home": "Home",
        "end": "End", "left": "←", "right": "→", "up": "↑", "down": "↓",
    }
    def show(p: str) -> str:
        if p in name:
            return name[p]
        if re.fullmatch(r"f\d+", p):
            return p.upper()          # f2 → F2
        if len(p) == 1 and p.isalpha():
            return p.upper()          # ctrl-b → Ctrl+B(慣例の見せ方)
        return p
    return "+".join(show(p) for p in parts)


def rows_of(table_name: str):
    src = LIB.read_text(encoding="utf-8")
    m = re.search(rf"pub const {table_name}[^=]*= &\[(.*?)\n\];", src, re.S)
    if not m:
        sys.exit(f"{LIB} に {table_name} が見つかりません(書き方が変わったらこの道具も直す)")
    body = m.group(1)
    rows = re.findall(r'\(\s*"((?:[^"\\]|\\.)*)"\s*,\s*"([A-Za-z]+)"\s*\)', body)
    if not rows:
        sys.exit(f"{table_name} から1行も読めません(書き方が変わったらこの道具も直す)")
    return rows


def table_for(app: str, lang: str) -> str:
    rows = rows_of("KEYS_COMMON") + rows_of("KEYS_CALC" if app == "calc" else "KEYS_WRITER")
    # 操作ごとに鍵を束ねる(出てきた順)
    order, keys = [], {}
    for key, action in rows:
        if action not in keys:
            order.append(action)
            keys[action] = []
        keys[action].append(pretty(key))
    missing = [a for a in order if a not in DESC]
    if missing:
        sys.exit(f"説明の無い操作があります(tools/keys_doc.py の DESC に足してください): {missing}")
    # **手引きは AsciiDoc です**(2026-08-18 に .md から移した)。表は
    # `|===` で囲み、セルの間に空白を1つ置く(詰めると前のセルの終わりが
    # 桁の指定として読まれる)
    見出し = "|キー |操作" if lang == "ja" else "|Key |Action"
    lines = ['[cols="1,1"]', "|===", 見出し, ""]
    for a in order:
        desc = DESC[a][0 if lang == "ja" else 1]
        lines.append(f"|{' / '.join(keys[a])} |{desc}")
    lines.append("|===")
    # 表の後ろに空行を1つ。**書き出しの正規形に合わせる**ためです
    # (engine の `adoc::write` が表の後ろに空行を置きます)
    lines.append("")
    return "\n".join(lines)


MARK_S = "// keys:gen:start"
MARK_E = "// keys:gen:end"

TARGETS = [
    ("docs/calc-manual.ja.adoc", "calc", "ja"),
    ("docs/calc-manual.adoc", "calc", "en"),
    ("docs/writer-manual.ja.adoc", "writer", "ja"),
    ("docs/writer-manual.adoc", "writer", "en"),
]


def main():
    write = "--write" in sys.argv
    bad = 0
    for rel, app, lang in TARGETS:
        p = ROOT / rel
        s = p.read_text(encoding="utf-8")
        if MARK_S not in s or MARK_E not in s:
            print(f"::error::{rel} に keys:gen の印がありません")
            bad = 1
            continue
        pre, rest = s.split(MARK_S, 1)
        markline, rest = rest.split("\n", 1)
        _, post = rest.split(MARK_E, 1)
        gen = table_for(app, lang)
        new = f"{pre}{MARK_S}{markline}\n{gen}\n{MARK_E}{post}"
        if write:
            if new != s:
                p.write_text(new, encoding="utf-8")
                print(f"{rel}: 書き直しました")
            else:
                print(f"{rel}: 揃っています")
        elif new != s:
            print(f"::error::{rel} のキーの一覧が束縛の表とずれています"
                  "(python3 tools/keys_doc.py --write で直ります)")
            bad = 1
    sys.exit(bad)


if __name__ == "__main__":
    main()
