#!/usr/bin/env python3
"""Check the documents published together with the engine (officework on PyPI).

    python3 tools/engine_docs_check.py           # check that everything is in place
    python3 tools/engine_docs_check.py --screen  # show where the screen words appear

The list is `docs/engine-docs.txt`. **The files are not moved**, decided on 2026-08-26,
because the PyPI page and links that are already published point at paths under docs/.
This list is the only place that says what gets published.

There are four checks.

1. every path in the list exists
2. the Japanese and English pairs are complete (English is the original; PyPI readers
   read English)
3. the links in `pysheet/README.md` point at something in the list
4. how often screen words (ribbon, button, panel and so on) appear

Only check 4 never fails. The mapping table is looked up from the buttons on screen, so
those words belong there. The tool just counts them and prints the count, and a person
decides whether it has grown. `--screen` also prints where they appear.
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
    """The paths in the list. Lines starting with `#` and blank lines are notes"""
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
    """Whether the docs/ paths the PyPI page points at are in the list.

    **A link outside the list sends readers to something that is not published.**
    Only files that are introduced with a note are allowed, through `SOTO_DEMO_II`.
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
