# officework エンジン

*正本(primary)は英語版: [engine.md](engine.md)。この日本語版は副です。*

`writer` と `calc` は、**1つのエンジンの上に立つ2つのアプリ**です。エンジンは
文書を読み、意味を理解し、**触っていない所を壊さずに**書き戻す部分。画面を
持たないので、どこでも動きます — アプリの中でも、`pip install officework` でも、
そして 2026-08-10 からは**他人が書いた表計算の中**でも。

| | クレート |
|---|---|
| xlsx の読み書き・式・書式 | `sheet` |
| docx の読み書き | `ooxml` |
| 行組み・禁則・字幅・紙面の座標 | `engine`(kumihan) |
| 紙面を紙へ写す | `paper` |

**どれも GPUI に依りません。** これは副作用ではなく、分けた理由そのものです。

## Python から使う

```console
$ pip install officework
```

[Python の手引き](python-manual.ja.md)へ。以下はこのために読む必要はありません
— **別のアプリにエンジンを入れる**話です。

## genoffice の中で動かす

[genoffice](https://github.com/genspark-ai/genoffice) は Electron のオフィス
スイートで、その表計算は Rust の助手と **stdin/stdout で JSON 1行ずつ**、
12 のコマンドで話します。助手の径路は `XLSX_SIDECAR_PATH` から読みます。

**その環境変数が全部です。** officework のエンジンを指せば、genoffice の
表計算が officework で動きます:

```bash
# 先に genoffice 自身の助手の控えを取る — エンジンはそちらへ転送する
cp apps/sheets/native/xlsx-engine/target/release/xlsx-sidecar /tmp/genoffice-sidecar

XLSX_SIDECAR_PATH=/path/to/officework/target/release/xlsx-sidecar \
GENOFFICE_SIDECAR=/tmp/genoffice-sidecar \
  npm run dev -w @genoffice/sheets
```

**genoffice には1行のパッチも要りません。** 変数を外せば元の助手に戻ります。
エンジンは `cargo build --release -p sidecar` で組みます。

**ここで面白いのは、うちのエンジンではありません。** genoffice が継ぎ目を
**継ぎ目であるべき場所に置いていた**こと、そしてその継ぎ目が本当に持ったこと
です — 向こうのビルドに触れない他人が書いた別実装が、そこに嵌まりました。

## 何を差し替えているのか

**読みと計算。書きは差し替えていません。** ここは大事なので、12 のコマンドを
全部並べます:

| コマンド | 誰がやるか |
|---|---|
| `open` `read_range` `read_formula_cells` `read_media` `recalc_cells` | **officework** |
| `close` `cancel` | officework(セッションの世話) |
| `archive_manifest` `read_entries` `scan_entries` `save_archive` `convert_workbook` | **genoffice の助手へ、行をそのまま転送** |

genoffice の Rust は xlsx を「書か」ないからです。TypeScript が XML のパッチを
組み立て、助手は ZIP に当てるだけ — 触らない部品は圧縮済みのまま丸ごと写し、
前後のマニフェストを CRC32 と大きさで照合します。**頼まれていない所をこの道が
どう変えるかを測りました。何も変えません。** だから今日置き換える利益が無く、
5つは素通しにしてあります。

**この形のエンジンは独り立ちしていません。** その5つのために genoffice の助手を
子として起こすので、**そのバイナリの控えが要り**、`GENOFFICE_SIDECAR` がそれを
指している必要があります。エンジン自身を指すと、**自分を無限に呼び続けます**。

## どこまで確かめたか

- **genoffice 自身の助手との突き合わせ** — 実物の帳票 26 枚(日銀の資金循環、
  統計局の家計調査ほか)で、値・式・結合・範囲・書式を欄ごとに比較
- **genoffice 自身の試験をエンジンに当てる** — 21 件中 18 件。残る3件は
  **わざと**です: 向こうが返すピボットの出力範囲は持っていない、書式表の番号を
  向こうは原本の `cellXfs` の索引で振りこちらは振り直す、そして
  **`CELL("filename")` が失敗することを期待している試験**があり、うちは正しく
  答えを返します
- **実機** — 手で開いて動かす。文字・罫線・結合・書式・**依存3段の再計算**・
  名前を付けて保存

**どの段も、上の段には見えない欠陥を出しました。** 突き合わせは**両側に同じ
簡略化を教えた所を構造的に見られません**。試験の schema が `passthrough()` なら、
本番の `strict()` が撥ねる欄は見えません。**5件は実機を起こして初めて出ました。**

## 免許

officework は **AGPL-3.0-or-later**。genoffice は Apache-2.0 で、**genoffice の
物はここで一切再配布していません** — 上の手順が動かすのは、使う人が自分で
組んだ控えです。2つを1つにして配れば、その配り物は AGPL になります。それは
**配る人の判断**で、このページが与えられる許可ではありません。
