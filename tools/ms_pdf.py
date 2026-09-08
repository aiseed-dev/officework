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


def word_pdf(src, out):
    src, out = os.path.abspath(src), os.path.abspath(out)
    _osa(
        f'''
with timeout of 600 seconds
tell application "Microsoft Word"
    open "{src}"
    delay 1
    set d to active document
    save as d file name "{out}" file format format PDF
    close d saving no
end tell
end timeout
'''
    )


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
    name = os.path.basename(src)
    hfs, out = _hfs(src), os.path.abspath(out)
    _excel_ready()
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
    repeat 180 times
        try
            if (name of every workbook) contains "{name}" then set wb to workbook "{name}"
        end try
        if wb is not missing value then exit repeat
        delay 1
    end repeat
    if wb is missing value then error "Excel が 3 分たっても開きませんでした(壊れたファイルと見た可能性): {hfs}"
    activate object wb
    save as active sheet filename "{out}" file format PDF file format
    close wb saving no
    close nb saving no
    set display alerts to true
end tell
end timeout
'''
    )


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
