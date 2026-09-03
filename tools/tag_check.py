#!/usr/bin/env python3
"""**docx / xlsx に出てくる札を全部数えて、読み手が見ていない物を出す。**

    python3 tools/tag_check.py                 # 手元の見本を全部
    python3 tools/tag_check.py あるファイル.docx

いままでは画面を見て「ここが違う」と気づいた1件ずつを直していました。
それだと、**気づかなかった物はいつまでも残ります**。この道具は逆で、
ファイルの側から「書いてあるのに、うちのコードがどこにも書いていない札」
を並べます。

見ているかどうかは、**読み手の原文にその名前が出てくるか**で判断します。
粗い網です — 名前があっても正しく読めているとは限りません。
それでも「1文字も触れていない」を見つけるには十分です。

数の多い順に並べます。よく出る札ほど、直したときの効きが大きいためです。
"""
import re
import sys
import zipfile
from collections import Counter
from pathlib import Path

NE = Path(__file__).resolve().parent.parent

# 読み手の原文。ここに名前が出てくれば「見ている」とみなします
YOMITE = [
    "ooxml/src/read.rs", "ooxml/src/theme.rs", "ooxml/src/write.rs",
    "sheet/src/xlsx/read.rs", "sheet/src/xlsx/theme.rs", "sheet/src/xlsx/styles.rs",
    "sheet/src/xlsx/write.rs", "engine/src/doc.rs", "engine/src/layout.rs",
    "engine/src/theme.rs", "paper/src/lib.rs", "paper/src/grid.rs",
]

# **わざと見ない札**と、その理由。ここに書いた物は数から外します。
# 理由を書けない物は外しません — 黙って落とす所を作らないためです
WAZATO = {
    "chart": "グラフの模型は持たない(描くのは matplotlib。原文は持ち越す)",
    "chartSpace": "同上",
    "extLst": "拡張の入れ物。中身は版ごとに違い、原文のまま持ち越す",
    "ext": "同上",
    "mc:AlternateContent": "選択肢の入れ物。中の Choice / Fallback を見る",
    "rsid": "編集の履歴の印。見た目に関わらない",
    "proofErr": "綴りの検査の印。見た目に関わらない",
    "bookmarkEnd": "しおりの終わり。始まりだけ見る",
    "lastRenderedPageBreak": "Word が入れる組み直しの印。こちらで組み直す",
    "rsidR": "編集の履歴の印", "rsidRPr": "同上", "rsidP": "同上",
    "rsidRDefault": "同上", "rsidTr": "同上", "rsidSect": "同上", "rsidDel": "同上",
    "paraId": "Word が段落に振る番号。見た目に関わらない",
    "textId": "同上",
    "dyDescent": "Excel が控える行の下端。行の高さは自分で組む",
    "nsid": "箇条書きの定義の番号。印は lvlText から読む",
    "tmpl": "同上", "multiLevelType": "同上",
    "charset": "書体の情報の控え。書体は名前で探す",
    "family": "同上", "pitch": "同上", "sig": "同上", "panose1": "同上",
    "usb0": "同上", "usb1": "同上", "usb2": "同上", "usb3": "同上",
    "csb0": "同上", "csb1": "同上",
    "element": "組み込みのスキーマ(customXml)。本文ではない",
    "complexType": "同上", "sequence": "同上", "schema": "同上",
    "attribute": "同上", "simpleType": "同上", "restriction": "同上",
    "compatSetting": "互換の切り替え。原文のまま持ち越す(compatibilityMode だけは"
                     "w:tblInd の測り方を決めるので読む)",
    # --- 表の決まりで、**わざと読まない**物(2026-09-03 に1つずつ判断しました)
    "cnfStyle": "行やセルが名乗る表スタイルの条件。うちは位置から出していて、"
                "正しく書かれた文書では同じ答えになる",
    "cantSplit": "行を紙の切れ目で割らない印。うちは元から割らない"
                 "(入る行は丸ごと次の紙へ送る)ので、有っても無くても同じ",
    "gridBefore": "行の頭の空いた格子。実物 55 冊の本文に1つも無い",
    "gridAfter": "同上(行の末)", "wBefore": "同上", "wAfter": "同上",
    "hMerge": "Word 2003 までの横の結合。いまの Word は w:gridSpan で書く",
    "noWrap": "セルの中で折り返さない指定。実物の本文に無い",
    "tblpPr": "紙に浮かせて置く表。実物の本文に無い",
    "tblOverlap": "同上(重なりの許し)", "tblCellSpacing": "セルとセルの間の空き。実物の本文に無い",
    "tblCaption": "表の題。読み上げのための字で、紙には出ない",
    "tblDescription": "同上", "hideMark": "空のセルの行の高さを詰める印。紙には出ない",
    "bidiVisual": "右から左へ読む表。日本語の様式では使わない",
    "hRule": "行の高さの決まり。exact(固定)は実物に1つも無く、いまは常に下限として扱う",
    "customStyle": "自分で作ったスタイルの印。定義は styleId で引く",
}

# **見ているのに一覧へ出る物**と、その理由。
# 属性の中には「解決済みの値が別の属性に入っている」ものがあり、
# こちらの属性を読まなくても正しい色が出ます
KAIKETSU_ZUMI = {
    "themeColor": "w:val に解決済みの色が入っている(テーマを変えると Word が書き直す)",
    "themeFill": "w:fill に解決済みの色が入っている",
    "themeFillTint": "同上", "themeFillShade": "同上",
    "themeTint": "同上", "themeShade": "同上",
}

def part_tags(xml: str):
    """1つの部品に出てくる札と属性を数える"""
    fuda = Counter(re.findall(r"<([A-Za-z0-9]+:[A-Za-z0-9_]+)", xml))
    zoku = Counter(re.findall(r"\s([A-Za-z0-9]+:[A-Za-z0-9_]+)=\"", xml))
    return fuda, zoku

def main(argv):
    mato = [Path(a) for a in argv[1:]]
    if not mato:
        for d in (Path.home() / "dev/test",):
            mato += sorted(d.rglob("*.docx")) + sorted(d.rglob("*.xlsx"))
        mato = [p for p in mato if "~$" not in p.name][:40]
    if not mato:
        print("見るファイルがありません")
        return 0

    src = "\n".join(
        (NE / f).read_text(encoding="utf-8") for f in YOMITE if (NE / f).exists()
    )

    fuda, zoku = Counter(), Counter()
    for p in mato:
        try:
            z = zipfile.ZipFile(p)
        except Exception:
            continue
        for n in z.namelist():
            if not n.endswith(".xml"):
                continue
            try:
                x = z.read(n).decode("utf-8", "replace")
            except Exception:
                continue
            a, b = part_tags(x)
            fuda.update(a)
            zoku.update(b)

    def mite_iru(name: str) -> bool:
        tan = name.split(":", 1)[1]
        # 読み手は接頭辞を落として見ることが多いので、名前だけで探します
        return (f'"{tan}"' in src or f"b\"{tan}\"" in src or f"<w:{tan}" in src
                or f"<a:{tan}" in src or f"{tan}=" in src or f'"{name}"' in src)

    for na, kaz, midashi in ((fuda, "札", "= 見ていない札"), (zoku, "属性", "= 見ていない属性")):
        nai = [(n, c) for n, c in na.items()
               if not mite_iru(n)
               and n not in WAZATO and n.split(":", 1)[1] not in WAZATO
               and n.split(":", 1)[1] not in KAIKETSU_ZUMI]
        nai.sort(key=lambda t: -t[1])
        miru = len(na) - len(nai)
        print(f"\n{midashi}(全 {len(na)} 種のうち {miru} 種は触れている)")
        for n, c in nai[:40]:
            print(f"  {c:6d} 回  {n}")
        if len(nai) > 40:
            print(f"  … ほか {len(nai) - 40} 種")
    print(f"\n見たファイル {len(mato)} 個")
    return 0

if __name__ == "__main__":
    sys.exit(main(sys.argv))
