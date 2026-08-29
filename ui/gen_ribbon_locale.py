#!/usr/bin/env python3
"""ribbon.rs(正)から、別ロケールのリボン表を起こす。

gen_ribbon.py(テンプレートから ja の表を起こす)と役割が違う:
こちらは **いまの face/src/ribbon.rs を構造の正** とし、語だけを
Euro-Office のロケール(vendor/web-apps の ja.json → <locale>.json の対訳)で
置き換える。手で足したボタン(AI タブなど本家に無いもの)は OVERRIDES 表で
訳す。**訳が見つからない語があれば止まる**(黙って日本語のまま出さない)。

    python3 ui/gen_ribbon_locale.py en > face/src/ribbon_en.rs

id・並び・ready・icon は ja と同一になる(試験 ribbon.rs 側で保証)。
"""
import json
import re
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from ribbon_parse import tables_or_die  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent / "vendor/web-apps/apps"
# **リボンの表は face(gpui を持たない層)へ移った**(2026-08-15)。
# ここを直し忘れると、13言語の生成が丸ごと止まる
RIBBON = Path(__file__).resolve().parent.parent / "face/src/ribbon.rs"

# 本家に無い・こちらで足した語の対訳。ここに無い未解決語が出たら
# このスクリプトは止まる — その語をここに足してから出し直す
# 英語の札 → **本家(vendor)の日本語の札**。
#
# 土台 `face/src/ribbon.rs` の札は英語です(2026-08-26 の段2)。本家を
# 引くときだけ、ここで日本語に直してから引きます。**英語で直に引くと
# 別の項目の訳を拾います** — 本家の en は同じ字を何度も使っていて
# (Italic・Group・Update など)、日本語ほど一意ではありません。
# 英語で引いてみたら、日本語の札が 22 か所変わりました
# (斜体 → イタリック、上付き → 上付き文字、グループ化 → グループ)。
#
# この表は段2で1度だけ起こしました。ボタンを足すときは、その札の
# 本家の日本語をここにも足してください(足さないと本家を引けず、
# `OVERRIDES` で全言語ぶん書くことになります)。
# **絵 → 本家(vendor)の日本語の札。**
#
# 土台 `face/src/ribbon.rs` の札は英語です(2026-08-26 の段2)。本家を
# 引くときだけ、ここで日本語に直してから引きます。**英語で直に引くと
# 別の項目の訳を拾います** — 本家の en は同じ字を何度も使っていて
# (Italic・Group・Update など)、日本語ほど一意ではありません。
# 英語で引いてみたら、日本語の札が 22 か所変わりました
# (斜体 → イタリック、上付き → 上付き文字、グループ化 → グループ)。
#
# **鍵は札ではなく絵です。** 同じ英語の札が2つあることがあります
# (描画の Select と ピボットの Select)。札で引くと、片方の訳を
# もう片方にも当ててしまいます。
#
# この表は段2で1度だけ起こしました。ボタンを足すときは、その絵の
# 本家の日本語をここにも足してください。
# **id(無ければ絵)→ 本家(vendor)の日本語の札。**
#
# 土台 `face/src/ribbon.rs` の札は英語です(2026-08-26 の段2)。本家を
# 引くときだけ、ここで日本語に直してから引きます。**英語で直に引くと
# 別の項目の訳を拾います** — 本家の en は同じ字を何度も使っていて
# (Italic・Group・Update など)、日本語ほど一意ではありません。
#
# **鍵は札でも絵でもなく id です。** 札は同じ字が2つあり(描画の
# Select とピボットの Select)、絵も共用があります(parastyle と
# cell-styles がどちらも styles)。id を持たない釦(x/xt/xm)だけ
# 絵で引きます。
#
# この表は段2で1度だけ起こしました。釦を足すときは、その id の
# 本家の日本語をここにも足してください。
# **`id|絵` → 本家(vendor)の日本語の札。**
#
# 土台 `face/src/ribbon.rs` の札は英語です(2026-08-26 の段2)。本家を
# 引くときだけ、ここで日本語に直してから引きます。**英語で直に引くと
# 別の項目の訳を拾います** — 本家の en は同じ字を何度も使っていて
# (Italic・Group・Update など)、日本語ほど一意ではありません。
#
# **鍵は id と絵の組です。** どれも単独では一意になりません —
# 札は 描画の Select とピボットの Select、絵は parastyle と
# cell-styles がどちらも styles、id は 重複の削除 が2つの絵で
# 出てきます。id を持たない釦(x/xt/xm)は id を空にします。
#
# この表は段2で1度だけ起こしました。釦を足すときは、その組の
# 本家の日本語をここにも足してください。
VENDOR_JA = {
    "add-text|add-text": "テキストの追加",
    "ai-furigana|ai-furigana": "ふりがな",
    "ai-macro|ai-macro": "マクロを書く",
    "align-center|align-center": "中央揃え",
    "align-dist|align-dist": "均等割付",
    "align-just|align-just": "両端揃え",
    "align-left|align-left": "左揃え",
    "align-right|align-right": "右揃え",
    "blankpage|blankpage": "空白ページの挿入",
    "bold|bold": "太字",
    "bookmarks|bookmarks": "ブックマーク",
    "borders|borders": "罫線",
    "bottom|bottom": "下揃え",
    "calc-mode|calculate": "計算方法",
    "caption|caption": "図表番号",
    "cell-del|cell-del": "セルを削除",
    "cell-format|cell-format": "セルの書式設定",
    "cell-ins|cell-ins": "セルを挿入",
    "cell-lock|cell-lock": "セルのロック",
    "cell-styles|styles": "セルのスタイル",
    "changecase|changecase": "大文字小文字を変更",
    "clear-filter|clear-filter": "フィルターを解除",
    "clearstyle|clearstyle": "スタイルのクリア",
    "clear|clear": "消去",
    "co-addcomment|co-addcomment": "コメントを追加",
    "co-addcomment|ins-comment": "コメント",
    "co-chat|co-chat": "チャット",
    "co-delcomment|co-delcomment": "コメントを削除",
    "co-history|co-history": "バージョン履歴",
    "co-showcomment|co-showcomment": "コメントの表示",
    "coauth-mode|coauth-mode": "共同編集モード",
    "colorschemas|colorschemas": "配色の変更",
    "columns|columns": "列の挿入",
    "comma|comma": "カンマスタイル",
    "condformat|condformat": "条件付き書式",
    "controls|controls": "コンテンツコントロールの挿入",
    "copystyle|copystyle": "書式のコピー",
    "copy|copy": "コピー",
    "crossref|crossref": "相互参照",
    "csv-kind|csv-kind": "CSV の形",
    "currency|currency": "通貨スタイル",
    "custom-sort|custom-sort": "並べ替え",
    "cut|cut": "切り取り",
    "darkmode|darkmode": "ダークモード",
    "darkmode|theme": "ダークモード",
    "data-external-links|data-external-links": "外部リンク(値で取り込む)",
    "data-from-text|data-from-text": "テキストからデータ",
    "data-validation|data-validation": "データの入力規則",
    "datatable|datatable": "データテーブル",
    "datetime|datetime": "日付/時刻",
    "decfont|decfont": "フォントサイズの縮小",
    "decoffset|decoffset": "インデントを減らす",
    "defname|named-range": "名前の管理",
    "defname|named-range-huge": "名前の管理",
    "digit-dec|digit-dec": "小数点以下の表示桁数を減らす",
    "digit-inc|digit-inc": "小数点以下の表示桁数を増やす",
    "direction|direction": "文字の向き(右横書き)",
    "draw-select|select-tool": "選択",
    "dropcap|dropcap": "ドロップキャップの挿入",
    "dv-mark|dv-mark": "無効データのマーク",
    "edit-footer|edit-footer": "フッターの編集",
    "edit-header|edit-header": "ヘッダーの編集",
    "edit-header|editheader": "ヘッダー/フッター",
    "eraser|eraser": "消しゴム",
    "fill-num|fill-num": "フィル",
    "fillparag|fillparag": "塗りつぶしの色",
    "fit-pages|fit-pages": "紙に収める",
    "fit-page|fit-page": "ページに合わせる",
    "fit-width|fit-width": "幅に合わせる",
    "flash-fill|flash-fill": "フラッシュフィル",
    "fn-datetime|datetime": "日付/時刻",
    "fn-financial|financial": "財務",
    "fn-logical|logical": "論理",
    "fn-lookup|lookup": "検索/行列",
    "fn-math|math": "数学/三角",
    "fn-more|more": "その他の関数",
    "fn-recent|recent": "最近使った関数",
    "fn-text|text": "文字列操作",
    "fontcolor|fontcolor": "フォントの色",
    "fontname|fontname": "フォント",
    "fontsize|fontsize": "フォントのサイズ",
    "footnote|footnote": "脚注",
    "forecast|forecast": "予測シート",
    "form-checkbox|form-checkbox": "チェックボックス",
    "form-combo|form-combo": "コンボボックス",
    "form-complex|form-complex": "複合フィールド",
    "form-dropdown|form-dropdown": "ドロップダウン",
    "form-email|form-email": "メールアドレス",
    "form-image|form-image": "画像",
    "form-name|form-name": "名前",
    "form-phone|form-phone": "電話番号",
    "form-radio|form-radio": "ラジオボタン",
    "form-signature|form-signature": "署名",
    "form-text|form-text": "テキストフィールド",
    "format|format": "数値の書式",
    "formula-bar|formula-bar": "数式バー",
    "freeze|freeze": "ウィンドウ枠の固定",
    "func-list|py-list": "Python の関数",
    "goal-seek|goal-seek": "ゴールシーク",
    "group|group": "グループ化",
    "hide-details|hide-details": "詳細の非表示",
    "hidenchars|hidenchars": "非表示文字",
    "highlighter|highlighter": "蛍光ペン",
    "highlight|highlight": "ハイライトの色",
    "hyphenation|hyphenation": "ハイフン設定の変更",
    "incfont|incfont": "フォントサイズの拡大",
    "incoffset|incoffset": "インデントを増やす",
    "inschart|inschart": "グラフを挿入",
    "inscheckbox|inscheckbox": "チェックボックス",
    "insequation|insequation": "方程式を挿入",
    "insert-function|additional-formula": "関数の挿入",
    "inshyperlink|inshyperlink": "ハイパーリンクを追加",
    "insimage|insertimage": "画像を挿入",
    "insimage|insimage-c": "画像を挿入",
    "insrecommend|insrecommend": "推奨チャートを挿入",
    "insshape|insshape": "図形を挿入",
    "insslicer|insslicer": "スライサーを挿入",
    "inssmartart|inssmartart": "SmartArtの挿入",
    "inssparkline|inssparkline": "スパークラインを挿入する",
    "inssymbol|inssymbol": "記号を挿入",
    "instable|instable": "表の挿入",
    "instextart|instextart": "テキストアートの挿入",
    "instext|instext": "テキストボックスの挿入",
    "italic|italic": "斜体",
    "line-numbers|line-numbers": "行番号を表示する",
    "linespace|linespace": "段落の行間",
    "markers|markers": "箇条書き",
    "merge|merge": "結合して、中央に配置する",
    "middle|middle": "上下中央揃え",
    "multilevels|multilevels": "複数レベルのリスト",
    "multipage|multipage": "複数ページ",
    "nav|nav": "ナビゲーション",
    "numbering|numbering": "ナンバリング",
    "numpages|numpages": "ページ数",
    "open|open": "開く",
    "pagebreak|pagebreak": "区切り",
    "pagecolor|pagecolor": "ページ色の変更",
    "pagemargins|pagemargins": "余白",
    "pagenum|pagenum": "ページ番号",
    "pageorient|pageorient": "印刷の向き",
    "pagesize|pagesize": "ページのサイズ",
    "paracolor|paracolor": "段落の背景色",
    "parastyle|styles": "段落のスタイル",
    "paste-name|paste-name": "名前を貼り付け",
    "paste|paste": "貼り付け",
    "pdf|print": "印刷",
    "pen|pen": "ペン",
    "percents|percents": "パーセントのスタイル",
    "pivot-blank|pivot-blank": "空行",
    "pivot-chart|pivot-chart": "ピボットグラフ",
    "pivot-fields|pivot-fields": "フィールドリスト",
    "pivot-insert|add-pivot": "ピボットテーブルを挿入",
    "pivot-insert|pivot-insert": "ピボットテーブルを挿入",
    "pivot-layout|pivot-layout": "レポートのレイアウト",
    "pivot-refresh-all|pivot-refresh-all": "すべて更新",
    "pivot-refresh|pivot-refresh": "更新",
    "pivot-select|pivot-select": "選択する",
    "pivot-showas|pivot-showas": "計算の種類",
    "pivot-source|pivot-source": "データソース",
    "pivot-style|pivot-style": "スタイル",
    "pivot-subtotals|pivot-subtotals": "小計",
    "pivot-totals|pivot-totals": "総計",
    "print-gridlines|print-gridlines": "枠線も印刷",
    "print-headings|print-headings": "見出しも印刷",
    "printarea-add|printarea-add": "範囲を足す",
    "printarea|printarea": "印刷範囲",
    "printtitles|printtitles": "タイトルを印刷する",
    "printview|printview": "印刷レイアウト",
    "prot-allow|prot-allow": "許可する操作",
    "prot-doc|prot-doc": "保護",
    "prot-doc|protect-sheet": "シートを保護する",
    "prot-encrypt|prot-encrypt": "暗号化する",
    "prot-sign|prot-sign": "デジタル署名を追加",
    "py-folder|py-folder": "置き場を開く",
    "py-list|plug-manage": "一覧",
    "py-list|py-list": "一覧",
    "py-new|py-new": "新しい .py",
    "read-only-rec|read-only-rec": "読み取り専用を勧める",
    "rec-toggle|py-run": "操作を記録",
    "recover-every|recover-every": "控えの間隔",
    "recover|recover": "復旧",
    "rem-duplicates|rem-duplicates": "重複の削除",
    "rem-duplicates|td-remdup": "重複データを削除",
    "remove-arrows|remove-arrows": "トレース矢印の削除",
    "replace|replace": "置き換え",
    "ribbon-list|py-line": "リボンのマクロ",
    "rtl-sheet|rtl-sheet": "最初の列が右側に来るようにシートの方向を切り替える",
    "ruby|ruby": "ルビ",
    "ruler|ruler": "ルーラー",
    "save|save": "保存",
    "scale|scale": "拡大縮小印刷",
    "scenario|scenario": "シナリオ",
    "selectall|select-all": "すべて選択",
    "setfilter|setfilter": "フィルター",
    "sheet-view|sheet-view": "シートの表示",
    "show-breaks|show-breaks": "紙の切れ目",
    "show-details|show-details": "詳細の表示",
    "show-formulas|show-formulas": "数式の表示",
    "show-gridlines|show-gridlines": "枠線表示",
    "show-headings|show-headings": "見出し",
    "show-left|show-left": "左パネル",
    "show-right|show-right": "右パネル",
    "show-statusbar|show-statusbar": "ステータスバー",
    "show-toolbar|show-toolbar": "ツールバーを常に表示する",
    "show-zeros|show-zeros": "0を表示する",
    "solver|solver": "ソルバー",
    "sort-asc|sortasc": "昇順並べ替え",
    "sort-desc|sortdesc": "降順並べ替え",
    "split|split": "分割",
    "strikeout|strikeout": "取り消し線",
    "subscript|subscript": "下付き",
    "subtotal|subtotal": "小計",
    "sum|autosum": "オートSUM",
    "superscript|superscript": "上付き",
    "table-tpl|table-tpl": "表として書式設定",
    "td-band-col|td-band-col": "縞模様の列",
    "td-band-row|td-band-row": "縞模様の行",
    "td-filter|td-filter": "フィルタのボタン",
    "td-first|td-first": "最初の列",
    "td-header|td-header": "ヘッダー行",
    "td-last|td-last": "最後の列",
    "td-resize|td-resize": "テーブルのサイズ変更",
    "td-torange|td-torange": "範囲に変換する",
    "td-total|td-total": "合計行",
    "text-column|text-column": "区切り位置",
    "text-from-file|text-from-file": "ファイルからのテキスト",
    "text-orient|text-orient": "方向",
    "toc-update|contents-update": "目次の更新",
    "toc|contents": "目次",
    "tof-update|tof-update": "図表目次の更新",
    "tof|tof": "図表目次",
    "top|top": "上揃え",
    "trace-dep|trace-dep": "参照先のトレース",
    "trace-prec|trace-prec": "参照元のトレース",
    "track-changes|track-changes": "変更履歴",
    "ui-bigger|ui-bigger": "画面の文字を大きく",
    "ui-smaller|ui-smaller": "画面の文字を小さく",
    "underline|underline": "下線",
    "ungroup|ungroup": "グループ解除",
    "watch|watch-window": "ウォッチウィンドウ",
    "watermark|watermark": "透かしを編集する",
    "wrap|wrap": "折り返して全体を表示する",
    "zoom-in|zoom-in": "拡大",
    "zoom-out|zoom-out": "縮小",
    "zoom100|zoom100": "100%に拡大する",
    # **id が付いた分**(2026-08-29。灰色のときは id が空でした)
    "img-align|img-align": "配置",
    "img-movebkwd|img-movebkwd": "背面ヘ移動",
    "img-movefrwd|img-movefrwd": "前面ヘ移動",
    "shapes-merge|shapes-merge": "図形を結合",
    "img-group|img-group": "グループ化",
    "prot-book|protect-workbook": "ブックを保護する",
    "view-normal|view-normal": "標準",
    "view-pagebreak|view-pagebreak": "改ページ プレビュー",
    "|img-align": "配置",
    "|img-group": "グループ化",
    "|img-movebkwd": "背面ヘ移動",
    "|img-movefrwd": "前面ヘ移動",
    "|protect-range": "範囲を保護する",
    "|protect-workbook": "ブックを保護する",
    "|shapes-merge": "図形を結合",
    "|view-normal": "標準",
    "|view-pagebreak": "改ページ プレビュー",
}

# タブの名前の橋。絵が無いので名前で引きます(名前は一意です)。
VENDOR_JA_TAB = {
    "Collaboration": "共同編集",
    "Data": "データ",
    "Draw": "描画",
    "File": "ファイル",
    "Forms": "フォーム",
    "Formula": "数式",
    "Home": "ホーム",
    "Insert": "挿入",
    "Layout": "レイアウト",
    "Macros": "マクロ",
    "Pivot Table": "ピボットテーブル",
    "Protection": "保護",
    "References": "参考資料",
    "Table Design": "表のデザイン",
    "View": "表示",
}

OVERRIDES = {
    "en": {
        # **セルの中の文字を回すボタン**(2026-08-21)。本家の日本語は
        # ページの向きと同じ「印刷の向き」で、押すまで区別できませんでした。
        # 日本語を Excel の「方向」にしたので、訳は本家の
        # SSE.Views.Toolbar.tipTextOrientation から取ります
        # text-orient(セルの中の字の向き)。ページの向きの「向き」と
        # 英語がかぶるので分ける(2026-08-26)
        "Text orientation": "Text orientation",
        # **セルの書式設定**(2026-08-21)。日本語は Excel の言葉にしたので
        # 本家の日本語(「セルをフォーマットする」)と字面が合いません。
        # 訳は本家の SSE.Views.DocumentHolder.txtCellFormat から取ります
        "Format cells": "Format cells",
        "Align middle": "Align middle",
        # 式から呼べる Python の関数の一覧(2026-08-16。本家に無い)
        "Python functions": "Python functions",
        # リボンに出るマクロの一覧(2026-08-16。本家に無い)
        "Ribbon macros": "Ribbon macros",
        "Format painter": "Format painter",
        "Style": "Style",
        "Field list": "Field list",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "Bigger UI text": "Bigger UI text",
        "Smaller UI text": "Smaller UI text",
        # **入力規則に合っていない値を洗い出すボタン**(2026-08-21 の D群)。
        # 本家にこの機能そのものがないので、語もこちらで用意します
        "Circle invalid data": "Circle invalid data",
        "Split": "Split",
        "Scenario": "Scenario",
        "Forecast Sheet": "Forecast Sheet",
        "PivotChart": "PivotChart",
        # タブ
        "AI": "AI",
        # ファイル
        "Print": "Print",
        "Print layout": "Print layout",
        # AI タブ(こちらの設計。calc-manual.md の英語版と同じ語)
        "要約": "Summarize",
        "書き直す": "Rewrite",
        "敬語にする": "Politer",
        "やさしく": "Plainer",
        "翻訳": "Translate",
        "Furigana": "Furigana",
        "続きを書く": "Continue",
        "表にする": "To table",
        "頼む": "Ask",
        # writer 独自
        "Ruby": "Ruby",
        "縦書き": "Vertical text",
        "Text direction": "Text direction",
        "Distributed": "Distributed",
        "図表番号の挿入": "Insert caption",
        "洋子さんの索引": "Index",
        "青空文庫の注記": "Aozora notes",
        "でんでん記法": "Denden markup",
        "履歴の記録": "Track changes",
        "変更履歴の表示": "Show changes",
        "校正": "Proofread",
        "文字数": "Character count",
        "スペルチェック": "Spell check",
        "類語辞典": "Thesaurus",
        "誤変換": "Misconversion",
        "表記ゆれ": "Inconsistency",
        # calc 独自
        "Subtotals": "Subtotals",
        # calc-mode。セルのスタイルの「計算」(Calculation)とかぶるので
        # 分ける。Excel の「計算方法の設定」に当たる
        "Calculation options": "Calculation options",
        "シートの方向": "Sheet direction",
        "Python": "Python",
        "Checkbox": "Checkbox",
        "外部リンク": "External links",
        "推奨チャート": "Recommended chart",
        # 共同編集・保護(writer/calc 共通の言い換え)
        "Co-editing mode": "Co-editing mode",
        "Version history": "Version history",
        "Chat": "Chat",
        "保護する": "Protect",
        "Encrypt": "Encrypt",
        "Add digital signature": "Add digital signature",
        "Macros": "Macros",
        "プラグインの管理": "Manage plugins",
        # 本家の語と言い回しが少し違うもの(Word/Excel の標準語で)
        "Show zeros": "Show zeros",
        "Zoom to 100%": "Zoom to 100%",
        "インターフェイステーマ": "Interface theme",
        "Watch window": "Watch window",
        "AutoSum": "AutoSum",
        "Record actions": "Record actions",
        "Delete comment": "Delete comment",
        "Solver": "Solver",
        "Text to data": "Text to data",
        "Remove arrows": "Remove arrows",
        "Fill": "Fill",
        "Clear filter": "Clear filter",
        "Write macro": "Write macro",
        "Text to columns": "Text to columns",
        "Caption": "Caption",
        "Table of figures": "Table of figures",
        "Update table of figures": "Update table of figures",
        "External links (import as values)": "External links (import as values)",
        "Math & Trig": "Math & Trig",
        "Show formulas": "Show formulas",
        "Right-to-left text": "Right-to-left text",
        "Text functions": "Text functions",
        "Date & Time": "Date & Time",
        "Recently used": "Recently used",
        "Print gridlines": "Print gridlines",
        "Update table of contents": "Update table of contents",
        "Banded columns": "Banded columns",
        "Print headings": "Print headings",
        "Hide detail": "Hide detail",
        "Remove duplicates": "Remove duplicates",
        "Insert function": "Insert function",
        # 2026-08-10 に足した21語(台帳の消し込みで増えたボタン)
        "CSV format": "CSV format",
        "Cell lock": "Cell lock",
        "Data table": "Data table",
        "Fill by example": "Fill by example",
        # py-list(マクロの一覧)。入力規則の「リスト」(List)と
        # かぶるので分ける
        "Macro list": "Macro list",
        "Paste name": "Paste name",
        "Recover": "Recover",
        "Wrap text": "Wrap text",
        "Backup interval": "Backup interval",
        "New .py": "New .py",
        "Add to area": "Add to area",
        "Fit to paper": "Fit to paper",
        "Page breaks": "Page breaks",
        "Open folder": "Open folder",
        "Show values as": "Show values as",
        "Allowed actions": "Allowed actions",
        "Suggest read-only": "Suggest read-only",
    },
    # vendor のロケールに無い語の穴埋め(gen_lang.py が材料の訳と併用する)
    "zh-tw": {
        # 本家の台湾語は「尋找和引用」— **引用は大陸の言い方**。
        # こちらの台湾語の材料は 參照 26 回・引用 0 回で、台湾の Excel も
        # 「查閱與參照」(2026-08-11、分類の耳を訳した下請けが数えて指摘)
        "Lookup & Reference": "查閱與參照",
        "Number of pages": "頁數",
        "Table Design": "表格設計",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "it": {
        "Filter button": "Pulsante filtro",
        "Header row": "Riga di intestazione",
        "Total row": "Riga totale",
        "Last column": "Ultima colonna",
        "Convert to range": "Converti in intervallo",
        "Table Design": "Struttura tabella",
        "Resize table": "Ridimensiona tabella",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "tr": {
        # 表の「罫線」。本家の日本語は「表の枠線」でしたが、セルに引く線
        # なので Excel は「罫線」です。この言語は文章の側の「罫線」から
        # 引けないので、表の側の鍵(tipBorders)を書きます
        "Borders": "Sınırlar",
        # **本家のトルコ語が誤訳**(2026-08-21)。Insert chart に
        # Tablo ekle(表を挿入)が入っていて、「表の挿入」と同じ
        # ラベルになっていました。tablo は表、grafik がグラフです。
        # 語は LibreOffice の公式訳(Insert Chart)から取りました
        "Insert chart": "Grafik ekle",
        "Protect Range": "Aralığı koru",
        "Merge shapes": "Şekilleri birleştir",
        "Page Break Preview": "Sayfa Sonu Önizlemesi",
        "Filter button": "Filtre düğmesi",
        "Header row": "Üst bilgi satırı",
        "Number of pages": "Sayfa sayısı",
        "印刷物で次のページを開始する位置に改行を追加する": "Yeni sayfanın başlayacağı yere sayfa sonu ekle",
        "Trace Precedents": "Etkileyenleri izle",
        "Trace Dependents": "Etkilenenleri izle",
        "Total row": "Toplam satırı",
        "Insert recommended chart": "Önerilen grafik ekle",
        "Switch the sheet direction so that the first column is on the right side": "Sayfa yönünü ilk sütun sağda olacak şekilde değiştir",
        "Last column": "Son sütun",
        "Convert to range": "Aralığa dönüştür",
        "Borders": "Kenarlıklar",
        "Highlighter": "Vurgulayıcı",
        "Table Design": "Tablo tasarımı",
        "Comma style": "Virgül stili",
        "Goal Seek": "Hedef Ara",
        "Resize table": "Tabloyu yeniden boyutlandır",
        "Text from File": "Dosyadan metin",
        "Insert SmartArt": "SmartArt ekle",
        "Update all": "Tümünü yenile",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "id": {
        "Merge shapes": "Gabungkan bentuk",
        "Goal Seek": "Pencarian Tujuan",
        "Resize table": "Ubah ukuran tabel",
        "Filter button": "Tombol filter",
        "Header row": "Baris header",
        "Number of pages": "Jumlah halaman",
        "Total row": "Baris total",
        "Switch the sheet direction so that the first column is on the right side": "Ubah arah lembar agar kolom pertama di kanan",
        "Last column": "Kolom terakhir",
        "Convert to range": "Konversi ke rentang",
        "Table Design": "Desain tabel",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "vi": {
        # 表の「罫線」。本家の日本語は「表の枠線」でしたが、セルに引く線
        # なので Excel は「罫線」です。この言語は文章の側の「罫線」から
        # 引けないので、表の側の鍵(tipBorders)を書きます
        "Borders": "Đường viền",
        "Protect sheet": "Bảo vệ trang tính",
        "Protect workbook": "Bảo vệ sổ làm việc",
        "Protect Range": "Bảo vệ phạm vi",
        "Merge shapes": "Hợp nhất hình dạng",
        "Page Break Preview": "Xem trước ngắt trang",
        "Insert SmartArt": "Chèn SmartArt",
        "Update all": "Làm mới tất cả",
        "More functions": "Hàm khác",
        "Freeze panes": "Cố định ngăn",
        "Comma style": "Kiểu dấu phẩy",
        "Combo box": "Hộp tổ hợp",
        "Goal Seek": "Tìm mục tiêu",
        "Sheet View": "Hiện trang tính",
        "Status Bar": "Thanh trạng thái",
        "Insert sparkline": "Chèn biểu đồ thu nhỏ",
        "Insert slicer": "Chèn slicer",
        "Print titles": "In tiêu đề",
        "Dark mode": "Chế độ tối",
        "Always Show Toolbar": "Luôn hiện thanh công cụ",
        "Add Text": "Thêm chữ",
        "Text Field": "Trường văn bản",
        "Resize table": "Đổi cỡ bảng",
        "Data Validation": "Xác thực dữ liệu",
        "Dropdown": "Danh sách thả xuống",
        "Navigation": "Dẫn hướng",
        "Change hyphenation": "Ngắt từ bằng dấu gạch nối",
        "Pivot Table": "PivotTable",
        "Insert Pivot Table": "Chèn PivotTable",
        "Text from File": "Văn bản từ tệp",
        "Filter button": "Nút lọc",
        "Filters": "Bộ lọc",
        "Forms": "Biểu mẫu",
        "Bookmark": "Dấu trang",
        "Header row": "Hàng tiêu đề",
        "Pen": "Bút",
        "Number of pages": "tổng số trang",
        "Page number": "số trang",
        "Change page colour": "Màu trang",
        "Email Address": "Địa chỉ email",
        "Radio Button": "Nút radio",
        "Rulers": "Thước",
        "Report Layout": "Bố cục báo cáo",
        "印刷物で次のページを開始する位置に改行を追加する": "Chèn ngắt trang tại vị trí bắt đầu trang mới",
        "Print Area": "Vùng in",
        "Trace Precedents": "Truy vết ô ảnh hưởng",
        "Trace Dependents": "Truy vết ô phụ thuộc",
        "Right Panel": "Bảng bên phải",
        "Total row": "Hàng tổng",
        "Change case": "Đổi chữ hoa/thường",
        "Left Panel": "Bảng bên trái",
        "Scale To Fit": "Co giãn khi in",
        "Insert recommended chart": "Chèn biểu đồ đề xuất",
        "Formula Bar": "Thanh công thức",
        "Italic": "Nghiêng",
        "Update": "Làm mới",
        "First column": "Cột đầu",
        "Switch the sheet direction so that the first column is on the right side": "Đổi hướng trang tính để cột đầu ở bên phải",
        "Conditional formatting": "Định dạng có điều kiện",
        "Lookup & Reference": "Tra cứu & tham chiếu",
        "Cross-reference": "Tham chiếu chéo",
        "Insert blank page": "Chèn trang trống",
        "Blank Rows": "Dòng trống",
        "Grand Total": "Tổng chung",
        "Banded Rows": "Hàng xen kẽ màu",
        "Borders": "Viền",
        "Highlighter": "Bút dạ quang",
        "Show line numbers": "Hiện số dòng",
        "Table Design": "Thiết kế bảng",
        "Complex Field": "Trường phức hợp",
        "Multiple pages": "Nhiều trang",
        "Headings": "Tiêu đề",
        "Insert symbol": "Chèn ký hiệu",
        "Logical": "Logic",
        "Financial": "Tài chính",
        "Edit watermark": "Sửa hình mờ",
        "Remove Duplicates": "Xóa dữ liệu trùng lặp",
        "Open": "Mở",
        "Phone Number": "Số điện thoại",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "de": {
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "es": {
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "fr": {
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "pt-br": {
        # ブラジル**だけ**を分ける札(2026-08-11 発注者)
        # 本家のブラジル語そのものが誤っていた3語。ブラジル語としても
        # 誤りなので、欧州版と一緒に直す(2026-08-11):
        #   Projeto da mesa   = 家具の机の設計(table を家具と取った)
        #   Total de linhas   = 行数(「合計の行」ではない)
        #   Faixa de proteção = 保護の帯(命令の動詞が要る所を名詞句に)
        "Table Design": "Design da Tabela",
        "Total row": "Linha de Totais",
        "Protect Range": "Proteger Intervalo",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "pt": {
        # 表の「罫線」。本家の日本語は「表の枠線」でしたが、セルに引く線
        # なので Excel は「罫線」です。この言語は文章の側の「罫線」から
        # 引けないので、表の側の鍵(tipBorders)を書きます
        "Borders": "Bordas",
        # **本家の欧州ファイル(pt-pt.json)は薄い。** 21 語は訳が無く、
        # 2 語はブラジル語が紛れていた("Estilo de porcentagem"、
        # データのタブが "Data"=日付)。**本家にあることは正しいことでは
        # ない** — 欠けたところは原文と英語から訳し、隣の言語から写さない
        # (2026-08-11。訳語の出どころは docs/sekkei/calc.ja.md)
        "Insert SmartArt": "Inserir SmartArt",
        "Comma style": "Estilo de vírgula",
        "Goal Seek": "Atingir objetivo",
        "Resize table": "Redimensionar tabela",
        "Change hyphenation": "Alterar a hifenização",
        "Text from File": "Texto de um ficheiro",
        "Filter button": "Botão de filtro",
        "Header row": "Linha de cabeçalho",
        "Number of pages": "Número de páginas",
        "Change page colour": "Alterar a cor da página",
        "印刷物で次のページを開始する位置に改行を追加する": "Adicione uma quebra no sítio onde a página seguinte deve começar na cópia impressa",
        "Trace Precedents": "Rastrear Precedentes",
        "Total row": "Linha de totais",
        "Merge shapes": "Unir formas",
        "Insert recommended chart": "Inserir gráfico recomendado",
        "Switch the sheet direction so that the first column is on the right side": "Inverta a direção da folha para que a primeira coluna fique do lado direito",
        "Last column": "Última coluna",
        "Convert to range": "Converter em intervalo",
        "Protect Range": "Proteger intervalo",
        "Borders": "Bordas",
        "Table Design": "Estrutura da Tabela",
        "Data": "Dados",
        "Percent style": "Estilo de percentagem",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "ru": {
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "ko": {
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "zh": {
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
}


# 本家の綴りがこちらと違うもの。中身と経緯は正本 ui/locales.py に —
# ポルトガル語は札の意味が逆(向こうの pt.json はブラジル)
import locales
VENDOR_LOCALE = locales.VENDOR


def load(app, loc):
    """本家の対訳を読む。綴りが違えば `VENDOR_LOCALE` で読み替える"""
    want = VENDOR_LOCALE.get(loc, loc)
    for name in (want, want.lower()):
        p = ROOT / app / f"main/locale/{name}.json"
        if p.exists():
            return json.load(open(p, encoding="utf-8"))
    sys.exit(
        f"ロケールの現物が見つかりません: {ROOT / app / f'main/locale/{want}.json'}"
        + (f"(こちらの {loc} を {want} と読み替えた)" if want != loc else "")
    )


def build_map(apps, target):
    """**本家の日本語 → target の語。** 同じ語に複数候補があれば多数決 →
    短い順 → 辞書順(決定的に選ぶ)。

    土台の札は英語ですが、本家を引く鍵は日本語のままです。理由は
    `VENDOR_JA` の註に書きました。
    """
    cand: dict[str, Counter] = {}
    for app in apps:
        ja = load(app, "ja")
        tr = load(app, target)
        for k, jv in ja.items():
            tv = tr.get(k)
            if not isinstance(jv, str) or not isinstance(tv, str):
                continue
            if not jv.strip() or not tv.strip():
                continue
            cand.setdefault(jv, Counter())[tv] += 1
    out = {}
    for jv, c in cand.items():
        best = sorted(c.items(), key=lambda kv: (-kv[1], len(kv[0]), kv[0]))[0][0]
        out[jv] = best
    return out


# `ribbon.rs` の読みは tools/ribbon_parse.py に集めた(2026-08-12)。
# ここにあった自前の正規表現は「合致する物を拾う」形で、**書き方が変われば
# 静かに減る**。5つの道具が同じ穴を持っていたので、1枚に寄せた。
#
# あちらは領域を**食べ尽くして、残りが1文字でも出たら落ちる**。読み落としが
# 無い代わりに、表の書き方を変えたら解析器も直すことになる — その取引で正しい。
# **この生成器は特に、読み落とすと生成物からボタンが消える**(黙って)。


# 本家の英語は米国綴りの1種類しかない。こちらの `en` は英国基準と
# 決めた(2026-08-11 発注者「英国基準がいいのでは」)ので、本家から
# 来た語を綴り直す。
#
# **札にだけ掛ける。** はじめ「米国綴りは Center の1語だけ」と数えて
# 上書き表に1行足して済ませたが、それは**大文字で始まる語しか
# 数えていなかった** — 実際には "Font color" のように語中に 7 件あった。
# そして id にも `align-center` があるので、ファイル全体に掛けると
# ボタンの id が変わって配線が切れる。掛ける場所を間違えると、
# 綴りが直る代わりにボタンが死ぬ。
BRITISH = {
    "color": "colour",
    "colors": "colours",
    "center": "centre",
    "centers": "centres",
    "centered": "centred",
    "organizer": "organiser",
    "customize": "customise",
    "customized": "customised",
    "analyze": "analyse",
    "gray": "grey",
}
_BRITISH_RE = re.compile(
    r"\b(" + "|".join(sorted(BRITISH, key=len, reverse=True)) + r")\b", re.I)


def respell(target, label):
    """米国綴りを英国綴りへ。`en` 以外はそのまま返す"""
    if target != "en":
        return label

    def one(m):
        w = m.group(0)
        b = BRITISH[w.lower()]
        return b[0].upper() + b[1:] if w[0].isupper() else b

    return _BRITISH_RE.sub(one, label)


def i18n_text(target: str) -> dict[str, str]:
    """**`ui/i18n/<言語>.json` に入っているリボンの語**(2026-08-21)。

    訳の置き場が2つあると必ずずれます。実際、2026-08-21 に2回踏みました
    — `OVERRIDES` だけ直して回すと、`gen_lang.py` が `ui/i18n` の古い語で
    上書きして戻します。

    *分担を決めました。*

    * `OVERRIDES["en"]` に載っている語(本家にどの言語でも無い、うちの
      ボタン)→ **訳は `ui/i18n/<言語>.json`**。i18n の手順で足します
    * `OVERRIDES["<言語>"]` に載っている語 → **その言語だけ本家に訳が
      無い**もの(ベトナム語は本家が 31% しか埋まっていないので 78 語)。
      ここでしか要らないので、ここに置きます

    どちらでも良い語はありません。重なっていた分は消しました。
    """
    if target == "en":
        return {}
    # **`ROOT` は vendor を指している。** ここは自分の隣の i18n を見る
    here = Path(__file__).resolve().parent
    p = here / "i18n" / f"{target}.json"
    if not p.exists() or not (here / "i18n" / "en.json").exists():
        return {}
    # **鍵は記号です**(2026-08-26)。`ui/i18n/en.json` が「記号 → 英語」
    # なので、そこから「英語の札 → その言語の訳」を作ります。
    # `OVERRIDES["en"]` の鍵はリボンの英語の札です
    translation = json.loads(p.read_text(encoding="utf-8"))
    english = json.loads((here / "i18n" / "en.json").read_text(encoding="utf-8"))
    needed = set(OVERRIDES["en"])
    out = {}
    for symbol, english_word in english.items():
        if english_word in needed and translation.get(symbol):
            out[english_word] = translation[symbol]
    return out


def main():
    if len(sys.argv) != 2:
        sys.exit("使い方: gen_ribbon_locale.py <locale>  (例: en)")
    target = sys.argv[1]
    # **その言語だけの穴埋め**を土台に、`ui/i18n` の訳を重ねます。
    # 重ねる順は gen_lang.py と同じ — 2つの道が同じ物を出すためです
    over = {**OVERRIDES.get(target, {}), **i18n_text(target)}
    doc_map = build_map(["documenteditor", "spreadsheeteditor"], target)
    cell_map = build_map(["spreadsheeteditor", "documenteditor"], target)
    tabs_of = tables_or_die(RIBBON)

    missing = []

    respelled = []

    def tr(label, m, icon="", tab_name=False):
        if label in over:
            return over[label]
        # **本家は日本語で引きます**(VENDOR_JA の註)。鍵は絵、
        # タブは名前(絵が無いため)
        label = VENDOR_JA_TAB.get(label, label) if tab_name \
            else VENDOR_JA.get(icon, label)
        if label not in m:
            missing.append(label)
            return label
        got = respell(target, m[label])
        if got != m[label]:
            respelled.append((m[label], got))
        return got

    out = []
    out.append(f"""//! リボンの {target} 版 — **語だけが ja(ribbon.rs)と違う**。
//! id・並び・ready・icon は ja と同一(ribbon.rs の試験が保証する)。
//!
//! このファイルは手で書かない:
//!
//! ```text
//! python3 ui/gen_ribbon_locale.py {target} > face/src/ribbon_{target}.rs
//! ```
//!
//! 対訳は vendor/web-apps のロケール(本家の語)。本家に無いこちらの
//! ボタンは gen_ribbon_locale.py の OVERRIDES 表で訳す。

use super::ribbon::{{{{import_of}}}};
""")
    def q(s):
        """Rust のリテラルに戻す。解析器は逃げを解いた素の字を渡してくる"""
        return s.replace("\\", "\\\\").replace('"', '\\"')

    # **並びは WRITER → CALC。** 解析器の dict は CALC が先なので、そのまま
    # 回すと生成物の2つの表が入れ替わる(受け入れ試験で気づいた)
    for const in ("WRITER", "CALC"):
        m = doc_map if const == "WRITER" else cell_map
        out.append(f"pub const {const}: &[Tab] = &[")
        for tab in tabs_of[const]:
            out.append(f'    Tab {{ name: "{q(tr(tab.name, m, tab_name=True))}", cmds: &[')
            for cmd in tab.cmds:
                # **書き方の名前をそのまま写す**(c / t / x / xt / xm)。
                # ボタンの性格は語ではないので、どの言語でも同じです
                if cmd.ready:
                    out.append(
                        f'        {cmd.kind}("{q(cmd.id)}", "{q(tr(cmd.label, m, f"{cmd.id or ''}|{cmd.icon}"))}",'
                        f' "{q(cmd.icon)}"),')
                else:
                    out.append(
                        f'        {cmd.kind}("{q(tr(cmd.label, m, f"{cmd.id or ''}|{cmd.icon}"))}", "{q(cmd.icon)}"),')
            out.append("    ]},")
        out.append("];\n")

    if missing:
        uniq = sorted(set(missing))
        sys.exit(
            f"訳の見つからない語が {len(uniq)} 個あります"
            f"(OVERRIDES に足してから出し直してください):\n  "
            + "\n  ".join(uniq))
    if target == "en" and not respelled:
        sys.exit(
            "::error::英語の綴り直しが1件も効いていません。"
            "本家の語が変わったか BRITISH 表が壊れています "
            "— 黙って米国綴りに戻さないため、ここで止めます")
    for a, b in sorted(set(respelled)):
        print(f"  綴り直し: {a} → {b}", file=sys.stderr)
    # **使った書き方だけを取り込む。** 使わない物を書くと警告になり、
    # clippy の門(-D warnings)で止まります
    body = "\n".join(out)
    used = [k for k in ("c", "t", "m", "x", "xt", "xm")
              if re.search(rf"^\s*{k}\(", body, re.M)]
    print(body.replace("{import_of}", ", ".join(used + ["Tab"])))


if __name__ == "__main__":
    main()
