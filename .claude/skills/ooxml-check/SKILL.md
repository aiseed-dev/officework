---
name: ooxml-check
description: docx や xlsx の表示・印刷が Word・Excel と違うとき、直す前に ECMA-376(OOXML)の本文と schema で要素・属性・値を確かめる手順。Word・Excel との比較、読み込みの直し、「どちらが正しいか」を決めるときに使う。
---

# OOXML で確かめる

Word・Excel との違いは、直す前に OOXML で確かめます。推測や、PDF から測った
数字だけでは直しません。Office と OOXML が違うときは OOXML で決め、こちらが
正しい所は残します(2026-09-23 発注者)。

## 規約の置き場所

`~/Documents/officework-cmp/ecma376/` に ECMA-376 第 5 版(2016 年)があります。

| ファイル | 中身 |
|---|---|
| `part1.txt` | Part 1(本体)の本文。PDF から取り出した字 |
| `part4.txt` | Part 4(Transitional の追加と置き換え)の本文 |
| `xsd-strict/` | Strict の schema(`sml.xsd`・`wml.xsd`・`dml-main.xsd` など) |
| `xsd-trans/` | Transitional の schema |

無いときは、発注者の許可を得てから ecma-international.org の
`ECMA-376-1_5th_edition_december_2016.zip` と `ECMA-376-4_…zip` を落とし、
pypdfium2 で本文を字にします(`.venv/bin/python` で 10 秒ほど)。

Office が規約からどう外れているかは、Microsoft の MS-OI29500
(learn.microsoft.com の open specifications)に項ごとに書いてあります。

## 手順

1. **ファイルの中を見ます。** 違いの出ている所の XML を取り出します。
   ```bash
   unzip -p 文書.docx word/document.xml | grep -o '<w:p .\{0,400\}' | head
   unzip -p 表.xlsx xl/worksheets/sheet1.xml | grep -o '<pageSetup[^>]*>'
   ```
2. **本文の項を探します。** 見出しは「番号 名前」の形です。
   ```bash
   grep -n '^17.3.1.15 keepNext' part1.txt      # 項の本文の場所
   grep -n 'ST_CellType' xsd-trans/sml.xsd       # schema の定義
   ```
3. **Transitional の置き換えを見ます。** Excel と Word がふだん書くのは
   Transitional です。Part 4 に「Modified content for … (Part 1, §…)」が
   あれば、Part 1 のその項は置き換わっています。
   ```bash
   grep -n 'Modified content for' part4.txt
   ```
4. **Strict と Transitional の違いを見ます。** 型や値が Strict にしか無いことが
   あります(例:セルの型 `d` は Strict の schema にだけあります)。
5. **規定と参考を分けます。** 付属書 L のように「(informative)」とある所は
   参考です。本文と食い違ったら本文に従います。
6. **決めます。**
   - OOXML に書いてあり、こちらが違う → OOXML に合わせて直します。
   - OOXML に書いてあり、Office が違う → こちらは OOXML のままにし、発注者に
     報告します(Office に合わせるかは発注者が決めます)。
   - OOXML に書いていない → 推測で直さず、分かったことと案を発注者に出します。
7. **記録します。** コミットと設計の文書(`docs/sekkei/`)に、項の番号と
   決めた理由を書きます。

## 直した後の確かめ方

直した文書だけを、Word・Excel の PDF と行ごとに比べます。全件の突き合わせは
毎回はしません。報告に全件の数を出しません。

```bash
# こちらの PDF
cargo run --release -p paper --example docx_pdf -- 元.docx officework.pdf
DYLD_FALLBACK_LIBRARY_PATH=$HOME/miniforge3/lib .venv/bin/python -c \
  "from officework import sheet; sheet.Book.open('表.xlsx').to_pdf('officework.pdf')"

# Word・Excel の PDF(開くのは発注者の許可を得てから。開いた文書は閉じる)
python3 tools/ms_pdf.py 元.docx office.pdf

# 行と位置の比較
.venv/bin/python tools/ms_compare.py officework.pdf office.pdf
```

- `.venv` の `officework` は `pysheet/officework/_sheet.abi3.so` を使います。
  エンジンを直した後は、`pysheet` の下で `../.venv/bin/maturin develop --release`
  を回して作り直します。作り直さないと、古いエンジンで PDF になります。
- 公式の PDF(厚労省の様式など)があれば、それも比べる相手にします。
  PDF のメタデータ(Creator)で、どの Office で作ったかを確かめます。
- xlsx は、作った Excel が Windows か Mac かで列の幅が変わります。読み込みの
  オプション(`ReadOptions` の `platform`)を合わせてから比べます。
