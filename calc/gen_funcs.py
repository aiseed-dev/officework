# 関数の一覧表(face/src/funcs.rs ほか)を Euro-Office の現物から起こす。
#
#   python3 calc/gen_funcs.py          # 素の日本語 face/src/funcs.rs
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
# **関数の表は face(gpui を持たない層)にある。** 2026-08-15 に
# calc/src から移した — 名前も分類も説明も絵を描かない物で、
# Kotlin / Swift のアプリも同じ表を読む
SRCDIR = ROOT / "face/src"

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
            "SEQUENCE TRUNC QUOTIENT CEILING.MATH FLOOR.MATH SUBTOTAL "
            "ACOSH ASINH ATANH ACOT ACOTH COT COTH CSC CSCH SEC SECH "
            "BASE DECIMAL COMBINA FACTDOUBLE MULTINOMIAL SQRTPI "
            "CEILING.PRECISE FLOOR.PRECISE ISO.CEILING ROMAN ARABIC "
            "SERIESSUM SUMX2MY2 SUMX2PY2 SUMXMY2 MDETERM "
            "AGGREGATE RANDARRAY",
    "統計": "AVERAGE COUNT MAX MIN COUNTA COUNTBLANK SUMIF SUMIFS COUNTIF "
            "COUNTIFS AVERAGEIF AVERAGEIFS MINIFS MAXIFS "
            "RANK RANK.EQ RANK.AVG LARGE SMALL MEDIAN MODE STDEV STDEVP "
            "VAR VARP PERCENTILE QUARTILE CORREL SLOPE INTERCEPT FORECAST "
            "AVERAGEA MAXA MINA MODE.MULT "
            "AVEDEV DEVSQ GEOMEAN HARMEAN KURT SKEW SKEW.P "
            "STDEVA STDEVPA VARA VARPA TRIMMEAN "
            "COVARIANCE.P COVARIANCE.S PEARSON RSQ STEYX "
            "PERCENTILE.EXC QUARTILE.EXC PERCENTRANK.INC PERCENTRANK.EXC "
            "PERMUTATIONA STANDARDIZE FISHER FISHERINV "
            "GAMMALN GAMMALN.PRECISE PHI GAUSS "
            "GAMMA NORM.DIST NORM.INV NORM.S.DIST NORM.S.INV "
            "LOGNORM.DIST LOGNORM.INV EXPON.DIST WEIBULL.DIST "
            "GAMMA.DIST GAMMA.INV CHISQ.DIST CHISQ.DIST.RT CHISQ.INV "
            "CHISQ.INV.RT POISSON.DIST BINOM.DIST BINOM.INV "
            "NEGBINOM.DIST HYPGEOM.DIST T.DIST T.DIST.RT T.DIST.2T "
            "T.INV T.INV.2T F.DIST F.DIST.RT F.INV F.INV.RT "
            "BETA.DIST BETA.INV CONFIDENCE.NORM CONFIDENCE.T "
            "BINOM.DIST.RANGE "
            # 古い名前と .INC 系(Excel の互換関数)。評価器は受けるので載せる
            "BETADIST BETAINV BINOMDIST CHIDIST CHIINV CONFIDENCE CRITBINOM "
            "FDIST FINV FORECAST.LINEAR GAMMADIST GAMMAINV HYPGEOMDIST "
            "MODE.SNGL NEGBINOMDIST NORMDIST NORMINV NORMSDIST NORMSINV "
            "PERCENTILE.INC PERCENTRANK POISSON QUARTILE.INC "
            "STDEV.P STDEV.S TDIST TINV VAR.P VAR.S WEIBULL",
    "文字列操作": "LEN LEFT RIGHT MID TRIM UPPER LOWER CONCATENATE CONCAT TEXT "
              "SUBSTITUTE FIND SEARCH VALUE TEXTJOIN REPT CHAR CODE "
              "UNICHAR UNICODE PROPER EXACT CLEAN FIXED YEN NUMBERVALUE "
              "LENB LEFTB RIGHTB MIDB FINDB SEARCHB REPLACEB "
              "ASC JIS DATESTRING PHONETIC VALUETOTEXT ARRAYTOTEXT "
              "DBCS DOLLAR REPLACE TEXTBEFORE TEXTAFTER TEXTSPLIT",
    "論理": "IF IFS SWITCH AND OR NOT TRUE FALSE IFERROR IFNA XOR LET",
    "日付/時刻": "TODAY NOW DATE DATEVALUE YEAR MONTH DAY WEEKDAY "
            "TIME HOUR MINUTE SECOND EDATE EOMONTH DATEDIF "
            "WORKDAY NETWORKDAYS DAYS DAYS360 YEARFRAC WEEKNUM ISOWEEKNUM "
            "TIMEVALUE NETWORKDAYS.INTL WORKDAY.INTL",
    "検索/行列": "VLOOKUP HLOOKUP XLOOKUP LOOKUP INDEX MATCH CHOOSE "
            "ROW COLUMN ROWS COLUMNS OFFSET INDIRECT ADDRESS HYPERLINK "
            "FILTER SORT UNIQUE TRANSPOSE XMATCH SORTBY "
            "VSTACK HSTACK TAKE DROP TOCOL TOROW DF",
    "財務": "PMT PV FV NPER NPV IRR RATE "
            "IPMT PPMT CUMIPMT CUMPRINC ISPMT SLN SYD DB DDB VDB "
            "EFFECT NOMINAL PDURATION RRI FVSCHEDULE DOLLARDE DOLLARFR "
            "MIRR XNPV XIRR",
    "情報": "ISBLANK ISERROR ISNA ISERR ISLOGICAL ISNONTEXT ISNUMBER ISTEXT "
            "ISEVEN ISODD NA T N TYPE ERROR.TYPE CELL PY",
    "エンジニアリング": "BIN2DEC BIN2HEX BIN2OCT OCT2BIN OCT2DEC OCT2HEX "
            "DEC2BIN DEC2HEX DEC2OCT HEX2BIN HEX2DEC HEX2OCT "
            "BITAND BITOR BITXOR BITLSHIFT BITRSHIFT DELTA GESTEP "
            "ERF ERF.PRECISE ERFC ERFC.PRECISE COMPLEX IMABS IMREAL "
            "IMAGINARY IMCONJUGATE IMSUM IMSUB IMPRODUCT IMDIV CONVERT",
    "データベース": "DSUM DAVERAGE DCOUNT DCOUNTA DMAX DMIN DGET "
            "DPRODUCT DSTDEV DSTDEVP DVAR DVARP",
}

# 本家の表に無い(日本語まわりの)関数は、こちらで書く。
# 訳は calc/funcs_hand.json — 言語ごとに同じ形で持つ
HAND_JA = {
    "YEN": {"a": "(数値, [桁数])", "d": "数値を円記号(¥)と桁区切りを付けた文字列にします。"},
    "JIS": {"a": "(文字列)", "d": "半角(1 バイト)文字を全角(2 バイト)文字に変換します。"},
    "DATESTRING": {"a": "(シリアル値)", "d": "日付を和暦の文字列にして返します。"},
    "PHONETIC": {"a": "(範囲)", "d": "セルのふりがなを返します(読み込んだ xlsx のふりがな情報を引きます)。"},
    # 本家の対訳に無い3つ(2026-09-02)。答えの正は Excel の仕様
    "VALUETOTEXT": {"a": "(値, [書式])", "d": "指定した値を文字列にして返します。",
        "ad": "文字列にして返す値!返す形式。0 または省略で簡潔な形式、1 で厳密な形式(文字列を引用符で囲みます)"},
    "ARRAYTOTEXT": {"a": "(配列, [書式])", "d": "指定した範囲の値を文字列にして返します。",
        "ad": "文字列にして返す配列!返す形式。0 または省略で簡潔な形式、1 で厳密な形式({} で囲み、文字列を引用符で囲みます)"},
    "BINOM.DIST.RANGE": {"a": "(試行回数, 成功率, 成功数, [成功数2])",
        "d": "二項分布を使用して、試行結果の確率を返します。",
        "ad": "独立した試行の回数!各試行の成功率!試行における成功数!指定した場合、成功数がこの値と成功数の間に入る確率を返します"},
    # 本家の対訳に無い3つと、こちらでは一部しか受けない CELL(2026-09-02)
    "DBCS": {"a": "(文字列)", "d": "半角(1 バイト)文字を全角(2 バイト)文字に変換します。JIS と同じです。",
        "ad": "変換する文字列、または文字列が入ったセル"},
    "LET": {"a": "(名前1, 値1, [名前2, 値2], ..., 計算)",
        "d": "値に名前を付け、その名前を使って最後の式を計算します。",
        "ad": "最初に付ける名前!その名前が表す値!結果を返す式。付けた名前を使えます"},
    "PY": {"a": "(コード)", "d": "Python のコードを実行して、その結果を返します。セルに単独でだけ使えます。",
        "ad": "実行する Python のコード"},
    # 列の定義(docs/ja/df-manual.adoc)。このアプリだけの関数(2026-09-02)
    "DF": {"a": "(定義1, [定義2], ...)",
        "d": "数式を表の列に属させます。列の各行をその数式で埋め、列が無ければ表に足します。「名前 = 数式」は、この df の中で使える定数です。",
        "ad": "定義。「表[列] = 数式」で列を定義し、「名前 = 数式」でこの df の中の定数を定義します!2つ目からの定義。順番は依存で決まります"},
    "CELL": {"a": "(検査の種類, [参照])",
        "d": "セルの情報を返します。検査の種類は \"filename\" だけを受けます(パス、[ ] で囲んだファイル名、シート名を返します)。",
        "ad": "情報の種類。\"filename\" だけを受けます!セル。同じブックならどのセルでも答えが同じなので、受け取って使いません"},
}
# 本家の対訳に穴があった語をここで埋める(言語 → 関数名 → {a, d, ad})。
# **穴を黙って日本語で埋めない** — 埋めていない穴があれば生成を止める。
#
# 2枚に分けてある:
#   funcs_hand.json — 本家に**項目ごと無い**もの(日本語まわりの4関数ほか)
#   funcs_args.json — 項目はあるが**引数の欄だけ英語のまま**のもの。
#     台湾語は 188/188 の引数名と 177/188 の引数の説明が英語だった。
#     インドネシア語・韓国語・ベトナム語も引数名だけ英語(2026-08-11 に数えた)
def _patch(name):
    p = ROOT / f"calc/{name}.json"
    return json.loads(p.read_text(encoding="utf-8")) if p.exists() else {}


PATCH, ARGS = _patch("funcs_hand"), _patch("funcs_args")

# 分類 → 画面の鍵(ui::tr で訳す記号)。生成する group の欄はこちら
GROUP_KEY = {
    "数学/三角": "math_trig",
    "統計": "statistics",
    "文字列操作": "text_functions",
    "論理": "logical",
    "日付/時刻": "date_time",
    "検索/行列": "lookup_reference",
    "財務": "financial",
    "情報": "information",
    "エンジニアリング": "engineering",
    "データベース": "database",
}

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
    # 引数の欄だけの差し替えは**欄ごとに重ねる** — まるごと置き換えると、
    # 本家が訳してある説明文まで消える
    for name, fix in ARGS.get(loc, {}).items():
        if name in d:
            d[name] = {**d[name], **fix}
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
            rows.append((name, GROUP_KEY[group], args, info["d"], ads))
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
    /// 分類の鍵(math_trig など)。画面に出すときに ui::tr で訳す。
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
        // 言葉は lang から直に取る。前は `ui::language()` 経由だったが、
        // ui は gpui の側なので face からは呼べない(lang は模型の層で
        // gpui を持たない)。**再公開を1つ剥がしただけで中身は同じ**
        let t = crate::funcs_tables::text(lang::i18n::language())?;
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


# マニュアルの英語版の見出し(分類の鍵は日本語のまま)
GROUP_EN = {
    "数学/三角": "Math and trigonometry",
    "統計": "Statistical",
    "文字列操作": "Text",
    "論理": "Logical",
    "日付/時刻": "Date and time",
    "検索/行列": "Lookup and reference",
    "財務": "Financial",
    "情報": "Information",
    "エンジニアリング": "Engineering",
    "データベース": "Database",
}


def manual_source(loc: str) -> str:
    """関数のマニュアル(docs/<loc>/functions.adoc)。

    関数の挿入の小窓と同じ材料から作るので、画面とマニュアルが
    食い違わない。載るのは実際に計算できる関数だけ。
    """
    d = load(loc)
    ja = loc == "ja"
    out = [
        "// このファイルは生成物です — 手で直さないでください。",
        "// 作り直し: python3 calc/gen_funcs.py --manual",
        "= 関数の一覧" if ja else "= Functions",
        ":toc: left",
        "",
    ]
    if ja:
        out += [
            "数式で使える関数の一覧です。載っているのは、このアプリが実際に",
            "計算できる関数だけです。説明は、関数の挿入のダイアログ",
            "(数式タブ、または Shift+F3)に出るものと同じ材料から",
            "作っています。",
            "",
            "引数の `[ ]` は、省略できる引数です。",
        ]
    else:
        out += [
            "The functions you can use in formulas. Only functions this",
            "app actually computes are listed. The descriptions come from",
            "the same source as the Insert Function dialog (Formulas tab,",
            "or Shift+F3).",
            "",
            "Arguments in `[ ]` can be omitted.",
        ]
    for group, names in GROUPS.items():
        out += ["", f"== {group if ja else GROUP_EN[group]}", ""]
        out += ['[cols="1,2"]', "|===",
                "|関数 |説明" if ja else "|Function |Description", ""]
        for name in sorted(names.split()):
            info = d.get(name)
            if not info or not info.get("d"):
                continue
            args = info.get("a", "(…)").replace("; ", ", ")
            # 本家の説明文は頭の空白と文末の句点がまちまち(188 件のうち
            # 21 件に句点が無く、2 件は頭に空白)。写すときにそろえる
            desc = info["d"].strip().replace("|", "\\|")
            owari = "。" if ja else "."
            if desc and desc[-1] not in "。.!?":
                desc += owari
            out.append(f"|`{name}{args}` |{desc}")
        out.append("|===")
    return "\n".join(out) + "\n"


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
    """face/src/lib.rs の mod の登録を書き換える"""
    p = SRCDIR / "lib.rs"
    src = p.read_text(encoding="utf-8")
    block = ("// gen_funcs:begin(この間は calc/gen_funcs.py が生成する — 手で書かない)\n"
             + "".join(f"pub mod {mod_name(l)};\n" for l in locs)
             + "pub mod funcs_tables;\n"
             + "// gen_funcs:end\n")
    if "// gen_funcs:begin" in src:
        src = re.sub(r"// gen_funcs:begin.*?// gen_funcs:end\n", block, src, flags=re.S)
    else:
        src = src.replace("pub mod funcs;\n", "pub mod funcs;\n" + block, 1)
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


def english_signatures(want: dict) -> list[str]:
    """**引数の欄が英語のまま残っていないか。**

    本家は言語によって訳の深さが違う。説明文だけ訳して引数の欄は英語、
    という状態が台湾語で 188/188 続いていた — 画面には中国語の説明の下に
    `ABS(number)` と出る。数えるまで気づかなかった(2026-08-11)。

    **引数の無い関数は数えない。** `PI()` `TODAY()` は英語と同じで当たり前で、
    正しく訳されている言語もちょうどその 7〜10 件だけが一致していた。
    引数があるのに英語と1字も違わないなら、それは訳し忘れ。

    **funcs_args.json に控えのある項目も数えない。** 控えに入れる = 人が
    本家(Excel など)を引いて確かめた印。インドネシア語は array・radix の
    ように英語の語をそのまま使う引数名が正しく、それを控えに書いてある
    (2026-09-02)。確かめていない一致だけを訳し忘れとして数える。
    """
    import re as _re

    def sigs(src: str) -> dict:
        return dict(_re.findall(r'FnText \{ name: "([^"]*)", args: "([^"]*)"', src))

    def helps(src: str) -> dict:
        return dict(_re.findall(
            r'FnText \{ name: "([^"]*)".*?arg_desc: &\[([^\]]*)\]', src))

    base = want[SRCDIR / "funcs_en.rs"]
    en_a, en_ad = sigs(base), helps(base)
    bad = []
    for p, text in want.items():
        loc = p.stem[len("funcs_"):]
        if not p.stem.startswith("funcs_") or loc in ("en", "tables"):
            continue
        # 控え(funcs_args.json)の鍵はダッシュ区切り(zh-tw)、ファイル名は
        # 下線区切り(funcs_zh_tw.rs)なので合わせる
        checked = ARGS.get(loc.replace("_", "-"), {})
        # 引数の欄。語がたまたま一致することはある(fr の DEGREES(angle) など)
        # ので少しだけ許す。**多数が一致していたら訳していない**
        same = [n for n, a in sigs(text).items()
                if a == en_a.get(n) and a.strip("()").strip()
                and "a" not in checked.get(n, {})]
        if len(same) > ALLOW_SAME_SIG:
            bad.append(f"{loc}: 引数の欄が英語のままの関数が {len(same)} 個 "
                       f"— {' '.join(same[:6])}")
        # 引数ごとの説明は**散文**。偶然一致することはまず無いので 0 で締める
        # (実測: 正しい8言語はいずれも 0、台湾語だけ 177 だった)
        same_ad = [n for n, a in helps(text).items()
                   if a.strip() and a == en_ad.get(n)
                   and "ad" not in checked.get(n, {})]
        if same_ad:
            bad.append(f"{loc}: 引数の説明が英語のままの関数が {len(same_ad)} 個 "
                       f"— {' '.join(same_ad[:6])}")
    return bad


# 引数の欄が英語と一致してよい数。**実測**では正しく訳されている9言語が
# 0〜3件(語がたまたま一致する DEGREES(angle) の類)。余裕を見て 8
ALLOW_SAME_SIG = 8


def report_sigs(want: dict) -> int:
    bad = english_signatures(want)
    for b in bad:
        print(f"::error::{b}")
    if bad:
        print("::error::calc/funcs_args.json で引数の欄を直してください")
        return 1
    print("引数の欄: どの言語も英語のままではありません")
    return 0


def main() -> int:
    if "--manual" in sys.argv:
        for loc in ("ja", "en"):
            p = ROOT / f"docs/{loc}/functions.adoc"
            p.write_text(manual_source(loc), encoding="utf-8")
            print(f"{p.relative_to(ROOT)} を書きました")
        return 0
    if "--all" not in sys.argv and "--check" not in sys.argv:
        sys.stdout.write(ja_source())
        return 0

    bad = european_pt_still_holds()
    if bad:
        print(f"::error::{bad}")
        return 1

    locs = sorted(VENDOR)
    want = {SRCDIR / "funcs.rs": ja_source(),
            SRCDIR / "funcs_tables.rs": tables_source(locs),
            # マニュアルも同じ材料の生成物 — 画面と食い違えば --check が止める
            ROOT / "docs/ja/functions.adoc": manual_source("ja"),
            ROOT / "docs/en/functions.adoc": manual_source("en")}
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
        return report_sigs(want)

    for p, s in want.items():
        p.write_text(s, encoding="utf-8")
    register(locs)
    print(f"関数の言葉を {len(locs)} 言語ぶん書きました({len(NAMES)} 関数)")
    # **書いてから見る。** 先に見て止めると、直す人が結果を確かめられない
    return report_sigs(want)


if __name__ == "__main__":
    raise SystemExit(main())
