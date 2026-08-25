#!/usr/bin/env python3
"""鍵の英語化 — t!/tf!/item! の鍵を ja から en へ裏返す機械。

**読み方は ui/gen_i18n.py に合わせてあります。** 前は自前の正規表現で
表とコードを読んでいて、次の3つが見えていませんでした(2026-08-26)。

* 行末の `\\` で次の行に継いだ鍵 — 表で 19 句、呼び出しで 28 か所
* `crate::t!`(ui クレートが自分を `ui::` と呼べないときの書き方)47 か所
* `lang::i18n::tr(`(face は ui に依存しないので直に呼ぶ)17 か所

見えない呼び出しは書き替えられず、「表に無い鍵」にも数えられません。
**同じ目の粗さで数えた「取りこぼし 0」は 0 の証しになりません。**
見張りと同じ道で読むこと。
"""
import json, pathlib, re, sys

ROOT = pathlib.Path("/home/dev/dev/officework")
HERE = pathlib.Path(__file__).parent
sys.path.insert(0, str(ROOT / "ui"))
import gen_i18n  # noqa: E402  表とコードの読み方の正本

SRC_DIRS = ["calc/src", "writer/src", "face/src", "ui/src", "ops/src",
            "officework/src", "sheet/src", "engine/src", "lang/src",
            "paper/src", "pyrun/src", "ooxml/src", "pysheet/src", "sidecar/src"]

# 呼び出しの書き方は3通りある。**書き替えるときは元の書き方を残します** —
# `crate::t!` を `ui::t!` にすると ui クレートの中で通らなくなります
呼び出し = re.compile(
    r'((?:ui|crate)::(?:t|tf|item)!\(\s*|lang::i18n::trf?\(\s*)"'
)

unescape = gen_i18n.unescape

def escape(s):
    return s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n")

def load_pairs():
    return [(unescape(k), unescape(v)) for k, v in gen_i18n.table_pairs()]

def load_fixes():
    m, canon = {}, {}
    for line in (HERE / "flip_en_fixes.tsv").read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        cols = line.split("\t")
        ja, en = cols[0], cols[1]
        m[ja] = en
        if len(cols) > 2 and cols[2].strip():
            canon[en] = cols[2]
    return m, canon

def mapping():
    fixes, canon = load_fixes()
    m = {}
    for ja, en in load_pairs():
        m[ja] = fixes.get(ja, en)
    inv = {}
    for ja, en in m.items():
        inv.setdefault(en, []).append(ja)
    bad = {e: js for e, js in inv.items() if len(js) > 1 and e not in canon}
    for e, js in bad.items():
        print(f"!! 統合の正の日本語が無い: {e!r} ← {js}")
    if bad:
        sys.exit(1)
    return m, canon

def rewrite_code(m, dry):
    """呼び出しの鍵を英語へ。**リテラルは gen_i18n と同じ読み方で切ります。**

    正規表現で終わりの `"` まで取ると、行継続の鍵で途中で切れます。頭だけを
    正規表現で見つけ、そこから先は `literal_at` に読ませます。
    """
    n_hit = n_miss = 0
    misses = {}
    # **試験の中の呼び出しも書き替えます。** 試験が `ui::t!("保存")` と
    # 書いていれば、それも鍵なので裏返さないと引けません。日本語の字を
    # 直に突き合わせる assert(鍵ではなく出来上がりの字を見る物)は
    # ここでは触れません — 段1の最後に赤から収束させます
    for d in SRC_DIRS:
        for p in (ROOT / d).rglob("*.rs"):
            t = p.read_text(encoding="utf-8")
            out, last = [], 0
            changed = False
            for mo in 呼び出し.finditer(t):
                start = mo.end() - 1  # `"` の位置
                try:
                    end, lit = gen_i18n.literal_at(t, start)
                except ValueError:
                    continue
                ja = unescape(lit)
                if ja in m:
                    out.append(t[last:mo.start()])
                    out.append(mo.group(1) + '"' + escape(m[ja]) + '"')
                    last = end
                    n_hit += 1
                    changed = True
                else:
                    n_miss += 1
                    misses.setdefault(ja, str(p.relative_to(ROOT)))
            out.append(t[last:])
            if changed and not dry:
                p.write_text("".join(out), encoding="utf-8")
    print(f"code: 書き替え {n_hit} / 表に無い鍵 {n_miss}")
    for ja, w in list(misses.items())[:15]:
        print(f"   表に無い: {ja[:60]!r} … {w}")
    外を言う()
    return n_miss


def 外を言う():
    """**SRC_DIRS の外にある呼び出しを数えて言う。**

    黙って外すと「全部やった」に見えます。`lang/tests/` には呼び出しが
    ありますが、i18n_soroi の分は*走査の試験のための作り物の字*で、
    裏返すと試験そのものが壊れます。機械では分けられないので、数だけ
    出して手で始末してもらいます。

    `packaging/` の下は組み立てのときの写しなので、数えません。
    """
    見る = {d.split("/")[0] for d in SRC_DIRS}
    外 = {}
    for p in ROOT.rglob("*.rs"):
        rel = str(p.relative_to(ROOT))
        if rel.startswith("packaging/") or "/target/" in rel:
            continue
        if any(rel.startswith(d + "/") for d in SRC_DIRS):
            continue
        if rel.split("/")[0] not in 見る:
            continue
        n = len(呼び出し.findall(p.read_text(encoding="utf-8", errors="ignore")))
        if n:
            外[rel] = n
    if not 外:
        return
    print(f"!! SRC_DIRS の外に呼び出しが {sum(外.values())} か所あります(手で始末)")
    for rel, n in sorted(外.items(), key=lambda x: -x[1]):
        print(f"     {n:3}  {rel}")

if __name__ == "__main__":
    dry = "--go" not in sys.argv
    m, canon = mapping()
    print(f"対 {len(m)} 句 → en 鍵 {len(set(m.values()))}(統合 {len(m) - len(set(m.values()))})")
    rewrite_code(m, dry)
    if dry:
        print("(--go で書き込む)")
