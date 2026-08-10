#!/usr/bin/env python3
"""画面の文言の対訳表の門番。

アプリの `ui::t!("…")` / `ui::tf!("…", …)` から**日本語の鍵**を全部抽出し、
lang/src/i18n_en.rs の対訳表と突き合わせる。

    python3 ui/gen_i18n.py            # 検査(未訳・不要訳があれば止まる)
    python3 ui/gen_i18n.py --missing  # 未訳の鍵を骨組み(("鍵", ""),)で出す

**未訳があるうちは en を名乗れない**(文言の揃った言語だけを名乗る方針)。
新しい文言を足したら、--missing の骨組みに訳を書いて表へ足すこと。
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# **両方のアプリと ui の全部屋を見る**(試験は除く)。
#
# 前は writer だけ main.rs を名指ししていた。writer も途中まで部屋割り
# されていて、cmds.rs・io.rs・view.rs の 187 句を門番が見ていなかった。
# 見ていない鍵の訳は「使われていない訳」に数えられる — つまり門番が
# **生きている訳 135 句を消せ**と言っていた(2026-08-10 に気づいた)。
# 部屋が増えたら足す、ではなく、**glob で全部見る**
SOURCES = sorted(
    p
    for d in ("calc/src", "writer/src", "ui/src")
    for p in (ROOT / d).glob("*.rs")
    if p.name != "tests.rs"
)
TABLE = ROOT / "lang/src/i18n_en.rs"


def literal_at(src, i):
    j = i + 1
    while j < len(src):
        if src[j] == "\\":
            j += 2
            continue
        if src[j] == '"':
            return j + 1, src[i:j + 1]
        j += 1
    raise ValueError("unterminated literal")


def keys_from(path):
    src = open(path, encoding="utf-8").read()
    # 試験モジュールから先は見ない。ただし `#[cfg(test)] mod tests;` の
    # **宣言**は本文の頭にある(calc の部屋割り)— そこでは切らない
    pos = 0
    while True:
        cut = src.find("#[cfg(test)]", pos)
        if cut < 0:
            break
        lines = src[cut:cut + 64].splitlines()
        next_line = lines[1].strip() if len(lines) > 1 else ""
        if next_line.startswith("mod ") and next_line.endswith(";"):
            pos = cut + 1  # 宣言 — 本文はこの先も続く
            continue
        src = src[:cut]
        break
    out = []
    # `ui::item!("…")` は一覧の項の鍵(訳すのは見出しだけ)。t!/tf! と同じ鍵。
    # **lang/tests/i18n_soroi.rs の走査と揃えること** — 片方だけ知っていると、
    # 生きている訳を「使われていない」と数えて消せと言い出す(2026-08-10 の一敗)
    for m in re.finditer(r"ui::(?:tf?|item)!\(\s*", src):
        j = m.end()
        if j < len(src) and src[j] == '"':
            _, lit = literal_at(src, j)
            out.append(lit)
    return out


def table_keys():
    """表の各行の鍵リテラルを、リテラル走査で取り出す(複数行の鍵も)"""
    src = open(TABLE, encoding="utf-8").read()
    out = []
    i = src.find("pub const")
    while True:
        i = src.find('("', i)
        if i < 0:
            break
        j, lit = literal_at(src, i + 1)
        out.append(lit)
        # 値のリテラルを読み飛ばす
        k = src.find('"', j)
        if k < 0:
            break
        i, _ = literal_at(src, k)
    return out


def unescape(lit):
    """リテラルを**実行時の文字列**に。行継続(行末の `\\`)も畳む。

    **字面で比べると重複が見えない。** 同じ文を、片方は1行で、片方は
    行末の `\\` で継いで書ける。ソースでは別物でも実行時は同じ鍵なので、
    `HashMap` に畳んだとき**片方の訳が画面に出なくなる**。
    2026-08-11 に 13 の表すべてで3件ずつ見つかった
    """
    s = lit[1:-1] if lit.startswith('"') else lit
    out, i = [], 0
    while i < len(s):
        if s[i] == "\\" and i + 1 < len(s):
            c = s[i + 1]
            if c == "\n":  # 行継続。続く字下げも食う
                i += 2
                while i < len(s) and s[i] in " \t":
                    i += 1
                continue
            out.append({"n": "\n", "t": "\t", "r": "\r"}.get(c, c))
            i += 2
        else:
            out.append(s[i])
            i += 1
    return "".join(out)


def main():
    used = []
    for p in SOURCES:
        used.extend(unescape(k) for k in keys_from(p))
    used_set = dict.fromkeys(used)  # 順を保った一意化
    table = [unescape(k) for k in table_keys()]
    table_set = set(table)

    missing = [k for k in used_set if k not in table_set]
    extra = [k for k in table_set if k not in used_set]
    dup = {k for k in table if table.count(k) > 1}

    if "--missing" in sys.argv:
        for k in missing:
            print(f"    ({k}, \"\"),")
        return

    ok = True
    if missing:
        ok = False
        print(f"未訳の鍵が {len(missing)} 個(--missing で骨組みを出せます)")
    if extra:
        ok = False
        print(f"使われていない訳が {len(extra)} 個:")
        for k in sorted(extra)[:20]:
            print(f"  {k}")
    if dup:
        ok = False
        print(f"重複した鍵が {len(dup)} 個: {sorted(dup)[:5]}")
    if not ok:
        sys.exit(1)
    print(f"OK: {len(used_set)} 句すべてに訳がある")


if __name__ == "__main__":
    main()
