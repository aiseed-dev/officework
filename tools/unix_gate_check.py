#!/usr/bin/env python3
"""**Windows に無い物を、旗の外から呼んでいないか**(2026-08-22)。

受け口(ソケット)は Windows では作りません。`ops::listen` / `ops::ask` /
`ops::sock_path` / `calc::rpc` は `#[cfg(unix)]` の向こうに居ます。

ところが**試験がうっかり旗の外から呼ぶ**と、手元(Linux)では緑のまま
Windows の CI だけが赤くなります。この機械では calc を Windows の的で
組めません(`ring` と `stacker` が Windows の C の道具を要る)ので、
組んで確かめる道がありません。だから字で探します。

2026-08-22 に2回踏みました。

* `ops::listen` の説明と `#[cfg(unix)]` の間に `ask` が挿し込まれ、
  **`listen` だけ旗の外**に出ていた
* 「開いて修復」の試験が `ops::Host::save` を旗の外から呼んでいた
  (同じ踏み跡が、すぐ上の試験の説明に書いてあったのに)

## 使い方

    python3 tools/unix_gate_check.py
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# Windows には無い物。呼ぶ側は `#[cfg(unix)]` の内側に居なければなりません
UNIX_ONLY = re.compile(r"\b(ops::Host\b|crate::rpc::|ops::listen|ops::ask|ops::sock_path|std::os::unix)")

# 見る場所。生成物と外から持ってきた物は見ません
check = ["calc/src", "writer/src", "ui/src", "officework/src", "ops/src", "sheet/src"]


def flag_line(l: str) -> bool:
    """その行が `#[cfg(unix)]` の**属性**か。

    **説明の中の字を数えません。** 数えると、「ここは cfg(unix) の外です」と
    書いた注意書きが、旗そのものに見えてしまいます(最初に書いた版がそうで、
    わざと旗を外した確かめを見逃しました)。
    """
    t = l.strip()
    return t.startswith("#[cfg(") and "unix" in t and "not(unix)" not in t


def per_file_flagged(p: pathlib.Path) -> bool:
    """このファイルを取り込む `mod` の行に `#[cfg(unix)]` が付いているか。

    `calc/src/rpc.rs` は `calc/src/lib.rs` の `#[cfg(unix)] pub mod rpc;` で
    取り込まれます。**ファイルごと Windows では組まれない**ので、中では
    旗が要りません。ここを見ないと、正しい物を誤って咎めます
    (最初に書いた版が3件そう言いました)。
    """
    parent = p.parent / "lib.rs"
    if not parent.exists() or p.name == "lib.rs":
        return False
    # `#[cfg(unix)]` と `mod` の間に説明の行(`///`)が挟まることがあります。
    # そこを見落として、正しい物を3件咎めました
    lines = parent.read_text(encoding="utf-8").split("\n")
    decl = re.compile(r"\s*(pub )?mod " + re.escape(p.stem) + r"\s*;")
    for i, l in enumerate(lines):
        if not decl.match(l):
            continue
        # 上へ遡り、説明と空行は飛ばして旗を探す
        for j in range(i - 1, max(-1, i - 8), -1):
            t = lines[j].strip()
            if t.startswith("///") or t.startswith("//") or not t:
                continue
            return flag_line(t)
    return False


def inside_flag(lines: list[str], i: int) -> bool:
    """`i` 行目が `#[cfg(unix)]` の効く所に居るか。

    近くの数行と、いま居る関数の頭の両方を見ます。**関数の頭に付いている
    ことが多い**ので、近くだけを見ると取りこぼします(最初に書いた版が
    そうでした)。
    """
    if any(flag_line(l) for l in lines[max(0, i - 6):i]):
        return True
    for j in range(i, -1, -1):
        if re.match(r"\s*(pub(\(\w+\))? )?(async )?fn ", lines[j]):
            return any(flag_line(l) for l in lines[max(0, j - 8):j + 1])
        # モジュールの頭まで来たら、そこを見て終わり
        if re.match(r"\s*(pub )?mod \w+ \{", lines[j]):
            return any(flag_line(l) for l in lines[max(0, j - 4):j + 1])
    return False


def main() -> int:
    leak = []
    numbers = 0
    for d in check:
        for p in sorted((ROOT / d).rglob("*.rs")):
            if per_file_flagged(p):
                continue  # `#[cfg(unix)] mod ...;` で取り込まれている
            lines = p.read_text(encoding="utf-8").split("\n")
            for i, l in enumerate(lines):
                # 自分の宣言と説明は数えない
                if l.lstrip().startswith(("///", "//", "#[cfg")):
                    continue
                if not UNIX_ONLY.search(l):
                    continue
                numbers += 1
                if not inside_flag(lines, i):
                    leak.append((p.relative_to(ROOT), i + 1, l.strip()[:70]))
    if numbers < 5:
        # **読めなくなったら落ちる。** 静かに緑になるのが一番悪い
        print(f"::error::Windows に無い物の呼び出しが {numbers} か所しか見つかりません(探し方が壊れた?)")
        return 1
    for f, n, t in leak:
        print(f"::error::{f}:{n} が `#[cfg(unix)]` の外から Windows に無い物を呼んでいます")
        print(f"    {t}")
    if leak:
        print("\n  受け口は Windows では作りません(ops と calc::rpc は cfg(unix))。")
        print("  呼ぶ側にも同じ旗を付けてください。この機械では Windows の的で")
        print("  calc を組めないので、CI の Windows の段まで気づけません。")
        return 1
    print(f"Windows に無い物の呼び出し {numbers} か所は、全部 `#[cfg(unix)]` の内側です")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
