---
name: app-check
description: 画面やアプリの動きを直した後に、この Mac で officework を実際に起動して確かめる手順。発注者の設定と窓に触れずに、アプリの API で操作し、窓を撮って見る。画面の文言・リボン・ダイアログ・詳細設定・セルの表示を直したとき、報告の前に使う。
---

# アプリを動かして確かめる

画面を直したら、実際に動かして見てから報告します。テストが通っただけでは
報告しません。

## 1. 組み立てる

```bash
cd ~/dev/officework
PYO3_PYTHON=$PWD/.venv/bin/python cargo build -q --release --bin officework
```

10〜25 分かかります。バックグラウンドで走らせて、終わりの知らせを待ちます。
`pgrep -f` で待つループは書きません(自分自身に一致して終わりません)。

## 2. 作業用の HOME で起動する

発注者の `~/.config/officework/settings.toml` を書き換えないように、HOME を
作業用のフォルダーに向けます。試す文書も、原本ではなく写しを使います。

```bash
S=<作業用のフォルダー>
mkdir -p $S/home/.config/officework
cp 原本.xlsx $S/test.xlsx
HOME=$S/home ./target/release/officework $S/test.xlsx > $S/app.log 2>&1 &
echo $! > $S/app.pid
sleep 8
```

- 設定を変えて試すときは、`$S/home/.config/officework/settings.toml` に書きます
  (例:`time_zone = "America/Los_Angeles"`)。
- 文書を指定せずに起動すると表の画面が無く、表の API は答えません。

## 3. API で操作する

Mac では、外からクリックやキーを送れません(補助アクセスの許可が要るため)。
アプリの API(ソケット)で操作し、答えで中身を確かめます。

ソケットは `$XDG_RUNTIME_DIR/officework/officework.sock` です。
`XDG_RUNTIME_DIR` が無い Mac では `$TMPDIR/officework-0/officework.sock` です。
Python の `officework.call` は同じ決め方で探します。

```bash
PYTHONPATH=pysheet .venv/bin/python - <<'PY'
from officework import call
print(call("officework", "ping"))
print(call("officework", "set", a1="A1", values=[["=NOW()"]]))
print(call("officework", "get", a1="A1"))
print(call("officework", "ui_state"))            # 開いている物・ステータスバー
PY
```

よく使う命令:

| 命令 | すること |
|---|---|
| `ping` | 答えるか、どの画面か、版 |
| `ui_state` | 開いているパネル、ステータスバーの文、選んでいるセル |
| `press` `id=` | リボンのボタンを押す(`face/src/ribbon.rs` の id) |
| `option` `id=` | ファイルの「詳細設定」を開き、その行を押す(空の id は開くだけ) |
| `set` `a1=` `values=` | セルに入れる(`=` で始まれば数式) |
| `get` / `get_formula` `a1=` | 値 / 数式を読む |
| `select` `a1=` / `autofit` `a1=` | セルを選ぶ / 列の幅を合わせる |
| `open` `path=` / `save` | 開く / 保存する |

## 4. 窓を撮って見る

```bash
W=$(.venv/bin/python -c "
import Quartz
for w in Quartz.CGWindowListCopyWindowInfo(Quartz.kCGWindowListOptionOnScreenOnly, Quartz.kCGNullWindowID):
    if (w.get('kCGWindowOwnerName') or '').lower().startswith('officework'): print(w['kCGWindowNumber'])")
screencapture -x -o -l $W $S/shot.png
```

撮った絵は、必要な所を切り出して目で見ます。画面収録の許可が無いと真っ黒に
なるので、黒い絵を「見た」ことにしません。

## 5. 終わらせる

自分が起動したアプリだけを終わらせます。

```bash
kill $(cat $S/app.pid)
```

`pkill -f release/officework` のように名前で終わらせると、発注者が開いている
窓まで終わります。

## Debian(Linux)では、見えない画面で開く

発注者の画面にアプリの窓を出しません。2026-10-15 に、開いた窓が前に出て、発注者が
打っていた字がセルに入りました。`tools/nested_display.py` で別の画面(`:5`)を作り、
その中で開きます。この画面は発注者の画面に何も出さず、キーボードも取りません。

```bash
python3 -I tools/nested_display.py start :5 1100x1000 &   # 止めるまで動く
echo $! > $S/nd.pid
env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY=:5 MESA_VK_WSI_DEBUG=sw \
  XDG_RUNTIME_DIR=$S/xdg HOME=$S/home ./target/release/officework $S/test.xlsx > $S/app.log 2>&1 &
echo $! > $S/app.pid
sleep 10
python3 -I tools/nested_display.py shot :5 $S/shot.png   # 描き直させてから撮る
kill $(cat $S/app.pid) $(cat $S/nd.pid)
```

- この画面(Xephyr)には DRI3 が無いので、`MESA_VK_WSI_DEBUG=sw` を付けます。付けないと
  窓が黒いままです。
- GPUI の窓は、描き直しを頼まれるまで最初の絵を描きません。`shot` は `xrefresh` を
  送ってから撮ります。
- `XDG_RUNTIME_DIR` は作業用のフォルダー(`chmod 700`)にします。分けないと、動いている
  発注者のアプリに相乗りします。

## 報告に書くこと

- 何を、どの手順で見たか(撮った絵を添える)
- 確かめられなかったこと。キー入力や入力欄への打ち込みは外から送れないので、
  発注者に試してもらうよう書きます。
- 知っている理由で落ちたテスト(matplotlib と scipy が無い calc の 5 件など)
