# 関数の一覧表(calc/src/funcs.rs ほか)を Euro-Office の現物から起こす。
#
#   python3 calc/gen_funcs.py          # 素の日本語 calc/src/funcs.rs
#   python3 calc/gen_funcs.py --all    # 全言語 + 登録簿 + mod の登録
#   python3 calc/gen_funcs.py --check  # 生成物が材料と合っているか
#
# 引数と説明は vendor/web-apps の formula-lang/<loc>_desc.json(本家の対訳)。
# **載せるのは calc が実際に計算できる関数だけ**(できないものを見せない)。
# 分類はうちの数式タブの族(fn-math 等)と同じ割り付け。
#
# **本家の綴りはここでも一致しない。** リボンの locale/ では `pt.json` が
# ブラジルで `pt-pt.json` が欧州だったが、この formula-lang/ では
# `pt_desc.json` が**欧州**("Devolve")で `pt-br_desc.json` がブラジル
# ("Retorna")。同じ本家の中で置き場ごとに約束が違うので、VENDOR に
# 明示して、さらに中身で確かめる(2026-08-11)。
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LANGDIR = ROOT / "vendor/web-apps/apps/spreadsheeteditor/main/resources/formula-lang"
SRCDIR = ROOT / "calc/src"

# こちらの札 → 本家の綴り。**この置き場では pt が欧州**
VENDOR = {
    "de": "de", "en": "en", "es": "es", "fr": "fr", "id": "id", "it": "it",
    "ko": "ko", "pt": "pt", "pt-br": "pt-br", "ru": "ru", "tr": "tr",
    "vi": "vi", "zh": "zh", "zh-tw": "zh-tw",
}

# 使える関数(sheet/src/calc.rs が計算できるもの)を分類ごとに。
# リボンの fn-* の一覧(「使える名前だけを出す」)と同じ中身
GROUPS = {
    "数学/三角": "SUM ROUND ROUNDUP ROUNDDOWN INT ABS MOD POWER SQRT "
            "PRODUCT SUMPRODUCT SUMSQ CEILING FLOOR MROUND EVEN ODD SIGN "
            "FACT COMBIN PERMUT GCD LCM PI SIN COS TAN ASIN ACOS ATAN ATAN2 "
            "SINH COSH TANH EXP LN LOG LOG10 DEGREES RADIANS RAND RANDBETWEEN "
            "SEQUENCE TRUNC QUOTIENT CEILING.MATH FLOOR.MATH SUBTOTAL",
    "統計": "AVERAGE COUNT MAX MIN COUNTA COUNTBLANK SUMIF SUMIFS COUNTIF "
            "COUNTIFS AVERAGEIF AVERAGEIFS MINIFS MAXIFS "
            "RANK RANK.EQ RANK.AVG LARGE SMALL MEDIAN MODE STDEV STDEVP "
            "VAR VARP PERCENTILE QUARTILE CORREL SLOPE INTERCEPT FORECAST "
            "AVERAGEA MAXA MINA",
    "文字列操作": "LEN LEFT RIGHT MID TRIM UPPER LOWER CONCATENATE CONCAT TEXT "
              "SUBSTITUTE FIND SEARCH VALUE TEXTJOIN REPT CHAR CODE "
              "UNICHAR UNICODE PROPER EXACT CLEAN FIXED YEN NUMBERVALUE "
              "LENB LEFTB RIGHTB MIDB ASC JIS DATESTRING PHONETIC",
    "論理": "IF IFS SWITCH AND OR NOT TRUE FALSE IFERROR IFNA",
    "日付/時刻": "TODAY NOW DATE DATEVALUE YEAR MONTH DAY WEEKDAY "
            "TIME HOUR MINUTE SECOND EDATE EOMONTH DATEDIF "
            "WORKDAY NETWORKDAYS DAYS DAYS360 YEARFRAC WEEKNUM ISOWEEKNUM",
    "検索/行列": "VLOOKUP HLOOKUP XLOOKUP LOOKUP INDEX MATCH CHOOSE "
            "ROW COLUMN ROWS COLUMNS OFFSET INDIRECT ADDRESS HYPERLINK "
            "FILTER SORT UNIQUE TRANSPOSE",
    "財務": "PMT PV FV NPER NPV IRR RATE",
    "情報": "ISBLANK ISERROR ISNA ISERR ISLOGICAL ISNONTEXT ISNUMBER ISTEXT "
            "ISEVEN ISODD NA T N TYPE",
}

# 本家の表に無い(日本語まわりの)関数は、こちらで書く。
# 訳は calc/funcs_hand.json — 言語ごとに同じ形で持つ
HAND_JA = {
    "YEN": {"a": "(数値, [桁数])", "d": "数値を円記号(¥)と桁区切りを付けた文字列にします。"},
    "JIS": {"a": "(文字列)", "d": "半角(1 バイト)文字を全角(2 バイト)文字に変換します。"},
    "DATESTRING": {"a": "(シリアル値)", "d": "日付を和暦の文字列にして返します。"},
    "PHONETIC": {"a": "(範囲)", "d": "セルのふりがなを返します(読み込んだ xlsx のふりがな情報を引きます)。"},
}
# 本家の対訳に穴があった語をここで埋める(言語 → 関数名 → {a, d, ad})。
# **穴を黙って日本語で埋めない** — 埋めていない穴があれば生成を止める
PATCH = json.loads((ROOT / "calc/funcs_hand.json").read_text(encoding="utf-8")) \
    if (ROOT / "calc/funcs_hand.json").exists() else {}

NAMES = [n for names in GROUPS.values() for n in names.split()]


def esc(s: str) -> str:
    return s.replace("\\", "\\\\").replace('"', '\\"')


def load(loc: str) -> dict:
    """その言語の説明。本家 + こちらで書いた分 + 穴埋め"""
    if loc == "ja":
        d = json.load(open(LANGDIR / "ja_desc.json", encoding="utf-8"))
        d.update(HAND_JA)
    else:
        d = json.load(open(LANGDIR / f"{VENDOR[loc]}_desc.json", encoding="utf-8"))
    d.update(PATCH.get(loc, {}))
    return d


def rows_for(loc: str):
    """(名前, 分類, 引数, 説明, 引数ごとの説明) を名前順に。穴は返り値の2つ目に"""
    d = load(loc)
    rows, holes = [], []
    for group, names in GROUPS.items():
        for name in names.split():
            info = d.get(name)
            if info is None or not info.get("d"):
                holes.append(name)
                continue
            args = info.get("a", "(…)").replace("; ", ", ")
            ads = [s for s in info.get("ad", "").split("!") if s.strip()]
            rows.append((name, group, args, info["d"], ads))
    rows.sort(key=lambda r: r[0])
    return rows, holes


HEAD = """//! 関数の{what}。**このファイルは手で書かない** —
//! `python3 calc/gen_funcs.py{arg}` が
//! Euro-Office の formula-lang/{src} から起こす。
//! 載っているのは calc が実際に計算できる関数だけ。
"""


def ja_source() -> str:
    rows, holes = rows_for("ja")
    if holes:
        sys.exit(f"::error::ja に穴があります: {' '.join(holes)}")
    out = [HEAD.format(what="一覧(名前・分類・引数・説明)", arg="",
                       src="ja_desc.json(本家の日本語)"), ""]
    out.append("""pub struct FnInfo {
    pub name: &'static str,
    /// 分類。**日本語のまま持つ** — 画面に出すときに ui::tr で訳す。
    /// 絞り込みの照合に使う鍵なので、訳した語を入れてはいけない
    pub group: &'static str,
    pub args_ja: &'static str,
    pub desc_ja: &'static str,
    /// 引数ごとの説明(引数の並び順。可変長引数は最後の1つが代表)
    pub arg_desc_ja: &'static [&'static str],
}

/// 1つの関数の、ある言語での言葉。`funcs_<loc>.rs` が並べる
pub struct FnText {
    pub name: &'static str,
    pub args: &'static str,
    pub desc: &'static str,
    pub arg_desc: &'static [&'static str],
}

impl FnInfo {
    /// いまの言語の言葉。無ければ日本語(素の言語)に落ちる
    fn text(&self) -> Option<&'static FnText> {
        let t = crate::funcs_tables::text(ui::language())?;
        // 並びは名前順(生成器が揃える)ので二分探索で引ける
        t.binary_search_by_key(&self.name, |r| r.name).ok().map(|i| &t[i])
    }

    pub fn args(&self) -> &'static str {
        self.text().map_or(self.args_ja, |t| t.args)
    }

    pub fn desc(&self) -> &'static str {
        self.text().map_or(self.desc_ja, |t| t.desc)
    }

    pub fn arg_desc(&self) -> &'static [&'static str] {
        self.text().map_or(self.arg_desc_ja, |t| t.arg_desc)
    }
}
""")
    out.append(f"pub static FUNCS: &[FnInfo] = &[  // {len(rows)} 関数")
    for name, group, args, desc, ads in rows:
        ad = ", ".join(f'"{esc(a)}"' for a in ads)
        out.append(f'    FnInfo {{ name: "{esc(name)}", group: "{esc(group)}", '
                   f'args_ja: "{esc(args)}", desc_ja: "{esc(desc)}", '
                   f'arg_desc_ja: &[{ad}] }},')
    out.append("];")
    return "\n".join(out) + "\n"


def loc_source(loc: str) -> tuple[str, list[str]]:
    rows, holes = rows_for(loc)
    out = [HEAD.format(what=f"言葉({loc})", arg=f" --all",
                       src=f"{VENDOR[loc]}_desc.json"), ""]
    out.append("use super::funcs::FnText;\n")
    out.append(f"pub static TEXT: &[FnText] = &[  // {len(rows)} 関数")
    for name, _group, args, desc, ads in rows:
        ad = ", ".join(f'"{esc(a)}"' for a in ads)
        out.append(f'    FnText {{ name: "{esc(name)}", args: "{esc(args)}", '
                   f'desc: "{esc(desc)}", arg_desc: &[{ad}] }},')
    out.append("];")
    return "\n".join(out) + "\n", holes


def mod_name(loc: str) -> str:
    return "funcs_" + loc.replace("-", "_").lower()


def tables_source(locs: list[str]) -> str:
    out = ["""//! 関数の言葉の登録簿。**このファイルは calc/gen_funcs.py が生成する。**
//! 手で書かない — 言語を足すときは gen_funcs.py --all を回す。

use super::funcs::FnText;

pub fn text(lang: &str) -> Option<&'static [FnText]> {
    match lang {"""]
    for loc in locs:
        out.append(f'        "{loc}" => Some(crate::{mod_name(loc)}::TEXT),')
    out.append("""        _ => None,
    }
}
""")
    return "\n".join(out)


def register(locs: list[str]) -> None:
    """calc/src/main.rs の mod の登録を書き換える"""
    p = SRCDIR / "main.rs"
    src = p.read_text(encoding="utf-8")
    block = ("// gen_funcs:begin(この間は calc/gen_funcs.py が生成する — 手で書かない)\n"
             + "".join(f"mod {mod_name(l)};\n" for l in locs)
             + "mod funcs_tables;\n"
             + "// gen_funcs:end\n")
    if "// gen_funcs:begin" in src:
        src = re.sub(r"// gen_funcs:begin.*?// gen_funcs:end\n", block, src, flags=re.S)
    else:
        src = src.replace("mod funcs;\n", "mod funcs;\n" + block, 1)
    p.write_text(src, encoding="utf-8")


def european_pt_still_holds() -> str | None:
    """**本家の pt が欧州のままか確かめる。** 置き場ごとに約束が違うので、
    札の対応表だけでは足りない — 中身で見る。欧州は `Devolve`、
    ブラジルは `Retorna`(2026-08-11 に数えた: pt 欧州 149 / ブラジル 0)"""
    def count(loc, word):
        d = json.load(open(LANGDIR / f"{loc}_desc.json", encoding="utf-8"))
        return sum(v.get("d", "").count(word) for v in d.values())
    pt_eu, pt_br = count("pt", "Devolve"), count("pt", "Retorna")
    br_eu, br_br = count("pt-br", "Devolve"), count("pt-br", "Retorna")
    if not (pt_eu > pt_br and br_br > br_eu):
        return (f"本家の pt/pt-br が入れ替わっている疑い "
                f"(pt: Devolve {pt_eu} / Retorna {pt_br}、"
                f"pt-br: Devolve {br_eu} / Retorna {br_br})")
    return None


def main() -> int:
    if "--all" not in sys.argv and "--check" not in sys.argv:
        sys.stdout.write(ja_source())
        return 0

    bad = european_pt_still_holds()
    if bad:
        print(f"::error::{bad}")
        return 1

    locs = sorted(VENDOR)
    want = {SRCDIR / "funcs.rs": ja_source(),
            SRCDIR / "funcs_tables.rs": tables_source(locs)}
    holes = {}
    for loc in locs:
        text, h = loc_source(loc)
        want[SRCDIR / f"{mod_name(loc)}.rs"] = text
        if h:
            holes[loc] = h
    if holes:
        for loc, h in holes.items():
            print(f"::error::{loc}: 訳の無い関数 {len(h)} 個 — {' '.join(h)}")
        print("::error::calc/funcs_hand.json に足してください"
              "(**日本語で埋めない** — その言語の人には読めません)")
        return 1

    if "--check" in sys.argv:
        for p, s in want.items():
            if not p.exists() or p.read_text(encoding="utf-8") != s:
                print(f"::error::{p.name} が材料と合っていません"
                      " — python3 calc/gen_funcs.py --all で作り直してください")
                return 1
        print(f"関数の言葉: {len(locs)} 言語とも材料と一致({len(NAMES)} 関数)")
        return 0

    for p, s in want.items():
        p.write_text(s, encoding="utf-8")
    register(locs)
    print(f"関数の言葉を {len(locs)} 言語ぶん書きました({len(NAMES)} 関数)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
