#!/usr/bin/env python3
"""エンジン(PyPI の officework)と一緒に公開する文書を見る。

    python3 tools/engine_docs_check.py           # 揃っているか見る
    python3 tools/engine_docs_check.py --screen  # 画面の言葉が出る所を出す

一覧は `docs/engine-docs.txt` です。**場所は動かしません**(2026-08-26 の
決め。PyPI の頁や公開済みのリンクが docs/ の径路に刺さっているためです)。
どれを公開するかは、この一覧だけが持ちます。

見るのは4つです。

1. 一覧に書いた径路が実在すること
2. 日英の対が揃っていること(英語が原本。PyPI の読み手は英語です)
3. `pysheet/README.md` のリンク先が、一覧の中にあること
4. 画面の言葉(リボン・ボタン・パネルなど)が出る所を数えること

4だけは**落としません**。対応表は画面のボタンから引く表なので、その言葉が
出るのが正しい形です。数えて出すだけにして、増えたかどうかを人が見ます。
`--screen` で場所まで出ます。
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ICHIRAN = ROOT / "docs/engine-docs.txt"

# 画面が要る話の目印。日本語と英語の両方を見ます
GAMEN_JA = ("リボン", "ボタン", "パネル", "ダイアログ", "右クリック")
GAMEN_EN = ("ribbon", "button", "panel", "dialog", "right-click")

# 数えない物。権利の表示は、画面の話ではなく出どころの話です
KAZOENAI = {"LICENSE", "NOTICE.md"}


def ichiran() -> list:
    """一覧の径路。`#` から始まる行と空行は註記です"""
    out = []
    for line in ICHIRAN.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            out.append(line)
    return out


def tsui(paths: list) -> list:
    """日英の対の欠け。`docs/ja/x` に対する `docs/en/x` を見ます"""
    ari = set(paths)
    warui = []
    for p in paths:
        if "/ja/" in p and p.replace("/ja/", "/en/") not in ari:
            warui.append(f"{p} は一覧にありますが、英語版が一覧にありません")
        if "/en/" in p and p.replace("/en/", "/ja/") not in ari:
            warui.append(f"{p} は一覧にありますが、日本語版が一覧にありません")
    return warui


# PyPI の頁が指してよい、一覧の外の物。**作る側の資料へ「リポジトリの中に
# あります」と断って案内する分**です(在庫台帳など)。読者を公開しない物へ
# 黙って送るのとは違うので、ここに名前を書いて許します
SOTO_DEMO_II = {
    "docs/pysheet-gokan.ja.adoc",   # 互換の在庫台帳。README が「in the repo」と断る
}


def pypi_no_link(paths: list) -> list:
    """PyPI の頁が指す docs/ の径路が、一覧の中にあるか。

    **一覧の外を指していたら、公開しない物へ読者を送っています。**
    断って案内している物だけ `SOTO_DEMO_II` で許します。
    """
    src = (ROOT / "pysheet/README.md").read_text(encoding="utf-8")
    ari = set(paths) | SOTO_DEMO_II
    warui = []
    for m in re.finditer(r"\]\(([^)]*docs/[^)#]+)", src):
        saki = "docs/" + m.group(1).split("/docs/")[-1]
        if saki not in ari and (ROOT / saki).exists():
            warui.append(f"pysheet/README.md が {saki} を指していますが、一覧にありません")
    return warui


# **エンジンだけで使える冊子**。一覧から落ちたら気づくための控えです。
# 一覧を消しても検査が緑のままなら、見張っていることになりません
ENGINE_SASSHI = (
    "python-manual", "functions", "df-manual", "tutorial-word",
    "tutorial-calc", "docx-xlsx-tono-chigai", "api-taiou", "genkou-manual",
)


# 一覧から落ちてはいけない物。PyPI の頁は公開の入り口そのものです
KANARAZU = ("pysheet/README.md", "LICENSE", "NOTICE.md")


def ochita(paths: list) -> list:
    """一覧から落ちた冊子。**消しても緑にならないため**の見張りです"""
    ari = set(paths)
    warui = [f"{p} が一覧から落ちています" for p in KANARAZU if p not in ari]
    for n in ENGINE_SASSHI:
        for d in ("ja", "en"):
            p = f"docs/{d}/{n}.adoc"
            if (ROOT / p).exists() and p not in ari:
                warui.append(f"{p} は実在しますが、一覧から落ちています")
    return warui


def gamen(paths: list) -> dict:
    """画面の言葉が出る回数。**落としません** — 数えて出すだけです"""
    out = {}
    for p in paths:
        if pathlib.Path(p).name in KAZOENAI:
            continue
        t = (ROOT / p).read_text(encoding="utf-8")
        go = GAMEN_JA if "/ja/" in p else GAMEN_EN
        n = {w: len(re.findall(w, t, re.I)) for w in go}
        n = {k: v for k, v in n.items() if v}
        if n:
            out[p] = n
    return out


def main() -> int:
    paths = ichiran()
    warui = [f"{p} がありません" for p in paths if not (ROOT / p).exists()]
    warui += tsui(paths)
    warui += pypi_no_link(paths)
    warui += ochita(paths)

    if "--screen" in sys.argv:
        for p, n in gamen(paths).items():
            print(f"{p}")
            t = (ROOT / p).read_text(encoding="utf-8").splitlines()
            go = GAMEN_JA if "/ja/" in p else GAMEN_EN
            for i, line in enumerate(t, 1):
                if any(re.search(w, line, re.I) for w in go):
                    print(f"  {i:5} {line.strip()[:100]}")
        return 0

    if warui:
        print(f"::error::エンジンと公開する文書の一覧が合っていません({len(warui)} 件)",
              file=sys.stderr)
        for w in warui:
            print(f"  {w}", file=sys.stderr)
        return 1

    n = len(gamen(paths))
    print(f"エンジンと公開する文書 {len(paths)} 件、そろっています。"
          f"画面の言葉が出る冊子は {n} 件です(--screen で場所が出ます)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
