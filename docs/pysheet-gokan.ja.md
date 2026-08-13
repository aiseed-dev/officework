# openpyxl・xlwings・python-docx 互換台帳の全量(2026-08-12 棚卸し)

3つのライブラリの中核クラスの公開メンバー **324 個**を、officework
(pysheet: sheet / calc / doc)と突き合わせた在庫台帳。設計(なぜこの方針か)は
[docs/sekkei/python.ja.md](sekkei/python.ja.md) の「上位互換は API とテストの
蒸留で作る」を見よ。**ここは在庫台帳 — 片づけたら行頭に ✔ を付ける。**

棚卸しの相手: openpyxl 3.1.5 / xlwings 0.36.14 / python-docx 1.2.0
(この日の .venv で inspect により採取)。

2026-08-12 発注者指摘で仕分け直した: **「控え(後回し)」という判定は
置かない** — 的の中の物は作るか、作らないかをその場で決める。

## 判定の言葉

| 判定 | 意味 |
|---|---|
| **ある** | 同じ役の API が既にある(名前が違えば互換層で別名を張るだけ) |
| **互換層** | Python の薄い互換層(pysheet/officework/*.py)だけで書ける。エンジンに手を入れない |
| **足す** | エンジン(Rust)か橋に API を足す。作ると決めた物 |
| **足す(書式)** | 書式の読み書き。2026-08-12 発注者確定「作る。合否の基準は**定義どおり動作するか**」— 相手の定義(ドキュメントとテスト)が合否の線 |
| **要らない** | 相手の内部事情・古い API・別の道具の代役。互換の的にしない |
| **作らない** | 設計と衝突する。理由を書いて残す |

## 集計

| 判定 | 件数 | |
|---|---|---|
| ある | 38 | 今日の時点で互換 |
| 互換層 | 75 | 互換層を書けば互換(エンジン無傷) |
| 足す | 102 | エンジン・橋への追加。実務の背骨はここ |
| 足す(書式) | 24 | 書式の読み書き(基準は定義どおりの動作) |
| 要らない | 57 | 的にしない(全件に理由) |
| 作らない | 28 | 設計と衝突(全件に理由) |
| **計** | **324** | ある+互換層 = 113(35%)は薄い層だけで届く |

(2026-08-12 の見立て直しで 互換層 78→75・足す 99→102: doc.rs と突き合わせ、
add_heading と Run の add_text / clear はエンジンの書き口待ちだった。該当行に注記)

---

## openpyxl(Workbook 28・Worksheet 69・Cell 28 = 125)

うちの相手は `officework.sheet`。**開いて保存しても様式が崩れず、式は
その場で再計算される** — この2点が openpyxl に無い上位分。

### Workbook(28)

| 相手 | 判定 | うちの対応・理由 |
|---|---|---|
| ✔ save | ある | Book.save(原本から図形・印刷設定を持ち越す) |
| ✔ sheetnames | ある | sheet_names。互換層で別名 |
| ✔ create_sheet | ある | add_sheet。名前の自動付け(Sheet, Sheet1, …)は互換層、index 引数も済(2026-08-12 に move_sheet が入った) |
| ✔ active | 互換層 | 先頭のシートを返す(xlsx の activeTab を読むならエンジンに小さな API) |
| ✔ close / worksheets / index / get_index / path | 互換層 | close は何もしないメソッドとして置く。worksheets = list(book)。path は**開いた元の径路**(openpyxl の内部定数 "/xl/workbook.xml" は真似しない — 誰の役にも立たない) |
| ✔ copy_worksheet | 足す | シート複製 — 月次の様式の写しは実務の定番(済 2026-08-12: Book.copy_sheet。中身・書式・結合・列幅ごと写し、写しは独立) |
| ✔ move_sheet | 足す | シートの並べ替え(済 2026-08-12: Book.move_sheet。openpyxl の「相対のずらし」は互換層で) |
| ✔ remove | 足す | シート削除(済 2026-08-12: Book.remove_sheet。**最後の1枚は抜けない** — シートの無い xlsx は無いので正直に断る。openpyxl は空のブックを許すが、あれは保存で壊れる側) |
| ✔ create_named_range | 足す | 名前付き範囲。式が参照する物なのでエンジンの計算にも絡む(**済 2026-08-12 夜**: エンジンに Sheet.names / define_name / delete_name(名前は属するシートの物 — 模型どおり)。openpyxl の defined_names(dict 風)と create_named_range の顔つき。定義した名前は式(=単価*数量)で効き、本家と両方向で往復。scope は持たない — 正直に断る) |
| ✔ epoch / excel_base_date | 足す | 1904 起点のブック(古い Mac 由来)を 1899-12-30 として読むと日付が4年ずれる — 黙って壊すのと同じ。起点の読みと解釈をエンジンに(**済 2026-08-13**: Book.date1904 の旗を評価器(funcs::call)・表示(format_value)・PDF(PrintSetup)・pysheet の datetime 受け渡しまで貫通。内部は 1899 のまま、通し番号↔暦日の**境目だけ**が excel_epoch(date1904) を通す(1904 は 24107)。元号・曜日・DATEDIF・WEEKNUM の類も同じ道。openpyxl の epoch / excel_base_date の顔つき — 読み書きとも、起点替えは「意味が4年動く」と注記して受ける。適合は本家の 1904 ブックと両方向 — 表示・YEAR・datetime 往復まで。**ついでに1つ塞いだ**: core.xml の欄の差し替えが要素に付いた xmlns 宣言を落とし、openpyxl 産のブックを保存すると lxml が開けない壊れた XML になっていた — 属性を保って差し替えるよう正した) |
| ✔ named_styles / style_names | 足す(書式) | 名前付き様式は書式の書き込みの一部(**済 2026-08-13**。実測で分かったこと: **往復は既に効いていた** —様式の一覧も、触っていないセルの様式名も、原本の styles.xml 据え置きで残る。口を張った: Book.named_styles / style_names(名前の並び)と named_style_fmt(様式の書式を fmt と同じ鍵で引く)。`cell.style = "見出し行"` は**その様式の書式をセルに写す** — 見た目は同じになるが、名前の帳簿はセルに持たない(模型の作り)ので、読みは openpyxl の既定どおり "Normal"。無い名前は KeyError で断る) |
| add_named_style | 足す(書式) | 様式を**作る**のは残 — 定義は原本の styles.xml の持ち物で、書き足す口(docx の styles_new と同じ外科術)が要る。正直に断っている |
| data_only | 要らない | 「式か値か」を開くときに選ばされるのが openpyxl の弱点。うちは常に両方ある(値も式も同時に読める) |
| get_sheet_by_name / get_sheet_names / remove_sheet | 要らない | 本家でも廃止予定の旧名 |
| mime_type / template | 要らない | 内部事情 |
| chartsheets / create_chartsheet | 作らない | グラフはアプリの分業どおり matplotlib を画像で貼る(SEKKEI「calc の分業」)。グラフ専用シートは作らない |
| read_only / write_only | 作らない | Python の遅さ・メモリを補うための別モード。エンジンは Rust — 巨大ファイルはモードを増やさず速さで解く |

### Worksheet(69)

| 相手 | 判定 | うちの対応・理由 |
|---|---|---|
| ✔ values | ある | Sheet.values(polars にそのまま渡せる) |
| ✔ insert_rows / delete_rows / insert_cols / delete_cols | ある | insert_row / remove_row / insert_col / remove_col。複数行の amount 引数は互換層で回す。**残った式の参照が付いて動く**のが上位分 |
| ✔ merged_cell_ranges | ある | merges。返す形だけ互換層で合わせる |
| ✔ title | ある | name(読み書き)。改名の書きは済(2026-08-12)— **式の参照と名前の定義が追随する**(openpyxl は追随しない = うちの上位分)。名前の規則(31字・使えない字)はアプリの改名と同じ検査 |
| ✔ BREAK_* / ORIENTATION_* / PAPERSIZE_* / SHEETSTATE_*(19 定数) | 互換層 | 定数を置くだけ(実値は棚卸しで採取済み。適合検査が openpyxl の実物と全19個を突き合わせる) |
| ✔ cell(row, column) | 互換層 | 数字指定 → A1 に写すだけ。移行コードで頻出 |
| ✔ append | 互換層 | 使われている範囲の次の行に1行置く。頻出。dict(列の字でも番号でも)も受ける |
| ✔ iter_rows / iter_cols / rows / columns | 互換層 | values から刻む |
| ✔ max_row / max_column | 互換層 | shape から |
| ✔ min_row / min_column | 互換層 | 値の入った最初の行・列を values の走査で出す(様式だけのセルまで数える正確さが要る事例が出たらエンジンに API を足す) |
| ✔ dimensions / calculate_dimension | 互換層 | min/max から "B2:G30" を組む(空は "A1:A1" — openpyxl と同じ) |
| ✔ parent | 互換層 | book へ戻る |
| ✔ merge_cells / unmerge_cells | 足す | 結合の**書き**(済 2026-08-12)。家の作法(アプリの結合と同じ — sheet::model::ops へ移して共有): 左上以外の中身は消え、空の左上へは最初の中身が書式ごと移る。unmerge は openpyxl どおり**厳密一致で ValueError**(掛かる結合をまとめて解く口はエンジン側) |
| ✔ freeze_panes | 足す | ウィンドウ枠の固定。実務多い(済 2026-08-12。openpyxl と同じ A1 形式 — "B2" = 上1行・左1列) |
| ✔ add_image | 足す | **matplotlib の図をシートに貼る** — アプリのグラフ分業と同じ道を Python にも。oneCellAnchor で書く(済 2026-08-12: 径路でも bytes でも。寸法は ops::image_px で測り、width_px/height_px で上書き。読み側の `images` も付けた — openpyxl はこれを公開 API で持たない) |
| ✔ oddHeader / oddFooter | 足す | 印刷ヘッダー・フッター(**済 2026-08-13**。模型(header / footer)と xlsx の読み書きは既にあり、口だけ欠けていた。openpyxl と同じ left / center / right の三分割(中身は &L&C&R の原文。&P は頁番号)。適合は両方向) |
| evenHeader / firstHeader / evenFooter / firstFooter | 足す | 奇数・偶数・先頭頁の**別**は模型に無い(1つだけ持つ)。**黙って同じ物を返さない** — 正直に断る。要る事例が出たら模型を太らせる |
| ✔ print_area | 足す | 印刷設定。calc はモデルに読む所まで済み — 書きを足す(**済 2026-08-12 夜**: 読みは openpyxl と同じ「'シート'!$A$1:$C$10」(複数域は , 区切り)、書きは $ もシート名! も付いていてよい。PDF と印刷がこれに従う。print_titles 系は模型に無い — 残) |
| ✔ print_titles / print_title_rows | 足す | 見出し行の繰り返し。**畑が無いと書いたのは誤り**(2026-08-13 実測で判明): 模型の print_title_rows・xlsx の読み書き(_xlnm.Print_Titles)・**PDF の繰り返し**(paper::grid の「タイトル行は2ページ目にも出る」)まで既に完動していた — 欠けていたのは Python の口だけ。張った(openpyxl と同じ "1:2" / print_titles は "'シート'!$1:$2")。適合は両方向 |
| print_title_cols | 足す | **列**の繰り返しは模型に畑が無い(行だけ持つ)。読みは None、書きは正直に断る — 日本の帳票では行の繰り返しが定番で、列は出てから足す |
| ✔ add_data_validation | 足す | エンジンは list 規則を読み・効かせ・往復済み。追加の API を足す(**済 2026-08-12 夜**: add_validation(範囲, formula1, kind, operator, formula2, allow_blank)+ validations の読み。openpyxl の DataValidation の実物を add_data_validation に渡しても効く(sqref ごと)。適合は両方向) |
| ✔ add_table / tables | 足す | テーブル(構造化参照・フィルタ)。**「原文持ち越しのみ」は誤り**(2026-08-13 実測): 模型の TableDef・xlsx の読み書き(table 部品・tableParts・関係・宣言)・**構造化参照の計算**(2026-08-08 実装)まで既に動いていた — 欠けは Python の口だけ。張った(openpyxl の Table / TableStyleInfo の形。本家の実物も受ける)。**`=SUM(明細[金額])` が計算まで効くのが上位分** — openpyxl は式を計算しない。適合は両方向。名前に空白は断る(式から引けなくなる) |
| ✔ array_formulae | 足す | 配列式。エンジンがスピルを覚える件(pyoffice の「返り値の形で広がる」)と同じ模型(済 2026-08-13: 模型の cse に口を張った。openpyxl と同じ {左上: 式} の形。**うちは値まで計算されている** — `=SUM(A1:A3*2)` が 12 を返す。openpyxl は式を持つだけ) |
| ✔ column_groups | 足す | 行・列のグループ化(outlineLevel)の読み書き(小)。画面の畳みはアプリの在庫(**済 2026-08-13**: 模型の row_outline / col_outline に口を張った。openpyxl と同じ row_dimensions.group(start, end, outline_level, hidden) / column_dimensions.group と、row_groups / column_groups の読み。**畳んだ状態は保存に残る** — 畳んだ台帳は畳んだまま次の人に渡る(絞り込みと違う)。適合は両方向) |
| ✔ move_range | 足す | 範囲の移動 — 式の参照も付いて動く。insert/remove と同族(**済 2026-08-13**: sheet::model::ops の Sheet::move_range。**参照の作法は Excel の切り貼りに合わせた** — 外から動かした範囲を指していた式は**付いて動く**(`=B1+1` → `=B6+1`)。openpyxl はここを古びたままにする(空のセルを指す)= うちの上位分。範囲の中の式はそのままで、translate=True なら中の相対参照がずれる(本家と同じ定義 — 適合検査で本家の結果と突き合わせ済み)。移った先は上書き、紙の外へは動かさない、結合も一緒に動く。橋にも rpc(語彙34)+ Range.move()。実機で確認) |
| ✔ show_gridlines | 足す | sheetView の読み書き(小)(済 2026-08-13: 画面の枠線(show_gridlines。指定なしは None)と**印刷**の枠線(print_gridlines)は別の設定 — 両方に口を張った。適合は両方向) |
| active_cell / selected_cell / sheet_view | 要らない | 画面の状態。ファイルの帳簿ではない |
| encoding / mime_type / path | 要らない | 内部事情 |
| set_printer_settings | 要らない | 内部部品(PrinterSettings)を直に差し込む API。印刷設定は print_area 系で足りる |
| add_chart | 作らない | グラフは matplotlib を画像で貼る(分業)。開いた帳票のグラフは持ち越しで壊さない — 作る機能だけ持たない |
| add_pivot | 作らない | 集計は polars の group_by / pivot(分業の境界線そのもの) |

### Cell(28)

うちはセルのオブジェクトを持たず、値を直に読み書きする。互換層として
**参照だけ持つ Cell**(座標+シートの札)を作れば、下の「互換層」は全部そこに載る
(済 2026-08-12 — sheet.py の Cell。`s["A1"]` が値を返すうちの口はそのまま、
Cell は `s.cell(row=, column=)` から)。

| 相手 | 判定 | うちの対応・理由 |
|---|---|---|
| ✔ value | ある | 値の読み書きそのもの。互換層の Cell では .value |
| ✔ row / column / col_idx / column_letter / coordinate | 互換層 | 座標の算術 |
| ✔ offset / parent | 互換層 | 参照の算術 |
| ✔ data_type | 互換層 | 値の型から('f' は式、's' 文字、'b' 真偽、'n' 数・空) |
| ✔ comment / hyperlink / protection | 足す | コメント・リンク・保護の読み書き。エンジンは持ち越し済み — 読み書きを足す(**済 2026-08-12 夜**: 模型の comments / links / fmt.unlocked に口を張った。openpyxl の Comment(.text)/ Hyperlink(.target)/ Protection(.locked)の形で受け、本家の実物の代入も効く。コメントの author と hidden(式を隠す)は模型に無い — 正直に断る/空で返す。適合は両方向) |
| ✔ alignment / border / fill / font / number_format | 足す(書式) | 読み書きとも作る(2026-08-12 発注者確定)。合否は**相手の定義どおり動くか** — openpyxl の Font/Border/PatternFill の形で受け、適合テストで証明する(**済 2026-08-12 夜**: エンジンに Sheet.fmt / set_fmt(dict の口)、sheet.py に Font / Border / Side / PatternFill / Alignment / Color。**属性名で受ける**ので openpyxl の実物の入れ物を代入しても効く。適合は両方向 — うちが書いた書式を openpyxl が読み、openpyxl が書いた書式をうちが読む。斜め罫線・solid 以外の塗り・indent は正直に断る) |
| ✔ is_date | 足す(書式) | 表示形式が日付か — number_format の読みと同じ一件(済 2026-08-12: 引用と [] を除いて y/m/d/h/s が残るか+中身が数か) |
| base_date / check_error / check_string / encoding / has_style / internal_value / pivotButton / quotePrefix / style / style_id | 要らない | 内部事情・古い API |

---

## xlwings(App 28・Book 21・Sheet 27・Range 63 = 139)

うちの相手は `officework.calc`(動いているアプリへの橋)。**Excel 本体が
要らない・Linux でも動く** — これが上位分。App クラスは作らない(下記)。

### App(28)

**App クラスそのものを作らない** — アプリは1つ、ブックも同時に1つ、
見えている物が全てなので、「どの Excel プロセスか」を選ぶ道具が要らない。
機能として意味のある物だけを Book かモジュールに置く。

| 相手 | 判定 | うちの対応・理由 |
|---|---|---|
| ✔ books | ある | モジュールの books(active / open / add) |
| ✔ calculate | 足す | 橋に「全再計算」のコマンド(済 2026-08-12: rpc calculate) |
| ✔ selection / get_selection | 足す | **いま選んでいる範囲を Python から読む** — 「選んで、Jupyter で加工」の入り方(済 2026-08-12: rpc selection → app.selection / Book.selection。get_selection は向こうの async 遠隔 API の対なので selection だけ) |
| ✔ version | 足す | アプリの版を ping の返事に足す(済 2026-08-12) |
| ✔ status_bar | 足す | アプリの status に文言を出す — 長い処理の進み具合を見せる(済 2026-08-12: rpc status。読みは最後に出した文言の覚え — 状態行は読み戻せない) |
| macro | 作らない | VBA を持たない(データとプログラムを分ける、2026-08-09 確定)。アプリ側の plugins が同じ役 |
| create_report / render_template | 作らない | 向こうの有料(PRO)機能。差し込みは replace と値の代入が本筋 |
| alert | 作らない | Python からアプリの画面に窓を割り込ませる道は作らない — 邪魔の経路。status_bar の文言で足りる |
| calculation | 作らない | 自動/手動の切替という状態を増やさない。エンジンが速いので手動モードの理由(遅さ)が無い |
| quit / activate / range | 作らない | 橋は動いているアプリに**付くだけ** — 起動も終了も画面の前後も人の物(App ごと作らない、の帰結) |
| api / engine / hwnd / pid / path / startup_path / properties | 要らない | COM・プロセスの露出 |
| cut_copy_mode / display_alerts / enable_events / interactive / screen_updating / kill / visible | 要らない | Excel の画面制御の事情。見えないアプリを作らない |

### Book(21)

| 相手 | 判定 | うちの対応・理由 |
|---|---|---|
| ✔ caller | ある | Book.caller()(attach と同じ物 — アドイン機構の境目が無い) |
| ✔ fullname / name / save | ある | |
| ✔ sheets | ある | _Sheets([] / active / iter / len) |
| ✔ app / sheet_names | 互換層 | sheets から。app は小さな取っ手(books だけ持つ — App クラスは作らない、の帰結のまま) |
| ✔ close | 足す | 橋に「閉じる」コマンド(済 2026-08-13: rpc close。**未保存があれば断る**(new / open と同じ作法)。アプリは常にブックを1つ持つ造りなので、閉じると新しい空のブックに戻る — 窓は閉じない(起動も終了も人の物)) |
| ✔ selection / get_selection | 足す | App の項と同じ(済 2026-08-12) |
| ✔ load | 足す | 範囲 → DataFrame の直行便。**polars を第一に**(pandas は options で従来どおり)(済 2026-08-12: 選択を読み、1マスなら表に広げる — xw.load / Book.load / Sheet.load) |
| ✔ to_pdf | 足す | アプリは PDF 書き出しを持っている(io.rs)— 橋から呼ぶだけ。**Sheet.to_pdf は済**。**ブック全体も済(2026-08-13 発注者「Book.to_pdf をつくりましょう」)**: paper に book_to_pdf を作り、sheet_to_pdf の中身を draw_sheet(共有の文書へ描く)と draw_header_footer(総頁が決まってから描く)に分けた。**頁番号(&P)と総頁(&N)はブック通し** — Excel がブック全体を刷るときと同じ数え方。紙・向き・余白・印刷範囲はシートごとに効き(1冊に縦と横が混ざってよい)、隠したシートは刷らない。xlwings の include / exclude は持たない(visible で選ぶ)。**ついでに1つ塞いだ**: ヘッダーの「&&」(素の & の書き方)が落ちていて「山田&田中」が壊れていた — 一度の走査に直し、頁番号の規則ごと hf_subst の試験で縛った |
| ✔ names | 足す | 名前付き範囲(openpyxl の create_named_range と同じ一件)(済 2026-08-12 夜: rpc names / define_name / delete_name(語彙25)。wb.names.add("単価", "=Sheet1!$A$1")・refers_to・delete — xlwings の形。実機で式の追随まで検査) |
| activate | 要らない | ブックは同時に1つの造り — 前に出す対象が無い |
| set_mock_caller | 要らない | Excel アドイン開発の道具。caller が attach と同じ物なのでモックが要らない |
| api / flush / json / sync | 要らない | COM と xlwings Server の事情 |
| macro / render_template | 作らない | App の項と同じ理由 |

### Sheet(27)

| 相手 | 判定 | うちの対応・理由 |
|---|---|---|
| ✔ name / range | ある | |
| ✔ used_range | ある | expand("table") が同じ役。互換層で別名 |
| ✔ book / cells / index | 互換層 | cells は全マス(A1:XFD1048576 — xlwings と同じ定義。読み書きするまで算術だけなので大きさは害にならない) |
| ✔ clear / clear_contents | 足す | 範囲・シートの中身消し。実務多い(済 2026-08-12: rpc clear / clear_contents。contents は書式据え置き=set の Null と同じ道。結合は消さない — 解くのは unmerge) |
| ✔ copy / delete | 足す | シート複製・削除(済 2026-08-12: rpc copy_sheet / delete_sheet。**耳のメニューと同じ関数**(picks.rs から切り出して共有)— 写しは右隣・sheet_ui/watch の帳尻・最後の1枚は断る・undo の束は消える、まで同じ。名前つき複製はシート名の決まりで検査) |
| ✔ freeze_panes | 足す | (済 2026-08-12: rpc freeze。xlwings の定義どおり freeze_at("B2")=上2行左2列・"1:1"=上1行・"A:A"=左1列・unfreeze。画面の固定(sheet_ui)に置き、保存の freeze_into_book で xlsx へ) |
| ✔ load | 足す | Book.load と同じ(済 2026-08-12: used_range から) |
| ✔ to_pdf | 足す | シート単位の PDF(済 2026-08-12: rpc to_pdf。印刷設定の言い分は note で返る) |
| ✔ activate / select | 足す | 画面のシートを Python から切り替える — 「見せる」のは見えるアプリならではの橋(済 2026-08-12: rpc activate_sheet。切替は画面と同じ switch_sheet — 打ちかけの確定・絞り込み解除ごと) |
| ✔ visible | 足す | 隠しシート(sheetState)の読み書き(済 2026-08-12: rpc sheet_visible。隠す作法は耳のメニューの「非表示」と同じ関数 — 最後の見えている1枚は断る・いまのシートを隠したら見える所へ移る) |
| ✔ autofit | 足す | 列幅の自動調整 — 文字の測りはアプリが持っている。橋から呼ぶ(**済 2026-08-13**: リボンの「自動調整」の腕を `Calc::autofit_at(a, b, col)` に切り出して rpc と共有(耳のメニューと同じ作法)。rpc autofit(語彙33)、Range.autofit("columns"/"rows") と Sheet.autofit。**DataFrame を落とした後に読める幅にする**のがこれ。実機で確認 — 検分中に自分の検査の落とし穴も1つ潰した(実機は前回の幅を持ち越すので、先に狭めてから測る)) |
| ✔ pictures | 足す | 画像の一覧と追加(sheet.add_image と対)(**済 2026-08-13**: rpc pictures / add_image(bytes は16進で運ぶ・片方だけの大きさは縦横比を保つ)。`sheet.pictures.add(図, anchor="F4")` — 図は 径路 / bytes / **matplotlib の figure**(xlwings と同じ)。「Python で描いて実機のシートに浮かべる」= SEKKEI「calc の分業」の筋が橋から一本通った。実機で目視まで済) |
| ✔ tables / names / page_setup | 足す | openpyxl 側の同じ一件(テーブル・名前付き範囲・印刷設定)(済 2026-08-13: rpc sheet_tables / page_setup と、names の絞り込み。page_setup は**読むだけ** — 紙と余白は見ながら決める物なのでアプリのレイアウトタブが正) |
| ✔ clear_formats | 足す(書式) | 書式を消すのも書式の書き込み(済 2026-08-12 夜: rpc clear_formats — 値は残る) |
| render_template | 作らない | PRO 機能 |
| charts | 作らない | グラフは matplotlib 画像(pictures で入る) |
| shapes | 作らない | 図形は見ながら作る領分(アプリの SVG 図形)。Python からは画像で貼る |
| to_html | 作らない | HTML 書き出しは範囲の外 — values を polars/pandas に渡せば to_html がある |
| api | 要らない | |

### Range(63)

| 相手 | 判定 | うちの対応・理由 |
|---|---|---|
| ✔ value / formula / expand / options | ある | options は pandas 済み。polars も済(2026-08-12): options(pl.DataFrame) の読みと、DataFrame / Series の代入の両方向。polars には index が無いので index 引数は polars では効かない |
| ✔ address / get_address / row / column / rows / columns / count / size / shape | 互換層 | 参照の算術 |
| ✔ offset / resize / last_cell / current_region | 互換層 | current_region は expand と同族 |
| ✔ sheet / get_value / raw_value / formula2 | 互換層 | formula2 = formula(動的配列の別名)。get_value は手(引数なし・value と同じ物 — 実物で確認) |
| ✔ merge_cells | 互換層 | merges(rpc)の一覧と自分の重なりで出す(済 2026-08-12 — 互換層はこれで全部済) |
| ✔ clear / clear_contents | 足す | (済 2026-08-12 — Sheet の項と同じ rpc) |
| ✔ insert / delete | 足す | 範囲の挿入・削除(詰める向きつき)(**済 2026-08-13**: rpc insert_rows / delete_rows / insert_cols / delete_cols(語彙32)。Range.insert(shift="down"/"right")・delete(shift="up"/"left")と Sheet.insert_rows(at, count) 系。**残った式の参照が付いて動く**のが上位分 — 実機で確認(挿入で =H1*3 が =H2*3 になり答えも合う)。**部分的なセルのずらしは持たない** — 行・列は丸ごと動く、が家の作法。正直に断る) |
| ✔ merge / unmerge / merge_area | 足す | 結合の書きと読み(済 2026-08-12: rpc merge / unmerge / merge_area。作法はアプリと同じ sheet::model の merge。merge(across=True) は行ごと。unmerge は掛かる物を全部 — xlwings の定義どおり) |
| ✔ end | 足す | Ctrl+矢印相当。橋に end のコマンド(済 2026-08-12。端は使っている範囲まで — 1048576 行目には飛ばない) |
| ✔ select | 足す | Python から選択を動かして見せる(済 2026-08-12: rpc select。打ちかけは確定してから動く) |
| ✔ add_hyperlink / hyperlink | 足す | リンクの読み書き(openpyxl Cell.hyperlink と同じ一件)(済 2026-08-13: rpc hyperlink。text_to_display も効く。screen_tip(吹き出し)は模型に無いので断る) |
| ✔ note | 足す | セルのコメント(openpyxl comment と同じ一件)(済 2026-08-13: rpc note。Note.text / .delete()) |
| ✔ name | 足す | 名前付き範囲の一件(済 2026-08-12 夜: Range.name の読み(範囲をちょうど指す名前)と代入 — xlwings の定義どおり) |
| ✔ table | 足す | テーブル(構造化参照)の一件(済 2026-08-13: この範囲を含む表を返す) |
| ✔ autofit | 足す | Sheet.autofit と同じ(済 2026-08-13) |
| ✔ group / ungroup | 足す | 行・列のグループ化(column_groups と同じ一件)(済 2026-08-13: rpc group。level=0 で外す・hidden で畳む(保存に残る)) |
| ✔ has_array / formula_array | 足す | 配列式の一件(array_formulae と一緒)(済 2026-08-13: rpc array_info) |
| ✔ height / width / left / top | 足す | **レイアウトの座標(ポイント)** — 画面の状態ではなく、モデルの列幅・行高から計算で出せる。画像・図形の置き場所の計算に使う(当初「画面のピクセル」と読み違えて要らないに入れていた — 2026-08-12 発注者指摘で正した)(済 2026-08-13: rpc layout。列幅(字数→px→pt)と行高から測る) |
| ✔ color / font / number_format / column_width / row_height / wrap_text / clear_formats | 足す(書式) | 書式の読み書き(openpyxl Cell と同じ一件)。合否は xlwings の定義どおり — color は**塗り**のタプル(RGB)、font は**性質ごと**の読み書き(font.bold = True は太字だけ変える — openpyxl の一式置き換えと逆の作法)、column_width は字数・row_height はポイント、範囲でまちまちなら None(**済 2026-08-12 夜**: rpc get_fmt / set_fmt / col_width / row_height / clear_formats。実機で検査) |
| ✔ adjust_indent | 足す(書式) | 字下げ(indent)はエンジンの CellFormat にまだ無い — openpyxl の Alignment.indent と同じ一件。模型に足してから(**済 2026-08-13**: CellFormat に indent を足し、styles.xml の読み書き・**画面と PDF の描き**・Python の口(openpyxl の Alignment(indent=))まで通した。**踏んだ穴**: 字下げのあるセルの書式を1つ触ると 2→0 に消えていた(値の書き替えは据え置きが効くので残る)— 「書式は据え置き」の破れを塞いだ。実機で階層が段に見えるのを目視。適合は両方向+「触っても消えない」) |
| autofill | 作らない | 連番・式の引き伸ばしの推測は画面の機能。Python 側では作って代入する方が明示的 |
| copy / copy_from / paste | 作らない | クリップボードは人の物 — Python が黙って上書きしない。値の移しは代入で |
| copy_picture / to_png / to_pdf | 作らない | 範囲を絵にする道は持たない。PDF はシート・ブック単位まで |
| api / impl / characters | 要らない | COM の露出 |

---

## python-docx(Document 19・Paragraph 13・Run 15・Table 13 = 60)

うちの相手は `officework.doc`。**読めない物を黙って落とさない(unsupported)・
書式の分かれ目を保った replace・保存で様式・変更履歴を持ち越す** — これが上位分。

### Document(19)

| 相手 | 判定 | うちの対応・理由 |
|---|---|---|
| ✔ add_paragraph / paragraphs / save / tables | ある | |
| ✔ add_heading | 足す | 当初「互換層(add_paragraph + style)」としたが、エンジンの Paragraph.style は**読みだけ**(2026-08-12 doc.rs と突き合わせて正した)。style の書きをエンジンに足してから(**済 2026-08-12 夜**: style の書き(body / heading1〜3。本家の "Heading 1" の名前でも受ける)+ add_heading(level 1〜3。0=Title は持たないので断る)。**まっさらの文書に最小の styles.xml**(Normal+見出し1〜3の名乗り)を書く — これが無いと読み手が pStyle を解決できず Normal に落ちる。原本があれば原本の定義を持ち越すだけ) |
| ✔ add_page_break | 足す | (済 2026-08-12 夜。本家は「改ページの run」だが、うちは**段落の性質(page_break_before)**で持つ — 紙の上の意味は同じで、本家の paragraph_format.page_break_before でも読める) |
| ✔ add_table | 足す | 表を新しく組む(明細の帳票づくりに要る)(済 2026-08-12。style 引数は「足す(書式)」待ち — 黙って捨てず断る。ついでに ooxml の書き手の穴を1つ塞いだ: 等分の表で tblGrid(必須部品)を省いていて、python-docx が読めなかった) |
| ✔ add_picture | 足す | 図を入れる(sheet.add_image と対)(済 2026-08-12 夜: 径路でも bytes でも。大きさは mm の数でも本家の Length(Mm(60))でも。片方だけなら縦横比を保つ。返りは画像を持つ段落 — 本家の InlineShape とはそこだけ流儀が違う) |
| ✔ iter_inner_content | 足す | 段落と表を**文書の順で**返す。うちの paragraphs / tables は種類別 — 順序を返す API をエンジンに(済 2026-08-12 夜) |
| ✔ core_properties | 足す | 文書情報の読み書き(小)(済 2026-08-12 夜: author / title / keywords / subject / comments — 呼び名は本家、中身は docProps/core.xml の5欄) |
| ✔ sections / add_section | 足す | エンジンの模型が1節しか持てない既知の残り —**実は持てていた**(途中の節は段落の sect、文書末は sect_raw / page)。sections は済 2026-08-12。**add_section も済(2026-08-13 発注者「これにすすみましょう」)**: 切るのは**末尾** — いままで書いた分が前の節になり、これから足す物が新しい節に入る(python-docx と同じ切り方)。中では、いまの文書末の節を「sectPr だけを持つ空の段落」として本文の末尾に置く(docx の途中の節の書き方そのもの)。新しい節は同じ紙と余白を継ぎ、変えたければ返ってきた節に書く。start_type は new_page(既定)と continuous (`<w:type w:val="continuous"/>` を原文に置く)だけ — 新しい段・偶数頁・奇数頁は模型に無いので**正直に断る**(黙って改ページに落とすと刷ったとき別物になる)。適合は本家の enum をそのまま渡して両方向 |
| ✔ comments / add_comment | 足す | コメントの読み書き。変更履歴と同じく、読めて書けて壊さない(済 2026-08-12 夜: 模型の粒度どおり**段落単位** — Paragraph.add_comment(text, author) と Doc.comments(Comment.paragraph でどの段落かが分かる)。本家(1.2)の comments API もうちの書いた物を読める) |
| ✔ inline_shapes | 足す | 画像の一件(add_picture と対の読み)(済 2026-08-12 夜: width / height は自前の Length(EMU。.mm / .pt が本家と同じ算術)。本文の段落の分 — セルの中は数えない(模型の粒度)) |
| ✔ styles | 足す(書式) | スタイルの一覧と定義 — 書式の一件(**済 2026-08-13**。発注者確定「スタイル定義は持たない主義では無理」で主義を改めた: (1) **知らないスタイル名も捨てない** — pStyle / rStyle の原文を Paragraph.style_id / CharFormat.style_id が運び、保存で返す(今までは開いて保存で消えていた = 「書式は据え置き」の穴。塞いだ)。(2) 定義の本体は**原文の styles.xml を正として持ち越し**、足した分だけ追記(core.xml と同じ外科術)。(3) Doc.styles(名前で引ける)・add_style(name, 種類)— 名乗りだけの最小定義で、見た目は直接書式が第一のまま。適合は両方向 — 本家の社内様式がうちの往復で残り、うちが足した様式を本家が読む) |
| element / part / settings | 要らない | lxml の露出 |

### Paragraph(13)

| 相手 | 判定 | うちの対応・理由 |
|---|---|---|
| ✔ text / style / runs | ある | |
| ✔ alignment | ある | align。互換層で別名 |
| ✔ clear | 互換層 | text = ""(段落の性質と先頭 run の書式は残る — 本家と同じ定義。自分を返す) |
| ✔ iter_inner_content | 互換層 | runs から(リンクは hyperlinks(足す)が来たら混ぜる) |
| ✔ add_run | 足す | 段落に run を継ぎ足す(済 2026-08-12 夜。書式は**末尾の run を継ぐ** — text の代入が先頭を継ぐのと対。style 引数は 2026-08-13 の主義転換で**効くようになった** — styles にある文字スタイルの名前) |
| ✔ insert_paragraph_before | 足す | 途中に差す(add_paragraph は末尾だけ)(済 2026-08-12 夜。手元の札は位置で指すので、差した後は引き直す — シートの札と同じ作法。style 引数は断る) |
| ✔ hyperlinks | 足す | リンクの一件(**済 2026-08-13**。踏んだ穴: docx のリンクは**読まれず・unsupported にも出ず・保存で消えて**いた(字だけ残るので気づきにくい)。CharFormat に link を足し(field・ruby と同じ持ち場 — run の切り貼りが効く)、読みは w:hyperlink の r:id を関係から URL に解き、書きは包み直して関係も宣言する。Paragraph.hyperlinks / add_hyperlink・Run.hyperlink。**add_run はリンクを継がない** — 掛かりを決めるのは囲みで字の書式ではない(検査が捕まえた)。適合は両方向) |
| ✔ paragraph_format | 足す(書式) | 段落書式(行間・字下げ・前後の間隔)(**済(部分)2026-08-12 夜**: alignment(本家の enum も受ける)・line_spacing・page_break_before — 模型が持つ物。space_before / space_after・left_indent は**模型に無い**ので読みは None・書きは正直に断る。模型の indent は段数(1段=全角2字)で、本家の Length との対応は決めてから) |
| part / contains_page_break / rendered_page_breaks | 要らない | 内部・レイアウト依存 |

### Run(15)

| 相手 | 判定 | うちの対応・理由 |
|---|---|---|
| ✔ text / bold / italic / underline / font | ある | ほかに color / size_pt も既にある(相手は font の下に置く — 互換層で両対応。_doc.py の _Font: str の子で `== "MS明朝"` も `.name` も通る) |
| ✔ add_text / clear | 足す | 当初「互換層」としたが、エンジンの Run は**凍った写し**で run 単位の書き口が無い(2026-08-12 doc.rs と突き合わせて正した)。run の書きをエンジンに足してから(**済 2026-08-12 夜**: Run を**位置で引き直す手**に変えた — text / bold / italic / underline / strike / color / size_pt / font の読み書き、add_text は書式を保って継ぎ足し、clear は字だけ消して自分を返す。font の両対応(str と .name)も書きが効くようになった) |
| ✔ add_break / add_tab | 足す | (済 2026-08-13: 読み書きは既にあり(w:br / w:tab ↔ 改行 / タブ)、口だけ張った。改ページの break_type は段落の性質(page_break_before)で持つので断る)。add_picture(run 単位)は段落の add_picture が同じ役 |
| ✔ iter_inner_content | 足す | run の中の改行・タブ・画像も順に返す(済 2026-08-13: 字は str、改行は Break、タブは Tab で**順のまま**。うちは両方を run の字(改行・タブ)で持つので、ここで解いて見せる。run の中の画像は模型では段落の持ち物 — そこは返らない) |
| mark_comment_range | 足す | コメントの一件。**うちのコメントは段落単位**(模型の粒度)で、run から run までの範囲は持てない — 正直に断り、Paragraph.add_comment を案内する(2026-08-13) |
| ✔ style | 足す(書式) | 文字スタイル(済 2026-08-13 — styles の一件と一緒。読みは styles の名前、書きは styles にある文字スタイルだけ(無い名前は add_style で作ってから — 黙って作らない)。add_run(style=)・insert_paragraph_before(style=)も同じ道で効くようになった) |
| part / contains_page_break | 要らない | |

### Table(13)

| 相手 | 判定 | うちの対応・理由 |
|---|---|---|
| ✔ rows | ある | ほかに shape / values(polars 直行)が既にあり、これは相手に**無い**上位分 |
| ✔ cell(r,c) / row_cells / column_cells / columns | 互換層 | rows[r].cells[c] から。結合で行の列数が違うとき column_cells は無い行を飛ばす(長方形しか持てない本家に、この場合の定義が無い) |
| ✔ add_row / add_column | 足す | 明細行の継ぎ足し。実務の定番(済 2026-08-12。add_column の width は python-docx と同じ EMU を受けて mm に直す。等分の表に幅つきの列は形が決まらないので正直に断る) |
| ✔ style / alignment / autofit | 足す(書式) | 表のスタイル・配置・幅の自動調整 — 書式の一件(**済 2026-08-12 夜**: kumihan の Table に style(**名前だけ運ぶ** — 定義は styles.xml の持ち物で、持たない主義のまま)・align・fixed_layout の畑、ooxml が tblPr の tblStyle / jc / tblLayout を読み書き。本家の enum・スタイルの物("Table Grid" は styleId "TableGrid" に寄せる)も受ける。適合は両方向) |
| table_direction | 作らない | 右→左の表(アラビア語系の bidi)— このソフトの的の外。日本語の右横書き・縦書きは kumihan の側で扱う |
| part / table | 要らない | |

---

## 消し込みの順番(2026-08-12 当方の見立て)

1. ✔ **互換層の一括**(済 2026-08-12)— openpyxl の口は pysheet/officework/sheet.py
   (新設。_sheet を包む純 Python — エンジン無傷)、xlwings の算術は calc.py、
   python-docx の口は _doc.py(再輸出から包みに変えた)。適合検査は
   pysheet/test_gokan.py — **書きは本家と同じ手順を並べて動かし、結果そのものを
   突き合わせる**(うちが書いた xlsx を openpyxl が読める・計算済みの値まで見える、
   うちが書いた docx を python-docx が読める、まで)。python_smoke.rs が回す。
   残り3件は上の注記のとおり: merge_cells(merge_area 待ち)・create_sheet の
   index 引数(move_sheet 待ち)・Range の options に polars(「足す」の背骨と一緒に)
2. **足すの背骨**(102 件のうち実務で頻出の物から)— **前半済 2026-08-12**:
   merge の書き(家の作法を sheet::model::ops へ移してアプリと共有)・
   freeze_panes・シートの複製/削除/改名(改名は参照が追随 — rename_sheet_refs を
   sheet::model::refs へ移してアプリと共有)/並べ替え。
   **後半も大方済(2026-08-12)**: add_image(oneCellAnchor・images の読みつき)・
   add_table/add_row/add_column(docx。ooxml の tblGrid の穴も塞いだ)・
   options の polars(読み書き両方向)。
   **橋の背骨も済(2026-08-12 夕)**: ops::handle に 15 語彙
   (calculate / selection / select / activate_sheet / status / to_pdf /
   copy_sheet / delete_sheet / merges / merge / unmerge / merge_area /
   clear / clear_contents / end)、Host に画面側の口(calc/src/rpc.rs。
   シートの複製・削除は耳のメニューと同じ関数を picks.rs から切り出して共有)。
   calc.py の対応する口ごと**実機の calc で検査済み** —
   tools/hashi_check.py(ribbon_sweep.py と対)。
   追って freeze / sheet_visible も同日に(語彙は 17 に)。
   橋の残り: Book.close・Book.to_pdf(束ね)・Sheet.autofit / pictures /
   tables / names / page_setup・Range.insert / delete・
   細目(note・hyperlink・group / ungroup・has_array / formula_array・
   height / width / left / top・name / table)
3. **書式の読み書き**(足す(書式)24 件)— エンジンの fmt に Python から書く道。合否は相手の定義どおり動くか
4. **適合テストの移植** — 3つのテストの実務部分を pysheet/tests/ へ(NOTICE.md に出所)。書式の書き込みの「定義どおり」もここで証明する

採取の道具: [tools/gokan_inventory.py](../tools/gokan_inventory.py)。
再採取は `.venv/bin/python tools/gokan_inventory.py inventory.json`
(3つが .venv に入っている前提)。
