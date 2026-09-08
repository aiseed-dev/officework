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


def excel_pdf(src, out):
    name = os.path.basename(src)
    name_src = src
    src, out = _hfs(src), os.path.abspath(out)
    # **Finder 経由で開かせます。** AppleScript の `open workbook` は、Excel が
    # 起動直後で窓を持たないときに -50 を返しました(2026-09-08)。
    # `open -a` は必ず開き、読み込みは後から終わるので、下で名前を待ちます
    subprocess.run(["open", "-a", "Microsoft Excel", os.path.abspath(name_src)], check=True)
    _osa(
        f'''
with timeout of 600 seconds
tell application "Microsoft Excel"
    set display alerts to false
    -- open はすぐ返り、読み込みは後から終わる。名前が一覧に出るまで待つ
    set wb to missing value
    repeat 180 times
        -- 読み込みの途中は workbooks を引くだけで -50 が返ることがある。
        -- 失敗は飲み込んで、次の秒にもう一度見る
        try
            -- `repeat with w in workbooks` は -50 を返すことがある(2026-09-08)。
            -- 名前の一覧で見てから、名前で引く
            if (name of every workbook) contains "{name}" then set wb to workbook "{name}"
        end try
        if wb is not missing value then exit repeat
        delay 1
    end repeat
    if wb is missing value then error "Excel が 3 分たっても開きませんでした(壊れたファイルと見た可能性): {src}"
    save as active sheet filename "{out}" file format PDF file format
    close wb saving no
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
