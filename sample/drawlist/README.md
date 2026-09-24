# 描画一覧の見本

officework のエンジンが組んだページを、描画一覧(`Book.draw_list()`)として受け取り、
ほかの仕組みで描く見本です。形の決まりは `docs/sekkei/drawlist.ja.adoc` にあります。

| ファイル | 内容 |
|---|---|
| `pil_draw.py` | 描画一覧を Pillow で PNG に描きます。位置や大きさを計算せず、一覧のとおりに描きます |
| `flet_form.py` | 描画一覧を Flet の Canvas に描き、欄をクリックして直せる部品です(下書き) |
| `main.py` | 厚労省の履歴書を `flet_form.py` で表示する見本のアプリです |

## Pillow で描く

```
python pil_draw.py drawlist.json out.png 96
```

最後の数は解像度(dpi)です。Raqm の無い Pillow は字の送りを 1 字ずつ整数の画素に
丸めるので、長い字の並びでは後ろの字ほどずれます。PDF と比べるときは、4 倍の
解像度で描いてから縮めてください。

## Flet の部品

```
flet run main.py
```

欄をクリックすると入力欄が出ます。Enter を押すか、ほかの所をクリックすると、
値がデータに入り、ページを描き直します。「PDF に書き出す」は同じページを PDF に
します。

書体は `assets/fonts` に写して登録します。Flet の Canvas は塗りの偶奇の決まりと
切り抜きを持たないので、描画一覧の `even_odd` と `clip` は使いません。

Flet 1.0.1(`flet` と `flet-desktop`)で、履歴書の表示と写真を画面で確かめました。
欄をクリックして直す流れは、画面を開かずに部品を組んで確かめました。
