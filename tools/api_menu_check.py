#!/usr/bin/env python3
"""メニューの項目に Python から呼ぶ道があるかを、**画面の並びのまま**出す。

発注者 2026-08-24「API は、メニュー(タブ)とボタンを項目にして分類したらどうか」。
分類を新しく考えず、画面のタブとボタンをそのまま項目にします。

**ファイルタブは2つの場所に分かれています**(2026-08-24 発注者「ファイルは
重要でもっと項目があるでしょう」)。`ribbon.rs` のファイルタブは3つ
(開く・保存・印刷)しかありません。実際のファイルタブは全面のページで、
項目は `writer/src/cmds.rs` の `file_menu()` に 23 個あります。
リボンだけを読むと*20 個が数から丸ごと落ちます*。だからここでは両方を読みます。

`wiring_check.py` が「押せるボタンに腕があるか」を見るのと同じ形です。
あちらは画面の中を見ますが、こちらは **Python から届くか**を見ます。

    python3 tools/api_menu_check.py            # 一覧を出す
    python3 tools/api_menu_check.py --adoc     # 設計に貼る形で出す
    python3 tools/api_menu_check.py --check    # 表に無い id があれば落ちる

*対応は下の MAP が持ちます。* ボタンが増えたら MAP にも足してください。
`--check` は、`ribbon.rs` にあって MAP に無い id を見つけて落ちます
(新しいボタンが黙って「無い」に数えられるのを防ぎます)。

**「無い」と「作らない」は分けて書きます。** 作らないと決めた物には理由を
書いてください。理由の無い空欄は「まだ作っていない」の意味です。
"""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import ribbon_parse  # noqa: E402

# ボタン id → Python の道。空文字は「まだ無い」。
# 作らないと決めた物は "×(理由)" の形で書く
MAP = {
    # ファイル(リボンの行。実体は下の FILE_MENU)
    "open": "Doc.open", "save": "Doc.save", "pdf": "",
    # ホーム
    "copy": "×(選択の考えが Python に無い)", "cut": "×(同上)", "paste": "×(同上)",
    "selectall": "×(同上)",
    "fontname": "Run.font", "fontsize": "Run.size_pt",
    "incfont": "Run.size_pt", "decfont": "Run.size_pt",
    "changecase": "", "ruby": "", "ai-furigana": "",
    "bold": "Run.bold", "italic": "Run.italic", "underline": "Run.underline",
    "strikeout": "Run.strike", "superscript": "", "subscript": "",
    "highlight": "", "fontcolor": "Run.color", "clearstyle": "Run.clear",
    "markers": "Paragraph.style", "numbering": "Paragraph.style", "multilevels": "",
    "decoffset": "ParagraphFormat", "incoffset": "ParagraphFormat",
    "linespace": "ParagraphFormat.line_spacing", "direction": "",
    "align-left": "Paragraph.align", "align-center": "Paragraph.align",
    "align-right": "Paragraph.align", "align-just": "Paragraph.align",
    "align-dist": "Paragraph.align",
    "hidenchars": "×(画面の表示。文書は変わらない)",
    "paracolor": "", "borders": "", "parastyle": "Paragraph.style", "replace": "Doc.replace",
    # 挿入
    "blankpage": "Doc.add_page_break", "pagebreak": "Doc.add_page_break",
    "instable": "Doc.add_table", "insimage": "Doc.add_picture",
    "insshape": "", "inssmartart": "", "inschart": "", "instext": "", "instextart": "",
    "dropcap": "", "text-from-file": "",
    "edit-header": "Doc.header", "edit-footer": "Doc.footer",
    "pagenum": "", "datetime": "", "numpages": "",
    "insequation": "", "inssymbol": "(字をそのまま書く)", "controls": "Doc.fields",
    # 描画
    "pen": "×(手書き。docx だけの機能)", "highlighter": "×(同上)", "eraser": "×(同上)",
    # レイアウト
    "pagemargins": "Section", "pageorient": "Section", "pagesize": "Section",
    "columns": "", "line-numbers": "", "hyphenation": "",
    "watermark": "", "pagecolor": "", "colorschemas": "",
    # 参考資料
    "toc": "", "add-text": "", "toc-update": "", "bookmarks": "", "caption": "",
    "crossref": "", "footnote": "", "tof": "", "tof-update": "",
    # フォーム
    "form-text": "Doc.fill", "form-combo": "Doc.fill", "form-dropdown": "Doc.fill",
    "form-checkbox": "Doc.fill", "form-radio": "Doc.fill", "form-image": "",
    "form-email": "Doc.fill", "form-phone": "Doc.fill", "form-complex": "",
    "form-signature": "", "form-name": "Doc.fields",
    # 共同編集
    "coauth-mode": "×(共同編集の状態。文書は変わらない)",
    "co-addcomment": "Paragraph.add_comment", "co-delcomment": "",
    "co-showcomment": "Doc.comments", "co-chat": "×(画面の機能)",
    "track-changes": "", "co-history": "×(履歴は git)",
    # 保護
    "prot-sign": "", "prot-doc": "",
    # 表示(全部が画面の操作。文書は変わらない)
    "nav": "×(画面)", "fit-page": "×(画面)", "fit-width": "×(画面)",
    "zoom100": "×(画面)", "zoom-in": "×(画面)", "zoom-out": "×(画面)",
    "printview": "×(画面)", "multipage": "×(画面)", "darkmode": "×(画面)",
    "ui-bigger": "×(画面)", "ui-smaller": "×(画面)", "ruler": "×(画面)",
    "show-toolbar": "×(画面)", "show-statusbar": "×(画面)",
    "show-left": "×(画面)", "show-right": "×(画面)",
    # マクロ(Python を動かす側なので、Python からは呼ばない)
    "py-list": "×(Python を動かす側)", "py-folder": "×(同上)", "ai-macro": "×(同上)",
}


# ファイルのページの項目 → Python の道。**項目そのものは読み取ります**
# (下の file_menu())。ここに持つのは対応だけで、並びと見出しは元から取ります
FILE_MAP = {
    "f-back": "×(画面の行き来)",
    "f-new": "Doc()",
    "f-tpl": "",
    "f-open": "Doc.open",
    "f-url": "",
    "f-recent": "×(画面が覚えている物。Python は径路を直に書く)",
    "f-find": "×(同上)",
    "f-folder": "×(画面が開く綴り。Python は径路を直に書く)",
    "f-recover": "",
    "f-save": "Doc.save",
    "f-saveas": "Doc.save",
    "f-print": "",
    "f-export": "",
    "f-merge": "",
    "f-html": "",
    "f-protect": "",
    "f-distill": "",
    "f-style": "×(書式の標準は3段の設定。Python からは触りません)",
    "f-info": "Doc.core_properties",
    "f-place": "×(画面。Python は os が持つ)",
    "f-quit": "×(画面)",
    "f-opts": "×(アプリの設定)",
    "f-help": "×(画面)",
    "f-req": "×(画面)",
}

FILE_SRC = pathlib.Path(__file__).parent.parent / "writer" / "src" / "cmds.rs"


def file_menu() -> list[tuple[str, str]]:
    """ファイルのページの項目を `writer/src/cmds.rs` から読む。

    **手で写さない。** 写すと必ずずれます(この道具を書いた日に、
    当方が 21 個を 23 個と数え違えました)。
    """
    import re

    src = FILE_SRC.read_text(encoding="utf-8")
    if "fn file_menu" not in src:
        raise SystemExit(f"{FILE_SRC}: file_menu() が見つかりません(名前が変わった?)")
    body = src[src.index("fn file_menu") :]
    body = body[: body.index("\n    }")]
    items = re.findall(r'I::new\("(f-[a-z]+)",\s*ui::t!\("([^"]+)"\)\)', body)
    if not items:
        raise SystemExit(f"{FILE_SRC}: 項目を1つも読めません(書き方が変わった?)")
    return items


def 届く(v: str) -> bool:
    """道がある物だけ数える。× は作らないと決めた物、空は未実装"""
    return bool(v) and not v.startswith("×")


def main() -> int:
    adoc = "--adoc" in sys.argv
    tabs = ribbon_parse.tables_or_die()["WRITER"]

    ファイル = file_menu()

    missing_ids = [c.id for t in tabs for c in t.cmds if c.id and c.id not in MAP]
    missing_ids += [i for i, _ in ファイル if i not in FILE_MAP]
    if missing_ids:
        print("この表に無い項目があります(MAP に足してください):", file=sys.stderr)
        for i in missing_ids:
            print(f"  {i}", file=sys.stderr)
        return 1
    if "--check" in sys.argv:
        n = sum(len(t.cmds) for t in tabs) + len(ファイル)
        print(f"writer の {n} 項目(リボン + ファイルのページ {len(ファイル)})は全部この表にあります")
        return 0

    # **ファイルのページを先に出す。** リボンのファイルタブは3つしか
    # ありませんが、実体はこちらの一覧です(2026-08-24 発注者の指摘)
    a = sum(1 for i, _ in ファイル if 届く(FILE_MAP[i]))
    総数 = len(ファイル)
    済 = a
    if adoc:
        print(f"==== ファイル(ページ。{len(ファイル)} 個中 {a} 個が Python から届く)\n")
        print("リボンのファイルタブは3つですが、*実体はこの全面のページ*です。")
        print("元は `writer/src/cmds.rs` の `file_menu()` で、この道具が読んでいます。\n")
        print('[cols="1,1,1"]')
        print("|===")
        print("|項目 |id |Python\n")
    else:
        print(f"■ ファイル(ページ)  {a}/{len(ファイル)}")
    for i, label in ファイル:
        v = FILE_MAP[i] or "*無い*"
        if adoc:
            print(f"|{label} |`{i}` |{v}")
        else:
            print(f"    {label:<22} {i:<12} {v}")
    if adoc:
        print("|===\n")

    for tab in tabs:
        押せる = [c for c in tab.cmds if c.id]
        a = sum(1 for c in 押せる if 届く(MAP[c.id]))
        総数 += len(押せる)
        済 += a
        if adoc:
            name = "ファイル(リボンの行)" if tab.name == "ファイル" else tab.name
            print(f"==== {name}({len(押せる)} 個中 {a} 個が Python から届く)\n")
            print('[cols="1,1,1"]')
            print("|===")
            print("|ボタン |id |Python\n")
        else:
            print(f"■ {tab.name}  {a}/{len(押せる)}")
        for c in tab.cmds:
            if not c.id:
                print("|" + c.label + " |(灰色) |—" if adoc else f"    {c.label}(灰色)")
                continue
            v = MAP[c.id] or "*無い*"
            if adoc:
                print(f"|{c.label.replace('|', chr(92) + '|')} |`{c.id}` |{v}")
            else:
                print(f"    {c.label:<16} {c.id:<16} {v}")
        if adoc:
            print("|===\n")
    print(f"\n押せる {総数} 個のうち、Python から届くのは {済} 個です。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
