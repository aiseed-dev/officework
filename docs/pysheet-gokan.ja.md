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
| epoch / excel_base_date | 足す | 1904 起点のブック(古い Mac 由来)を 1899-12-30 として読むと日付が4年ずれる — 黙って壊すのと同じ。起点の読みと解釈をエンジンに |
| add_named_style / named_styles / style_names | 足す(書式) | 名前付き様式は書式の書き込みの一部 |
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
| evenHeader / oddHeader / firstHeader / evenFooter / oddFooter / firstFooter | 足す | 印刷ヘッダー・フッター。writer のヘッダー・フッターの後に同じ模型で |
| ✔ print_area | 足す | 印刷設定。calc はモデルに読む所まで済み — 書きを足す(**済 2026-08-12 夜**: 読みは openpyxl と同じ「'シート'!$A$1:$C$10」(複数域は , 区切り)、書きは $ もシート名! も付いていてよい。PDF と印刷がこれに従う。print_titles 系は模型に無い — 残) |
| print_titles / print_title_rows / print_title_cols | 足す | 見出し行の繰り返し — 模型に畑が無い(pageSetup の titles)。模型を太らせてから |
| ✔ add_data_validation | 足す | エンジンは list 規則を読み・効かせ・往復済み。追加の API を足す(**済 2026-08-12 夜**: add_validation(範囲, formula1, kind, operator, formula2, allow_blank)+ validations の読み。openpyxl の DataValidation の実物を add_data_validation に渡しても効く(sqref ごと)。適合は両方向) |
| add_table / tables | 足す | テーブル(構造化参照・フィルタ)。いまは原文持ち越しのみ — 読み書きを足す |
| array_formulae | 足す | 配列式。エンジンがスピルを覚える件(pyoffice の「返り値の形で広がる」)と同じ模型 |
| column_groups | 足す | 行・列のグループ化(outlineLevel)の読み書き(小)。画面の畳みはアプリの在庫 |
| move_range | 足す | 範囲の移動 — 式の参照も付いて動く。insert/remove と同族 |
| show_gridlines | 足す | sheetView の読み書き(小) |
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
| close | 足す | 橋に「閉じる」コマンド |
| ✔ selection / get_selection | 足す | App の項と同じ(済 2026-08-12) |
| ✔ load | 足す | 範囲 → DataFrame の直行便。**polars を第一に**(pandas は options で従来どおり)(済 2026-08-12: 選択を読み、1マスなら表に広げる — xw.load / Book.load / Sheet.load) |
| to_pdf | 足す | アプリは PDF 書き出しを持っている(io.rs)— 橋から呼ぶだけ。**Sheet.to_pdf は済** — Book の分はアプリの PDF がシート単位なので、束ねる口が要る |
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
| autofit | 足す | 列幅の自動調整 — 文字の測りはアプリが持っている。橋から呼ぶ |
| pictures | 足す | 画像の一覧と追加(sheet.add_image と対) |
| tables / names / page_setup | 足す | openpyxl 側の同じ一件(テーブル・名前付き範囲・印刷設定) |
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
| insert / delete | 足す | 範囲の挿入・削除(詰める向きつき) |
| ✔ merge / unmerge / merge_area | 足す | 結合の書きと読み(済 2026-08-12: rpc merge / unmerge / merge_area。作法はアプリと同じ sheet::model の merge。merge(across=True) は行ごと。unmerge は掛かる物を全部 — xlwings の定義どおり) |
| ✔ end | 足す | Ctrl+矢印相当。橋に end のコマンド(済 2026-08-12。端は使っている範囲まで — 1048576 行目には飛ばない) |
| ✔ select | 足す | Python から選択を動かして見せる(済 2026-08-12: rpc select。打ちかけは確定してから動く) |
| add_hyperlink / hyperlink | 足す | リンクの読み書き(openpyxl Cell.hyperlink と同じ一件) |
| note | 足す | セルのコメント(openpyxl comment と同じ一件) |
| ✔ name | 足す | 名前付き範囲の一件(済 2026-08-12 夜: Range.name の読み(範囲をちょうど指す名前)と代入 — xlwings の定義どおり) |
| table | 足す | テーブル(構造化参照)の一件 — エンジンの add_table / tables(xlsx)と一緒に |
| autofit | 足す | Sheet.autofit と同じ |
| group / ungroup | 足す | 行・列のグループ化(column_groups と同じ一件) |
| has_array / formula_array | 足す | 配列式の一件(array_formulae と一緒) |
| height / width / left / top | 足す | **レイアウトの座標(ポイント)** — 画面の状態ではなく、モデルの列幅・行高から計算で出せる。画像・図形の置き場所の計算に使う(当初「画面のピクセル」と読み違えて要らないに入れていた — 2026-08-12 発注者指摘で正した) |
| ✔ color / font / number_format / column_width / row_height / wrap_text / clear_formats | 足す(書式) | 書式の読み書き(openpyxl Cell と同じ一件)。合否は xlwings の定義どおり — color は**塗り**のタプル(RGB)、font は**性質ごと**の読み書き(font.bold = True は太字だけ変える — openpyxl の一式置き換えと逆の作法)、column_width は字数・row_height はポイント、範囲でまちまちなら None(**済 2026-08-12 夜**: rpc get_fmt / set_fmt / col_width / row_height / clear_formats。実機で検査) |
| adjust_indent | 足す(書式) | 字下げ(indent)はエンジンの CellFormat にまだ無い — openpyxl の Alignment.indent と同じ一件。模型に足してから |
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
| sections / add_section | 足す | エンジンの模型が1節しか持てない既知の残り(「実物で測った」の表)— 模型を太らせる件と同じ一件 |
| ✔ comments / add_comment | 足す | コメントの読み書き。変更履歴と同じく、読めて書けて壊さない(済 2026-08-12 夜: 模型の粒度どおり**段落単位** — Paragraph.add_comment(text, author) と Doc.comments(Comment.paragraph でどの段落かが分かる)。本家(1.2)の comments API もうちの書いた物を読める) |
| ✔ inline_shapes | 足す | 画像の一件(add_picture と対の読み)(済 2026-08-12 夜: width / height は自前の Length(EMU。.mm / .pt が本家と同じ算術)。本文の段落の分 — セルの中は数えない(模型の粒度)) |
| styles | 足す(書式) | スタイルの一覧と定義 — 書式の一件。**注(2026-08-12 夜)**: 模型は「スタイル定義(styles.xml)は持たない — 見た目は直接書式で付ける」を明記している(engine/src/doc.rs の ParaStyle)。この項を作るのは主義の変更 — 発注者の判断待ち(Run.style も同じ一件) |
| element / part / settings | 要らない | lxml の露出 |

### Paragraph(13)

| 相手 | 判定 | うちの対応・理由 |
|---|---|---|
| ✔ text / style / runs | ある | |
| ✔ alignment | ある | align。互換層で別名 |
| ✔ clear | 互換層 | text = ""(段落の性質と先頭 run の書式は残る — 本家と同じ定義。自分を返す) |
| ✔ iter_inner_content | 互換層 | runs から(リンクは hyperlinks(足す)が来たら混ぜる) |
| ✔ add_run | 足す | 段落に run を継ぎ足す(済 2026-08-12 夜。書式は**末尾の run を継ぐ** — text の代入が先頭を継ぐのと対。style 引数(文字スタイル)はスタイル定義を持たない主義と衝突するので断る — 発注者判断待ちの一件) |
| ✔ insert_paragraph_before | 足す | 途中に差す(add_paragraph は末尾だけ)(済 2026-08-12 夜。手元の札は位置で指すので、差した後は引き直す — シートの札と同じ作法。style 引数は断る) |
| hyperlinks | 足す | リンクの一件 |
| ✔ paragraph_format | 足す(書式) | 段落書式(行間・字下げ・前後の間隔)(**済(部分)2026-08-12 夜**: alignment(本家の enum も受ける)・line_spacing・page_break_before — 模型が持つ物。space_before / space_after・left_indent は**模型に無い**ので読みは None・書きは正直に断る。模型の indent は段数(1段=全角2字)で、本家の Length との対応は決めてから) |
| part / contains_page_break / rendered_page_breaks | 要らない | 内部・レイアウト依存 |

### Run(15)

| 相手 | 判定 | うちの対応・理由 |
|---|---|---|
| ✔ text / bold / italic / underline / font | ある | ほかに color / size_pt も既にある(相手は font の下に置く — 互換層で両対応。_doc.py の _Font: str の子で `== "MS明朝"` も `.name` も通る) |
| ✔ add_text / clear | 足す | 当初「互換層」としたが、エンジンの Run は**凍った写し**で run 単位の書き口が無い(2026-08-12 doc.rs と突き合わせて正した)。run の書きをエンジンに足してから(**済 2026-08-12 夜**: Run を**位置で引き直す手**に変えた — text / bold / italic / underline / strike / color / size_pt / font の読み書き、add_text は書式を保って継ぎ足し、clear は字だけ消して自分を返す。font の両対応(str と .name)も書きが効くようになった) |
| add_break / add_tab / add_picture | 足す | |
| iter_inner_content | 足す | run の中の改行・タブ・画像も順に返す(add_break の一件) |
| mark_comment_range | 足す | コメントの一件 |
| style | 足す(書式) | 文字スタイル |
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
