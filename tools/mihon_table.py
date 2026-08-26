#!/usr/bin/env python3
"""「見本を作って止める」対象を、1行ずつの表にして出す。

発注者 2026-08-24「数だけ書かずにきちんと表を作れ」。
数を書くだけでは、何をどうするかが決まりません。項目ごとに1行を持ち、
*いまの状態*と*どうするか*を書きます。

    python3 tools/mihon_table.py           # 一覧を出す
    python3 tools/mihon_table.py --adoc    # 設計に貼る形で出す
    python3 tools/mihon_table.py --check   # 元と食い違えば落ちる

対象の選び方は「図形・グラフ・図解の類い」です。
`face/src/ribbon.rs` から拾うので、ボタンが増えれば表も増えます。
"""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import ribbon_parse  # noqa: E402

# 対象の id と、灰色のボタンの見出し。
# いまの状態は**実際のコードを読んで**書いています(推測ではありません)
IMA = {
    # writer。cmds.rs の "insimage" | "insshape" | … の腕
    ("WRITER", "insshape"): "画像を選ぶ画面が出る。状態行が「Python で描いて貼る」と案内する",
    ("WRITER", "inssmartart"): "同上",
    ("WRITER", "inschart"): "同上",
    ("WRITER", "instextart"): "同上",
    ("WRITER", "instext"): "1×1 の表として入る(動く)",
    ("WRITER", "insequation"): "LaTeX を打つ画面が出る(動く)",
    # calc
    ("CALC", "insshape"): "六種の図形が入る(動く)",
    ("CALC", "inschart"): "matplotlib で描いて入る(動く)",
    ("CALC", "instext"): "図形と文字で入る(動く)",
    ("CALC", "inssmartart"): "腕はあるが、まだ何も入らない",
    ("CALC", "instextart"): "同上",
    ("CALC", "insequation"): "同上",
    ("CALC", "insrecommend"): "同上",
    ("CALC", "insslicer"): "同上",
}

# 灰色(id が空)の見出し → どうするか
HAI = {
    "前面ヘ移動": "見本",
    "背面ヘ移動": "見本",
    "配置": "見本",
    "グループ化": "見本",
    "図形を結合": "見本",
    "ブックを保護する": "画面を作る",
    "範囲を保護する": "画面を作る",
    "標準": "画面を作る",
    "改ページ プレビュー": "画面を作る",
}

# どうするか(id のある物)
DOU = {
    ("WRITER", "insshape"): ("見本", "SVG の図形を描いて貼る"),
    ("WRITER", "inssmartart"): ("見本", "四角と矢印の図解を SVG で描く"),
    ("WRITER", "inschart"): ("見本", "matplotlib の棒グラフ。数字を差し替える"),
    ("WRITER", "instextart"): ("見本", "飾り文字を SVG で描く"),
    ("WRITER", "instext"): ("そのまま", "動いています"),
    ("WRITER", "insequation"): ("そのまま", "動いています。組むのは Python のまま"),
    ("CALC", "insshape"): ("そのまま", "動いています"),
    ("CALC", "inschart"): ("そのまま", "動いています"),
    ("CALC", "instext"): ("そのまま", "動いています"),
    ("CALC", "inssmartart"): ("見本", "writer と同じ図解の見本"),
    ("CALC", "instextart"): ("見本", "writer と同じ飾り文字の見本"),
    ("CALC", "insequation"): ("画面を作る", "writer と同じ LaTeX の画面を移す"),
    ("CALC", "insrecommend"): ("見本", "表から棒・折れ線・円を選んで描く"),
    ("CALC", "insslicer"): ("見本", "polars で絞り込んで別の表にする"),
}


def rows():
    """(アプリ, タブ, ボタン, id, いまの状態, どうするか, 見本の中身)"""
    tabs = ribbon_parse.tables_or_die()
    out = []
    for app in ("WRITER", "CALC"):
        for tab in tabs[app]:
            for c in tab.cmds:
                if not c.id:
                    if c.label in HAI:
                        dou = HAI[c.label]
                        naka = "図形を選んで動かす" if dou == "見本" else "—"
                        out.append((app, tab.name, c.label, "(灰色)",
                                    "押しても何も起きない", dou, naka))
                    continue
                k = (app, c.id)
                if k in IMA:
                    dou, naka = DOU[k]
                    out.append((app, tab.name, c.label, c.id, IMA[k], dou, naka))
    return out


def main() -> int:
    r = rows()
    missing_ids = [k for k in list(IMA) + [(a, i) for a, i in DOU] if
                not any(x[0] == k[0] and x[3] == k[1] for x in r)]
    if missing_ids:
        print("元に無い id が表にあります:", missing_ids, file=sys.stderr)
        return 1
    if "--check" in sys.argv:
        print(f"見本の表は {len(r)} 行、元と揃っています")
        return 0

    if "--adoc" in sys.argv:
        print('[cols="1,1,2,1,3,1,3"]')
        print("|===")
        print("|アプリ |タブ |ボタン |id |いまの状態 |どうする |見本の中身\n")
        for a, t, lb, i, ima, dou, naka in r:
            app = "writer" if a == "WRITER" else "calc"
            print(f"|{app} |{t} |{lb} |{'—' if i == '(grey_of)' else '`' + i + '`'} "
                  f"|{ima} |{dou} |{naka}")
        print("|===")
    else:
        for a, t, lb, i, ima, dou, naka in r:
            print(f"{a:6} {t:6} {lb:22} {i:14} {dou:8} {ima}")
    n = sum(1 for x in r if x[5] == "見本")
    print(f"\n{len(r)} 行。見本にするのは {n} 個です。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
