# flet_insatsu — docx / xlsx を Flet から刷る

officework が PDF を作り、[flet-printing](https://github.com/aiseed-dev/flet-printing)
が OS の印刷ダイアログ(プリンターの選択・部数・両面)を開きます。

```bash
pip install flet flet-printing officework
flet run --web sample/flet_insatsu/main.py     # 画面と PDF まで
```

flet-printing は Flutter の拡張なので、`flet run` の実行には入っていません。
その場合、「印刷…」は代わりに PDF をブラウザで開きます(ブラウザから刷れます)。
印刷ダイアログまで見るには、組みます。

```bash
cd sample/flet_insatsu && flet build macos     # windows / linux / apk / ipa も同じ
```

組んだアプリでは `PdfPreview`(PDF の表示と印刷・共有のボタン)も出ます。

やっていることは 3 行です。

```python
pdf = doc.Doc.open("報告書.docx").to_pdf("報告書.pdf")   # xlsx なら sheet.Book
printing = Printing(); page.services.append(printing)
await printing.print_pdf(open(pdf, "rb").read(), name="報告書")
```
