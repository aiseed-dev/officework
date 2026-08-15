#!/usr/bin/env python3
"""対訳表 `lang/src/i18n_*.rs` を安全に読み書きする。

**自前の正規表現で触らないこと。** 項目は行末のバックスラッシュで複数行に
またがる。2026-08-10、`\\n("` を頼りにした走査で i18n_en.rs から 864 件中
432 件を消した(すぐ戻した)。ここは `ui/gen_i18n.py` と**同じ字句走査**を
使い、読み書きを1箇所に集める。

    python3 tools/i18n_edit.py --dead        # 使われていない訳を並べる(消さない)
    python3 tools/i18n_edit.py --drop-dead   # それを i18n_en.rs から外す
    python3 tools/i18n_edit.py --count       # 表ごとの件数

**直すのは en だけ。** 他の 12 個は `ui/gen_lang.py` が
`ui/i18n/<loc>.json` から作る生成ファイル(頭に「手で書かない」と
書いてある)。en を直したら `--todo` で材料を作り直し、各言語を
生成し直すのが順番。
"""
import sys
import types
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
_src = (ROOT / "ui/gen_i18n.py").read_text(encoding="utf-8")
_g = types.ModuleType("gen_i18n")
_g.__dict__["__file__"] = str(ROOT / "ui/gen_i18n.py")
exec(compile(_src.replace("if __name__", "if False and __name__"), "gen_i18n", "exec"), _g.__dict__)

literal_at = _g.literal_at


def app_keys():
    """アプリが実際に使っている鍵(門番と同じ見方)。

    **実行時の文字列で返す。** ソースの字面で比べてはいけない — 同じ文でも
    片方が行末の `\\` で継いであれば字面は別物になり、**生きている訳を
    「使われていない」と数えて消す**。2026-08-15、それで 4 句を消した
    (i18n_en.rs の口上が同じ罠を警告していたのに、比べ方の側に穴があった)。
    lang/tests/i18n_soroi.rs は最初から unescape して比べている — 揃える。
    """
    keys = []
    for p in _g.SOURCES:
        keys.extend(_g.unescape(lit) for lit in _g.keys_from(p))
    return keys


def tables():
    return sorted(
        p for p in (ROOT / "lang/src").glob("i18n_*.rs") if p.name != "i18n_tables.rs"
    )


def parse(path):
    """表を (前置き, [(鍵, 訳, その項目の原文まるごと)], 後書き) に割る。

    3つ目は**前の項目の終わりからこの項目の終わりまで**(改行も字下げも
    込み)。だから何も落とさずに繋ぎ直せば**1バイトも動かない**。
    整形し直すと 13 個の表が全面差分になる — 触った所だけが差分に出るのが
    見られる差分の条件
    """
    s = path.read_text(encoding="utf-8")
    head_end = s.index("= &[") + 4
    head, body = s[:head_end], s[head_end:]
    items, i, chunk_start = [], 0, 0
    while True:
        j = body.find('("', i)
        if j < 0:
            break
        try:
            k, key = literal_at(body, j + 1)
        except ValueError:
            break
        m = body.find('"', k)
        if m < 0:
            break
        try:
            n, val = literal_at(body, m)
        except ValueError:
            break
        end = body.find("),", n)
        if end < 0:
            break
        end += 2
        items.append((key, val, body[chunk_start:end]))
        i = chunk_start = end
    return head, items, body[chunk_start:]


def write(path, head, items, tail):
    path.write_text(head + "".join(raw for _, _, raw in items) + tail, encoding="utf-8")


def main():
    keys = set(app_keys())
    if "--count" in sys.argv:
        print(f"アプリの鍵 {len(keys)}")
        for p in tables():
            _, items, _ = parse(p)
            got = {k for k, _, _ in items}
            print(f"  {p.name:18} {len(items):5} 訳 / 未訳 {len(keys - got):4} / 不要 {len(got - keys):3}")
        return 0

    # **見るのは en。** ここが正本で、他の 12 個は生成物。前は
    # `tables()[0]`(並びの頭 = i18n_de.rs)を読んでいて、en を直した直後に
    # 「まだ 18 件ある」と言っていた(2026-08-15)
    _, items, _ = parse(ROOT / "lang/src/i18n_en.rs")
    dead = [k for k, _, _ in items if _g.unescape(k) not in keys]
    if "--dead" in sys.argv:
        print(f"使われていない訳 {len(dead)} 件:")
        for k in dead:
            print("  " + k)
        return 0

    if "--drop-dead" in sys.argv:
        # **en だけ**。他の 12 個は ui/gen_lang.py が ui/i18n/<loc>.json から
        # 作る「手で書かない」ファイルで、en を直して生成し直せば付いてくる
        p = ROOT / "lang/src/i18n_en.rs"
        head, items, tail = parse(p)
        before = len(items)
        items = [it for it in items if _g.unescape(it[0]) in keys]
        write(p, head, items, tail)
        print(f"{p.name}: {before} → {len(items)}({before - len(items)} 件を外した)")
        return 0

    print(__doc__)
    return 1


if __name__ == "__main__":
    sys.exit(main())
