# SEKKEI — pyoffice(genoffice に officework エンジンを載せる)

2026-08-09 発注者確定。**製品は2本・エンジンは1つ**(`office-two-shells` の線):

| | officework | pyoffice |
|---|---|---|
| 殻 | GPUI(ネイティブ)。いま作っているもの | [genoffice](https://github.com/genspark-ai/genoffice)(Electron) |
| 中身 | **共通の officework エンジン**(`sheet`) | 同じ |

出来上がりは AGPL-3.0(Apache-2.0 は AGPL に片道で取り込める)。`ee/` は
別免許なので**必ず外す**。`LICENSE` / `NOTICE` を残し、変えた所を記す。

## 向こうの作り(2026-08-09 調査)

xlsx サイドカーは `apps/sheets/native/xlsx-engine/`(Rust 7,070行)。
TypeScript とは **stdin/stdout の JSON 1行ずつ**で話す
(`apps/sheets/src/main/xlsx-sidecar-client.ts:178-185` が `spawn`)。

要求は `{version, requestId, command, …}`、答えは `{version, requestId, ok,
result|error}`。時間切れは 30 秒(書庫の操作だけ 180 秒)。

**要点: 向こうの Rust は xlsx を「書か」ない。**

- 読み: サイドカーが解いて、セル・書式・結合・条件付き書式・入力規則・
  図形・グラフ・スパークライン・コメント・名前の定義まで返す
- 書き: **TypeScript が XML のパッチ計画を作り**(`planCellEditsToXlsx`)、
  Rust は `save_archive(source, target, replacements, removals, additions)` で
  **ZIP の差分を当てるだけ**。変えないエントリは圧縮済みのまま丸ごと写し、
  前後のマニフェストを CRC32 と大きさで照合する
- 計算: `recalc_cells` が `ironcalc`。セッションに Model を residents させる

## だから、そのままでは嵌まらない

officework の `sheet` は**モデルを持って自分で書き戻す**作り(`write_with` で
原本を据え置き)。向こうの「TypeScript がパッチを決め、Rust は ZIP を差し替える」
とは**書き手が違う**。ここを見ずに「エンジンを載せる」と言うと詰む。

## 切る場所 — 12 のコマンドを3つに分ける

| 組 | コマンド | 扱い |
|---|---|---|
| **エンジンの仕事** | `open` `read_range` `read_formula_cells` `read_media` `recalc_cells` | **`sheet` に差し替える**(ここが本題) |
| セッションの世話 | `close` `cancel` | そのまま(`sheet::Book` を id で持つだけ) |
| **ZIP の配管** | `archive_manifest` `read_entries` `scan_entries` `save_archive` `convert_workbook` | **触らない**。エンジンの仕事ではない |

**12 のうち置き換えるのは 5 つだけ。** 残り 7 つは ZIP とセッションの世話で、
向こうの実装が既に堅い(マニフェスト照合まである)。ここを書き直す理由はない。

## 進め方 — 通信の言葉は1文字も変えない

**同じ 12 コマンドを、同じ JSON の形で喋る。** そうすればサイドカーの
実行ファイルを差し替えるだけで済み、TypeScript 側は何も知らなくていい。
A/B で戻せるのも大きい(環境変数でどちらを起動するか切り替える)。

1. **読みから**。`open` / `read_range` / `read_formula_cells` を `sheet` で
   実装し、向こうの実装と**同じ入力に同じ答えを返すか**を突き合わせる
   (実物の xlsx を何十枚か通して JSON を diff する試験を先に作る)
2. **計算**。`recalc_cells` を `sheet::calc` に。ironcalc が外れる
3. **書き**は最後。ここだけは TypeScript 側の作り替えが要るので、1と2で
   エンジンの信用ができてから。**急がない** — 向こうの ZIP 差分保存は
   よく出来ていて、当面そのままで困らない

## 埋まっていない穴(先に知っておく)

1. **リッチテキスト**。向こうは共有文字列の中で色が変わる run を読んで返す。
   officework は**セルの中で書式が変わることをモデルに持たない**
   (2026-08-09 の決定 — 代わりにマークダウンで描く)。ここは
   「読んで返すだけ」の形で足すか、平文に潰して報告するかを決める必要がある
2. **グラフとドローイングの深さ**。向こうは chart XML の系列まで構造化して
   返す。officework は図形・画像は持つが、グラフは matplotlib で画像として
   置く作りなので、**返せる中身が違う**
3. **セッションの居座り**。向こうは Model をセッションに残し、ファイルの
   mtime と大きさで自動的に捨てる。officework 側も同じ仕組みを用意する

この3つは「難しい」のではなく「**返す中身が違う**」問題。1 は正直に潰し、
2 は当面向こうの実装を残す(ZIP 配管の組に置いてよい)、3 は素直に作る。

## 最初の一手

**突き合わせの試験を先に書く。** 実物の xlsx を両方のサイドカーに通し、
`open` と `read_range` の JSON を比べる。ここで差が出た所が、そのまま
やることの一覧になる。**実装から入らない** — 何を満たすべきかが数字で
出てから作る。
