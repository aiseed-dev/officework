#!/usr/bin/env python3
"""利用者向けの文書に、内輪の注記が入っていないかを見る。

CLAUDE.md「利用者向けの文書は、さらに厳しい」の2つ目 —
*内輪の日付や決定の注記を入れない*。「(2026-08-09 発注者確定)」の類は
設計文書のものです。

    python3 tools/manual_uchiwa_check.py

**なぜ機械が要るか。** 書いている本人には内輪に見えません。
決めた経緯を覚えているうちは、その注記が親切に見えます。
使う人は経緯を知らないので、*読む邪魔にしかなりません*。

見るのは*使い方の帳簿*だけです。設計(SEKKEI と docs/sekkei/)と
在庫の台帳は経緯を残す場所なので見ません。
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# 見る文書(利用者が読む物)
MIRU = [
    # 一覧そのものも利用者が読む(2026-08-26 に「(発注者 …)」の注記を
    # 頭に書いてしまい、ここに入っていなかったので素通りした)
    "docs/ja/README.adoc",
    "docs/ja/from-excel.adoc",
    "docs/ja/genkou-manual.adoc",
    "docs/ja/api-taiou.adoc",
    "docs/ja/docx-xlsx-tono-chigai.adoc",
    "docs/ja/writer-manual.adoc",
    "docs/ja/calc-manual.adoc",
    "docs/ja/python-manual.adoc",
    "docs/ja/macro-manual.adoc",
    "docs/ja/writer-macro-manual.adoc",
    "docs/engine.ja.adoc",
]
# コマンドごとの手引きも全部見ます(段のフォルダの下)
MIRU += sorted(
    str(p.relative_to(ROOT))
    for p in (ROOT / "docs/ja/commands").rglob("*.adoc")
) if (ROOT / "docs/ja/commands").exists() else []

# 内輪の印。**書き方ではなく中身**で見る
NG = [
    (r"発注者", "決めた人の呼び名。使う人には関係がありません"),
    (r"20\d\d-\d\d-\d\d\s*(発注者|確定|決め|に足し|に入れ|改名|指摘)",
     "内輪の日付と決定の注記"),
    (r"20\d\d-\d\d-\d\d、", "内輪の日付"),
    (r"(当方|こちら)の案", "決める前の話。決まった事だけ書きます"),
    (r"手で直さないでください", "作る側への注意"),
    (r"が起こします", "生成の仕組み。使う人には関係がありません"),
    (r"仕事の一覧", "作る側の話"),
]

# 中身の日付(見本のデータなど)は内輪ではない
NOT = re.compile(r'"20\d\d-\d\d-\d\d"|\[20\d\d-\d\d-\d\d\]')


def main() -> int:
    悪い = []
    for rel in MIRU:
        p = ROOT / rel
        if not p.exists():
            continue
        for n, line in enumerate(p.read_text(encoding="utf-8").splitlines(), 1):
            if NOT.search(line):
                continue
            for pat, why in NG:
                if re.search(pat, line):
                    悪い.append((rel, n, why, line.strip()[:70]))
                    break
    if not 悪い:
        print(f"利用者向けの {len(MIRU)} 枚に、内輪の注記はありません")
        return 0
    print(f"**内輪の注記が {len(悪い)} 箇所あります。**", file=sys.stderr)
    print("使う人は経緯を知りません。設計(SEKKEI)に移してください。\n", file=sys.stderr)
    for rel, n, why, line in 悪い:
        print(f"  {rel}:{n}  {why}", file=sys.stderr)
        print(f"      {line}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
