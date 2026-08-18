# 包み方と、試し方

## 作る

```
packaging/make-linux.sh            # tar.gz と .deb(Python 同梱)
packaging/make-linux.sh --no-py    # Python を同梱しない(小さい)
```

出来上がりは `packaging/out/` に置かれます(git には入れません)。
初回は CPython(約36MB)を落とすので少し待ちます — 2回目からは
`packaging/cache/` の物を使うので速いです。

大きさの目安(0.1.0-alpha):

| 形 | 大きさ | 中身 |
|---|---|---|
| tar.gz | 77MB | calc・writer・Python 3.14・見本の .py・手引き |
| .deb | 59MB | 同じ物を /opt/officework へ。`officework-calc` で起動 |

## 試す

**tar.gz(入れずに試す)**

```
tar xzf officework-0.1.0-alpha-linux-x86_64.tar.gz
cd officework-0.1.0-alpha-linux-x86_64
./calc            # 表計算
./writer          # 文書
```

**.deb(入れて試す)**

```
sudo dpkg -i officework_0.1.0-alpha_amd64.deb
sudo apt-get -f install     # 足りない物があれば
officework-calc
```

アプリの一覧にも出ます(Office の下)。消すときは `sudo apt remove officework`。

## Python のマクロを試す

見本を置き場にコピーして、calc を開き直します。

```
mkdir -p ~/.config/officework/plugins
cp share/officework/plugins/*.py ~/.config/officework/plugins/     # tar.gz の場合
cp /opt/officework/share/officework/plugins/*.py ~/.config/officework/plugins/   # .deb の場合
```

calc のセルに打ってみてください:

```
=税込(1000)          → 1100
=合計(A1:A10)        → 範囲をまとめて
```

**Python は同梱しています。** 機械に Python が入っていなくても動きます。
`~/.config/officework/plugins/` に `.py` を置くだけで、`def` の名前がそのまま
セルの関数になります(日本語の名前も使えます)。

天気予報(`@天気`)と家計簿(`@家計簿`)の見本もありますが、こちらは
手続きなので calc の中から呼びます。家計簿は AI が要ります(詳しくは
`share/officework/plugins/README.ja.adoc`)。

## 動かないとき

- **窓が開かない** — 依存が足りない可能性があります。`.deb` なら
  `sudo apt-get -f install` で入ります。tar.gz なら
  `libxkbcommon0 libxkbcommon-x11-0 libxcb1 libxcb-xkb1 libfontconfig1` を
  入れてください
- **字が豆腐(□)になる** — 日本語のフォントが要ります:
  `sudo apt install fonts-noto-cjk`
- **セルの関数が `#PY?` のまま** — 置き場を確かめてください
  (`~/.config/officework/plugins/*.py`)。置いた後は calc を開き直します
- **画面が小さすぎる** — 設定 → ディスプレイ → 拡大/縮小(200% など)。
  字だけ大きくする設定(GNOME の文字倍率・Tweaks のフォント)は使わないで
  ください — 箱が置いていかれて崩れます

## まだ無い形

- **Microsoft ストア** — 落ち着いたら出します。それまで Windows の
  `setup.exe` は署名が無いので「WindowsによってPCが保護されました」が
  出ます(**詳細情報 → 実行**で入ります)
- **Flatpak** — 下ごしらえは `packaging/flatpak/` にありますが、
  Flathub の AI の方針で当面出せません(理由はそこの README に)
