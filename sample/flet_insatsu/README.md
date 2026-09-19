# flet_insatsu — docx / xlsx を Flet から刷る

officework が PDF と頁の画像を作り、[flet-printing](https://github.com/aiseed-dev/flet-printing)
が OS の印刷ダイアログ(プリンターの選択・部数・両面)を開きます。

```bash
pip install flet flet-printing officework
flet run --web sample/flet_insatsu/main.py     # プレビューと PDF まで
```

「PDF にする」を押すと、頁ごとの画像が画面に並びます。これはエンジンが
印刷と同じ組み方で描いた物(`save("x.png")`)なので、画面に出た物がそのまま
紙に出ます。拡張は要りません。

flet-printing は Flutter の拡張なので、`flet run` の実行には入っていません。
その場合「印刷…」は押せず、「PDF を開く」でブラウザから刷ります。
印刷ダイアログまで見るには、組みます。

```bash
cd sample/flet_insatsu && flet build macos     # windows / linux / apk / ipa も同じ
```

やっていることは 4 行です。

```python
d = doc.Doc.open("報告書.docx")                          # xlsx なら sheet.Book
d.save("報告書.pdf"); d.save("報告書.png", dpi=96)       # 紙と、頁ごとの絵
printing = Printing(); page.services.append(printing)
await printing.print_pdf(open("報告書.pdf", "rb").read(), name="報告書")
```
