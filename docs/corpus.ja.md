# 他人が書いた xlsx の目録(pyoffice の突き合わせ用)

**現物はこの repo に置かない。** 置き場は `~/xlsx-corpus/`(既定)。
ここに残すのは**目録**(出所・大きさ・sha256 の頭・何を試すための1枚か)と
**突き合わせの結果**だけ。

**置かない理由は2つ。**

1. **免許** — 他人の xlsx を Apache/AGPL のツリーに入れると配れなくなる。
   政府の統計は「出典を書けば使ってよい」形が多いが、**再配布と利用は別の話**で、
   目録から取り直せるなら現物を持ち歩く理由がない
2. **個人情報** — 実物の台帳には人名・口座・単価が入る。**repo に入れたら消せない**

    # 取り直す
    cd ~/xlsx-corpus && curl -sSLO <目録の URL>
    # 通す
    cd ~/dev/officework
    OFFICEWORK_PYTHON=<wheel を入れた仮想環境の python> \
      python3 tools/pyoffice_diff.py ~/xlsx-corpus/*.xlsx

## 何を狙って集めるか

**枚数ではなく書き手の幅。** 同じ Excel で作った台帳を 50 枚集めても、
落ちる穴は1種類しか出ない。

- **Excel** — 新しい物と、古い `.xls` から変換した物の**両方**
- **LibreOffice Calc / Google スプレッドシート**の書き出し
- **ONLYOFFICE 自身**(向こうの読みと突き合わせるのだから外せない)
- **会計・給与ソフト**の書き出し(機械が書く xlsx は人が作る物と癖が違う)
- **統計の表** — 発注者 2026-08-09:「国は csv に移した物が多いので、
  **日銀などの外郭から**取るとよい。**統計局の過去のデータ**もよい」

## 第1便(2026-08-09。10枚)

| ファイル | 大きさ | sha256 の頭 | 出所 |
|---|---|---|---|
| bojsjds.xlsx | 28,679 | `aa7bbe20ed9cf27f` | https://www.boj.or.jp/statistics/sj/sjds.xlsx |
| bojsjfs.xlsx | 22,877 | `18204ee9131c18d3` | https://www.boj.or.jp/statistics/sj/sjfs.xlsx |
| bojsjfy.xlsx | 220,783 | `7021807d502facdf` | https://www.boj.or.jp/statistics/sj/sjfy.xlsx |
| bojsjlong.xlsx | 325,405 | `a5622c695a141d88` | https://www.boj.or.jp/statistics/sj/sjlong.xlsx |
| bojsjmatu.xlsx | 37,499 | `596bb4fe4caf68eb` | https://www.boj.or.jp/statistics/sj/sjmatu.xlsx |
| bojsjpen.xlsx | 11,329 | `60b144e6fac87d01` | https://www.boj.or.jp/statistics/sj/sjpen.xlsx |
| bojsjpre.xlsx | 220,521 | `edf2380ca863ed08` | https://www.boj.or.jp/statistics/sj/sjpre.xlsx |
| stat_fies_t1.xlsx | 43,726 | `58429e46d0330379` | https://www.stat.go.jp/data/kakei/sokuhou/tsuki/zuhyou/fies_t1.xlsx |
| stat_fies_t2.xlsx | 63,454 | `e220fd07bd38c5de` | https://www.stat.go.jp/data/kakei/sokuhou/tsuki/zuhyou/fies_t2.xlsx |
| stat_fies_t3.xlsx | 31,742 | `681fbf75dd3bb881` | https://www.stat.go.jp/data/kakei/sokuhou/tsuki/zuhyou/fies_t3.xlsx |

日銀は資金循環統計、統計局は家計調査。どちらも**日本語の実物**で、
ふりがな(`rPh`)とシートの多い帳面が入っている。

### 結果 — **10枚中8枚で差**(56,487 セルを比べて)

うちが書いた 9 枚では 664 セル全部一致していたのに、**他人の物を通した
とたんに落ちた。** 落ち方は**きれいに2種類**に分かれた。

| | 枚数 | どちらが正しいか |
|---|---|---|
| **ふりがなの連結** | 7 | **うちが正しい**(向こうの欠陥) |
| **シートの取り違え** | 3 | **向こうが正しい**(うちの欠陥。[大]) |

そして **3枚は全部シートが 10 枚以上ある帳面**、7枚は 10 枚未満。
**境目が見事に「シートが 10 枚あるか」で割れた。**

#### ふりがなの連結(向こうの欠陥)

向こうは共有文字列の `<rPh>`(ふりがな)の中の `<t>` を、本文の `<t>` と
**繋げて**返す。`<si><t>実</t><rPh sb="0" eb="1"><t>ジツ</t></rPh></si>` が
`実ジツ` になる。`年、年度、期` は `年、年度、期ネンネンドキ` になる。

**日本語の xlsx でふりがなの入っていない物のほうが珍しい**ので、
これは向こうの実装が日本の帳票で必ず踏む欠陥。officework は `phonetics` に
別に持っていて本文を汚さない(SEKKEI の「日本語の xlsx の宝」)。

**突き合わせでは、この差は「うちの穴」に数えない。** 数え間違えると
やることの一覧が汚れる。

#### シートの取り違え(うちの欠陥。**中身が丸ごと入れ替わる**)

`sheet/src/xlsx.rs:1204-1209` は `xl/worksheets/sheet*.xml` を**文字列で
並べ替えて**、`workbook.xml` の `<sheet>` の並びと**位置で対にしている**。
`xl/_rels/workbook.xml.rels` を**読んでいない**。

穴は二重:

1. **`r:id` を解いていない。** Excel でシートを消したり並べ替えたりすると
   `<sheet r:id="rId23"/>` が `sheet23.xml` を指すとは限らない
2. **文字列の並べ替え。** `sheet10.xml` は `sheet2.xml` より**前**に来る。
   つまり **シートが 10 枚以上ある帳面は、rels が素直でも必ず狂う**

実際に確かめた(`bojsjfy.xlsx`、シート 30 枚):

- `<sheet name="47" sheetId="24" r:id="rId23"/>` → 正しくは `sheet23.xml`。
  その `D4` は `（２）金融機関 (Financial Institutions)`、`D69` は `調整差額`
- officework が掴むのは並べ替えた 23 番目 = **`sheet3.xml`**。
  その `D4` は `（２）金融機関（Financial Institutions）`(括弧が全角)、
  `D69` は `資金過不足`
- **officework が返したのは後者だった。** 推測ではなく一致で確かめた

**黙って別のシートの中身を返す。** 読めないと言うのではなく、
**それらしい別の答えを返す**のがいちばん悪い型。しかもこの状態で保存すると
書き戻し先も狂う。台帳へ [大] で入れた。

**この1件だけで corpus を集めた元は取れた。** 自分で書いた xlsx は
シートが少なく `sheet1..9` しか作らないので、**この穴は永久に出なかった**。

## 第2便(2026-08-09。狙い撃ちで作った14枚 — `tools/corpus_make.py`)

第1便で分かったこと: **統計の表は値・結合・固定枠には強いが、条件付き書式・
入力規則・配列数式・リッチテキストはまず入っていない。** 穴だと分かっている所を
突く1枚を作るほうが、実物を10枚足すより速い。

**種は第三者に書かせる。** `pyoffice_diff.py` は向こうの答えを正解表にするので、
**両方が同じように間違えていると差が出ない**。だから

1. **openpyxl** が書く(書き手その1)
2. それを **LibreOffice** で焼き直す(書き手その2。`lo_*.xlsx`)

7 種 × 2 = 14 枚。**現物は置かない** — `python3 tools/corpus_make.py` で作り直す。

| 種 | 狙い |
|---|---|
| `cond` | 条件付き書式 9 種 + `expression` |
| `valid` | 入力規則 7 種(list/whole/decimal/textLength/date/time/custom) |
| `rich` | リッチテキスト(セルの中で書式が変わる) |
| `arrays` | CSE(昔の配列数式)・名前の定義・式いろいろ |
| `manysheets` | **シート12枚**で、並びが部品の番号と食い違う帳面 |
| `furniture` | 固定枠・結合・列幅・アウトライン・隠し・コメント・リンク・保護・隠しシート・右横書き |
| `formats` | 表示形式(日付・時刻・通貨・％・桁区切り・負の赤・指数・分数) |

### 結果 — 14枚中6枚で差。**3つ新しく出た**

#### 1. シートの取り違えの**最小再現**ができた

`make_manysheets` / `lo_manysheets` が、日銀の 30 枚の帳面と**同じ壊れ方**を
再現した。`表3` に `表30` の中身、`表10` に `表31` の中身…と綺麗にずれる。

**免許の心配のない再現が手に入った。** 直しの受入試験はこれで書ける。

#### 2. 条件付き書式の `expression`(数式で指定)を読めない [中]

`sheet/src/xlsx.rs:1283-1309` が読むのは cellIs / containsText / duplicateValues /
top10 / aboveAverage / dataBar / colorScale / iconSet の 8 種。**`expression` が無い。**

officework は黙って落とさず「条件付き書式(読めない種類。保存で失われる)」と
**報告している**(そこは正しい)。しかし `=MOD(ROW(),2)=0` の縞模様は実物の帳票で
いちばんよく使う条件付き書式で、**読めないでは済まない**。

#### 3. **LibreOffice が書いた名前の定義が、全部死ぬ** [大に近い中]

`=SUM(名前つき)` が `make_arrays.xlsx` では `15.0` になるのに、
**LibreOffice で焼き直した `lo_arrays.xlsx` では `#NAME?`** になる。
中身の `<definedName>` は同じ `式!$A$1:$A$5`。違うのは**属性の数だけ**:

    openpyxl    <definedName name="名前つき">
    LibreOffice <definedName function="false" hidden="false" name="名前つき" vbProcedure="false">

`sheet/src/xlsx.rs:1181` は
`e.attributes().flatten().count() == known` で「単純な名前か」を決めている。
**属性が1つでも余計に付いていたら素通し**(式から引けない)扱いになる。

`function="false"` も `hidden="false"` も `vbProcedure="false"` も**既定値**で、
意味を何も足していない。**LibreOffice はこれを全部の名前に書く。**
つまり **LibreOffice で保存された xlsx の名前つき範囲は、officework では
ひとつも使えない**。数を数えるのではなく、**既定でない値が付いているか**で
見分けるのが正しい。

#### 4. キャッシュ値の無い式 — **決めが要る**(欠陥ではない)

openpyxl は式のセルに `<v>`(前回の答え)を**書かない**。

- **向こう**: 空を返す(ファイルに書いていないものは返さない)
- **うち**: 開いたときに計算して `165.0` を返す。CSE のスピル(`D3:D7`)も
  ちゃんと広がる

**うちのほうが役に立つ**が、サイドカーとして「同じ答えを返す」を目標にすると
これは差として出続ける。**どちらに寄せるかを決める必要がある**
(設計 docs/sekkei/pyoffice.ja.md)。当方の見立ては
**うちに寄せる** — 空欄を返しても呼ぶ側は結局 `recalc_cells` を呼ぶので、
最初から埋まっているほうが往復が減る。
