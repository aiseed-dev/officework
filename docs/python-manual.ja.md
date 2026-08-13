# Python の手引き — 配列と API

*正本(primary)は英語版: [python-manual.md](python-manual.md)。この日本語版は副です。*

ボタンの使い方は [calc](calc-manual.ja.md) / [writer](writer-manual.ja.md) の手引きに。
ここは**コードを書く人のための1冊** — とくに「範囲⇄配列」のやり取りは
画面から見えないので、ここが正本。全部この機械で実測してある。

## コードはファイル、データはファイル — 混ぜない

**交換されるファイルはデータだけ**(2026-08-09 発注者確定)。xlsm のように
データとプログラムを1つのファイルに入れる仕組みは持たない。関数も手続きも
`~/.config/office/plugins/*.py` にあり、**受け取ったブックには1バイトも
コードが入っていない**。

- 人から受け取るのは表(データ)。**処理は自分のコードでする**
- だから `xl/joPython.xml`(ブック搭載コード)は**廃止**した。古いブックの
  コードは読んで見せるが実行せず、保存で消える(開くときに報告する。
  `@export 名前` で .py に取り出せる)
- コードが自分のものだけになったので、**サンドボックス(bubblewrap)は
  着せない**。`@名前 net` の区別も無くなった
- デコレータ(`@xw.func` / `@xw.ret`)も要らない。**普通の `def` を書けばいい**

## Python が動く場所と、渡される束縛

| 場所 | 書き方 | 束縛 |
|---|---|---|
| セルの関数(UDF) | `=倍(A1)` — plugins の `def 倍(x)` を呼ぶ | 引数が値で渡る(下記) |
| 手続き | `@モジュール` / `@モジュール.関数` | 自分で `xw.Book.caller()` を呼ぶ |
| 外(Jupyter など) | `from officework import calc as xw` | 同上 |
| calc: データ > Python(一行 / .py) | パネルに直接書く | `b` = ブック、`s` = いまのシート |
| calc / writer: マクロ・プラグイン | — | calc は `b`/`s`、**writer は `d` = python-docx の文書** |
| writer: ページの Python(HTML) | — | `form` = 記入欄の名前→値の辞書 |

**手続きと外からのコードは、動いている calc をそのまま操る**(一時ファイルの
複製ではない — 記事の言う「ファイルではなく Excel そのものを操作する」)。
1つの手続きが何回セルを書いても **Ctrl+Z 一回**で手続きの前に戻る。

データ > Python の一行と writer 側は、いまも複製の上で走る
(失敗しても表・文書は無傷、成功したら結果が1手として入る)。

## `officework.sheet` の API

エンジンは PyPI に **`officework`** の名で出ています。xlsx のエンジンは
`sheet` の副モジュールで、**アプリは要りません**。

```console
$ pip install officework
```

```python
from officework import sheet            # calc の中では import 済みで b, s が来る
b = sheet.Book.open("帳票.xlsx")
s = b["シート名"]                        # 番号でも: b[0]
b.sheet_names                           # ['見積書', …]
b.add_sheet("新しいシート")              # 同名があればエラー
b.recalc()                              # 式を計算し直す(値を読む前に)
b.save("out.xlsx")                      # 原本の部品は据え置き
b.unsupported                           # 読めなかった部品の一覧(空 = 全部読めた)
```

### セルの読み書き

```python
s["A1"]            # 読み: 数は float、文字は str、☑/☐ は bool、式セルは計算値
s.formula("E2")    # 式そのもの("=SUM(B2:D2)"。式でなければ None)
s.display("E2")    # 表示の文字("238"。表示形式を通した見た目)
s["A1"] = 100      # 書き: 数
s["A1"] = "文字"   #        文字
s["A1"] = True     #        真偽(calc では ☑/☐ に見える)
s["A1"] = "=B1*C1" #        式("=" で始まる文字列)
s["A1"] = date(2026, 8, 5)  # datetime.date/datetime/time → Excel の通し番号
s["A1"] = None     #        消す
```

- **書式は据え置き** — 値を入れても罫線・結合・表示形式は変わらない
- 空のセルは **None か ""** で返る(触ったことのないセルは None。
  "" は**原本に空文字のセルがあった**とき — こちらから `s["A1"] = ""` と
  書くのは消すのと同じで、読み直すと None。どちらも偽なので
  `if s["A1"]:` で足りるが、厳密には `s["A1"] in (None, "")` で見る)

### 配列(範囲)のやり取り — ここが本題

**範囲の添字は無い**(`s["A2:C3"]` はエラー)。**2次元の一括代入も無い**
(`s["A1"] = [[…]]` はエラー)。配列はこう扱う:

```python
# 読み: values() が使っている広さ全体の2次元リスト(行×列、0 始まり)
rows, cols = s.shape          # (10, 6) — shape は属性(() を付けない)
v = s.values()                # v[0] = 1行目(見出し)、v[1][1] = B2 の値
tbl = [r[0:3] for r in v[1:6]]   # A2:C6 を切り出す

# 書き: ループで1セルずつ(行番号は A1 表記なので 1 始まりに注意)
data = [["ペン", 10, 150], ["ノート", 5, 180]]
for i, row in enumerate(data):
    n = 2 + i                              # 2行目から
    s[f"A{n}"], s[f"B{n}"], s[f"C{n}"] = row
    s[f"D{n}"] = f"=B{n}*C{n}"             # 式も文字列で入れる
b.recalc()
```

### polars との往復

```python
import polars as pl
# シート → DataFrame(1行目を見出しに)
v = s.values()
df = pl.DataFrame({h: [r[i] for r in v[1:]] for i, h in enumerate(v[0])})

# DataFrame → シート(見出しの下へ)
for i, row in enumerate(df.rows()):
    for j, val in enumerate(row):
        s[f"{chr(65 + j)}{2 + i}"] = val
```

集計・結合・絞り込みは polars 側でやるのが分業の流儀
(シートは帳票の形、データの計算は Python)。

## `officework.doc` の API

同じ wheel、同じ約束を docx で。`doc` の副モジュールです。`Doc.open` が
**元のバイトを抱えたまま**持ち、`save` は変えた所だけ書き戻すので、
様式・ヘッダー・フッター・図形・変更履歴がそのまま通ります。
python-docx が約束できないのはここです。

```python
from officework import doc

d = doc.Doc.open("報告書.docx")
d.unsupported          # [(何が, 何個)] — 読めなかった物。まずここを見る
d.paragraphs           # 本文の段落(表の中の段落はここに入らない)
d[3]                   # 本文の4つ目の段落。d[-1] も引ける
len(d)                 # 本文の段落の数
d.text                 # 本文を "\n" で繋いだ1本の字
d.header, d.footer     # 読むだけ。ページ番号は "#"、総ページ数は "##" で出る
d.tables               # 表(文書に出てくる順)
d.add_paragraph("…")   # 本文の末尾に足す
d.save("out.docx")
```

**何より先に `d.unsupported` を読んでください。** 空なら全部読めています。
空でなければ何が読めなかったかが出ます — **そこに出た物も、保存では原本から
持ち越されます**。「読めなかった」と「落とした」は別の話です。

### 段落

```python
p = d[3]
p.text = "差し替え"        # 字を替える。見出しの段は見出しのまま、寄せもそのまま
p.text                    # run をつないだ字
p.replace("旧", "新")      # → 置き換えた数。run の切れ目は全部残る
p.runs                    # [Run] — 読み書き両方: .text .bold .italic .underline
                          #   .strike .color .size_pt .font .style .hyperlink
p.runs[0].bold = True     # 書式は run 単位で書ける(2026-08-12 から)
p.add_run("続き")          # 末尾に run を継ぎ足す(書式は末尾の run を継ぐ)
p.style                   # "body" / "heading1"〜"heading9" / "toc1"… / "tof"
p.align                   # "left" | "center" | "right" | "justify" | "distribute"
p.in_table                # 表のセルの中の段落なら True
```

**字を替える口は2つあり、同じ道具ではありません。**

`p.text = "…"` は段落を丸ごと置き替え、新しい字は**先頭 run の書式**を継ぎます。
これは writer で表のセルを編集したときと同じ規則なので、Python とアプリの
結果が食い違いません。ただし鈍器です — 「請求先: 」が素で「株式会社甲」が
太字の段落に代入すると、**全部が素になります**。

`p.replace(old, new)`(文書ぜんぶなら `d.replace(...)`)は run の中で編集するので、
書式の分かれ目が全部残ります。run を跨いだ語も拾います — Word は見た目に
理由もなく「旧社名」を 旧/社名 に割るので、これは効きます。
**帳票の差し込みは `replace` を使ってください。**

```python
d.find("旧社名")                    # それを含む段落。本文も表のセルも同じに拾う
d.replace("旧社名", "新社名")        # → 置き換えた数
```

### 名前で差し込む(記入欄)

文字を探して差し替えるより確かなのが、**名前つき記入欄**
(コンテンツコントロール。writer の 挿入 > 記入欄 で置き、
「記入欄に名前を付ける」で名前 = docx の w:tag が付く)です。
本文でも表のセルの中でも同じに引けます:

```python
d.fields()                          # [(名前, いまの値)] の一覧
d.fill("宛先", "日本フネン株式会社")  # その名前の欄すべてに書く → 書いた数
                                    # (0 なら欄が無い — 黙って成功にしない)
d.extract("宛先")                   # 最初の欄の値。無ければ None
```

書式は欄の先頭のものが残ります(太字の欄は太字のまま)。
**writer のマクロの `fill` / `extract` / `fields` と同じ言葉**なので、
マクロで書いた台本の知識がそのまま使えます。全部この機械で実測してあります。

### 表

```python
t = d.tables[0]
t.shape                # (行数, いちばん長い行の列数)
len(t), t.rows         # 行
t[1][2]                # 表・行・セル
t[1][2].text = "…"     # 改行を入れるとセルの中で段落が分かれる
t.values()             # list[list[str]] — そのまま polars に渡せる
t[1][2].paragraphs     # そのセルの段落(Paragraph として)
```

### やらないことと、報告に出るが失われないもの

本文として読めるのは段落と表です(節は `d.sections`、コメントは
`d.comments`、画像の一件は `d.inline_shapes` で別に引ける)。

脚注・数式は **`d.unsupported` に出ますが、保存で失われません** —
報告の文言がそのまま言います(「脚注・文末脚注の印(本文には出ないが、
保存で残る)」「数式(段落の頭に寄るが、保存で残る)」)。unsupported は
「読めなかった」の帳簿であって「捨てた」の帳簿ではない、が読み方です。

既存の数式は本文の text に出ません(原文のまま持ち越すだけ)。
**新しく数式を書くなら `officework.tex`**(下の節)— LaTeX で受けて
絵に組み、原文ごと文書に入ります。

## よその語彙でも書ける — openpyxl・xlwings・python-docx

手持ちのコードと、頭に入っている語彙を捨てなくてよい(2026-08-12)。
**API と試験は写す・実装は写さない**が方針(docs/sekkei/python.ja.md)で、
在庫は台帳 [docs/pysheet-gokan.ja.md](pysheet-gokan.ja.md) — 3ライブラリの
中核 324 メンバーを1件ずつ判定してある(できる物・作る物・作らない物と
その理由)。ここに書くのは**いま動く物だけ**、全部この機械で実測済み。

そして書式据え置きと再計算という上位分は、どの語彙で書いても付いてくる。

### openpyxl の語彙(officework.sheet)

```python
from officework import sheet
wb = sheet.Book.open("売上台帳.xlsx")
ws = wb.active                        # 先頭のシート
ws.title, ws.max_row, ws.max_column   # ('売上台帳', 37, 6)
ws.dimensions                         # 'A1:F37'
ws.cell(2, 3).value                   # 'ボールペン(黒)'(行・列は1始まり)
ws.append(["8月", "筆記具", "万年筆", 1, 5000, 5000])   # 末尾に1行
for row in ws.iter_rows(min_row=2, max_row=3, values_only=True):
    print(row)                        # ('4月', '筆記具', 'ボールペン(黒)', 12.0, …)
ws2 = wb.create_sheet()               # 名前は自動(Sheet, Sheet1, …)
wb.copy_worksheet(ws)                 # 複製 — 中身・書式・結合・列幅ごと
wb.remove(ws2)                        # 最後の1枚は抜けない(正直に断る)
wb.save("out.xlsx")
```

- まだ何も無い所も `ws.cell(50, 1)` で**参照だけの Cell** が返る
  (value は None。書けばその場に入る — openpyxl と同じ感触)
- `insert_rows` / `delete_rows` / `insert_cols` / `delete_cols` は
  `amount=` つき。`merged_cell_ranges`・`freeze_panes` も通る
- **こちらの流儀もそのまま生きている**: `ws["A1"]` は今までどおり
  **値**を返す(openpyxl は Cell を返す — ここだけ流儀が違う。
  Cell が欲しいときは `cell()` で)
- openpyxl が読めるかは向こうの試験でも確かめてある — こちらが書いた
  xlsx を openpyxl がそのまま読む(**こちらが計算した値も** —
  openpyxl 自身は式を計算できない)

書式と印刷も openpyxl の形で通る(2026-08-12〜13 に台帳 324 件を閉じた):

```python
from officework.sheet import Font, Border, Side, PatternFill, Alignment
ws.cell(1, 1).font = Font(bold=True, size=14) # openpyxl の実物を渡してもよい
                                              # (ws["A1"] は値を返す流儀なので cell() で)
ws.column_dimensions["A"].width = 20          # 列幅(字数)・行高・hidden
ws.print_title_rows = "1:1"                   # 毎ページ繰り返す見出し(列は "A:A")
ws.freeze_panes = "B2"
ws.add_table(...)                             # 表 — =SUM(明細[金額]) が計算まで効く
wb.add_named_style(...)                       # 名前付き様式も運ぶ
```

入力規則・名前付き範囲・グループ化・画像(`add_image`)・ヘッダー/フッター
(奇数・偶数・先頭の別まで)・1904 起点・`move_range`(式の参照が付いて動く)
も同じ流儀で。**何がどこまであるかの正本は台帳**
([pysheet-gokan.ja.md](pysheet-gokan.ja.md) — 324 件に判定と理由)。

### xlwings の語彙(officework.calc — 動いている calc へ)

参照の算術が入った。**繋がっていなくても算術は使える**(実測):

```python
from officework import calc as xw
xw.Range("B2").address                    # '$B$2'
xw.Range("B2").offset(1, 2).address       # '$D$3'
xw.Range("B2").resize(3, 2).address       # '$B$2:$C$4'
xw.Range("B2:D5").last_cell.address       # '$D$5'
xw.Range("A1").current_region             # 地続きの表全体(expand の同族)

b = xw.Book.attach()                      # 動いている calc に(caller() も同じ)
b.sheets.active["A1"].value = 42
```

calc が居なければ黙って何かの振りをせず、そう言う:
`OfficeworkError: calc に繋がりません(…/officework/calc.sock: Connection refused)`

### python-docx の語彙(officework.doc)

```python
from officework import doc
d = doc.Doc.open("報告書.docx")
t = d.tables[0]
t.cell(0, 1).text                    # '件名'
[c.text for c in t.row_cells(1)]     # ['7月3日', '外壁塗装工事', '株式会社みほん商事', '640,200円']
len(t.columns)                       # 4
p = d[3]
p.runs[0].font == "MS明朝"           # font は文字列としても比べられ、
p.runs[0].font.name                  # .name でも引ける(書体が run に明示されていなければ None)
```

段落には `clear` / `iter_inner_content` も。**Run は位置で引き直す手**
(python-docx と同じ使い方 — `r.bold = True` も `r.add_text("続き")` も効く。
2026-08-12 に「凍った写し」から改めた)。段落の `text` 代入や `replace` で
run の並びが変わった後は、`p.runs` から引き直すこと。

書く方の口も一通りある — `d.add_heading(字, level)`(1〜3。0=Title は
持たないので正直に断る)、`d.add_paragraph(字, style=)`、`d.add_picture`、
`d.add_section()`、`d.add_table(rows, cols)`、`d.styles.add_style`、
`p.add_comment(字)`(段落単位)、`d.core_properties`。在庫の全量と
判定の理由は台帳([pysheet-gokan.ja.md](pysheet-gokan.ja.md))に。

## 数式を組む(officework.tex)

数式は **LaTeX で受けて絵に組む**(2026-08-13)。組版は自前で書かず、
TeX(pdflatex)があればそちらで、無ければ matplotlib の mathtext で組む。
**要るのは matplotlib だけ**で、TeX はあれば品質が上がる(行列の列まで揃う)。

```python
from officework import tex
tex.kumi_kata()                       # "tex" | "mathtext" | None(今なにで組めるか)
svg = tex.to_svg(r"\frac{a+b}{2}")    # bytes。字は輪郭になるので書体不要
png, w_mm, h_mm = tex.to_png(r"\sqrt{x^2+y^2}", size_pt=11)  # 文書に入れるのはこちら
```

- 組めない式は **`tex.Muri` で断る**(理由つき — 黙って空の絵を返さない)
- mathtext は LaTeX の**部分集合**。`\begin{matrix}` 系は `\substack` に
  寄せて組む(**列は揃わない** — TeX があれば揃う)
- SymPy から起こす `from_sympy()` もあるが、**式は書き直される**
  (`(a+b)/2` → `a/2 + b/2`)。書いたとおりの見た目が要るなら LaTeX を直に
- writer で数式を挿すと(挿入 > 方程式)、**絵と LaTeX の原文が二枚組**で
  docx に入る — 渡した先の Word では絵として見え、officework では式として直せる

## セルの関数(UDF)と配列

`~/.config/office/plugins/道具.py` に普通に `def` を書けば、その名前で
セルから呼べる。**デコレータは要らない**(`@xw.func` も
`@xw.ret(expand='table')` も不要 — 返り値の形が広がり方を決める)。

```
=集計(A1:B10, 100, "甲")
```

- 関数の名前は**日本語でよい**(`=集計(A1:B10)`)
- 引数の範囲は**行×列の2次元リスト**(値。1セルはスカラ)で def に渡る
- 返り値: スカラ → そのセルへ / **1次元リスト → 下へ展開** /
  **2次元リスト → 右下へスピル**。展開先に他人のデータがあれば
  `#SPILL!` で止まる(潰さない)
- **引数が変われば裏で計算し直す**(`@計算` を押さなくていい)。計算は
  別スレッドでまとめて回し、答えが揃ってから1手で書き戻す
- 組み込みの関数名(SUM など)は譲らない。同じ名前の `def` は見送る
- 同じ名前が2つの .py にあるときだけ `=道具.集計(…)` とモジュール名を付ける
- 古い書き方 `=PY("集計", …)` も動く

```python
def 集計(r, 上限, 種別):        # r = [[行1列1, 行1列2], [行2列1, …], …]
    hit = [row for row in r if row[0] == 種別 and row[1] <= 上限]
    return [[row[0], row[1]] for row in hit]   # 2次元 → スピル
```

## writer のマクロ(d = python-docx)

**専用の手引き: [writer-macro-manual.ja.md](writer-macro-manual.ja.md)** —
名前つき記入欄(`fill` / `extract` / `fields`)、雛形(`render` /
`tpl_fields`、docxtpl)、サンドボックスの中身、AI に台本を書かせる話まで。

```python
# d が python-docx の Document。API は python-docx の公式文書のまま
d.paragraphs[12].runs[0].text = "商号 例示工務店"
for r in d.paragraphs[12].runs[1:]:
    r.text = ""                  # 先頭ランに書き、残りを空に(書式が残る作法)
fill("代表・商号", "例示工務店")  # ラベル走査より名前つき記入欄 — 手引き参照
```

保存は writer 側がやる(スクリプトの中で d.save は不要)。

## ページの Python(HTML の form)

```python
# form = 記入欄の名前 → 値の辞書。返した値が紙面に書き戻る
qty = int(form.get("qty") or 0)
form["total"] = qty * 150
```

## 実行の枠

- **サンドボックスは着せない**(2026-08-09 に外した)。plugins は自分で
  据えたコードなので、ファイルもネットワークも普通に使える。
  `@名前 net` の区別も無くなった(打つと「要らなくなりました」と言う)
- 時間制限つき(手続き60秒・セルの関数30秒)。超えたら止めてそう言う
- 機械に入っているライブラリ(polars・scipy・matplotlib 等)は使える
- print した文字はステータスバーに出る(進み具合や件数はそこで言う)
- データ > Python の一行と writer 側は、いまも複製の上で走る
  (サンドボックスがあれば使う)

## AI と書く — 共働の手引き

マクロは自分で書かなくてよい。**AI(Claude 等)に頼んで、検分して、
サンドボックスで回す**のがこのソフトの想定する形 — VBA の移し替えもこの道で
やる。ただし AI は世間の常識(openpyxl・xlwings・VBA)で書いてくるので、
**この家の流儀を最初に渡す**こと。下の枠をそのまま貼ればいい。

### AI への申し送り(コピペ用)

```
次の環境で動く Python を書いてください。

【calc のマクロ】b(ブック)と s(いまのシート)が束縛済み。
- 読み: s["A1"](数=float・文字=str・☑=bool・式セルは計算値。
  空は None か ""。式が要るなら s.formula("A1")、見た目は s.display("A1"))
- 全面の読み: s.values()(行×列の2次元リスト。0 始まり)、広さは
  s.shape(属性。() を付けない)
- 書き: s["A1"] = 値。式は "=B1*C1" の文字列。消すのは None
- 【重要】範囲の添字(s["A2:C3"])と2次元の一括代入は無い —
  書き込みはループで1セルずつ。行番号は A1 表記で1始まり
- 式を入れたら b.recalc() してから値を読む
- b.save() は呼ばない(適用はアプリ側の仕事)。print は画面のステータスバーに出る
- 書式(罫線・結合・表示形式)は値を入れても壊れない — 触らなくてよい

【writer のマクロ】d(python-docx の Document)が束縛済み。
python-docx の普通の API が使える。d.save() は呼ばない。
様式の欄に書くときは「先頭ランに書き、残りのランを空にする」
(p.runs[0].text = 値; 以降の run は ""; — 段落の書式が残る作法)

【手続き(plugins の .py)】~/.config/office/plugins/名前.py に置き、
`@名前` か `@名前.関数` で動かす。動いている calc をそのまま操る:
  from officework import calc as xw
  def 貼り付け():
      s = xw.Book.caller().sheets.active
      s["A1"].value = [["受信", "名前"], ["2026-08-09", "山田"]]
デコレータは要らない(普通の def)。何回書いても Ctrl+Z 一回で戻る。

【実行環境】素の Python(サンドボックスは着せない — plugins は利用者が
自分で据えたコードだから)。ファイルもネットワークも普通に使える。
polars・scipy・matplotlib も使える。

【セルの関数(UDF)を書くとき】plugins の .py に普通に def を書けば
`=名前(A1:B9)` で呼べる(名前は日本語でよい)。範囲の引数は行×列の
2次元リスト(値)で来る。返りはスカラ / 1次元(縦に展開)/
2次元(右下へスピル)。引数が変われば自動で計算し直される。
```

これに**やりたいことを日本語で**添える(シート名・見出しの行・
何をどうしたいか)。表の形が要るなら `s.values()[0]`(見出し行)を
貼ると話が早い。

### 受け取ったコードの検分

**サンドボックスは着せない。** plugins に置く .py は利用者が自分で据えた
コードで、VS Code で書いたスクリプトを走らせるのと同じ扱いにしてある。
だから**置く前に読む**のが唯一の門になる:

1. **どこに書くか**(壊してほしくない列・行に書いていないか)
2. **何を消すか**(None 代入・行削除があるか)
3. **外と何をするか**(通信・ファイルの読み書き。宛先は意図した所か)

残る安全網は undo — 手続きが何回書いても **Ctrl+Z 一回**で手続きの前に
戻る。だから**まず回して、結果を見て、気に入らなければ戻す**が正しい試し方。
検分の済んだものを `~/.config/office/plugins/名前.py` に置く(以後 `@名前`)。
**ブックにコードは載せられない** — データとプログラムは別のファイル
(2026-08-09 確定)。

### VBA の移し替え

現場の .xlsm の VBA は、コードを取り出して(`oletools` の `olevba` が
定番)AI に貼り、「上の申し送りの環境で同じ仕事をする Python に」と
頼む。Range/Cells のループは s[f"A{n}"] のループに、Worksheet は
b["名前"] に、だいたい素直に写る。**移した後に元の VBA と同じ入力で
突き合わせる**こと(答え合わせまでが移し替え)。

### 頼み方の実例

> (申し送りを貼った後で)
> シート「受注台帳」: 1行目が見出し(受付・社名・品番・品名・数量・
> 単価・金額・発送済)。社名ごとの金額の合計を J5 から下に
> 「社名, 合計」で書き出すマクロを書いて。

AI が書く → 検分(J 列は空きか・net 不要)→ 実行 → 結果を見る →
必要なら「合計の大きい順に」と続きを頼む。この回転が共働の基本形。

## 実例(そのまま読める見本)

**エンジンだけ(pip install officework — アプリ不要)。**
6本とも実測してから置いてある(sample/README.md に出力の数字ごと):

- [sample/差し込み.py](../sample/差し込み.py) — 見積書に宛名と数量を差し込む。
  書式据え置き・式は合計まで追従
- [sample/量産.py](../sample/量産.py) — 1つの型紙から宛先3件の見積書
- [sample/集計.py](../sample/集計.py) — `values()` で36行を一気読み → 区分別集計
- [sample/差し替え.py](../sample/差し替え.py) — docx の `replace()`(書式を保つ置換)
- [sample/表の吸い上げ.py](../sample/表の吸い上げ.py) — 文書の表 → CSV
- [sample/点検.py](../sample/点検.py) — フォルダ一括で `unsupported` を数える検品

**アプリと組む(plugins の手続き)。**

- [templates/](../templates/README.md) — 問い合わせ台帳(`@取り込み` の
  CSV 取り込み・=PY の状態集計)ほか
- [sample/注文書.xlsx](../sample/README.md) — マスタの入れ替え(`@更新`)と
  JSON の送信(`@送信`)
- [sample/受注台帳.xlsx](../sample/README.md) — 取り込みの控え(K2)で
  重複を防ぐ増分の取り込み
