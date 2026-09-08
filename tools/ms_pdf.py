#!/usr/bin/env python3
"""**Word / Excel で開いて PDF にする道具(macOS)。**

officework が書いた docx / xlsx を、本物の Word / Excel に開かせて PDF に
します。officework 自身が書いた PDF と並べて、ページの割り・行の折れ・
値の見え方を比べるための材料です(`tools/ms_compare.py` が比べます)。

    python3 tools/ms_pdf.py 文書.docx [出力.pdf]
    python3 tools/ms_pdf.py 台帳.xlsx [出力.pdf]

出力を省くと、同じ名前の `.ms.pdf` を隣に置きます。

## 踏んだ跡

* Word / Excel は AppleScript で操ります(`osascript`)。最初の1回は
  「オートメーションを許可するか」を macOS が聞くので、画面で許可します
* Word の `save as` は `file format format PDF`、Excel の `save as` は
  `file format PDF file format` と書き方が違います
* Excel の `open` は「Macintosh HD:Users:…」の形の径路しか受けません。
  POSIX の径路を渡すと何も開かずに黙って返ります。Word は POSIX で開けます
* 開くときの警告(互換モード・リンクの更新)を止めるため、Excel は
  `display alerts` を切ります。**その状態では、壊れたファイルは黙って
  開かれません**(`active workbook` が missing value のまま)。
  2026-09-08 に、テーマの関係だけあって部品が無い xlsx がこれで見つかりました
* Word の `save as` は、名指しした文書でなく**手前の文書**を書きます。
  5枚を開いたまま名指しで回すと、5枚とも同じ PDF になりました。
  1枚ずつ開いて `active document` を書き、閉じてから次へ進みます
* 開くのに2分を超えることがあり、既定の AppleEvent の待ち(2分)で
  切れます。`with timeout of 600 seconds` で包みます
* Excel の `open workbook` は読み込みが終わる前に返ります。すぐに
  `active workbook` を見ると missing value です。名前が `workbooks` に
  出るまで 1 秒ずつ待ちます(式の多いブックは 10 秒を超えます)
* 出力先は `~/Documents` の下など、普通のフォルダにします。Excel は
  `/private/tmp` の下へは書けませんでした
"""
import os
import subprocess
import sys


def _osa(script):
    r = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(r.stderr.strip())
    return r.stdout.strip()


def _hfs(path):
    """POSIX の径路を「Macintosh HD:Users:…」の形にします。Excel の open は
    この形しか受けません(POSIX の径路を渡すと、何も開かずに黙って返る)。
    `POSIX file` を tell の中に書くと Excel に送られて -50 になるので、
    ここで別に変えておきます。"""
    return _osa(f'POSIX file "{os.path.abspath(path)}" as string')


def word_close_ours():
    """こちらが開かせた文書(径路に officework-cmp を含む物)を全部閉じます。
    発注者の文書には触りません(2026-09-09 発注者「画面にひらけているファイルは
    閉じながらやって」)"""
    # `repeat with d in (every document)` は Word が -1708 で断る(Excel の
    # `repeat with w in workbooks` が -50 なのと同じ癖)。名前を先に取り、名指しで閉じる
    _osa('''
with timeout of 120 seconds
tell application "Microsoft Word"
    repeat with i from (count documents) to 1 by -1
        try
            set d to document i
            if (full name of d) contains "officework-cmp" then close d saving no
        end try
    end repeat
end tell
end timeout
''')


def word_pdf(src, out):
    src, out = os.path.abspath(src), os.path.abspath(out)
    # 前の回で閉じ損ねた文書が残っていれば先に閉じる(`save as` が失敗すると
    # 閉じる前に止まり、Word に文書が溜まっていった。2026-09-09 に 70 枚溜まった)
    try:
        word_close_ours()
    except RuntimeError:
        pass
    try:
        _osa(
            f'''
with timeout of 600 seconds
tell application "Microsoft Word"
    open "{src}"
    delay 1
    set d to active document
    save as d file name "{out}" file format format PDF
end tell
end timeout
'''
        )
    finally:
        # 成否にかかわらず閉じる
        try:
            word_close_ours()
        except RuntimeError:
            pass
    # **閉じられなかったら止まる。** 2026-09-09、Word が `close` を -1708 で断る
    # 状態になったのに気づかず、100 枚の窓を開いたまま次々に進んでしまった。
    # 残っている写しを数え、残っていれば呼ぶ側に止めてもらう
    nokori = _osa('''
with timeout of 60 seconds
tell application "Microsoft Word"
    set n to 0
    repeat with i from 1 to (count documents)
        try
            if (full name of document i) contains "officework-cmp" then set n to n + 1
        end try
    end repeat
    n
end tell
end timeout
''')
    if nokori.strip() not in ("", "0"):
        raise RuntimeError(f"Word が文書を閉じません({nokori.strip()} 枚残っています)。これ以上は開かずに止めます")


def _excel_ready():
    """Excel を、AppleScript の `open workbook` と PDF の保存が効く状態にする。

    踏んだ跡(2026-09-08、一日かけて分かった組み合わせ):
    * Finder 経由(`open -a` にファイル)で開いたブックは、`save as` が黙って
      何も書かず、`close` も効かない。さらにそのブックがある間は、AppleScript の
      `open workbook` が -50 で断られる。**目当てのファイルを `open -a` で開いては
      いけない**
    * `quit saving no` で静かに終了すると、次の起動で最初の画面(テンプレートの
      一覧)が出て、`make new workbook` が 2 分待たされる
    * 効いた順: 開いているブックを全部閉じる → `killall` → ファイル無しで
      `open -a` → `make new workbook`(空のブックで窓を持たせる)→ AppleScript の
      `open workbook`(Macintosh HD: の径路)→ `save as … PDF`。
      未保存のブックを残したまま落とすと、次の起動に復旧の画面が出て崩れるので、
      先に閉じる
    """
    import time

    r = subprocess.run(["pgrep", "-f", "MacOS/Microsoft Excel"], capture_output=True, text=True)
    if r.stdout.strip():
        subprocess.run(["osascript", "-e", 'tell application "Microsoft Excel" to close every workbook saving no'],
                       capture_output=True, text=True, timeout=120)
        subprocess.run(["killall", "Microsoft Excel"], capture_output=True, text=True)
        for _ in range(30):
            r = subprocess.run(["pgrep", "-f", "MacOS/Microsoft Excel"], capture_output=True, text=True)
            if not r.stdout.strip():
                break
            time.sleep(1)
    subprocess.run(["open", "-a", "Microsoft Excel"], check=False)
    time.sleep(10)


def excel_pdf(src, out):
    """Excel に開かせて、見えているシートを PDF にします。

    効いた手順(2026-09-08、3通り試して唯一これだけ書けた):
    1. 先に空のブックを1つ作って窓を持たせる(窓が無いと `open workbook` が -50)
    2. `open workbook workbook file name` に「Macintosh HD:…」の径路で開かせる。
       `open -a`(Finder 経由)で開いたブックは、別の径路への `save as` が
       黙って何も書かない(サンドボックスの都合と思われる)
    3. 名前が `workbooks` に出るまで待つ(open は読み込みの前に返る)
    4. `save as active sheet … file format PDF file format`。保存先は POSIX の径路
    """
    _excel_ready()
    _excel_one(src, out)


def _excel_one(src, out, wait=180):
    """開いている Excel に1枚だけ開かせて PDF にします。

    `_excel_ready()` が済んでいることが前提です。空のブック(窓を持たせる
    ため)は毎回作って毎回閉じます。まとめて回すときは
    [`excel_pdf_many`] を使ってください — Excel の起動と終了は1回で済みます。
    """
    name = os.path.basename(src)
    hfs, out = _hfs(src), os.path.abspath(out)
    _osa(
        f'''
with timeout of 600 seconds
tell application "Microsoft Excel"
    set display alerts to false
    -- 空のブックで窓を持たせる。最初の画面が出ていると待たされるので 10 秒で切って繰り返す
    set nb to missing value
    repeat 20 times
        try
            with timeout of 10 seconds
                set nb to make new workbook
            end timeout
            exit repeat
        on error
            delay 3
        end try
    end repeat
    if nb is missing value then error "Excel が空のブックを作れませんでした(最初の画面が消えない)"
    open workbook workbook file name "{hfs}"
    set wb to missing value
    repeat {wait} times
        try
            if (name of every workbook) contains "{name}" then set wb to workbook "{name}"
        end try
        if wb is not missing value then exit repeat
        delay 1
    end repeat
    if wb is missing value then error "Excel が {wait} 秒たっても開きませんでした(壊れたファイルと見た可能性): {hfs}"
    activate object wb
    save as active sheet filename "{out}" file format PDF file format
    close wb saving no
    close nb saving no
    set display alerts to true
end tell
end timeout
'''
    )


def excel_pdf_many(pairs, wait=120, on_done=None):
    """**何枚もまとめて PDF にします。** `pairs` は (xlsx の径路, 出す PDF)の一覧。

    Excel を落として起動し直すのは**最初の1回だけ**です。1枚あたり 30 秒
    ほど縮みます(集めた xlsx を1周させるとき、この差が効きます)。
    1枚が失敗しても続けます。返りは (径路, 誤りの文言 or None) の一覧です。

    Excel が一度おかしくなると後の全部が失敗するので、続けて 3 枚落ちたら
    起動し直します。`on_done` を渡すと1枚ごとに (径路, 誤り) で呼びます。
    """
    _excel_ready()
    out = []
    renzoku = 0
    for src, dst in pairs:
        try:
            _excel_one(src, dst, wait=wait)
            err = None if os.path.exists(dst) else "PDF ができていない"
        except Exception as e:  # noqa: BLE001 — 1枚の失敗で残りを止めない
            err = str(e).splitlines()[-1][:200] if str(e).strip() else type(e).__name__
        renzoku = renzoku + 1 if err else 0
        out.append((src, err))
        if on_done:
            on_done(src, err)
        if renzoku >= 3:
            _excel_ready()
            renzoku = 0
    _excel_quit()
    return out


def _excel_quit():
    """自分が起動した Excel を閉じます(発注者の Word には触りません)。"""
    subprocess.run(["osascript", "-e",
                    'tell application "Microsoft Excel" to close every workbook saving no'],
                   capture_output=True, text=True, timeout=120)
    subprocess.run(["killall", "Microsoft Excel"], capture_output=True, text=True)


def to_pdf(src, out=None):
    if out is None:
        base, _ = os.path.splitext(src)
        out = base + ".ms.pdf"
    ext = os.path.splitext(src)[1].lower()
    if ext in (".docx", ".doc", ".adoc"):
        word_pdf(src, out)
    elif ext in (".xlsx", ".xlsm", ".xls"):
        excel_pdf(src, out)
    else:
        raise SystemExit(f"docx か xlsx を渡してください: {src}")
    if not os.path.exists(out):
        raise SystemExit(f"PDF ができていません: {out}")
    return out


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    print(to_pdf(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None))
