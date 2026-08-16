#!/usr/bin/env python3
"""**押せるのに見えないボタン**を見張る(writer)。

writer のリボンは**正本が2つ**ある:

1. `face/src/ribbon.rs` — 何があるか(id・札・ready)
2. `writer/src/view.rs` の `*_ROWS` — 段ごとにどう並べるか

`ready` なのに並びに載っていないボタンは、**押せる約束なのに画面に出ない**。
灰色(できないのに押せそうに見える)の裏返しで、同じくらい嘘。
配線の門番(wiring_check)は「腕があるか」しか見ないので、ここを素通りする。

2026-08-17 に `printview`(印刷レイアウト)がこれで消えていた。
そのとき手で書いた数え手は**正規表現が最初の表だけを掴み、残りを黙って
飛ばして「0件」**と言った。**黙って飛ばす検査は、無い検査より悪い** —
だから、この門番は表を見つけられなかったらそれ自体を落とす。

    python3 tools/writer_rows_check.py
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
import ribbon_parse  # noqa: E402

# 段の名前 → 並びの表の名前。**段に表があるなら必ずここに書く** —
# 書き忘れた段は下で「表が見つからない」として落ちる
ROWS_OF = {
    "ホーム": "HOME_ROWS",
    "挿入": "INS_ROWS",
    "描画": "DRAW_ROWS",
    "レイアウト": "LAYOUT_ROWS",
    "参考資料": "REF_ROWS",
    "フォーム": "FORM_ROWS",
    "共同編集": "COLLAB_ROWS",
    "保護": "PROT_ROWS",
    "表示": "VIEW_ROWS",
    "マクロ": "PLUG_ROWS",
}


def row_tables(src):
    """`const 名_ROWS: &[&[LItem]] = &[ … ];` の中の id を名前ごとに拾う。

    **括弧を数えて終わりを見つける** — 字下げの形に頼ると、書き方が変わった
    ときに黙って隣の表まで飲み込む(それで「0件」と言った)。
    """
    out = {}
    for m in re.finditer(r"const (\w+_ROWS): &\[&\[LItem\]\] = &\[", src):
        name = m.group(1)
        i = m.end() - 1  # 最初の `[`
        depth = 0
        while i < len(src):
            if src[i] == "[":
                depth += 1
            elif src[i] == "]":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        body = src[m.end() : i]
        out[name] = set(re.findall(r'\("([^"]+)"', body))
    return out


def main():
    src = (ROOT / "writer/src/view.rs").read_text(encoding="utf-8")
    rows = row_tables(src)
    if not rows:
        print("並びの表が1つも読めません(writer/src/view.rs の形が変わった?)")
        return 1

    tabs = ribbon_parse.tables_or_die()["WRITER"]
    missing_tables = []
    invisible = []
    for tab in tabs:
        key = ROWS_OF.get(tab.name)
        if key is None:
            continue  # ファイルの段など、並びを持たない段
        if key not in rows:
            missing_tables.append((tab.name, key))
            continue
        for c in tab.cmds:
            if c.ready and c.id and c.id not in rows[key]:
                invisible.append((tab.name, c.id, c.label))

    if missing_tables:
        print("**並びの表が見つかりません**(名前が変わった? この門番が嘘をつく前に落とす)")
        for name, key in missing_tables:
            print(f"  {name} → {key}")
        return 1

    if invisible:
        print(f"**押せるのに見えないボタンが {len(invisible)} 件あります。**")
        print("リボンの表では ready なのに、段の並び(*_ROWS)に載っていません。")
        print("writer/src/view.rs の並びに足すか、リボンの表から外してください。\n")
        for name, cid, label in invisible:
            print(f"  {name}: {cid}  {label}")
        return 1

    n = sum(len(v) for v in rows.values())
    print(f"writer: 押せるボタンは全部 段の並びに載っています(並びの表 {len(rows)} 枚・{n} 項目)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
