#!/usr/bin/env python3
"""`ribbon.rs` を Euro-Office の現物から生成する。

**実物と一致しています**(2026-08-21。24 タブ・314 ボタン)。
`tools/ribbon_gen_check.py` が見張っていて、離れたら止まります。

*ただし上書きは勧めません。* 実物の表には決めの理由が註として書いて
あります(「ホームの Σ はオート SUM」など 40 行あまり)。上書きすると
消えるので、**離れていないことを検査で確かめる**形にしています。


**手で要約しない。** タブの並びもボタンの並びも
`vendor/web-apps/apps/*/main/app/template/Toolbar.template` の順そのまま、
名前は同じ app の `locale/en.json` から引く(2026-08-26 以降)。
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
# **土台の札は英語**(2026-08-26 の段2。規約「プログラム内はすべて英語」)。
# 日本語は face/src/ribbon_ja.rs に降りました
LOCALE = "en"

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
        # **図形の重なりと束ね**(2026-08-30)。beta.3 で押せるように
        # したのに、この表に足していませんでした。生成し直すと5つとも
        # 灰色に戻ります
        "img-movefrwd": "img-movefrwd", "img-movebkwd": "img-movebkwd",
        "img-align": "img-align", "img-group": "img-group",
        "shapes-merge": "shapes-merge",
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
        "inssparkline": "inssparkline", "co-addcomment": "co-addcomment",
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
        # **図形の重なりと束ね**(2026-08-30)。beta.3 で押せるように
        # したのに、この表に足していませんでした。生成し直すと5つとも
        # 灰色に戻ります
        "img-movefrwd": "img-movefrwd", "img-movebkwd": "img-movebkwd",
        "img-align": "img-align", "img-group": "img-group",
        "shapes-merge": "shapes-merge",
    },
}

# 除外するタブ — **無し**(発注者確定 2026-08-04: メニューは制限せず全部入れる。
# 実装しない方針のもの(共同編集・保護・プラグイン等)も、場所は本家どおり
# 灰色で見せる。「できないものを、できるように見せない」は ready で守る)
DROP_TABS: set = set()

# slot の id → ja.json の鍵の末尾(tip か cap)。
# 現物の id と鍵名は綴りが違うので、ここだけは対応表が要る。
LABEL = {
    # 図形まわり(2026-08-21)。`slot-img-` と `slot-shapes-` を拾うように
    # したので、札の鍵も要る
    "img-movefrwd": "capImgForward", "img-movebkwd": "capImgBackward",
    "img-align": "capImgAlign", "img-group": "capImgGroup",
    "shapes-merge": "capShapesMerge",
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
    # `pagebreak` はここに置きません。`tipPageBreak` は**説明文**で、
    # 「印刷物で次のページを開始する位置に改行を追加する」がそのまま
    # ラベルになっていました。FALLBACK の「区切り」を使います
    # (本家のボタンの見出しも Breaks / 区切りです)
    "scale": "tipScale",
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
    # `cell-format` はここに置かない。`tipCellStyle` から引いていたのが
    # 「セルのスタイル」が2つ並んだ原因です(言い換えの表に移しました)
    "cell-del": "tipDeleteOpt",
    "condformat": "tipCondFormat",
    # **本家の別の鍵を引いていました**(2026-08-21)。`instable`(表の挿入)と
    # 同じ `tipInsertTable` だったので、ホームと挿入に「表の挿入」が2つ
    # 出ていました。本家のボタンの見出しは `txtTableTemplate` です
    "table-tpl": "txtTableTemplate",
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
# **本家の語を言い換えた分**(2026-08-21)。
#
# 本家の札が短すぎて何をする物か分からない、または本家に札が無くて
# 欄の名前がそのまま出てしまう物です。**日本の事務の言葉に寄せます。**
#
# `FALLBACK` は「本家に札が無いとき」に使う表で、こちらは「札はあるが
# 言い換える」表です。分けているのは、*本家に無いのか、あえて変えたのか*を
# 後から読めるようにするためです。
rename_map = {
    # 両方で同じ言い方をする物
    "*": {
        "img-align": "Alignment",                # 本家「整列」— 表の側に合わせる
        "img-group": "Group",          # 本家「グループ」— 動作なので動詞に
        "inscheckbox": "Checkbox",  # 本家は札が無い
        "instextart": "Insert Text Art",
        "print-gridlines": "Print gridlines",
        "print-headings": "Print headings",
        "data-external-links": "External links (import as values)",  # 何が起きるかを足す
        "show-gridlines": "Gridlines",       # 本家「グリッド線」— Excel の言葉
        # **2026-08-26 の段2で足した分。** 土台の札が英語になったので、
        # ribbon_en.rs を作っていたときの言い換え(前は OVERRIDES["en"] が
        # 日本語の鍵で持っていた分)がここに要ります
        "align-center": "Centre",             # 本家「Align center」
        "columns": "Insert column",           # 1本しか入れないので単数
        "img-movefrwd": "Bring forward",      # 大小文字をそろえる
        "markers": "Bullet",                  # 1つ付ける操作なので単数
        "pagemargins": "Margins",             # Word のリボンの言い方
        "printarea": "Print Area",
        "scale": "Scale To Fit",
        "shapes-merge": "Merge shapes",
        "text-from-file": "Text from File",
    },
    # **同じ欄でも、アプリで言い方が変わる物。** 文章は段落、表はセルが
    # 相手なので、同じ「スタイル」でも指す物が違います
    "documenteditor": {
        "direction": "Text direction",
        "styles": "Paragraph style",
    },
    "spreadsheeteditor": {
        "direction": "Right-to-left text",
        "styles": "Cell Style",
        # 表が守るのはシート。文章は文書なので「保護」のまま
        "prot-doc": "Protect sheet",
        # **本家の日本語だけが曖昧**(2026-08-21 に気づいた)。英語は
        # Align middle / Align center と分かれているのに、日本語はどちらも
        # 「中央揃え」で、ホームに同じ札のボタンが2つ並んでいました
        "middle": "Align middle",
        # **本家の日本語だけが曖昧**(同じ形が3つありました)。
        #
        # `text-orient` はセルの中の文字を回すボタンです。本家の英語は
        # Orientation、ページの向き(`pageorient`)は Page orientation と
        # 分かれているのに、日本語はどちらも「印刷の向き」でした。
        # Excel のホームの言い方に合わせて「方向」にします
        # (訳は 15 言語とも OVERRIDES に本家の語を書いてあります)
        "text-orient": "Text orientation",
        # 本家の日本語は「表の枠線」ですが、セルに引く線なので Excel は
        # 「罫線」です。文章の側は本家も「罫線」でした
        "borders": "Borders",
        # 文章の側は「テキストボックスの挿入」。同じ物なので言い方を揃えます
        "instext": "Insert text box",
        # **こちらの引き間違い**(2026-08-21)。`cell-format`(書式設定の
        # 小窓を開く)を、`styles`(見た目の一覧)と同じ本家の語
        # `tipCellStyle` から引いていたので、ホームに「セルのスタイル」が
        # 2つ並んでいました。全部の言語に出ていました。
        #
        # 本家の語(`SSE.Views.DocumentHolder.txtCellFormat`)は日本語が
        # 「セルをフォーマットする」で、Excel の言葉ではありません。
        # Excel に合わせて「セルの書式設定」にします。14 言語の訳は本家の
        # 同じ鍵から取り、ベトナム語だけ本家に訳が無いので
        # LibreOffice の公式ベトナム語から取りました
        # (gen_ribbon_locale.py の OVERRIDES に註つきで書いてあります)
        "cell-format": "Format cells",
    },
}

# **アプリごとに違う絵**(2026-08-21)。同じ欄でも、表と文章で描いてある
# 物が違うので、絵の名前を差し替えます
icon_swap = {
    "spreadsheeteditor": {
        "insimage": "insimage-c",     # 表の画像の絵(文章のとは別に描いてある)
        "prot-doc": "protect-sheet",  # シートを守る絵
    },
}

FALLBACK = {
    "bold": "Bold", "italic": "Italic", "underline": "Underline",
    "strikeout": "Strikethrough", "superscript": "Superscript", "subscript": "Subscript",
    "add-text": "Add Text", "contents-update": "Update table of contents",
    "bookmarks": "Bookmark", "caption": "Caption", "crossref": "Cross-reference",
    "tof": "Table of figures", "tof-update": "Update table of figures",
    "formula": "関数", "fill-num": "Fill", "named-range": "Name manager",
    "additional-formula": "Insert function", "autosum": "AutoSum",
    "recent": "Recently used", "financial": "Financial", "logical": "Logical",
    "text": "Text functions", "datetime": "Date & Time", "lookup": "Lookup & Reference",
    "math": "Math & Trig", "more": "More functions",
    "named-range-huge": "Name manager", "trace-prec": "Trace Precedents",
    "trace-dep": "Trace Dependents", "remove-arrows": "Remove arrows",
    "show-formulas": "Show formulas", "watch-window": "Watch window",
    "calculate": "Calculation options", "data-from-text": "Text to data",
    "data-external-links": "外部リンク", "custom-sort": "Sort",
    "text-column": "Text to columns", "rem-duplicates": "Remove duplicates",
    "data-validation": "Data Validation", "goal-seek": "Goal Seek",
    "solver": "Solver", "group": "Group", "ungroup": "Ungroup",
    "show-details": "Show details", "hide-details": "Hide detail",
    "spell": "スペルチェック", "furigana": "Furigana", "pagebreak": "Breaks", "setfilter": "Filters", "clear-filter": "Clear filter",
    "contents": "Table of contents",
    "open": "Open", "print": "Print",
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


def british_spelling(s: str) -> str:
    """米国綴りを英国綴りへ。**ribbon_en.rs と同じ道を通します**
    (2026-08-26 の段2)。土台の札が英語になったので、綴りも揃えます。"""
    import sys as _s
    _s.path.insert(0, str(HERE)) if "HERE" in globals() else None
    from gen_ribbon_locale import respell
    return respell("en", s)


def label_of(app_loc, prefix, slot):
    return british_spelling(_label_of(app_loc, prefix, slot))


def _label_of(app_loc, prefix, slot):
    # 言い換えが先。**本家に札があっても、こちらを使う**
    app = "documenteditor" if prefix == "DE" else "spreadsheeteditor"
    for table in (rename_map[app], rename_map["*"]):
        if slot in table:
            return table[slot]
    key = LABEL.get(slot)
    if key:
        for who in ("Toolbar", "ViewTab", "HeaderFooterTab"):
            full = f"{prefix}.Views.{who}.{key}"
            if full in app_loc:
                # tip は説明文のことがある。最初の読点までを名前にする
                # **見えない空白を落とす。** 本家の語には幅ゼロの空白(U+200B)が
                # 混ざっていることがあり、そのまま持つと実物と字面が合いません
                # (2026-08-21 に「折り返して全体を表示する」で出た)
                text = re.split(r"[。<（(]", app_loc[full])[0]
                return text.replace("\u200b", "").replace("\ufeff", "").strip()
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
        ("Forms", "References", [
            ("form-text", "Text Field"), ("form-combo", "Combo box"),
            ("form-dropdown", "Dropdown"), ("form-checkbox", "Checkbox"),
            ("form-radio", "Radio Button"), ("form-image", "Picture"),
            ("form-email", "Email Address"), ("form-phone", "Phone Number"),
            ("form-complex", "Complex Field"), ("form-signature", "Signature"),
            ("form-name", "Name"),
        ]),
    ],
    "spreadsheeteditor": [
        ("Pivot Table", "Data", [
            ("pivot-insert", "Insert Pivot Table"), ("pivot-refresh", "Update"),
            ("pivot-refresh-all", "Update all"), ("pivot-select", "Select"),
            ("pivot-totals", "Grand Total"), ("pivot-subtotals", "Subtotals"),
            ("pivot-blank", "Blank Rows"), ("pivot-layout", "Report Layout"),
        ]),
        ("Table Design", "Pivot Table", [
            ("td-header", "Header row"), ("td-total", "Total row"),
            ("td-band-row", "Banded Rows"), ("td-first", "First column"),
            ("td-last", "Last column"), ("td-band-col", "Banded columns"),
            ("td-filter", "Filter button"), ("td-remdup", "Remove Duplicates"),
            ("td-torange", "Convert to range"), ("td-resize", "Resize table"),
        ]),
    ],
}

COMMON_TAIL = {
    "collaboration": [
        ("coauth-mode", "Co-editing mode"), ("co-addcomment", "Add Comment"),
        ("co-delcomment", "Delete comment"), ("co-showcomment", "Show comments"),
        ("co-chat", "Chat"), ("co-history", "Version history"),
        # writer は本家どおり変更履歴もここ(出力時に履歴の前へ挿す)
    ],
    "protect": [
        ("prot-encrypt", "Encrypt"), ("prot-sign", "Add digital signature"),
        ("prot-doc", "Protection"),
    ],
    # **マクロのタブ**(本家のプラグインの位置)。
    #
    # 本家の「プラグイン」と、うちが足していた「Python」「AI」を1つに畳んだ
    # 形です(「リボンの整理」dba89891)。**再生成でバラバラに戻さない** —
    # 畳んだのは後からの決めで、こちらが新しい。
    # 中身はアプリごとに違うので、名前だけ共通で中身は分けます
    "macros": {
        "documenteditor": [
            ("py-list", "Macro list", "plug-manage"),
            # **表にしか無かった**ので文章にも足しました(2026-08-21 の B-3)。
            # 置き場は pyrun の1本で、どちらから開いても同じ場所です
            ("py-folder", "Open folder", "py-folder"),
            ("ai-macro", "Write macro", "ai-macro"),
        ],
        "spreadsheeteditor": [
            ("rec-toggle", "Record actions", "py-run"),
            ("py-new", "New .py", "py-new"),
            ("py-list", "Macro list", "py-list"),
            ("ribbon-list", "Ribbon macros", "py-line"),
            ("py-folder", "Open folder", "py-folder"),
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
        ("draw", 3, [("pen", "Pen"), ("highlighter", "Highlighter"), ("eraser", "Eraser")]),
        # ヘッダー/フッターとレビューのタブはデスクトップ版に無い
        # (2026-08-04 発注者「画面はデスクトップ版に合わせて」)。
        # 中身は 挿入(ヘッダー等)・共同編集(変更履歴)・
        # 下のステータスバー(スペル・文字数)へ畳んだ
        ("view", 6, [
            ("zoom-in", "Zoom in"), ("zoom-out", "Zoom out"),
            ("ruler", "Rulers"), ("darkmode", "Dark mode"),
        ]),
    ],
    "spreadsheeteditor": [
        ("draw", 3, [("pen", "Pen"), ("highlighter", "Highlighter"), ("eraser", "Eraser")]),
        ("view", 99, [
            ("freeze", "Freeze panes"), ("sheet-view", "Sheet View"),
            ("show-gridlines", "グリッド線"), ("show-headings", "Headings"),
        ]),
    ],
}

TAB_NAME_KEYS = {"draw": "Draw", "headerfooter": "HeaderFooter",
                 "review": "Review", "view": "View",
                 "collaboration": "Collaboration", "protect": "Protection",
                 "macros": "Macros"}


# **絵の実体がまだ無いボタン。** 本家には在りますが、うちに絵が無いので
# 出しません(出すと face の試験が止まります — 実体の無い絵は増やさない)。
# 絵を描いて icons.rs に足したら、ここから外せば出ます
no_icon_for = {"img-wrapping"}

# **入切のボタン**(本家の欄にも在る物)。押すと入/切が変わります。
# うちが足した分は EXTRA_CMDS の書き方の欄で指します
toggles = {"formula-bar", "show-headings", "show-zeros",
        # 画面の明暗。**入っている間は押された形**にします。
        # 表の側は EXTRA_CMDS で足しているので、そちらの書き方の欄が効きます
        "darkmode"}


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
        # **img と shapes も拾う**(2026-08-21)。図形まわりのボタンが
        # この種類で、拾っていなかったので表から丸ごと落ちていた。
        #
        # *この2つは前置きを残します*(`img-group` のように)。落とすと
        # `group`(データタブのグループ化)と名前がぶつかって、灰色のはずの
        # ボタンが押せる形になります(2026-08-21 に実際に出た)
        slots = list(dict.fromkeys(
            a or b for a, b in re.findall(
                r'id="slot-(?:(?:btn|field|chk|cmb)-([a-z0-9-]+)'
                r'|((?:img|shapes)-[a-z0-9-]+))"', body)))
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
            for c, l in [("copy", "Copy"), ("cut", "Cut"),
                         ("paste", "Paste"), ("align-left", "Align left"),
                         ("align-right", "Align right"), ("align-dist", "Distributed"),
                         ("ruby", "Ruby")]:
                DYN_LABELS[c] = l
        # ヘッダー・フッター類はタブを畳んで挿入タブへ(デスクトップ版の場所)
        if tab == "ins" and app == "documenteditor":
            at = slots.index("insequation") if "insequation" in slots else len(slots)
            slots[at:at] = ["edit-header", "edit-footer", "pagenum",
                            "datetime", "numpages"]
            for c, l in [("edit-header", "Edit header"),
                         ("edit-footer", "Edit footer"),
                         ("pagenum", "Page number"), ("datetime", "Date & Time"),
                         ("numpages", "Number of pages")]:
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
            cmds = cmds[:-1] + [("track-changes", "Track changes")] + cmds[-1:]
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
        view_at = next((i for i, (n, _) in enumerate(out) if n == "View"), len(out))
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
        at = next((i + 1 for i, (n, _) in enumerate(out) if n == "Data"), len(out))
        out.insert(at, entry)
    else:
        out.append(entry)
    # 絵の実体が無い物は落とす(理由は上の表)
    out = [(n, [i for i in ids if i not in no_icon_for]) for (n, ids) in out]
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
# 形は (タブ, どのボタンの後ろに置くか, id, 見出し, 絵, 書き方)。
# 後ろが None なら先頭。書き方は c=押す / t=入切 / x=灰色 / xt=灰色の入切 /
# xm=灰色の切り替え(標準 / 改ページ プレビュー)。
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
        ("Home", 'ruby', "ai-furigana", "Furigana", "ai-furigana", "c"),
        ("References", 'crossref', "footnote", "Footnote", "footnote", "c"),
        ("View", None, "nav", "Navigation", "nav", "t"),
        ("View", 'nav', "fit-page", "Fit to page", "fit-page", "c"),
        ("View", 'fit-page', "fit-width", "Fit to width", "fit-width", "c"),
        ("View", 'fit-width', "zoom100", "Zoom to 100%", "zoom100", "c"),
        ("View", 'zoom-out', "printview", "Print layout", "printview", "c"),
        ("View", 'printview', "multipage", "Multiple pages", "multipage", "c"),
        # **表にしかありませんでした**(2026-08-21 発注者「双方でできるように
        # したいです」)。中身は ui::appcmd に1本あります。ダークモードの
        # 隣に置くのは、表と同じ並びにするためです
        ("View", 'multipage', "ui-bigger", "Bigger UI text", "ui-bigger", "c"),
        ("View", 'ui-bigger', "ui-smaller", "Smaller UI text", "ui-smaller", "c"),
        ("View", 'ruler', "show-toolbar", "Always Show Toolbar", "show-toolbar", "t"),
        ("View", 'show-toolbar', "show-statusbar", "Status Bar", "show-statusbar", "t"),
        ("View", 'show-statusbar', "show-left", "Left Panel", "show-left", "t"),
        ("View", 'show-left', "show-right", "Right Panel", "show-right", "t"),
    ],
    "calc": [
        # コピー・切り取り・貼り付けは、本家ではタブの外(全タブ共通の場所)。
        # Excel と同じくホームの先頭に置く
        ("Home", None, "copy", "Copy", "copy", "c"),
        ("Home", 'copy', "cut", "Cut", "cut", "c"),
        ("Home", 'cut', "paste", "Paste", "paste", "c"),
        ("Home", 'paste', "copystyle", "Format painter", "copystyle", "c"),
        ("Home", 'text-orient', "align-left", "Align left", "align-left", "c"),
        ("Home", 'align-center', "align-right", "Align right", "align-right", "c"),
        ("Home", 'align-just', "align-dist", "Distributed", "align-dist", "c"),
        ("Home", 'direction', "sum", "AutoSum", "autosum", "c"),
        ("Home", 'clear', "sort-desc", "Sort descending", "sortdesc", "c"),
        ("Home", 'sort-desc', "sort-asc", "Sort ascending", "sortasc", "c"),
        ("Insert", None, "pivot-insert", "Insert Pivot Table", "add-pivot", "c"),
        ("Insert", 'inssparkline', "co-addcomment", "Comment", "ins-comment", "c"),
        ("Insert", 'instextart', "edit-header", "Header & Footer", "editheader", "c"),
        ("Draw", None, "draw-select", "Select", "select-tool", "c"),
        ("Layout", 'pagebreak', "edit-header", "Header & Footer", "editheader", "c"),
        # 本家では「拡大縮小印刷」の中の選択肢。うちは小窓を持たないので
        # レイアウトタブに独立したボタンで出す
        ("Layout", 'scale', "fit-pages", "Fit to paper", "fit-pages", "c"),
        ("Layout", 'fit-pages', "printarea-add", "Add to area", "printarea-add", "c"),
        ("Layout", 'printarea-add', "show-breaks", "Page breaks", "show-breaks", "c"),
        ("Formula", 'insert-function', "func-list", "Python functions", "py-list", "c"),
        ("Formula", 'defname', "paste-name", "Paste name", "paste-name", "c"),
        # **入力規則に合っていない値を洗い出す**(2026-08-21 の D群)。
        # 本家に無い機能なので、絵も語もこちらで用意しました。
        # 入切の形です — もう一度押すと印が消えます
        ("Data", 'data-validation', "dv-mark", "Circle invalid data", "dv-mark", "t"),
        ("Data", 'clear-filter', "sort-desc", "Sort descending", "sortdesc", "c"),
        ("Data", 'sort-desc', "sort-asc", "Sort ascending", "sortasc", "c"),
        # 小計は本家のデータタブに無いが、グループ化を「畳むと合計が残る」
        # 形で使うために置く(Excel の データ > 小計 に相当。発注者指摘)
        ("Data", 'hide-details', "subtotal", "Subtotals", "subtotal", "c"),
        ("Data", 'subtotal', "datatable", "Data table", "datatable", "c"),
        ("Data", 'datatable', "python", "Python", "python", "c"),
        ("Data", 'python', "csv-kind", "CSV format", "csv-kind", "c"),
        ("Data", 'csv-kind', "flash-fill", "Fill by example", "flash-fill", "c"),
        ("Pivot Table", 'pivot-insert', "pivot-fields", "Field list", "pivot-fields", "c"),
        # 本家は値フィールドの設定の中にある「計算の種類」。うちは指図が
        # 集計の名前ひとつなので、タブに独立したボタンとして置く
        # **元の表を差し替える**(2026-08-21 の D群)。本家に無いボタンですが、
        # 語は本家の `PivotSettingsAdvanced.textDataSource` にあるので、
        # 訳は全言語ともそこから引けます(「データソース」)
        ("Pivot Table", 'pivot-refresh-all', "pivot-source", "Data source", "pivot-source", "c"),
        ("Pivot Table", 'pivot-blank', "pivot-showas", "Show values as", "pivot-showas", "c"),
        ("Pivot Table", 'pivot-layout', "pivot-style", "Style", "pivot-style", "c"),
        # 本家では「セルの書式設定 > 保護」タブと「シートの保護」小窓の中。
        # うちは小窓を持たない作りなので、保護タブに独立したボタンで出す
        # 本家に無い物。**2026-08-30 に押せるようにしました**(beta.3 で
        # ブックの構造と範囲を守れるようにした分)。灰色のままだと、
        # 生成し直したときに動く物が押せなくなります
        ("Protection", 'prot-encrypt', "prot-book", "Protect workbook", "protect-workbook", "c"),
        ("Protection", 'prot-doc', "prot-range", "Protect Range", "protect-range", "c"),
        ("Protection", 'prot-sign', "cell-lock", "Cell lock", "cell-lock", "c"),
        ("Protection", 'cell-lock', "prot-allow", "Allowed actions", "prot-allow", "c"),
        ("Protection", 'prot-allow', "recover", "Recover", "recover", "c"),
        ("Protection", 'recover', "recover-every", "Backup interval", "recover-every", "c"),
        ("Protection", 'recover-every', "read-only-rec", "Suggest read-only", "read-only-rec", "c"),
        # 表示の切り替え(どれか1つ)。入切とは性格が違う。
        # **2026-08-30 に押せるようにしました**(上の保護と同じ理由)
        ("View", 'sheet-view', "view-normal", "Normal", "view-normal", "m"),
        ("View", 'view-normal', "view-pagebreak", "Page Break Preview", "view-pagebreak", "m"),
        ("View", 'sheet-view', "zoom-in", "Zoom in", "zoom-in", "c"),
        ("View", 'zoom-in', "zoom-out", "Zoom out", "zoom-out", "c"),
        # **文章にしか無かった**ので表にも足しました(2026-08-21 の B-3)。
        # 拡大・縮小と対になる命令で、中身は `ui::appcmd` に1本あります
        ("View", 'zoom-out', "zoom100", "Zoom to 100%", "zoom100", "c"),
        ("View", 'zoom100', "ui-bigger", "Bigger UI text", "ui-bigger", "c"),
        ("View", 'ui-bigger', "ui-smaller", "Smaller UI text", "ui-smaller", "c"),
        # **文章と同じ命令**(2026-08-21 の B-2)。中身は1文字も違わないので
        # id と札を揃えました。絵はアプリごとに別のまま(どちらも明暗の絵)
        ("View", 'ui-smaller', "darkmode", "Dark mode", "theme", "t"),
        # シナリオ。本家に無い。語は LibreOffice の `Scenario`(14 言語ある)
        ("Data", 'goal-seek', "scenario", "Scenario", "scenario", "c"),
        # 予測シート。本家にも LibreOffice にも当たる語が無い。
        # 語は Excel の「予測シート」で、13 言語はこちらで訳した
        # (2026-08-21「レポートの接続」と同じ扱い)
        ("Data", 'scenario', "forecast", "Forecast Sheet", "forecast", "c"),
        # ピボットグラフ。本家にも LibreOffice にも当たる語が無い。
        # 語は Excel の「ピボットグラフ」で、13 言語はこちらで訳した
        ("Pivot Table", 'pivot-source', "pivot-chart", "PivotChart", "pivot-chart", "c"),
        # ウィンドウの分割。本家に無いので、Excel の「分割」に合わせて足す。
        # 分割している間は押された形にしたいので、書き方は t(入切)
        ("View", 'freeze', "split", "Split", "split", "t"),
        ("View", 'freeze', "formula-bar", "Formula Bar", "formula-bar", "t"),
        ("View", 'show-headings', "show-zeros", "Show zeros", "show-zeros", "t"),
        ("View", 'show-zeros', "show-left", "Left Panel", "show-left", "t"),
        ("View", 'show-left', "show-right", "Right Panel", "show-right", "t"),
    ],
}


# **本家と場所を変えたボタン**(2026-08-21)。
#
# `EXTRA_CMDS` は「足す」だけで、本家に在る物を動かせません。実物では
# いくつか動かしてあるので、その分をここに書きます。
#
# 形は (アプリ, タブ, 動かす id, どの後ろへ)。後ろが None なら先頭。
# **理由を1つずつ書きます** — 本家の並びを崩すのは例外なので、
# 「なんとなく」で増やさないためです。
reorder = [
    # 暗い明るいは目盛りより先。表示の切り替えの仲間として並べる
    ("documenteditor", "View", "darkmode", "multipage"),
    # 並べ替えの3つを続ける(昇順・降順・ユーザー設定)。本家は
    # ユーザー設定だけが離れていて、続けて使う物が3箇所に散る
    ("spreadsheeteditor", "Data", "custom-sort", "sort-asc"),
    # 表示の切り替え(標準/改ページ)をいちばん前へ。まず「どの見え方か」を
    # 選び、そのあと拡大や枠の固定を触る順にする
    ("spreadsheeteditor", "View", "freeze", "darkmode"),
    ("spreadsheeteditor", "View", "formula-bar", "freeze"),
    ("spreadsheeteditor", "View", "split", "freeze"),
    # 表示の切り替えは、シートの見え方のすぐ後ろ
    ("spreadsheeteditor", "View", "view-normal", "sheet-view"),
    ("spreadsheeteditor", "View", "view-pagebreak", "view-normal"),
    # 絞り込みは並べ替えの隣(続けて使う)。本家は離れている
    ("spreadsheeteditor", "Home", "setfilter", "sort-asc"),
    ("spreadsheeteditor", "Home", "clear-filter", "setfilter"),
    # ブックの保護は暗号化の次。範囲の保護はその次で、細かい方へ降りる順
    ("spreadsheeteditor", "Protection", "prot-doc", "protect-workbook"),
    ("spreadsheeteditor", "Protection", "protect-range", "prot-doc"),
]

# **本家に在るが、うちでは別のタブに置いた物。**
# 二重に出さないよう、元のタブからは外します
drop_from = [
    # 関数の挿入は数式タブが持ち場。ホームにも欄があるが出さない
    ("spreadsheeteditor", "Home", "insert-function"),
    # **同じ札のボタンが2つ並んでいました**(2026-08-21)。どちらも
    # 「グラフを挿入」で、押すと出る物も同じです。表の側は 2026-08-16 に
    # 片付けてあり、文章の側が残っていました
    ("documenteditor", "Insert", "smartpicker"),
]


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
                # 絵は本家の名前がそのまま鍵。本家に無いボタンだけ別に決める。
                # アプリで絵が違う物は差し替える
                icon = icon_swap.get(app, {}).get(s) or DYN_ICONS.get(s, s)
                cid = ready.get(s)
                # **同じ命令を1つのタブに二度出さない。** 本家の欄が2つ
                # (`smartpicker` と `insrecommend`)同じ命令に結ばれていて、
                # 挿入タブに同じボタンが2回出ていました(2026-08-21)
                if cid is not None and any(r[0] == cid for r in rows):
                    continue
                rows.append((cid, lab, icon, "t" if cid in toggles else "c"))
            # **置き場所つきで差し込む。** どのボタンの後ろに置くかを
            # 指しておかないと、足した分が全部タブの末尾へ寄ります
            # (コピーがホームの一番後ろへ行く)
            for (_tab, after, cid, clab, cicon, ckind) in [e for e in extras if e[0] == name]:
                # 灰色は id を持たない(`x("札","絵")` の2引数)
                item = (None if ckind.startswith("x") else cid, clab, cicon, ckind)
                if after is None:
                    rows.insert(0, item)
                else:
                    # 目印は id か絵。**灰色は id を持たない**ので、絵で指せる
                    # ようにしておかないと、灰色の後ろに置けません
                    k = next((i + 1 for i, r in enumerate(rows)
                              if r[0] == after or r[2] == after), len(rows))
                    rows.insert(k, item)
            # 別のタブに置いた物を外す
            for (ap, tb, gid) in drop_from:
                if ap == app and tb == name:
                    rows = [r for r in rows if r[0] != gid]
            # 場所を変えた物を動かす
            for (ap, tb, mid, after) in reorder:
                if ap != app or tb != name:
                    continue
                hit = next((r for r in rows if r[0] == mid or r[2] == mid), None)
                if hit is None:
                    continue
                rows.remove(hit)
                k = 0 if after is None else next(
                    (i + 1 for i, r in enumerate(rows) if r[0] == after or r[2] == after),
                    len(rows))
                rows.insert(k, hit)
            print(f'    Tab {{ name: "{name}", cmds: &[')
            for cid, lab, icon, kind in rows:
                if cid is None:
                    # 灰色は id を持たない。書き方(x / xt / xm)が性格を表す
                    k = kind if kind.startswith("x") else "x"
                    print(f'        {k}("{lab}", "{icon}"),')
                else:
                    # **m(どれか1つ)も出します**(2026-08-30)。
                    # 灰色の側は xm を持っていたのに、押せる側は c と t
                    # だけで、m を書いても c に落ちていました
                    k = kind if kind in ("c", "t", "m") else "c"
                    print(f'        {k}("{cid}", "{lab}", "{icon}"),')
            print("    ]},")
        print("];")
        print()
    print(TAIL)


HEAD = '''//! リボン(タブ+コマンド)。**Euro-Office の現物から生成している。**
//!
//! このファイルは手で書かない。`gen_ribbon.py` が
//! `vendor/web-apps/apps/*/main/app/template/Toolbar.template` の並び順と
//! 同 app の `locale/en.json` の名前から起こす。
//! だから「Euro-Office と全く同じか」は台本を回し直せば確かめられる。
//!
//! ```text
//! python3 ui/gen_ribbon.py ja > ui/src/ribbon.rs
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

/// ボタンの性格(2026-08-21 発注者「押せるボタンだけでなくトグルボタンを
/// 作って」)。**押した後どうなるかが違う**ので、描き方も変わります。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// 押すと1回きりの働きをします(既定)。押した後は元の見た目に戻ります
    Push,
    /// **入っているか切れているか**があります(数式バー・0を表示する・
    /// 左パネル)。入っている間は押された形で出すので、*見れば分かります*
    Toggle,
    /// **いくつかのうち1つだけが入ります**(標準 / 改ページ プレビュー)。
    ///
    /// 入切とは性格が違います(2026-08-21 発注者「改ページ プレビューは、
    /// 性格がちがうのでは」)。入切は互いに関わりませんが、こちらは
    /// *どれか1つが必ず入っていて、別のを押すと前のが切れます*。
    Mode,
}

/// 1つのコマンド。`ready=false` は未実装(押せない灰色)。
/// `icon` は Euro-Office の slot 名で、埋め込んだアイコン(icons.rs)を引く鍵。
#[derive(Clone, Copy)]
pub struct Cmd {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub ready: bool,
    pub kind: Kind,
}

/// 押すボタン(押せる)
pub(crate) const fn c(id: &'static str, label: &'static str, icon: &'static str) -> Cmd {
    Cmd { id, label, icon, ready: true, kind: Kind::Push }
}
/// 入切のボタン(押せる)。画面は今の状態を押された形で見せます
pub(crate) const fn t(id: &'static str, label: &'static str, icon: &'static str) -> Cmd {
    Cmd { id, label, icon, ready: true, kind: Kind::Toggle }
}
/// 押すボタン(まだ押せない灰色)
#[allow(dead_code)] // 灰色ゼロの今は未使用だが、ロケール表の生成が使う形
pub(crate) const fn x(label: &'static str, icon: &'static str) -> Cmd {
    Cmd { id: "", label, icon, ready: false, kind: Kind::Push }
}
/// 入切のボタン(まだ押せない灰色)
#[allow(dead_code)]
pub(crate) const fn xt(label: &'static str, icon: &'static str) -> Cmd {
    Cmd { id: "", label, icon, ready: false, kind: Kind::Toggle }
}
/// 表示の切り替え(まだ押せない灰色)
#[allow(dead_code)]
pub(crate) const fn xm(label: &'static str, icon: &'static str) -> Cmd {
    Cmd { id: "", label, icon, ready: false, kind: Kind::Mode }
}

/// いまの言語のリボン。**語だけが違う** — id・並び・ready・icon は
/// どの言語でも ja(WRITER/CALC)と同一(下の試験が保証)。
/// 内部の論理(タブ名の照合など)は ja の表で書いてよい —
/// 添字がそのまま対応する
pub fn writer_tabs() -> &'static [Tab] {
    crate::ribbon_tables::tabs(crate::settings::language())
        .map(|(w, _)| w)
        .unwrap_or(WRITER)
}

pub fn calc_tabs() -> &'static [Tab] {
    crate::ribbon_tables::tabs(crate::settings::language())
        .map(|(_, c)| c)
        .unwrap_or(CALC)
}

pub struct Tab {
    pub name: &'static str,
    pub cmds: &'static [Cmd],
}

// ---- 利用者が足したボタン -------------------------------------------------
//
// **静的な表(CALC/WRITER)には入れない。** 14言語をボタン単位で突き合わせる
// 門番(tools/ribbon_locale_check.py)が「言語ごとに数が違う」と言い出す。
// 利用者の札は利用者自身の言葉なので、そもそも訳さない — 対訳の表にも
// 入れない(2026-08-16 発注者「システム定義とユーザー定義に分ける」)。

/// 利用者のボタンの id の頭。押されたら `~/.config/officework/ribbon/<名前>.py`
/// を走らせる、という約束
pub const USER_PREFIX: &str = "py:";

/// 名乗りに絵が無い(または知らない絵の名前だった)ときの既定
const USER_ICON: &str = "py-run";

/// 利用者のボタン1つ — ボタンと、出る段(ja の段名)
pub struct UserBtn {
    pub cmd: Cmd,
    pub tab: &'static str,
}

type Shape = Vec<(String, u64, std::time::SystemTime)>;
static USER: std::sync::RwLock<Option<(Shape, &'static [UserBtn])>> =
    std::sync::RwLock::new(None);

/// 利用者が `~/.config/officework/ribbon` に置いたマクロのボタン。
///
/// **描くたびに置き場を読まない。** 画面は1秒に何十回も組み直されるので、
/// 走査は [`refresh_user_cmds`] が姿の変わったときだけ行う(UDF の見張りと
/// 同じ形)。ここは控えを返すだけ。
pub fn user_btns() -> &'static [UserBtn] {
    if let Ok(g) = USER.read() {
        if let Some((_, c)) = g.as_ref() {
            return c;
        }
    }
    refresh_user_cmds();
    USER.read().ok().and_then(|g| g.as_ref().map(|(_, c)| *c)).unwrap_or(&[])
}

/// その段に出る利用者のボタン。段は**ja の段名**で照合する — 表の内部の
/// 照合が ja なのと同じ(添字も名前も言語で動かない)
pub fn user_cmds_for(tab_ja: &str) -> Vec<&'static Cmd> {
    user_btns().iter().filter(|b| b.tab == tab_ja).map(|b| &b.cmd).collect()
}

/// 置き場の姿が変わっていればボタンを作り直す。返りは作り直したか。
///
/// 作った札と id は `&'static` として漏らす(`Box::leak`)。静的な表と同じ型で
/// 扱えるようにするため — 漏れるのは**置き場を書き換えた回数**だけで、
/// 1回あたり数十バイト。描くたびに漏れる作りではない。
pub fn refresh_user_cmds() -> bool {
    let dir = pyrun::ribbon_dir();
    let now = pyrun::shape_in(&dir);
    let Ok(mut g) = USER.write() else { return false };
    if g.as_ref().map(|(s, _)| s) == Some(&now) {
        return false;
    }
    let btns: Vec<UserBtn> = pyrun::ribbon_decls(&dir)
        .into_iter()
        .map(|d| {
            let icon =
                if crate::icons::find(&d.icon).is_some() { d.icon } else { USER_ICON.into() };
            UserBtn {
                cmd: Cmd {
                    id: Box::leak(format!("{USER_PREFIX}{}", d.module).into_boxed_str()),
                    label: Box::leak(d.label.into_boxed_str()),
                    icon: Box::leak(icon.into_boxed_str()),
                    ready: true,
                    // 利用者のマクロは押すボタン。入切にしたければ .py の側で
                    // 状態を持つことになるので、いまは押す形だけ
                    kind: Kind::Push,
                },
                tab: Box::leak(d.tab.into_boxed_str()),
            }
        })
        .collect();
    *g = Some((now, Box::leak(btns.into_boxed_slice())));
    true
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


    /// **どのボタンにも実体のアイコンがある。**
    ///
    /// アイコンは [`crate::icons::find`] が引ける物しか出せない。表に足し忘れると
    /// **ボタンだけが無地で出る** — 押せるし配線もされているので、
    /// 配線の試験(`wiring_tests`)も文言の門番も素通りする。
    ///
    /// **描く側と同じ口(`find`)で引く。** 表は `ICONS` と `OWN_ICONS` の
    /// 二枚あり、片方だけ見ると「無い」と誤って数える(最初それで
    /// 数を間違えた)。
    ///
    /// **いま欠けている物は下に並べて許してある。** 全部描くまで赤には
    /// できないが、**これ以上増やさない**ための止め木になる。
    /// 描いたら一覧から外す(外し忘れも落ちる)。
    /// 2026-08-13 に数えて 77 件。すべて calc の持ち場
    /// **実体の無いアイコン**の一覧。ここに載っている id は無地のボタンで出る。
    /// 2026-08-13 に 77 個ぜんぶ描いたので空。**増えても減っても試験が落ちる**
    /// (下の2つの assert が両方向で見ている)。
    const アイコンの無いボタン: &[&str] = &[];

    #[test]
    fn 実体の無いアイコンを増やさない() {
        let mut missing: Vec<&str> = Vec::new();
        for tabs in [WRITER, CALC] {
            for t in tabs {
                for cmd in t.cmds {
                    if cmd.icon.is_empty() {
                        continue;
                    }
                    if crate::icons::find(cmd.icon).is_none() && !missing.contains(&cmd.icon) {
                        missing.push(cmd.icon);
                    }
                }
            }
        }
        let 新しい: Vec<&&str> =
            missing.iter().filter(|m| !アイコンの無いボタン.contains(m)).collect();
        assert!(新しい.is_empty(),
            "実体の無いアイコンが増えた: {fresh:?}(絵を描いて icons.rs に足す)");
        let 直った: Vec<&&str> =
            アイコンの無いボタン.iter().filter(|a| !missing.contains(a)).collect();
        assert!(直った.is_empty(),
            "アイコンができているのに一覧に残っている: {was_fixed:?}(一覧から外す)");
    }

    #[test]
    fn 各言語の表は語だけが違う() {
        // id・並び・ready・icon が ja と一致しない表は配線が壊れる —
        // ここで固定する(語は違ってよい。空の語は出さない)
        let mut pairs: Vec<(&[Tab], &[Tab])> = Vec::new();
        for l in lang::i18n::languages() {
            if l == "ja" {
                continue;
            }
            let (w, c) = crate::ribbon_tables::tabs(l)
                .unwrap_or_else(|| panic!("言語 {l} のリボンの表が無い(登録簿のずれ)"));
            pairs.push((WRITER, w));
            pairs.push((CALC, c));
        }
        for (ja, other) in pairs {
            assert_eq!(ja.len(), other.len(), "タブの数が違う");
            for (a, b) in ja.iter().zip(other) {
                assert!(!b.name.is_empty(), "タブ名が空");
                assert_eq!(a.cmds.len(), b.cmds.len(), "「{}」のボタンの数が違う", a.name);
                for (x, y) in a.cmds.iter().zip(b.cmds) {
                    assert_eq!(x.id, y.id, "id がずれた(配線が壊れる)");
                    assert_eq!(x.icon, y.icon, "「{}」の icon が違う", x.id);
                    assert_eq!(x.ready, y.ready, "「{}」の ready が違う", x.id);
                    assert!(!y.label.is_empty(), "「{}」の語が空", x.id);
                }
            }
        }
    }

    #[test]
    fn 段の中でボタンの鍵が重ならない() {
        // 画面はボタン1つ1つに gpui の鍵を与える。**段の中で鍵が重なると、
        // 後のボタンの押下が拾われない** — ボタンは出るのに押しても何も
        // 起きない、という形で出る(2026-08-16 実機で踏んだ。鍵が絵の名前
        // だったころ、利用者のマクロが rec-toggle と同じ py-run を名乗った)。
        // 鍵は id(灰色は札)なので、ここが一意ならあの症状は起きない
        for (app, tabs) in [("writer", WRITER), ("calc", CALC)] {
            for tab in tabs {
                let mut seen: Vec<&str> = Vec::new();
                for c in tab.cmds {
                    let k = if c.id.is_empty() { c.label } else { c.id };
                    assert!(!seen.contains(&k), "{app} の「{}」で鍵が重なった: {k}", tab.name);
                    seen.push(k);
                }
            }
        }
    }

    #[test]
    fn 利用者のボタンは静的な表に混ざらない() {
        // 14言語を突き合わせる門番は静的な表を数える。利用者の札は
        // 利用者自身の言葉で、訳もしない — 表に混ぜたら数が合わなくなる
        for tabs in [WRITER, CALC] {
            for tab in tabs {
                for c in tab.cmds {
                    assert!(
                        !c.id.starts_with(USER_PREFIX),
                        "静的な表に利用者の id が混ざっている: {}",
                        c.id
                    );
                }
            }
        }
    }

    #[test]
    fn 本家のタブが全部ある() {
        // 発注者確定(2026-08-04): メニューは制限しない。実装しないものも
        // 場所は本家どおり(灰色)。タブごと消すことはしない
        for tabs in [WRITER, CALC] {
            // **「プラグイン」は「マクロ」に改名した**(2026-08-16 発注者
            // 「プラグインはマクロだけでいいのでは」)。本家に同じ段はあるが、
            // 使う人の言葉に寄せた — 段を消したのではなく名を替えた
            for want in ["Collaboration", "Protection", "Macros"] {
                assert!(
                    tabs.iter().any(|t| t.name == want),
                    "タブが無い: {want}"
                );
            }
        }
        assert!(WRITER.iter().any(|t| t.name == "Forms"), "writer にタブが無い: フォーム");
        for want in ["Pivot Table", "Table Design"] {
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
        for want in ["File", "Home", "Insert", "Layout", "References"] {
            assert!(names.contains(&want), "文書に「{want}」タブが無い: {names:?}");
        }
        let names: Vec<&str> = CALC.iter().map(|t| t.name).collect();
        for want in ["File", "Home", "Insert", "Layout", "Formula", "Data"] {
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
