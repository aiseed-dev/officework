#!/usr/bin/env python3
"""**単体の `run()` にあって officework に無い仕掛けを数える**(2026-08-21)。

配るのは `officework` の1本です。`calc` と `writer` の単体は開発と試験の
道具として残っています。ところが**起動のときの仕掛けが単体の `run()` の
中に書いてある**と、配っているアプリでは動きません。

## この検査が見つけた実物

2026-08-21 に数えて、**2つ**見つかりました。どちらも実機で確かめています。

* **自動復旧の控え** — 表も文章も、控えを1つも取っていませんでした。
  書き替えて落ちると、打った分は全部失われます
* **式から呼ぶ Python の関数(UDF)** — `funcs/*.py` を読む所が
  呼ばれておらず、`=倍(A1)` が `#NAME?` になっていました

どちらも単体を起こせば動くので、開発中は気づけません。

## 直し方

`officework/src/main.rs` から呼ぶようにします。単体にしか要らない物
(開発用の旗など)は、下の `単体だけ` に理由を書いてください。

## 見方の限界

**字面で呼び出しの名前を拾うだけ**です。名前が同じでも別の物かもしれず、
名前が違っても同じ仕事をしているかもしれません。*これは思い出す仕掛けで、
証明ではありません* — 出てきた名前を人が見て、要るかどうかを決めます。

## 使い方

    python3 tools/tougou_ochi_check.py
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# **単体にしか要らない物**。鍵は呼び出しの名前、値は理由。
単体だけ = {
    "new": "画面を作る所。officework は 作る表() / 作る文書() で作る",
    "font_data": "書体の登録。officework は自分で登録している",
    "start": "受け口(rpc::start)。officework は自分の名前で1つ開く",
    "observe_keystrokes": "JO_KEYLOG を立てたときだけの開発用。配る形では動かない",
    "forget": "上の開発用と対",
    "quit": "終わり方。officework は自分の窓で決める",
    "request_quit": "同上",
    "commit": "終わる前の打ちかけの確定。officework は 書きかけの名前() で見る",
    "release_lock": "錠は Drop で外す(両方の画面が持っている)",
    "write_recover": "控えは officework の見張りが取る(2026-08-21 に移した)",
    # 言葉の部品(判定や小道具)は仕掛けではない
    "as_ref": "小道具", "as_secs": "小道具", "clamp": "小道具",
    "elapsed": "小道具", "insert": "小道具", "is_ok": "小道具",
    "is_some": "小道具", "name": "小道具", "var": "小道具", "var_os": "小道具", "Python": "文言の中の字",
}

無視 = {"if", "match", "for", "while", "px", "size", "format", "Some", "None", "vec"}


def 本体(path: pathlib.Path, fn: str) -> str:
    s = path.read_text(encoding="utf-8")
    m = re.search(rf"^pub fn {fn}\(\) \{{\n", s, re.M)
    if not m:
        sys.exit(f"::error::{path} に {fn}() がありません(書き方が変わった?)")
    i, depth = m.end(), 1
    while i < len(s) and depth:
        if s[i] == "{":
            depth += 1
        elif s[i] == "}":
            depth -= 1
        i += 1
    return s[m.end() : i]


def 呼び(s: str) -> set[str]:
    """呼び出しの名前。**`::` の最後の部分で比べます。**

    同じ物を `crate::py::start_udf_watch` と `calc::start_udf_watch` の
    ように別の道から呼ぶので、頭を付けたままだと別物に見えます。
    """
    out = {
        m.split("::")[-1]
        for m in re.findall(r"\b([a-zA-Z_:]*[a-z_]{3,})\s*\(", s)
    }
    return out_filter(out)


def out_filter(xs: set[str]) -> set[str]:
    return {x for x in xs if x not in 無視}


def main() -> int:
    calc = 呼び(本体(ROOT / "calc/src/lib.rs", "run"))
    writer = 呼び(本体(ROOT / "writer/src/lib.rs", "run"))
    office = 呼び((ROOT / "officework/src/main.rs").read_text(encoding="utf-8"))

    # **読めなくなったら落ちる。** 静かに緑になるのが一番悪い
    if len(calc) < 20 or len(writer) < 20 or len(office) < 50:
        print(
            f"::error::呼び出しが拾えていません(表 {len(calc)} / 文章 {len(writer)} / "
            f"統合 {len(office)})。書き方が変わった?"
        )
        return 1

    rest = sorted((calc | writer) - office - set(単体だけ))
    for x in rest:
        どこ = "/".join(n for n, s in (("表", calc), ("文章", writer)) if x in s)
        print(
            f"::error::{x} は {どこ} の run() にありますが、officework にはありません。"
            "配る形で動かない仕掛けかもしれません — officework から呼ぶか、"
            "tools/tougou_ochi_check.py の 単体だけ に理由を書いてください"
        )
    if rest:
        return 1

    余り = sorted(set(単体だけ) - (calc | writer))
    if 余り:
        print(
            f"::error::単体だけ に書いてあるのに、もう run() に無い物があります: "
            f"{余り}。表から消してください"
        )
        return 1

    print(
        f"単体の run() の仕掛けは、全部 officework にもあります"
        f"(表 {len(calc)} / 文章 {len(writer)} を見て、単体だけと書いた物が {len(単体だけ)})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
