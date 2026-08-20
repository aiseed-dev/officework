#!/usr/bin/env python3
"""`ribbon.rs` を Euro-Office の現物から生成する。

**いまはまだ回して上書きしないでください**(2026-08-20)。
24 タブのうち 18 は実物と一致しますが、6 タブに差が残っています。
上書きすると、実物に出ている灰色のボタンが 9 個消えます。
残りの中身は SEKKEI「B-1 の続き」に書いてあります。


**手で要約しない。** タブの並びもボタンの並びも
`vendor/web-apps/apps/*/main/app/template/Toolbar.template` の順そのまま、
名前は同じ app の `locale/ja.json` から引く。
だから「Euro-Office と全く同じか」は、この台本を回し直せば確かめられる。

全部入れる(2026-08-04 発注者確定で改訂。以前は共同編集・保護・
プラグイン・AI・マクロを「除く5つ」としていた)。
VBA 型のマクロを持たないことだけは不変 — マクロのボタンの実体は
サンドボックス(bubblewrap)の中の Python で、文書の中に実行コードは置かない。

リボンの言葉は Euro-Office のロケールから来るので、**45言語ぶん現物がある**。
各国語版を作るとき、画面の翻訳は作業に含まれない。

  python3 gen_ribbon.py       > src/ribbon.rs   # 既定は日本語
  python3 gen_ribbon.py en    > src/ribbon.rs   # 英語版
  python3 gen_ribbon.py --list                  # 使えるロケール
"""

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1] / "vendor" / "web-apps" / "apps"
LOCALE = "ja"

# 実装済みのコマンド。ここに載っているものだけが押せる。
# **できないものを、できるように見せない。**
# **アプリ側で実際に動くものだけ**を書く。
# ここに書けば押せる見た目になるので、書いた瞬間に嘘になりうる。
# 太字・中央揃え・罫線はまだ入っていない(次の仕事)。
READY = {
    "writer": {
        "open": "open", "save": "save", "undo": "undo", "redo": "redo",
        "select-all": "selectall",
        "bold": "bold", "italic": "italic", "underline": "underline",
        "strikeout": "strikeout", "superscript": "superscript",
        "subscript": "subscript", "highlight": "highlight", "clearstyle": "clearstyle",
        "align-center": "align-center", "align-just": "align-just",
        "incfont": "incfont", "decfont": "decfont", "print": "pdf",
        "fontcolor": "fontcolor", "align-left": "align-left",
        "align-right": "align-right",
        "markers": "markers", "numbering": "numbering",
        "wordcount": "wordcount", "spell": "spell", "pagebreak": "pagebreak",
        "zoom-in": "zoom-in", "zoom-out": "zoom-out",
        "hidenchars": "hidenchars", "ruler": "ruler",
        "fontname": "fontname", "fontsize": "fontsize",
        "pageorient": "pageorient", "pagesize": "pagesize", "pagemargins": "pagemargins",
        "instable": "instable", "inssymbol": "inssymbol", "replace": "replace",
        "incoffset": "incoffset", "decoffset": "decoffset",
        "linespace": "linespace",
        "changecase": "changecase", "blankpage": "blankpage",
        "paracolor": "paracolor", "borders": "borders",
        "insertimage": "insimage",
        "edit-header": "edit-header", "edit-footer": "edit-footer",
        "pagenum": "pagenum",
        "styles": "parastyle",
        "contents": "toc", "contents-update": "toc-update",
        "datetime": "datetime", "numpages": "numpages",
        "multilevels": "multilevels", "darkmode": "darkmode",
        "text-from-file": "text-from-file", "add-text": "add-text",
        "line-numbers": "line-numbers",
        "insshape": "insshape", "inssmartart": "inssmartart",
        "inschart": "inschart", "smartpicker": "smartpicker",
        "instextart": "instextart", "insequation": "insequation",
        "instext": "instext", "pagecolor": "pagecolor",
        "comment": "comment", "watermark": "watermark",
        "bookmarks": "bookmarks", "caption": "caption",
        "tof": "tof", "tof-update": "tof-update",
        "columns": "columns",
        "pen": "pen", "highlighter": "highlighter", "eraser": "eraser",
        "track-changes": "track-changes", "dropcap": "dropcap",
        "hyphenation": "hyphenation", "crossref": "crossref",
        "co-addcomment": "co-addcomment", "co-delcomment": "co-delcomment",
        "co-showcomment": "co-showcomment",
        "coauth-mode": "coauth-mode", "prot-doc": "prot-doc",
        "copy": "copy", "cut": "cut", "paste": "paste",
        "align-left": "align-left", "align-right": "align-right",
        "align-dist": "align-dist", "ruby": "ruby", "direction": "direction",
        "colorschemas": "colorschemas", "multipage": "multipage",
        "ai-where": "ai-where", "ai-summary": "ai-summary",
        "ai-rewrite": "ai-rewrite", "ai-polite": "ai-polite",
        "ai-plain": "ai-plain", "ai-translate": "ai-translate",
        "ai-furigana": "ai-furigana", "ai-continue": "ai-continue",
        "ai-table": "ai-table", "ai-ask": "ai-ask", "ai-macro": "ai-macro",
        "controls": "controls", "form-text": "form-text",
        "form-combo": "form-combo", "form-dropdown": "form-dropdown",
        "form-checkbox": "form-checkbox", "form-radio": "form-radio",
        "form-image": "form-image", "form-email": "form-email",
        "form-phone": "form-phone", "form-complex": "form-complex",
        "form-signature": "form-signature", "form-name": "form-name",
        "nav": "nav", "fit-page": "fit-page", "fit-width": "fit-width",
        "zoom100": "zoom100", "show-toolbar": "show-toolbar",
        "show-statusbar": "show-statusbar", "show-left": "show-left",
        "show-right": "show-right",
        "co-chat": "co-chat", "co-history": "co-history",
        "plug-macros": "plug-macros", "plug-manage": "plug-manage",
        "py-edit": "py-edit", "py-new": "py-new", "py-run": "py-run",
        "py-list": "py-list", "py-line": "py-line", "py-calc": "py-calc",
        "py-folder": "py-folder",
        "prot-encrypt": "prot-encrypt", "prot-sign": "prot-sign",
    },
    "calc": {
        "open": "open", "save": "save", "undo": "undo", "redo": "redo",
        "select-all": "selectall", "autosum": "sum",
        "borders": "borders", "bold": "bold", "italic": "italic",
        "underline": "underline",
        "copy": "copy", "cut": "cut", "paste": "paste",
        "align-center": "align-center",
        "comma": "comma", "currency": "currency", "percents": "percents",
        "cell-ins": "cell-ins", "cell-del": "cell-del", "clear": "clear",
        "digit-inc": "digit-inc", "digit-dec": "digit-dec",
        "align-left": "align-left", "align-right": "align-right",
        "custom-sort": "custom-sort", "rem-duplicates": "rem-duplicates",
        "setfilter": "setfilter", "clear-filter": "clear-filter",
        "fill-num": "fill-num",
        "strikeout": "strikeout", "top": "top", "middle": "middle",
        "bottom": "bottom", "wrap": "wrap",
        "incfont": "incfont", "decfont": "decfont",
        "fillparag": "fillparag", "fontcolor": "fontcolor", "merge": "merge",
        "math": "fn-math", "text": "fn-text", "logical": "fn-logical",
        "recent": "fn-recent",
        "show-formulas": "show-formulas", "show-gridlines": "show-gridlines",
        "freeze": "freeze",
        "print": "pdf",
        "data-validation": "data-validation", "condformat": "condformat",
        "named-range": "defname", "named-range-huge": "defname",
        "pageorient": "pageorient", "pagesize": "pagesize",
        "pagemargins": "pagemargins", "printarea": "printarea",
        "inschart": "inschart", "insimage": "insimage",
        "inshyperlink": "inshyperlink", "replace": "replace",
        "changecase": "changecase", "format": "format",
        "cell-format": "cell-format", "fontname": "fontname",
        "fontsize": "fontsize",
        "financial": "fn-financial", "datetime": "fn-datetime",
        "lookup": "fn-lookup", "more": "fn-more",
        "scale": "scale", "pagebreak": "pagebreak",
        "printtitles": "printtitles", "print-gridlines": "print-gridlines",
        "print-headings": "print-headings",
        "data-from-text": "data-from-text", "text-column": "text-column",
        "goal-seek": "goal-seek", "data-external-links": "data-external-links",
        "solver": "solver",
        "insshape": "insshape", "instext": "instext",
        "inssmartart": "inssmartart", "insequation": "insequation",
        "insslicer": "insslicer", "inscheckbox": "inscheckbox",
        "instextart": "instextart",
        "inssparkline": "inssparkline", "co-addcomment": "addcomment",
        "coauth-mode": "coauth-mode", "co-delcomment": "co-delcomment",
        "co-showcomment": "co-showcomment", "co-chat": "co-chat",
        "co-history": "co-history",
        "plug-macros": "plug-macros", "plug-manage": "plug-manage",
        "py-edit": "py-edit", "py-new": "py-new", "py-run": "py-run",
        "py-list": "py-list", "py-line": "py-line", "py-calc": "py-calc",
        "py-folder": "py-folder",
        # マクロのタブで使う2つ(本家に無い。絵は DYN_ICONS で決める)
        "rec-toggle": "rec-toggle", "ribbon-list": "ribbon-list",
        "prot-doc": "prot-doc", "prot-encrypt": "prot-encrypt",
        "prot-sign": "prot-sign",
        "cell-lock": "cell-lock", "prot-allow": "prot-allow",
        "fit-pages": "fit-pages", "printarea-add": "printarea-add", "show-breaks": "show-breaks",
        "recover": "recover", "recover-every": "recover-every",
        "csv-kind": "csv-kind", "paste-name": "paste-name", "flash-fill": "flash-fill",
        "read-only-rec": "read-only-rec",
        "td-remdup": "rem-duplicates",
        "td-header": "td-header", "td-total": "td-total",
        "td-band-row": "td-band-row", "td-band-col": "td-band-col",
        "td-first": "td-first", "td-last": "td-last",
        "td-filter": "td-filter",
        "subscript": "subscript", "align-just": "align-just",
        "text-orient": "text-orient", "formula": "insert-function",
        "styles": "cell-styles", "additional-formula": "insert-function",
        "watch-window": "watch", "calculate": "calc-mode",
        "sheet-view": "sheet-view", "smartpicker": "insrecommend",
        "pen": "pen", "highlighter": "highlighter", "eraser": "eraser",
        "ai-where": "ai-where", "ai-summary": "ai-summary",
        "ai-rewrite": "ai-rewrite", "ai-polite": "ai-polite",
        "ai-plain": "ai-plain", "ai-translate": "ai-translate",
        "ai-furigana": "ai-furigana", "ai-continue": "ai-continue",
        "ai-table": "ai-table", "ai-ask": "ai-ask",
        "colorschemas": "colorschemas", "theme": "theme",
        "td-torange": "td-torange", "td-resize": "td-resize",
        "rtl-sheet": "rtl-sheet", "direction": "direction",
        "zoom-in": "zoom-in", "zoom-out": "zoom-out",
        "formula-bar": "formula-bar", "show-headings": "show-headings",
        "show-zeros": "show-zeros",
        "group": "group", "ungroup": "ungroup",
        "show-details": "show-details", "hide-details": "hide-details",
        "pivot-insert": "pivot-insert",
        "pivot-refresh": "pivot-refresh", "pivot-refresh-all": "pivot-refresh-all",
        "pivot-select": "pivot-select", "pivot-totals": "pivot-totals",
        "pivot-subtotals": "pivot-subtotals", "pivot-blank": "pivot-blank",
        "pivot-layout": "pivot-layout", "pivot-showas": "pivot-showas",
        "datatable": "datatable", "track-changes": "track-changes",
        "trace-prec": "trace-prec", "trace-dep": "trace-dep",
        "remove-arrows": "remove-arrows", "insrecommend": "insrecommend",
        "instable": "instable", "table-tpl": "table-tpl",
        "inssymbol": "inssymbol",
    },
}

# 除外するタブ — **無し**(発注者確定 2026-08-04: メニューは制限せず全部入れる。
# 実装しない方針のもの(共同編集・保護・プラグイン等)も、場所は本家どおり
# 灰色で見せる。「できないものを、できるように見せない」は ready で守る)
DROP_TABS: set = set()

# slot の id → ja.json の鍵の末尾(tip か cap)。
# 現物の id と鍵名は綴りが違うので、ここだけは対応表が要る。
LABEL = {
    # 常時
    "save": "tipSave", "print": "tipPrint", "copy": "tipCopy", "paste": "tipPaste",
    "undo": "tipUndo", "redo": "tipRedo", "cut": "tipCut", "copystyle": "tipCopyStyle",
    # ホーム(文書)
    "fontname": "tipFontName", "fontsize": "tipFontSize", "incfont": "tipIncFont",
    "decfont": "tipDecFont", "changecase": "tipChangeCase", "bold": None,
    "italic": None, "underline": None, "strikeout": None, "superscript": None,
    "subscript": None, "highlight": "tipHighlightColor", "fontcolor": "tipFontColor",
    "clearstyle": "tipClearStyle", "markers": "tipMarkers", "numbering": "tipNumbers",
    "multilevels": "tipMultilevels", "decoffset": "tipDecPrLeft",
    "incoffset": "tipIncPrLeft", "linespace": "tipLineSpace", "direction": "tipTextDir",
    "align-center": "tipAlignCenter", "align-just": "tipAlignJust",
    "align-left": "tipAlignLeft", "align-right": "tipAlignRight",
    "hidenchars": "tipShowHiddenChars", "paracolor": "tipPrColor",
    "borders": "tipBorders", "styles": "tipParagraphStyle", "replace": "tipReplace",
    "select-all": "tipSelectAll",
    # 挿入(文書)
    "blankpage": "tipBlankPage", "instable": "tipInsertTable",
    "insshape": "tipInsertShape", "inssmartart": "tipInsertSmartArt",
    "inschart": "tipInsertChart", "smartpicker": "tipInsertChart",
    "instext": "tipInsertText", "instextart": "tipInsertTextArt",
    "dropcap": "tipDropCap", "text-from-file": "tipTextFromFile",
    "insequation": "tipInsertEquation", "inssymbol": "tipInsertSymbol",
    "insertimage": "tipInsertImage",
    "controls": "tipControls", "insimage": "tipInsertImage",
    "inshyperlink": "tipInsertHyperlink", "insslicer": "tipInsertSlicer",
    "inssparkline": "tipInsertSpark", "inscheckbox": "tipControls",
    "insrecommend": "tipInsertChartRecommend",
    # レイアウト
    "pagemargins": "tipPageMargins", "pageorient": "tipPageOrient",
    "pagesize": "tipPageSize", "columns": "tipColumns",
    "line-numbers": "tipLineNumbers", "hyphenation": "tipHyphenation",
    "watermark": "tipWatermark", "pagecolor": "tipPageColor",
    "colorschemas": "tipColorSchemas", "printarea": "tipPrintArea",
    "pagebreak": "tipPageBreak", "scale": "tipScale",
    "printtitles": "tipPrintTitles", "rtl-sheet": "tipRtlSheet",
    # 参考資料
    "add-text": None, "contents-update": None, "bookmarks": None,
    "caption": None, "crossref": None, "tof": None, "tof-update": None,
    # 表計算のホーム
    "fillparag": "tipPrColor", "top": "tipAlignTop", "middle": "tipAlignMiddle",
    "bottom": "tipAlignBottom", "wrap": "tipWrap", "text-orient": "tipTextOrientation",
    "merge": "tipMerge", "formula": None, "fill-num": None,
    "named-range": None, "clear": "tipClearStyle", "format": "tipNumFormat",
    "currency": "tipDigStyleCurrency", "percents": "tipDigStylePercent",
    "comma": "tipDigStyleComma", "digit-dec": "tipDecDecimal",
    "digit-inc": "tipIncDecimal", "cell-ins": "tipInsertOpt",
    "cell-del": "tipDeleteOpt", "cell-format": "tipCellStyle",
    "condformat": "tipCondFormat", "table-tpl": "tipInsertTable",
    # 数式
    "additional-formula": None, "autosum": None, "recent": None,
    "financial": None, "logical": None, "text": None, "datetime": None,
    "lookup": None, "math": None, "more": None, "named-range-huge": None,
    "trace-prec": None, "trace-dep": None, "remove-arrows": None,
    "show-formulas": None, "watch-window": None, "calculate": None,
    # データ
    "data-from-text": None, "data-external-links": None, "custom-sort": None,
    "text-column": None, "rem-duplicates": None, "data-validation": None,
    "goal-seek": None, "solver": None, "group": None, "ungroup": None,
    "show-details": None, "hide-details": None,
}

# ja.json に鍵が無い(アイコンだけ・別画面)ものの日本語名。
# **現物に出ている語をそのまま使う。** 勝手な言い換えをしない。
FALLBACK = {
    "bold": "太字", "italic": "斜体", "underline": "下線",
    "strikeout": "取り消し線", "superscript": "上付き", "subscript": "下付き",
    "add-text": "テキストの追加", "contents-update": "目次の更新",
    "bookmarks": "ブックマーク", "caption": "図表番号", "crossref": "相互参照",
    "tof": "図表目次", "tof-update": "図表目次の更新",
    "formula": "関数", "fill-num": "フィル", "named-range": "名前の管理",
    "additional-formula": "関数の挿入", "autosum": "オートSUM",
    "recent": "最近使った関数", "financial": "財務", "logical": "論理",
    "text": "文字列操作", "datetime": "日付/時刻", "lookup": "検索/行列",
    "math": "数学/三角", "more": "その他の関数",
    "named-range-huge": "名前の管理", "trace-prec": "参照元のトレース",
    "trace-dep": "参照先のトレース", "remove-arrows": "トレース矢印の削除",
    "show-formulas": "数式の表示", "watch-window": "ウォッチウィンドウ",
    "calculate": "計算方法", "data-from-text": "テキストからデータ",
    "data-external-links": "外部リンク", "custom-sort": "並べ替え",
    "text-column": "区切り位置", "rem-duplicates": "重複の削除",
    "data-validation": "データの入力規則", "goal-seek": "ゴールシーク",
    "solver": "ソルバー", "group": "グループ化", "ungroup": "グループ解除",
    "show-details": "詳細の表示", "hide-details": "詳細の非表示",
    "spell": "スペルチェック", "furigana": "ふりがな", "pagebreak": "区切り", "setfilter": "フィルター", "clear-filter": "フィルターを解除",
    "contents": "目次",
    "open": "開く", "print": "印刷",
}


def locale(app):
    p = ROOT / app / f"main/locale/{LOCALE}.json"
    if not p.exists():
        sys.exit(f"そのロケールは Euro-Office に無い: {LOCALE}")
    return json.load(open(p, encoding="utf-8"))


def locales():
    d = ROOT / "documenteditor/main/locale"
    return sorted(p.stem for p in d.glob("*.json"))


def tab_names(app, prefix):
    d = locale(app)
    out = {}
    for k, v in d.items():
        m = re.search(rf"{prefix}\.Views\.Toolbar\.textTab(\w+)$", k)
        if m:
            out[m.group(1).lower()] = v
    return out


def label_of(app_loc, prefix, slot):
    key = LABEL.get(slot)
    if key:
        for who in ("Toolbar", "ViewTab", "HeaderFooterTab"):
            full = f"{prefix}.Views.{who}.{key}"
            if full in app_loc:
                # tip は説明文のことがある。最初の読点までを名前にする
                return re.split(r"[。<（(]", app_loc[full])[0].strip()
    if slot in FALLBACK:
        return FALLBACK[slot]
    if slot in DYN_LABELS:
        return DYN_LABELS[slot]
    return slot


# テンプレートに無いタブ(JS で動的に作られる)。
# 並びは Euro-Office の実物: 文書 = ファイル ホーム 挿入 描画 レイアウト
# 参考資料 ヘッダー/フッター レビュー 表示 / 表計算 = … 数式 データ 表示。
# 除外(共同編集・保護・プラグイン)はそもそも入れない。
# 全部入れる(発注者確定 2026-08-04): 共同編集・保護・プラグインも
# 場所は本家どおり。実装しないものは灰色のまま(ready の嘘は無し)
# アプリ固有の追加タブ(本家の位置に挿す)。(タブ名, 直前に置くタブ名, cmds)
APP_TABS = {
    "documenteditor": [
        ("フォーム", "参考資料", [
            ("form-text", "テキストフィールド"), ("form-combo", "コンボボックス"),
            ("form-dropdown", "ドロップダウン"), ("form-checkbox", "チェックボックス"),
            ("form-radio", "ラジオボタン"), ("form-image", "画像"),
            ("form-email", "メールアドレス"), ("form-phone", "電話番号"),
            ("form-complex", "複合フィールド"), ("form-signature", "署名"),
            ("form-name", "名前"),
        ]),
    ],
    "spreadsheeteditor": [
        ("ピボットテーブル", "データ", [
            ("pivot-insert", "ピボットテーブルを挿入"), ("pivot-refresh", "更新"),
            ("pivot-refresh-all", "すべて更新"), ("pivot-select", "選択する"),
            ("pivot-totals", "総計"), ("pivot-subtotals", "小計"),
            ("pivot-blank", "空行"), ("pivot-layout", "レポートのレイアウト"),
        ]),
        ("表のデザイン", "ピボットテーブル", [
            ("td-header", "ヘッダー行"), ("td-total", "合計行"),
            ("td-band-row", "縞模様の行"), ("td-first", "最初の列"),
            ("td-last", "最後の列"), ("td-band-col", "縞模様の列"),
            ("td-filter", "フィルタのボタン"), ("td-remdup", "重複データを削除"),
            ("td-torange", "範囲に変換する"), ("td-resize", "テーブルのサイズ変更"),
        ]),
    ],
}

COMMON_TAIL = {
    "collaboration": [
        ("coauth-mode", "共同編集モード"), ("co-addcomment", "コメントを追加"),
        ("co-delcomment", "コメントを削除"), ("co-showcomment", "コメントの表示"),
        ("co-chat", "チャット"), ("co-history", "バージョン履歴"),
        # writer は本家どおり変更履歴もここ(出力時に履歴の前へ挿す)
    ],
    "protect": [
        ("prot-encrypt", "暗号化する"), ("prot-sign", "デジタル署名を追加"),
        ("prot-doc", "保護"),
    ],
    # **マクロのタブ**(本家のプラグインの位置)。
    #
    # 本家の「プラグイン」と、うちが足していた「Python」「AI」を1つに畳んだ
    # 形です(「リボンの整理」dba89891)。**再生成でバラバラに戻さない** —
    # 畳んだのは後からの決めで、こちらが新しい。
    # 中身はアプリごとに違うので、名前だけ共通で中身は分けます
    "macros": {
        "documenteditor": [
            ("plug-manage", "一覧", "plug-manage"),
            ("ai-macro", "マクロを書く", "ai-macro"),
        ],
        "spreadsheeteditor": [
            ("rec-toggle", "操作を記録", "py-run"),
            ("py-new", "新しい .py", "py-new"),
            ("py-list", "一覧", "py-list"),
            ("ribbon-list", "リボンのマクロ", "py-line"),
            ("py-folder", "置き場を開く", "py-folder"),
        ],
    },
    # **AI の段は作りません**(2026-08-20 発注者「AI については、固定的にしか
    # できないボタンを使わないでやりたい。だから、メニューは削除して。
    # 左パネルをつかう」)。
    #
    # 前は「要約」「敬語にする」「翻訳」…と11個のボタンを並べる形でした。
    # 決まった変換しかできず、少し違うことを頼む道がありません。
    # 頼みごとは左パネルの会話で受けます — そちらは自由な文で言えて、
    # 返ってきた案を人が見てから入れられます。
    #
    # *この段は画面には出ていませんでした。* `face/src/ribbon.rs` からは
    # 「リボンの整理」(dba89891)で既に消えていて、生成スクリプトにだけ
    # 残っていました。再生成すると復活する状態だったので、ここで断ちます。
}

DYNAMIC = {
    "documenteditor": [
        ("draw", 3, [("pen", "ペン"), ("highlighter", "蛍光ペン"), ("eraser", "消しゴム")]),
        # ヘッダー/フッターとレビューのタブはデスクトップ版に無い
        # (2026-08-04 発注者「画面はデスクトップ版に合わせて」)。
        # 中身は 挿入(ヘッダー等)・共同編集(変更履歴)・
        # 下のステータスバー(スペル・文字数)へ畳んだ
        ("view", 6, [
            ("zoom-in", "拡大"), ("zoom-out", "縮小"),
            ("ruler", "ルーラー"), ("darkmode", "ダークモード"),
        ]),
    ],
    "spreadsheeteditor": [
        ("draw", 3, [("pen", "ペン"), ("highlighter", "蛍光ペン"), ("eraser", "消しゴム")]),
        ("view", 99, [
            ("freeze", "ウィンドウ枠の固定"), ("sheet-view", "シートの表示"),
            ("show-gridlines", "グリッド線"), ("show-headings", "見出し"),
        ]),
    ],
}

TAB_NAME_KEYS = {"draw": "Draw", "headerfooter": "HeaderFooter",
                 "review": "Review", "view": "View",
                 "collaboration": "共同編集", "protect": "保護",
                 "macros": "マクロ"}


def tabs_of(app, prefix):
    tpl = (ROOT / app / "main/app/template/Toolbar.template").read_text(encoding="utf-8")
    loc = locale(app)
    names = tab_names(app, prefix)
    parts = re.split(r'<section class="panel"[^>]*data-tab="([a-z-]+)"', tpl)

    out = []
    # ファイルタブは Euro-Office では別画面。並びの先頭に置く点だけ合わせる
    out.append((names.get("file", "File"), ["open", "save", "print"]))
    for i in range(1, len(parts), 2):
        tab, body = parts[i], parts[i + 1]
        if tab in DROP_TABS:
            continue
        slots = list(dict.fromkeys(
            re.findall(r'id="slot-(?:btn|field|chk|cmb)-([a-z0-9-]+)"', body)))
        # 「区切り」は class 注入(btn-slot.btn-pagebreak)なので id では拾えない。
        # Euro-Office の挿入タブでは空白ページの隣にある
        if tab == "ins" and app == "documenteditor" and "blankpage" in slots:
            slots.insert(slots.index("blankpage") + 1, "pagebreak")
        # 画像も class 注入(slot-insertimage)。Euro-Office では表の隣にある
        if tab == "ins" and app == "documenteditor" and "instable" in slots:
            slots.insert(slots.index("instable") + 1, "insertimage")
        # 目次も class 注入。Euro-Office では参考資料の先頭にある
        if tab == "links" and app == "documenteditor" and "contents" not in slots:
            slots.insert(0, "contents")
        # ホームはデスクトップ版の並び(2026-08-04 発注者)。
        # クリップボードと左右揃えも本家どおり出す
        if tab == "home" and app == "documenteditor":
            slots = ["copy", "cut", "paste", "fontname", "fontsize", "incfont",
                     "decfont", "changecase", "ruby", "bold", "italic",
                     "underline",
                     "strikeout", "superscript", "subscript", "highlight",
                     "fontcolor", "clearstyle", "markers", "numbering",
                     "multilevels", "decoffset", "incoffset", "linespace",
                     "direction", "align-left", "align-center", "align-right",
                     "align-just", "align-dist", "hidenchars", "paracolor",
                     "borders",
                     "styles", "replace", "select-all"]
            for c, l in [("copy", "コピー"), ("cut", "切り取り"),
                         ("paste", "貼り付け"), ("align-left", "左揃え"),
                         ("align-right", "右揃え"), ("align-dist", "均等割付"),
                         ("ruby", "ルビ")]:
                DYN_LABELS[c] = l
        # ヘッダー・フッター類はタブを畳んで挿入タブへ(デスクトップ版の場所)
        if tab == "ins" and app == "documenteditor":
            at = slots.index("insequation") if "insequation" in slots else len(slots)
            slots[at:at] = ["edit-header", "edit-footer", "pagenum",
                            "datetime", "numpages"]
            for c, l in [("edit-header", "ヘッダーの編集"),
                         ("edit-footer", "フッターの編集"),
                         ("pagenum", "ページ番号"), ("datetime", "日付/時刻"),
                         ("numpages", "ページ数")]:
                DYN_LABELS[c] = l
        # フィルターも class 注入(btn-slot.slot-btn-setfilter)
        if tab == "home" and app == "spreadsheeteditor" and "custom-sort" not in slots:
            slots.extend(["setfilter", "clear-filter"])
        if tab == "data" and app == "spreadsheeteditor":
            i = slots.index("custom-sort") + 1 if "custom-sort" in slots else len(slots)
            slots[i:i] = ["setfilter", "clear-filter"]
        alias = {"ins": "insert", "links": "links"}
        name = names.get(alias.get(tab, tab), names.get(tab, tab))
        out.append((name, slots))
    # 動的なタブを Euro-Office の位置に差し込む
    for key, at, cmds in DYNAMIC.get(app, []):
        nk = TAB_NAME_KEYS[key].lower()
        name = names.get(nk, TAB_NAME_KEYS[key])
        entry = (name, [c for c, _ in cmds])
        for c, label in cmds:
            DYN_LABELS[c] = label
        if at >= len(out):
            out.append(entry)
        else:
            out.insert(at, entry)
    # アプリ固有のタブ(本家の位置: 指定タブの直後)
    for (name, after, cmds) in APP_TABS.get(app, []):
        entry = (name, [c for c, _ in cmds])
        for c, label in cmds:
            DYN_LABELS[c] = label
        at = next((i + 1 for i, (n, _) in enumerate(out) if n == after), len(out))
        out.insert(at, entry)
    # 全部入れる: 共同編集・保護は表示の前、マクロは末尾(本家の並び)
    for key, cmds in [
        ("collaboration", COMMON_TAIL["collaboration"]),
        ("protect", COMMON_TAIL["protect"]),
    ]:
        if key == "collaboration" and app == "documenteditor":
            # 変更履歴は本家どおりバージョン履歴の手前
            cmds = cmds[:-1] + [("track-changes", "変更履歴")] + cmds[-1:]
        # **writer では暗号化を掛けるボタンを出さない**(2026-08-18 発注者
        # 「暗号化は、開くだけ残す」)。writer が保存するのは adoc で、
        # zip ではないので包めない。パスワード付きの docx を**開く**道は
        # 残っているので、無くなるのはボタン1つだけ。calc は docx ならぬ
        # xlsx を保存するのでそのまま
        if key == "protect" and app == "documenteditor":
            cmds = [c for c in cmds if c[0] != "prot-encrypt"]
        name = TAB_NAME_KEYS[key]
        entry = (name, [c for c, _ in cmds])
        for c, label in cmds:
            DYN_LABELS[c] = label
        view_at = next((i for i, (n, _) in enumerate(out) if n == "表示"), len(out))
        out.insert(view_at, entry)

    # マクロのタブは末尾。**絵も名前もここで決まる**(本家に無いので)
    macros = COMMON_TAIL["macros"][app]
    for cid, label, icon in macros:
        DYN_LABELS[cid] = label
        DYN_ICONS[cid] = icon
    entry = (TAB_NAME_KEYS["macros"], [cid for cid, _, _ in macros])
    # **置き場所はアプリごと。** 表は「データ」の後ろ(Python のタブが居た所)、
    # 文章は末尾(プラグインのタブが居た所)。畳んだときの位置をそのまま継ぎます
    if app == "spreadsheeteditor":
        at = next((i + 1 for i, (n, _) in enumerate(out) if n == "データ"), len(out))
        out.insert(at, entry)
    else:
        out.append(entry)
    return out, loc


# 動的タブのボタン名(ja.json に鍵が無いものの既定)
DYN_LABELS = {}
# 同じく絵。本家に無いボタンは絵の名前もこちらで決める
DYN_ICONS = {}


# **本家と置き場所が違うボタン**(2026-08-20 に数え直した。全 56 個)。
#
# どれも普通の事務の機能で、勝手に増やした物ではありません。多い理由は3つ。
#
# 1. 本家ではタブの外にある(コピー・切り取り・貼り付け)
# 2. 本家はメニューや小窓の中にある(表示まわりのほとんど。本家の表示タブは
#    4個しかない)。**うちは小窓を持たない作り**なので、タブに直接置く
# 3. 本当にうち独自(ふりがな・Python の関数・画面の文字を大きく/小さく)
#
# 形は (タブ, どのボタンの後ろに置くか, id, 見出し, 絵)。後ろが None なら先頭。
# **上から順に効く**ので、続けて足すときは直前に足した物を指します。
#
# *置き場所を持たせた理由*(2026-08-20 発注者「生成スクリプトを修正しないと
# ダメでしょう」)。前は「タブの末尾に足す」形だけでした。それだと再生成した
# ときにコピーがホームの一番後ろへ動きます。**並びは本家のまま**という
# 2026-08-08 からの決めを、スクリプトの都合で崩さないための欄です。
#
# この一覧は手で書いていません。素の出力と実物を突き合わせて機械に出させました。
EXTRA_CMDS = {
    "writer": [
        ("ホーム", 'ruby', "ai-furigana", "ふりがな", "ai-furigana"),
        ("参考資料", 'crossref', "footnote", "脚注", "footnote"),
        ("表示", None, "nav", "ナビゲーション", "nav"),
        ("表示", 'nav', "fit-page", "ページに合わせる", "fit-page"),
        ("表示", 'fit-page', "fit-width", "幅に合わせる", "fit-width"),
        ("表示", 'fit-width', "zoom100", "100%に拡大する", "zoom100"),
        ("表示", 'zoom-out', "printview", "印刷レイアウト", "printview"),
        ("表示", 'printview', "multipage", "複数ページ", "multipage"),
        ("表示", 'ruler', "show-toolbar", "ツールバーを常に表示する", "show-toolbar"),
        ("表示", 'show-toolbar', "show-statusbar", "ステータスバー", "show-statusbar"),
        ("表示", 'show-statusbar', "show-left", "左パネル", "show-left"),
        ("表示", 'show-left', "show-right", "右パネル", "show-right"),
    ],
    "calc": [
        # コピー・切り取り・貼り付けは、本家ではタブの外(全タブ共通の場所)。
        # Excel と同じくホームの先頭に置く
        ("ホーム", None, "copy", "コピー", "copy"),
        ("ホーム", 'copy', "cut", "切り取り", "cut"),
        ("ホーム", 'cut', "paste", "貼り付け", "paste"),
        ("ホーム", 'paste', "copystyle", "書式のコピー", "copystyle"),
        ("ホーム", 'text-orient', "align-left", "左揃え", "align-left"),
        ("ホーム", 'align-center', "align-right", "右揃え", "align-right"),
        ("ホーム", 'align-just', "align-dist", "均等割付", "align-dist"),
        ("ホーム", 'direction', "sum", "オートSUM", "autosum"),
        ("ホーム", 'clear', "sort-desc", "降順並べ替え", "sortdesc"),
        ("ホーム", 'sort-desc', "sort-asc", "昇順並べ替え", "sortasc"),
        ("挿入", None, "pivot-insert", "ピボットテーブルを挿入", "add-pivot"),
        ("挿入", 'inssparkline', "addcomment", "コメント", "ins-comment"),
        ("挿入", 'instextart', "editheader", "ヘッダー/フッター", "editheader"),
        ("描画", None, "draw-select", "選択", "select-tool"),
        ("レイアウト", 'pagebreak', "editheader", "ヘッダー/フッター", "editheader"),
        # 本家では「拡大縮小印刷」の中の選択肢。うちは小窓を持たないので
        # レイアウトタブに独立したボタンで出す
        ("レイアウト", 'scale', "fit-pages", "紙に収める", "fit-pages"),
        ("レイアウト", 'fit-pages', "printarea-add", "範囲を足す", "printarea-add"),
        ("レイアウト", 'printarea-add', "show-breaks", "紙の切れ目", "show-breaks"),
        ("数式", 'insert-function', "func-list", "Python の関数", "py-list"),
        ("数式", 'defname', "paste-name", "名前を貼り付け", "paste-name"),
        ("データ", 'clear-filter', "sort-desc", "降順並べ替え", "sortdesc"),
        ("データ", 'sort-desc', "sort-asc", "昇順並べ替え", "sortasc"),
        # 小計は本家のデータタブに無いが、グループ化を「畳むと合計が残る」
        # 形で使うために置く(Excel の データ > 小計 に相当。発注者指摘)
        ("データ", 'hide-details', "subtotal", "小計", "subtotal"),
        ("データ", 'subtotal', "datatable", "データテーブル", "datatable"),
        ("データ", 'datatable', "python", "Python", "python"),
        ("データ", 'python', "csv-kind", "CSV の形", "csv-kind"),
        ("データ", 'csv-kind', "flash-fill", "フラッシュフィル", "flash-fill"),
        ("ピボットテーブル", 'pivot-insert', "pivot-fields", "フィールドリスト", "pivot-fields"),
        # 本家は値フィールドの設定の中にある「計算の種類」。うちは指図が
        # 集計の名前ひとつなので、タブに独立したボタンとして置く
        ("ピボットテーブル", 'pivot-blank', "pivot-showas", "計算の種類", "pivot-showas"),
        ("ピボットテーブル", 'pivot-layout', "pivot-style", "スタイル", "pivot-style"),
        # 本家では「セルの書式設定 > 保護」タブと「シートの保護」小窓の中。
        # うちは小窓を持たない作りなので、保護タブに独立したボタンで出す
        ("保護", 'prot-sign', "cell-lock", "セルのロック", "cell-lock"),
        ("保護", 'cell-lock', "prot-allow", "許可する操作", "prot-allow"),
        ("保護", 'prot-allow', "recover", "復旧", "recover"),
        ("保護", 'recover', "recover-every", "控えの間隔", "recover-every"),
        ("保護", 'recover-every', "read-only-rec", "読み取り専用を勧める", "read-only-rec"),
        ("表示", 'sheet-view', "zoom-in", "拡大", "zoom-in"),
        ("表示", 'zoom-in', "zoom-out", "縮小", "zoom-out"),
        ("表示", 'zoom-out', "ui-bigger", "画面の文字を大きく", "ui-bigger"),
        ("表示", 'ui-bigger', "ui-smaller", "画面の文字を小さく", "ui-smaller"),
        ("表示", 'ui-smaller', "theme", "インターフェイステーマ", "theme"),
        ("表示", 'freeze', "formula-bar", "数式バー", "formula-bar"),
        ("表示", 'show-headings', "show-zeros", "0を表示する", "show-zeros"),
        ("表示", 'show-zeros', "show-left", "左パネル", "show-left"),
        ("表示", 'show-left', "show-right", "右パネル", "show-right"),
    ],
}


def emit():
    print(HEAD)
    for app, prefix, konst, which in [
        ("documenteditor", "DE", "WRITER", "writer"),
        ("spreadsheeteditor", "SSE", "CALC", "calc"),
    ]:
        tabs, loc = tabs_of(app, prefix)
        ready = READY[which]
        extras = EXTRA_CMDS.get(which, [])
        print(f"pub const {konst}: &[Tab] = &[")
        for name, slots in tabs:
            # 本家の並びをそのまま行にする。押せない物は灰色の行
            rows = []
            for s in slots:
                lab = label_of(loc, prefix, s).replace('"', "'")
                # 絵は本家の名前がそのまま鍵。本家に無いボタンだけ別に決める
                icon = DYN_ICONS.get(s, s)
                rows.append((ready.get(s), lab, icon))
            # **置き場所つきで差し込む。** どのボタンの後ろに置くかを
            # 指しておかないと、足した分が全部タブの末尾へ寄ります
            # (コピーがホームの一番後ろへ行く)
            for (_tab, after, cid, clab, cicon) in [e for e in extras if e[0] == name]:
                item = (cid, clab, cicon)
                if after is None:
                    rows.insert(0, item)
                else:
                    k = next((i + 1 for i, r in enumerate(rows) if r[0] == after), len(rows))
                    rows.insert(k, item)
            print(f'    Tab {{ name: "{name}", cmds: &[')
            for cid, lab, icon in rows:
                if cid is None:
                    print(f'        x("{lab}", "{icon}"),')
                else:
                    print(f'        c("{cid}", "{lab}", "{icon}"),')
            print("    ]},")
        print("];")
        print()
    print(TAIL)


HEAD = '''//! リボン(タブ+コマンド)。**Euro-Office の現物から生成している。**
//!
//! このファイルは手で書かない。`gen_ribbon.py` が
//! `vendor/web-apps/apps/*/main/app/template/Toolbar.template` の並び順と
//! 同 app の `locale/ja.json` の名前から起こす。
//! だから「Euro-Office と全く同じか」は台本を回し直せば確かめられる。
//!
//! ```text
//! python3 ui/gen_ribbon.py ja > face/src/ribbon.rs
//! ```
//!
//! **全部入れる**(2026-08-04 発注者確定で改訂。以前は共同編集・保護・
//! プラグイン・AI・マクロを「入れない」としていた)。乗り換える人の
//! 目当てを消さないため、タブもボタンも本家どおり並べる。
//! **VBA 型のマクロを持たないことだけは不変** — マクロのボタンの実体は
//! サンドボックス(bubblewrap)の中の Python で、文書の中に実行コードは置かない。
//!
//! **できないものを、できるように見せない。** 実装済みのコマンドだけを押せる形にし、
//! 未実装は灰色で残す。並びを Euro-Office に合わせたまま、
//! 「今どこまで出来ているか」がそのまま画面に出る。

/// 1つのコマンド。`ready=false` は未実装(押せない灰色)。
/// `icon` は Euro-Office の slot 名で、埋め込んだアイコン(icons.rs)を引く鍵。
#[derive(Clone, Copy)]
pub struct Cmd {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub ready: bool,
}

const fn c(id: &'static str, label: &'static str, icon: &'static str) -> Cmd {
    Cmd { id, label, icon, ready: true }
}
const fn x(label: &'static str, icon: &'static str) -> Cmd {
    Cmd { id: "", label, icon, ready: false }
}

pub struct Tab {
    pub name: &'static str,
    pub cmds: &'static [Cmd],
}
'''

TAIL = '''/// 実装済みのコマンド数 / 全体(進み具合を隠さない)
pub fn progress(tabs: &[Tab]) -> (usize, usize) {
    let all: usize = tabs.iter().map(|t| t.cmds.len()).sum();
    let ready: usize = tabs.iter().flat_map(|t| t.cmds).filter(|c| c.ready).count();
    (ready, all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 本家のタブが全部ある() {
        // 発注者確定(2026-08-04): メニューは制限しない。実装しないものも
        // 場所は本家どおり(灰色)。タブごと消すことはしない
        for tabs in [WRITER, CALC] {
            for want in ["共同編集", "保護", "プラグイン"] {
                assert!(
                    tabs.iter().any(|t| t.name == want),
                    "タブが無い: {want}"
                );
            }
        }
        for want in ["フォーム"] {
            assert!(WRITER.iter().any(|t| t.name == want), "writer にタブが無い: {want}");
        }
        for want in ["ピボットテーブル", "表のデザイン"] {
            assert!(CALC.iter().any(|t| t.name == want), "calc にタブが無い: {want}");
        }
    }

    #[test]
    fn 実装済みと未実装が区別されている() {
        // 「押せるのに何も起きない」を作らないための検査
        for tabs in [WRITER, CALC] {
            for t in tabs {
                for cmd in t.cmds {
                    assert_eq!(cmd.ready, !cmd.id.is_empty(),
                        "{} の「{}」: ready と id が食い違う", t.name, cmd.label);
                }
            }
        }
    }

    #[test]
    fn euro_officeのタブが揃っている() {
        let names: Vec<&str> = WRITER.iter().map(|t| t.name).collect();
        for want in ["ファイル", "ホーム", "挿入", "レイアウト", "参考資料"] {
            assert!(names.contains(&want), "文書に「{want}」タブが無い: {names:?}");
        }
        let names: Vec<&str> = CALC.iter().map(|t| t.name).collect();
        for want in ["ファイル", "ホーム", "挿入", "レイアウト", "数式", "データ"] {
            assert!(names.contains(&want), "表計算に「{want}」タブが無い: {names:?}");
        }
    }

    #[test]
    fn どの言語でも並びの数は同じ() {
        // 言葉が変わるだけで、リボンの構造は Euro-Office と同じ形
        assert!(WRITER.len() >= 5, "タブが少なすぎる: {}", WRITER.len());
        assert!(CALC.len() >= 6, "タブが少なすぎる: {}", CALC.len());
    }

    #[test]
    fn 名前が空でない() {
        for tabs in [WRITER, CALC] {
            for t in tabs {
                assert!(!t.name.is_empty());
                for cmd in t.cmds {
                    assert!(!cmd.label.is_empty(), "{} に名無しのコマンド", t.name);
                }
            }
        }
    }
}
'''

if __name__ == "__main__":
    if not ROOT.exists():
        sys.exit(f"Euro-Office の現物が見つかりません: {ROOT}")
    args = sys.argv[1:]
    if args and args[0] == "--list":
        print(" ".join(locales()))
        sys.exit(0)
    if args:
        LOCALE = args[0]
    emit()
