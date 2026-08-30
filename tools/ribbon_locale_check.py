"""**13言語のリボンが ja と同じ骨組みかを、組み立てずに見る。**

`face/src/ribbon.rs`(ja)と `face/src/ribbon_<loc>.rs`(生成物)は、
**語だけが違って id・並び・ready・icon は同じ**でなければならない。
ずれると、日本語では押せるボタンがドイツ語では灰色、といったことが起きる。
`c(…)` が `x(…)` に化けていれば、その言語だけボタンが死ぬ。

同じ照合は `face/src/ribbon.rs` の `各言語の表は語だけが違う` がしている。
**書いた当時、ui は CI で走っていなかった** — `.github/workflows/ci.yml` が
gpui の連結を避けて calc・writer・ui を外していたため。2026-08-10、同じ
理由で `wiring_tests`(押しても何も起きないボタンを止める検査)が CI の
外にあることが分かり、`tools/wiring_check.py` で塞いだ。**これはその一段下**
— 塞いだのは ja の配線だけで、他の12言語の骨組みは誰も見ていなかった。

*2026-08-21 に ui の試験も CI に入りました。* この検査は残す。あちらは
gpui を組むので遅く、こちらは字面を読むだけで速い — **先に落ちてくれる**
方が直しやすい。

だからここも**原文の表を読む**。`ribbon.rs` は gpui を1度も使っておらず、
生成物も素のリテラルなので、コンパイラは要らない。

**この検査は、見えなくなったときに落ちる。** 字面を読む検査の危険は
「書き方が変わって何も拾えなくなり、静かに緑になる」ことで、それは
この検査が止めたい欠陥と同じ形。だから拾えた数が少なすぎたら落とす。
"""

from __future__ import annotations

import functools
import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import ribbon_parse  # noqa: E402  (表を読むのはここだけ)

ROOT = pathlib.Path(__file__).resolve().parent.parent
# リボンの表と14言語は face(gpui を持たない層)へ移った(2026-08-15)
UI = ROOT / "face/src"

# 拾えた数がこれを下回ったら「読めていない」と見なす。
# いまは CALC・WRITER とも 200 前後なので、半分を切ったら書き方が変わった
FLOOR = 80


@functools.lru_cache(maxsize=None)
def _tables(path: pathlib.Path):
    """**表を読むのは `ribbon_parse` だけ。**

    ここには3つの読み手(骨組み・タブ・語)があり、**3つとも別々の正規表現で
    同じ物を拾っていた**(2026-08-12 に統合)。拾う形は拾えなかった物を黙って
    捨てるので、書き方が変われば3つとも静かに減る。いまは食べ尽くす形で、
    知らない書き方が1つでもあれば読む前に落ちる。
    """
    return ribbon_parse.tables_or_die(path)


def buttons(path: pathlib.Path, table: str) -> list[tuple[str, str, str]]:
    """(id, icon, 書き方)を順に。

    **灰色も印だけ積む** — 数だけ合っていて並びがずれる、を見逃さないため。

    書き方(`c` 押す / `t` 入切 / `x` `xt` `xm` の灰色)も見ます。
    ボタンの性格は語ではないので、**どの言語でも同じ**でなければなりません
    (2026-08-21 に入切を足したとき、片方の言語だけ押す形のまま、が起きうる)。
    """
    return [
        (c.id, c.icon, c.kind) if c.ready else ("", "", c.kind)
        for tab in _tables(path)[table]
        for c in tab.cmds
    ]


def tabs(path: pathlib.Path, table: str) -> list[str]:
    """タブの並び。**名前は訳されるので数だけ**見る。"""
    return [tab.name for tab in _tables(path)[table]]


def labels(path: pathlib.Path, table: str) -> list[str]:
    """ボタンに**出る語**を順に。骨組みではなく中身を見るときに使う。"""
    return [c.label for tab in _tables(path)[table] for c in tab.cmds]


def same_words(locales: list[str]) -> int:
    """**語まで丸ごと同じ2言語が無いこと。**

    2026-08-11、欧州ポルトガル語の札を `pt-PT` から `pt` に変えたら、
    生成器が本家の `pt.json`(こちらの綴りと逆で**ブラジル語**)を
    黙って拾い、欧州版のリボンがブラジル語で出来上がっていた。
    骨組みの検査は id と並びしか見ないので、**中身が別言語でも緑**だった。

    2つの言語が1文字も違わないなら、まず同じ材料から作っている。

    **この検査だけでは足りない。** 当時は pt-BR がまだ無く、比べる相手が
    いなかったので、これでも見つけられなかった。実際に効いたのは
    「本家の欧州ファイルは薄いので、読み替えれば訳の欠けが露見する」
    ほうだった。二重に見る。
    """
    bad = 0
    for table in ("CALC", "WRITER"):
        seen: dict[tuple[str, ...], str] = {}
        for loc in locales:
            key = tuple(labels(UI / f"ribbon_{loc}.rs", table))
            if key in seen:
                print(
                    f"::error::{table}: {seen[key]} と {loc} の語が完全に同じです。"
                    "同じ材料から作っていませんか"
                    "(gen_ribbon_locale.py の VENDOR_LOCALE を確かめてください)"
                )
                bad = 1
            seen[key] = loc
    return bad


def duplicate_labels(locales: list[str]) -> int:
    """**同じタブに同じラベルのボタンが2つ無いこと。**

    2026-08-21、表のホームに「セルのスタイル」が2つ並んでいました。
    書式設定の小窓を開くボタンと、見た目の一覧を開くボタンです。
    生成スクリプトが両方を本家の同じ語から引いていたのが原因で、
    日本語だけでなく全部の言語に出ていました。

    同じ日に、これを数えて**別に5件**見つかりました。

    * トルコ語「表の挿入」と「グラフを挿入」— 本家の誤訳
    * ロシア語・中国語・ポルトガル語(ブラジル)の
      「印刷タイトル」と「見出しも印刷」— 上書き表の訳が重なっていた
    * 中国語(簡体)の「ルビ」と「ふりがな」— 上書き表で同じ語にしていた

    どれも日本語には出ないので、日本語だけ見ていては気づけません。
    ボタンがアイコンだけのときラベルは吹き出しにしか出ないので、
    画面を見ていても見落とします。だから機械で数えます。
    """
    bad = 0
    for loc in ["ja"] + locales:
        name = "ribbon.rs" if loc == "ja" else f"ribbon_{loc}.rs"
        for table in ("CALC", "WRITER"):
            for tab in _tables(UI / name)[table]:
                seen: dict[str, list[str]] = {}
                for c in tab.cmds:
                    seen.setdefault(c.label, []).append(c.id or c.icon)
                for label, who in seen.items():
                    if len(who) > 1:
                        print(
                            f"::error::{loc} {table}/{tab.name}: "
                            f"{label!r} というラベルのボタンが {len(who)} 個あります "
                            f"({' and_of '.join(who)})。"
                            "押すまでどちらか分かりません"
                        )
                        bad = 1
    return bad


# **同じラベルなのに id が違ってよい物**(2026-08-21 の B-2)。
#
# 文章と表は本家の別々のアプリから写したので、同じ働きのボタンに別の id が
# 付いていました。4組を揃えましたが、*ラベルが同じでも別の働き*の物が
# 残ります。それをここに理由つきで書きます。表に無い組が出たら止まります。
#
# 揃えるかどうかは**処理を読んで決めます**。ラベルだけで数えると間違えます
# (実際、2026-08-21 に私がチェックボックスと日付/時刻を「同じ物」と数えて
# いました)。
#
# **鍵は英語です**(2026-08-26 の段2でリボンの札が英語になりました)。
different_job = {
    "Group": (
        {"img-group"}, {"group", "img-group"},
        "図形をひとまとまりにする img-group は両方にある。表にはもう1つ、"
        "データタブの group がある — こちらは行や列を折りたたむ"
        "アウトラインで、図形とは別の働き(Excel も同じ札を使う)",
    ),
    "Checkbox": (
        {"form-checkbox"}, {"inscheckbox"},
        "文章は文書の入力欄(記入欄の仲間)、表は選んだセルに TRUE/FALSE を書く",
    ),
    "Date & Time": (
        {"datetime"}, {"fn-datetime"},
        "文章は今日の日付を差し込む、表は日付の関数の一覧を出す",
    ),
}


def cross_app_ids() -> int:
    """**同じラベルなら同じ id**(理由を書いた物を除く)。

    同じ働きのボタンに別の id が付いていると、rpc・MCP・Python から
    アプリごとに違う名前で呼ぶことになります。押した先も別々に書くことに
    なるので、**写しがずれます**(`ai-where` が実際にずれていました)。
    """
    t = _tables(UI / "ribbon.rs")

    def collect_into(table: str) -> dict[str, set[str]]:
        d: dict[str, set[str]] = {}
        for tab in t[table]:
            for c in tab.cmds:
                if c.id:
                    d.setdefault(c.label, set()).add(c.id)
        return d

    w, c = collect_into("WRITER"), collect_into("CALC")
    bad = 0
    seen = set()
    for label in sorted(set(w) & set(c)):
        if w[label] == c[label]:
            continue
        exc = different_job.get(label)
        if exc and exc[0] == w[label] and exc[1] == c[label]:
            seen.add(label)
            continue
        print(
            f"::error::{label!r} が、文章では {sorted(w[label])}・"
            f"表では {sorted(c[label])} という別の id です。"
            "同じ働きなら id を揃えてください。別の働きなら "
            "tools/ribbon_locale_check.py の 別の働き に理由を書いてください"
        )
        bad = 1
    remainder = set(different_job) - seen
    if remainder:
        print(
            f"::error::別の働き に書いてあるのに、いま食い違っていない組があります: "
            f"{sorted(remainder)}。直したのなら表からも消してください"
        )
        bad = 1
    return bad


def main() -> int:
    locales = sorted(
        p.stem[len("ribbon_"):]
        for p in UI.glob("ribbon_*.rs")
        if p.stem not in ("ribbon_tables",)
    )
    if not locales:
        print("::error::face/src に ribbon_<loc>.rs がありません")
        return 1

    bad = 0
    for table in ("CALC", "WRITER"):
        ja = buttons(UI / "ribbon.rs", table)
        if len(ja) < FLOOR:
            print(f"::error::{table}: ja の表が読めていません(ボタン {len(ja)} 件)")
            bad = 1
            continue
        ja_tabs = len(tabs(UI / "ribbon.rs", table))
        for loc in locales:
            got = buttons(UI / f"ribbon_{loc}.rs", table)
            if len(got) != len(ja):
                print(
                    f"::error::{table} {loc}: ボタンの数が違います"
                    f"(ja {len(ja)} / {loc} {len(got)})"
                )
                bad = 1
                continue
            n = len(tabs(UI / f"ribbon_{loc}.rs", table))
            if n != ja_tabs:
                print(f"::error::{table} {loc}: タブの数が違います(ja {ja_tabs} / {loc} {n})")
                bad = 1
            for i, (a, b) in enumerate(zip(ja, got)):
                if a != b:
                    print(
                        f"::error::{table} {loc}: {i} 番目のボタンがずれています "
                        f"— ja (id={a[0]!r} icon={a[1]!r} ready={a[2]}) / "
                        f"{loc} (id={b[0]!r} icon={b[1]!r} ready={b[2]})"
                    )
                    bad = 1
                    break
        if not bad:
            print(f"{table}: {len(locales)} 言語とも ja と同じ骨組み(ボタン {len(ja)} 件)")
    bad |= same_words(locales)
    dup = duplicate_labels(locales)
    bad |= dup
    if not dup:
        print(f"ラベルの重複: ja と {len(locales)} 言語とも、同じタブに同じラベルはありません")
    cross = cross_app_ids()
    bad |= cross
    if not cross:
        print(
            "アプリをまたぐ id: 同じラベルのボタンは同じ id です"
            f"(別の働きだと書いてある物が {len(different_job)} 組)"
        )
    if not bad:
        print(f"語の重なり: {len(locales)} 言語とも別の語で出来ています")
    return bad


if __name__ == "__main__":
    raise SystemExit(main())
