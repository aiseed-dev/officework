#!/usr/bin/env python3
"""関数の答えの表を作る。

kansu_cases.tsv の式を openpyxl で xlsx に並べ、LibreOffice を画面なしで
動かして計算させ、答えを kansu_kotae.tsv に書く。

答えの正は「本家が実際に計算した値」で、人も AI もここに手を入れない。
いまの本家は LibreOffice。M365 が使えるようになったら、同じ xlsx を
Excel で開いて保存し、--kotae にそのファイルを渡せば Excel の答えに
差し替わる(道具は同じ、正だけ替える)。

使い方:
    python3 test/kansu_oracle.py            # 生成 → 計算 → kansu_kotae.tsv
    python3 test/kansu_oracle.py --kotae 計算済み.xlsx   # 答えだけ吸い直す
    python3 test/kansu_oracle.py --excel    # Mac の Excel に式を打たせて計算(2026-09-08)

--excel は、Excel を AppleScript で操り、**式を Excel 自身に打たせて**から
保存し、その xlsx から答えを吸います。openpyxl が書いた xlsx を Excel に
開かせる道は取りません — 2010 年以降の関数は XML の中で `_xlfn.` を頭に
付ける決まりがあり、付けずに書いた式が混ざると Excel は「壊れたファイル」と
見て、警告を止めた状態では開かないからです(2026-09-08 に実際に踏んだ)。
Excel が打った式は Excel 自身が正しい形で書くので、この問題が起きません。
"""

import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
CASES = HERE / "kansu_cases.tsv"
KOTAE = HERE / "kansu_kotae.tsv"


def load_cases():
    out = []
    for line in CASES.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        name, formula = line.split("\t", 1)
        out.append((name.strip(), formula.strip()))
    return out


def build_xlsx(cases, path):
    from openpyxl import Workbook

    wb = Workbook()
    ws = wb.active
    ws.title = "kotae"
    for i, (name, formula) in enumerate(cases, start=1):
        ws.cell(row=i, column=1, value=name)
        # 式の字そのもの(答えと並べて読むため)
        ws.cell(row=i, column=2, value="'" + formula)
        ws.cell(row=i, column=3, value=formula)
    wb.save(path)


def compute(src, outdir):
    # LibreOffice の既定は「xlsx の式を読み直しても再計算しない」。
    # 専用のプロファイルに「常に再計算」を書いてから動かす
    prof = outdir / "profile"
    (prof / "user").mkdir(parents=True, exist_ok=True)
    (prof / "user/registrymodifications.xcu").write_text(
        '<?xml version="1.0"?>\n'
        '<oor:items xmlns:oor="http://openoffice.org/2001/registry">\n'
        ' <item oor:path="/org.openoffice.Office.Calc/Formula/Load">'
        '<prop oor:name="OOXMLRecalcMode" oor:op="fuse"><value>0</value></prop></item>\n'
        ' <item oor:path="/org.openoffice.Office.Calc/Formula/Load">'
        '<prop oor:name="ODFRecalcMode" oor:op="fuse"><value>0</value></prop></item>\n'
        "</oor:items>\n",
        encoding="utf-8",
    )
    # 出力は別の入れ物に(同じ場所だと入力と同じ名前になり、変換されない)
    saki = outdir / "out"
    saki.mkdir(exist_ok=True)
    r = subprocess.run(
        [
            "soffice",
            f"-env:UserInstallation=file://{prof}",
            "--headless",
            "--convert-to",
            "xlsx",
            "--outdir",
            str(saki),
            str(src),
        ],
        capture_output=True,
        text=True,
        timeout=300,
    )
    out = saki / (Path(src).stem + ".xlsx")
    if not out.exists():
        raise SystemExit(f"LibreOffice が書き出せませんでした: {r.stderr}")
    return out


def suck(path, cases):
    import datetime

    from openpyxl import load_workbook

    wb = load_workbook(path, data_only=True)
    ws = wb.active
    rows = []
    for i, (name, formula) in enumerate(cases, start=1):
        v = ws.cell(row=i, column=3).value
        if v is None:
            v = ""
        elif isinstance(v, bool):
            v = "TRUE" if v else "FALSE"
        elif isinstance(v, datetime.datetime):
            # 日付の表示形式が付くと openpyxl は datetime で返す。
            # 中身は通し番号(1899-12-30 起点)なので、数に戻して書く
            delta = v - datetime.datetime(1899, 12, 30)
            v = repr(delta.days + delta.seconds / 86400)
        elif isinstance(v, datetime.time):
            # 時刻の表示形式も同じ — 日の割合に戻す
            v = repr((v.hour * 3600 + v.minute * 60 + v.second) / 86400)
        elif isinstance(v, float):
            v = repr(v)
        rows.append(f"{name}\t{formula}\t{v}")
    return rows


def excel_rows(cases, outdir):
    """Mac の Excel に式を打たせて計算させ、答えの行(name, 式, 答え)を返す。

    * 器のブック(名前と式の字だけ。式は入れない)を openpyxl で作り、
      **Finder 経由(`open -a`)で Excel に開かせます**。AppleScript の
      `open workbook` と `make new workbook` は、Excel が起動直後で窓を
      持たないときに -50 を返しました(2026-09-08 に実際に踏んだ)
    * 開くのは非同期なので、`workbooks` に名前が出るまで待ちます
    * 式は `set formula of range` で Excel 自身に打たせます。式の文字列の
      `"` は AppleScript の文字列の中では `\\"` にします
    * **答えは保存せずに、セルの値を AppleScript で読みます。** `save workbook as`
      は「保存した」と言いながらファイルを書かないことがありました
      (2026-09-08 に2回)。値は種類ごとに字にします — 論理値は TRUE / FALSE、
      日付は 1899-12-30 起点の通し番号、数は AppleScript の字(15 桁)
    """
    from openpyxl import Workbook

    src = outdir / "kansu_utsuwa.xlsx"
    wb = Workbook()
    ws = wb.active
    ws.title = "kotae"
    for i, (name, formula) in enumerate(cases, start=1):
        ws.cell(row=i, column=1, value=name)
        ws.cell(row=i, column=2, value="'" + formula)
    wb.save(src)

    def esc(t):
        return t.replace("\\", "\\\\").replace('"', '\\"')

    lines = [f'set formula of range "C{i}" of ws to "{esc(f)}"' for i, (_, f) in enumerate(cases, start=1)]
    body = "\n".join(lines)
    script = f'''with timeout of 900 seconds
tell application "Microsoft Excel"
    set display alerts to false
    set wb to missing value
    repeat 120 times
        try
            if (name of every workbook) contains "{src.name}" then set wb to workbook "{src.name}"
        end try
        if wb is not missing value then exit repeat
        delay 1
    end repeat
    if wb is missing value then error "Excel が {src.name} を開きませんでした"
    set ws to active sheet
    {body}
    calculate
    -- 財務の関数の答えは通貨の書式が付き、AppleScript が通貨(小数 4 桁)に
    -- 丸めて渡してくる。書式を標準に戻してから読む(2026-09-08)
    set number format of range ("C1:C" & {len(cases)}) of ws to "General"
    set epoch to current date
    set year of epoch to 1899
    set month of epoch to 12
    set day of epoch to 30
    set time of epoch to 0
    set outs to ""
    repeat with i from 1 to {len(cases)}
        set vv to value of range ("C" & i) of ws
        -- `class of vv is boolean` は Excel の辞書の語と衝突して構文エラーになる。
        -- 値そのもので判じ、日付は引き算ができるかで判じる
        if vv is true then
            set tt to "TRUE"
        else if vv is false then
            set tt to "FALSE"
        else
            try
                set tt to ((vv - epoch) / 86400) as text
            on error
                set tt to vv as text
            end try
        end if
        -- 区切りは改行でなく ASCII 31。空の答えが並ぶと改行では数が崩れる
        set outs to outs & tt & (ASCII character 31)
    end repeat
    close wb saving no
    outs
end tell
end timeout'''
    subprocess.run(["osascript", "-e", f'''tell application "Microsoft Excel"
    try
        close workbook "{src.name}" saving no
    end try
end tell'''], capture_output=True, text=True, timeout=120)
    subprocess.run(["open", "-a", "Microsoft Excel", str(src)], check=True)
    r = subprocess.run(["osascript", "-e", script], capture_output=True, text=True, timeout=1200)
    if r.returncode != 0:
        raise SystemExit(f"Excel が計算できませんでした: {r.stderr.strip()}")
    vals = r.stdout.rstrip("\n").split("\x1f")[:-1]
    if len(vals) != len(cases):
        raise SystemExit(f"答えの数が合いません: {len(vals)} / {len(cases)}")
    rows = []
    for (name, formula), v in zip(cases, vals):
        # AppleScript の数の字は "6.0" や "1.0E+3" の形。数に戻して repr に揃える。
        # 字の答え("00000111" など)は数に見えても触らない — 小数点も E も
        # 無い物は AppleScript の数ではない
        if v not in ("TRUE", "FALSE") and ("." in v or "E" in v.upper()):
            try:
                f = float(v)
                v = repr(int(f)) if f.is_integer() else repr(f)
            except ValueError:
                pass
        rows.append(f"{name}\t{formula}\t{v}")
    return rows


def main():
    cases = load_cases()
    if len(sys.argv) > 1 and sys.argv[1] == "--excel":
        tmp = Path(tempfile.mkdtemp(prefix="kansu_", dir=str(Path.home() / "Documents")))
        rows = excel_rows(cases, tmp)
        KOTAE.write_text(
            "# 関数の答えの表(機械が計算した物 — 手で直さない)。\n"
            "# 作り直し: python3 test/kansu_oracle.py --excel。正はいま Excel(Mac、2026-09-08)。\n"
            + "\n".join(rows) + "\n",
            encoding="utf-8",
        )
        print(f"{len(rows)} 行を Excel の答えで書きました → {KOTAE}")
        return
    if len(sys.argv) > 2 and sys.argv[1] == "--kotae":
        done = Path(sys.argv[2])
        rows = suck(done, cases)
    else:
        tmp = Path(tempfile.mkdtemp(prefix="kansu_"))
        src = tmp / "kansu_moto.xlsx"
        build_xlsx(cases, src)
        done = compute(src, tmp)
        rows = suck(done, cases)
        # 2010年以降の関数は xlsx の中では `_xlfn.` を頭に付けて書く決まり。
        # 付けずに #NAME? になった式へ機械的に付けて、もう1周だけ試す
        nokori = [
            (i, (name, formula.replace(name + "(", "_xlfn." + name + "(")))
            for i, ((name, formula), row) in enumerate(zip(cases, rows))
            if row.endswith("\t#NAME?")
        ]
        if nokori:
            src2 = tmp / "kansu_xlfn.xlsx"
            build_xlsx([c for _, c in nokori], src2)
            done2 = compute(src2, tmp)
            for (i, (name, _)), row2 in zip(nokori, suck(done2, [c for _, c in nokori])):
                v = row2.split("\t")[2]
                if v != "#NAME?":
                    # 表に載せる式は元の書き方のまま(答えだけ差し替え)
                    rows[i] = f"{name}\t{cases[i][1]}\t{v}"
    KOTAE.write_text(
        "# 関数の答えの表(機械が計算した物 — 手で直さない)。\n"
        "# 作り直し: python3 test/kansu_oracle.py。正はいま LibreOffice。\n"
        + "\n".join(rows)
        + "\n",
        encoding="utf-8",
    )
    print(f"{KOTAE.name}: {len(rows)} 件")


if __name__ == "__main__":
    main()
