#!/usr/bin/env python3
"""**月名・曜日名・日付の既定を、本家の表から起こす。**

`sheet/src/model.rs` の書式の描き手は、月を数でしか出せず(`mmmm` → `08`)、
曜日は `YOBI` と「曜日」を**日本語で焼き付け**ていた。どの言語で開いても
「木曜日」が出る。13言語ぶんを人が打つと 247 語 — 打ち間違いも、
チェコ語やロシア語の**属格**(「8月」と「8月の」で形が違う)の取りこぼしも
避けられない。

材料は**すでに木の中にある**: `vendor/sdkjs/common/NumFormat.js` の
`cultureInfo` に 152 ロケールぶんの月名・曜日名・属格・日付の既定が入って
いる。本家 ONLYOFFICE は AGPL-3.0、こちらも AGPL なので通る。
`calc/gen_funcs.py`(関数の説明を web-apps から起こす)と同じ作法。

    python3 sheet/gen_datetime_names.py          # 生成して書く
    python3 sheet/gen_datetime_names.py --check  # 変わっていないか見る(CI 向き)

**通貨記号は載せない。** 表には `CurrencySymbol` があるが、
**通貨は読む人の言語ではなく、その帳票のお金**(2026-08-10 発注者確定、
`docs/sekkei/calc.ja.md`)。ここに載せると「言語から通貨を引く」道を
作ってしまうので、構造として持たせない。通貨は選ばせる。
"""

from __future__ import annotations

import re
import sys
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "vendor/sdkjs/common/NumFormat.js"
OUT = ROOT / "sheet/src/datetime_names.rs"

# うちの言語 → 本家の Name。**素直に引けない2つに注意**:
#   pt  本家の "pt" はブラジル(R$・綴りも別)。うちの訳は欧州なので pt-PT
#   zh-tw 台湾。"zh-Hant" は香港(通貨 HK$)を指すので使わない
# 言語と国を混ぜると壊れる、の実例が材料の側にもある
LOCALES = {
    "ja": "ja",
    "de": "de",
    "en": "en",
    "es": "es",
    "fr": "fr",
    "id": "id",
    "it": "it",
    "ko": "ko",
    "pt": "pt-PT",
    "ru": "ru",
    "tr": "tr",
    "vi": "vi",
    "zh": "zh-Hans",
    "zh-tw": "zh-TW",
}


def strings(js_array: str) -> list[str]:
    """JS の文字列の並びを読む。`\\"` などの逃げも通す"""
    out = []
    for raw in re.findall(r'"((?:[^"\\]|\\.)*)"', js_array):
        out.append(re.sub(r"\\(.)", r"\1", raw))
    return out


def field(body: str, key: str) -> str:
    m = re.search(rf"\b{key}: (\[.*?\]|\"(?:[^\"\\]|\\.)*\")", body)
    if not m:
        sys.exit(f"::error::{key} が読めません(本家の書き方が変わった?)")
    return m.group(1)


def unescape_code(code: str) -> str:
    """書式コードの `\\,` `\\ ` を素の字に戻す。

    こちらの字句走査(`model.rs`)はバックスラッシュを**逃げとして扱わない** —
    `\\,` はそのまま「バックスラッシュと読点」になってしまう。外して渡す。

    **英字を包む逃げがあったら止める。** `\\d` を素の `d` にすると
    「日」の札に化け、静かに別の日付が出る。いまの14言語には無いが、
    本家の表が変わったときに黙って壊れないようにする。
    """
    out = []
    i = 0
    while i < len(code):
        if code[i] == "\\" and i + 1 < len(code):
            c = code[i + 1]
            if c.isascii() and c.isalpha():
                sys.exit(f"::error::書式コードが英字を逃がしています: {code!r}")
            out.append(c)
            i += 2
        else:
            out.append(code[i])
            i += 1
    return "".join(out)


def rs(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def cultures() -> dict[str, str]:
    src = SRC.read_text(encoding="utf-8")
    out = {}
    for m in re.finditer(r'^\t\d+: \{LCID: \d+, Name: "([^"]+)",(.*)$', src, re.M):
        out[m.group(1)] = m.group(2)
    if len(out) < 100:
        sys.exit(f"::error::cultureInfo が読めていません({len(out)} 件)")
    return out


def lcid_rows() -> list[tuple[int, str]]:
    """LCID → うちの言語。**書式コードの `[$-407]` を引くのに要る。**

    本家の表は LCID を鍵に 152 ロケール持っている。それぞれの `Name`
    (`de-AT` など)を枝で落として、うちの14言語に当たるものだけを拾う。
    **暗記で書かない** — pt は 416=ブラジル / 816=欧州 で、覚えていると
    必ず踏み外す(2026-08-10、実際に食い違った)。

    中国語だけは枝で落とせない。繁体(台湾・香港・マカオ)は `zh-tw` の
    表へ、簡体は `zh` へ送る
    """
    src = SRC.read_text(encoding="utf-8")
    ours = set(LOCALES)
    out = []
    for m in re.finditer(r'^\t\d+: \{LCID: (\d+), Name: "([^"]+)"', src, re.M):
        lcid, name = int(m.group(1)), m.group(2)
        if name.startswith("zh"):
            lang = "zh-tw" if name in ("zh-TW", "zh-HK", "zh-MO", "zh-Hant") else "zh"
        else:
            lang = name.split("-")[0]
        if lang in ours:
            out.append((lcid, lang))
    if len(out) < 20:
        sys.exit(f"::error::LCID の対応が取れていません({len(out)} 件)")
    return sorted(out)


def build() -> str:
    cs = cultures()
    rows = []
    for ours, theirs in LOCALES.items():
        if theirs not in cs:
            sys.exit(f"::error::本家に {theirs} がありません({ours} の材料)")
        b = cs[theirs]
        mon = strings(field(b, "MonthNames"))[:12]
        mon_a = strings(field(b, "AbbreviatedMonthNames"))[:12]
        day = strings(field(b, "DayNames"))[:7]
        day_a = strings(field(b, "AbbreviatedDayNames"))[:7]
        gen = strings(field(b, "MonthGenitiveNames"))[:12]
        long_p = unescape_code(strings(field(b, "LongDatePattern"))[0])
        # 通貨記号の**置き場所**。.NET の CurrencyPositivePattern:
        #   0 = 記号n / 1 = n記号 / 2 = 記号␣n / 3 = n␣記号
        # **記号そのものは載せない**(お金は帳票のもの)。置き場所だけが
        # 読む人の言語の作法なので、ここに持つ
        cpp = re.search(r"CurrencyPositivePattern: (\d+)", b)
        cpp = cpp.group(1) if cpp else "0"
        for name, arr, want in (
            ("MonthNames", mon, 12), ("AbbreviatedMonthNames", mon_a, 12),
            ("DayNames", day, 7), ("AbbreviatedDayNames", day_a, 7),
        ):
            if len(arr) != want or any(not x for x in arr):
                sys.exit(f"::error::{ours}: {name} が {len(arr)} 件({want} 件のはず)")
        genitive = (
            "Some([" + ", ".join(rs(x) for x in gen) + "])"
            if len(gen) == 12 and all(gen)
            else "None"
        )
        rows.append(
            f"    Names {{\n"
            f"        lang: {rs(ours)},\n"
            f"        months: [{', '.join(rs(x) for x in mon)}],\n"
            f"        months_abbr: [{', '.join(rs(x) for x in mon_a)}],\n"
            f"        months_genitive: {genitive},\n"
            f"        days: [{', '.join(rs(x) for x in day)}],\n"
            f"        days_abbr: [{', '.join(rs(x) for x in day_a)}],\n"
            f"        long_date: {rs(long_p)},\n"
            f"        currency_pattern: {cpp},\n"
            f"    }},"
        )
    body = "\n".join(rows)
    lcids = "\n".join(f'    (0x{k:x}, "{v}"),' for k, v in lcid_rows())
    return f'''//! 月名・曜日名と、言語ごとの「長い日付」の既定。
//!
//! **このファイルは sheet/gen_datetime_names.py が生成する。手で書かない。**
//! 材料は vendor/sdkjs/common/NumFormat.js の cultureInfo(本家 ONLYOFFICE、
//! AGPL-3.0)。`calc/gen_funcs.py` と同じ作法で、依存は増やさない。
//!
//! **通貨記号は載せていない。** 本家の表は持っているが、通貨は読む人の
//! 言語ではなくその帳票のお金なので、言語から引ける形にしない
//! (docs/sekkei/calc.ja.md「通貨だけは言語に引かせない」)。
//!
//! 曜日は **0 が日曜**(`calc::weekday0` と `YOBI` に合わせてある)。

/// ひとつの言語の暦の語。
pub struct Names {{
    pub lang: &'static str,
    pub months: [&'static str; 12],
    pub months_abbr: [&'static str; 12],
    /// 属格(「8月**の**」)。チェコ語・ロシア語・ギリシャ語・フィンランド語
    /// などは日付の中で形が変わる。持たない言語は None
    pub months_genitive: Option<[&'static str; 12]>,
    /// 0 = 日曜
    pub days: [&'static str; 7],
    pub days_abbr: [&'static str; 7],
    /// その言語の「長い日付」の既定(Excel の書式コード)。
    /// 発注者の「各国で一つに決めて置いたほうがいい」に当たる物で、
    /// **本家が決めた既定をそのまま使う** — こちらで13本を考え直さない
    pub long_date: &'static str,
    /// 通貨記号の**置き場所**だけ(0=記号n / 1=n記号 / 2=記号␣n / 3=n␣記号)。
    /// **記号そのものは持たない** — お金は読む人の言語ではなく帳票のもの
    /// (docs/sekkei/calc.ja.md)。並びだけが言語の作法
    pub currency_pattern: u8,
}}

pub const TABLE: &[Names] = &[
{body}
];

/// LCID → うちの言語。書式コードの `[$-407]`(独)`[$-409]`(米)から引く。
/// **本家の表から起こしてある** — 暗記だと pt の 416/816 を踏み外す
pub const LCID_LANG: &[(u32, &str)] = &[
{lcids}
];

/// 書式コードの地域指定から言語を引く。**知らない番号は None** —
/// 勝手に近い言語へ寄せない(寄せた先が違えば、静かに別の月名が出る)
pub fn lang_of_lcid(lcid: u32) -> Option<&'static str> {{
    LCID_LANG
        .binary_search_by_key(&lcid, |(k, _)| *k)
        .ok()
        .map(|i| LCID_LANG[i].1)
}}

/// その言語の暦の語。**知らない言語は日本語に落ちる** — 素の言語だから。
/// 黙って英語にすると、日本語で使っている人に英語が出る事故になる
pub fn names(lang: &str) -> &'static Names {{
    TABLE
        .iter()
        .find(|n| n.lang == lang)
        // "zh-tw" のような枝が無ければ "zh" へ、それも無ければ ja
        .or_else(|| lang.split_once('-').and_then(|(base, _)| {{
            TABLE.iter().find(|n| n.lang == base)
        }}))
        .unwrap_or(&TABLE[0])
}}
'''


def main() -> int:
    got = build()
    if "--check" in sys.argv:
        if not OUT.exists() or OUT.read_text(encoding="utf-8") != got:
            print(f"::error::{OUT.name} が材料と合っていません"
                  " — python3 sheet/gen_datetime_names.py で作り直してください")
            return 1
        print(f"{OUT.name}: 材料と一致({len(LOCALES)} 言語)")
        return 0
    OUT.write_text(got, encoding="utf-8")
    print(f"{OUT} に {len(LOCALES)} 言語ぶん書きました")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
