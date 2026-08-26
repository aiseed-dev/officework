#!/usr/bin/env python3
"""日本語の識別子を英語にする機械(移行の段3)。

    python3 tools/rename_ids.py                # 数える(書かない)
    python3 tools/rename_ids.py --dict         # 辞書の骨組みを出す
    python3 tools/rename_ids.py --go           # 書き替える

## 文字列と註釈の中は触りません

これが一番大事です。`ui::t!("保存")` の「保存」は鍵で、`// 保存する` は
註釈です。どちらも識別子ではありません。字面で置き換えると、鍵が壊れて
訳が引けなくなり、註釈が英語混じりの読めない文になります。

だから**ソースを字句で読んで、文字列と註釈を先に伏せてから**識別子を
探します。伏せるのは `_伏せる()` の仕事です。

## 何を識別子と見るか

Rust は識別子に日本語を使えます(`let 段落 = …`)。日本語を含む語の
かたまりを1つの識別子と見ます。英数字と `_` は語の一部です
(`段落2` や `行_数` は1つの識別子)。

## 同じ名前にしないこと

2つの違う日本語を同じ英語にすると、同じ場所で衝突するか、静かに片方が
もう片方を隠します。`--go` の前に、辞書の中で英語が重なっていないかを
見ます。重なっていたら止まります。
"""
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
辞書の場所 = ROOT / "tools/rename_ids.json"

# **対象の綴り。** sample/ と templates/ は利用者向けの実例なので外します。
# packaging/ の下は組み立てのときの写しです
RS_DIRS = ["engine", "sheet", "ooxml", "paper", "lang", "ops", "pyrun",
           "face", "ui", "writer", "calc", "officework", "pysheet", "sidecar"]

# 日本語(かな・カナ・漢字)を含む語のかたまり
word = re.compile(r"[0-9A-Za-z_぀-ヿ一-鿿]+")
日本語 = re.compile(r"[぀-ヿ一-鿿]")


def _伏せる(src: str) -> str:
    """文字列・文字・註釈を空白で伏せた写しを返す。

    位置は変えません(伏せた分も同じ長さの空白にします)。こうすると、
    伏せた写しで見つけた位置を、そのまま元の字に使えます。
    """
    out = list(src)
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        # 行の註釈
        if c == "/" and i + 1 < n and src[i + 1] == "/":
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
            continue
        # 塊の註釈(入れ子あり)
        if c == "/" and i + 1 < n and src[i + 1] == "*":
            depth, j = 1, i + 2
            while j < n and depth:
                if src[j] == "/" and j + 1 < n and src[j + 1] == "*":
                    depth += 1
                    j += 2
                    continue
                if src[j] == "*" and j + 1 < n and src[j + 1] == "/":
                    depth -= 1
                    j += 2
                    continue
                j += 1
            for k in range(i, j):
                out[k] = " "
            i = j
            continue
        # 生の文字列 r"…" / r#"…"#
        if c == "r" and i + 1 < n and src[i + 1] in '"#':
            m = re.match(r'r(#*)"', src[i:])
            if m:
                closing = '"' + m.group(1)
                j = src.find(closing, i + m.end() - 1 + 1)
                j = n if j < 0 else j + len(closing)
                for k in range(i, j):
                    out[k] = " "
                i = j
                continue
        # 普通の文字列
        if c == '"':
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == '"':
                    j += 1
                    break
                j += 1
            for k in range(i, min(j, n)):
                out[k] = " "
            i = j
            continue
        # 文字リテラル。`'a'` と寿命 `'static` を見分ける
        if c == "'":
            m = re.match(r"'(?:\\.|[^\\'])'", src[i:])
            if m:
                for k in range(i, i + m.end()):
                    out[k] = " "
                i += m.end()
                continue
        i += 1
    return "".join(out)


def 対象のファイル():
    for d in RS_DIRS:
        for p in sorted((ROOT / d).rglob("*.rs")):
            yield p
    for p in sorted((ROOT / "tools").glob("*.py")):
        yield p
    for p in sorted((ROOT / "ui").glob("*.py")):
        yield p


def _伏せる_py(src: str) -> str:
    """Python 用。`#` の註釈と三重引用符・普通の引用符を伏せます。"""
    out = list(src)
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c == "#":
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
            continue
        if c in "\"'":
            # **f-string の `{…}` は中身がコードです。** 伏せてしまうと、
            # そこに出てくる識別子が書き替わりません。文字の部分だけ伏せて、
            # 波括弧の中は見えるようにします
            f付き = i > 0 and src[i - 1] in "fF"
            三 = src[i:i + 3]
            if 三 in ('"""', "'''"):
                j = src.find(三, i + 3)
                j = n if j < 0 else j + 3
            else:
                j = i + 1
                while j < n:
                    if src[j] == "\\":
                        j += 2
                        continue
                    if src[j] == c or src[j] == "\n":
                        j += 1
                        break
                    j += 1
            深 = 0
            for k in range(i, min(j, n)):
                if f付き and src[k] == "{":
                    深 += 1
                if not (f付き and 深):
                    out[k] = " "
                if f付き and src[k] == "}":
                    深 -= 1
            i = j
            continue
        i += 1
    return "".join(out)


def collect_into():
    """{識別子: {ファイル: 回数}} を返す。"""
    out = {}
    for p in 対象のファイル():
        src = p.read_text(encoding="utf-8", errors="replace")
        伏せた = _伏せる_py(src) if p.suffix == ".py" else _伏せる(src)
        for m in word.finditer(伏せた):
            s = m.group(0)
            if 日本語.search(s):
                out.setdefault(s, {}).setdefault(str(p.relative_to(ROOT)), 0)
                out[s][str(p.relative_to(ROOT))] += 1
    return out


def 辞書を読む():
    if not 辞書の場所.exists():
        return {}
    return json.loads(辞書の場所.read_text(encoding="utf-8"))


def 埋め込みも(src: str, 辞書) -> str:
    """文字列の中の `{識別子}` / `{識別子:書式}` を書き替える。

    Rust は `format!("{name}")` のように、文字列の中へ識別子を直に書けます。
    伏せた写しでは文字列を見ないので、ここだけ別に見ます。`{}` や `{0}`
    のような位置指定は識別子ではないので触りません。

    **画面の文言には効きません。** 鍵は記号(`save` など)で、穴埋めは
    位置指定の `{}` だけだからです(2026-08-26 に鍵を記号にしました)。
    """
    def one(m):
        新 = 辞書.get(m.group(1))
        return m.group(0) if 新 is None else "{" + 新 + (m.group(2) or "") + "}"

    return re.sub(r"\{([0-9A-Za-z_぀-ヿ一-鿿]+)(:[^}]*)?\}", one, src)


def 書き替える(辞書, dry=True):
    """辞書に載っている識別子だけを置き替える。

    **伏せた写しで位置を決め、元の字を切り貼りします。** 文字列や註釈に
    同じ字があっても動きません。
    """
    件数 = 0
    触った = 0
    for p in 対象のファイル():
        src = p.read_text(encoding="utf-8", errors="replace")
        伏せた = _伏せる_py(src) if p.suffix == ".py" else _伏せる(src)
        out, before = [], 0
        n = 0
        for m in word.finditer(伏せた):
            新 = 辞書.get(m.group(0))
            if not 新:
                continue
            out.append(src[before:m.start()])
            out.append(新)
            before = m.end()
            n += 1
        out.append(src[before:])
        新しい = 埋め込みも("".join(out), 辞書)
        if 新しい != src:
            件数 += max(n, 1)
            触った += 1
            if not dry:
                p.write_text(新しい, encoding="utf-8")
    return 件数, 触った


def main():
    item = collect_into()
    if "--dict" in sys.argv:
        既に = 辞書を読む()
        骨 = {k: 既に.get(k, "") for k in sorted(item, key=len, reverse=True)}
        print(json.dumps(骨, ensure_ascii=False, indent=1))
        return 0
    辞書 = {k: v for k, v in 辞書を読む().items() if v}
    重なり = {}
    for k, v in 辞書.items():
        重なり.setdefault(v, []).append(k)
    bad = {v: ks for v, ks in 重なり.items() if len(ks) > 1}
    if bad:
        print(f"!! 同じ英語に2つ以上の日本語が当たっています({len(bad)} 組)")
        for v, ks in list(bad.items())[:10]:
            print(f"   {v}: {ks}")
        return 1
    rs = sum(1 for p in 対象のファイル() if p.suffix == ".rs")
    print(f"日本語の識別子 {len(item)} 種 / 出てくる回数 "
          f"{sum(sum(d.values()) for d in item.values())}")
    print(f"  辞書に書いてある: {len(辞書)} 種")
    rest = [k for k in item if k not in 辞書]
    print(f"  まだ書いていない: {len(rest)} 種")
    件数, 触った = 書き替える(辞書, dry="--go" not in sys.argv)
    mark = "書き替えました" if "--go" in sys.argv else "書き替えます(--go で書き込み)"
    print(f"{件数} か所 / {触った} ファイルを{mark}(rs {rs} 枚を見ています)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
