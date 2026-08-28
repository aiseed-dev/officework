"""書体を名指しして PDF を作り、本当にその書体で出るか確かめる。

    .venv/bin/python pysheet/test_font.py

**名指しが効いているかは、PDF の中を見ないと分かりません。** 2026-08-28 に
発注者から「Noto やモリサワ UD 書体は使えますか」と聞かれて調べたところ、
どれを頼んでも同じ書体が埋まっていました。文書ぜんたいの書体を入れる口が
無く、Python 側が黙って受け取っていただけだったためです。

見るのは3つです。

- 頼んだ書体が実際に埋まったか(PDF の BaseFont を読む)
- 標準の顔が来たか。BIZ UD の Bold は OS/2 の標準の旗も立てているので、
  太字と斜体でないことも見ないと選び分けられません
- 字が取り出せるか(豆腐なら pdftotext で出ません)

`pdffonts` と `pdftotext`(poppler)が要ります。無ければ飛ばします。
"""
import os
import shutil
import subprocess
import sys
import tempfile

from officework import doc

tmp = tempfile.mkdtemp()
HON = "四月の売上は前月比で 12% 増えました。角に丸みのある UD 書体です。"

EXPECT = {
    "Noto Sans CJK JP": "NotoSansCJKjp-Regular",
    "Noto Serif CJK JP": "NotoSerifCJKjp-Regular",
    "BIZ UDPゴシック": "BIZUDPGothic-Regular",
    "BIZ UDP明朝": "BIZUDPMincho-Regular",
    "BIZ UDゴシック": "BIZUDGothic-Regular",
    "IPAexゴシック": "IPAexGothic",
}
warui = 0
if not shutil.which("pdffonts"):
    print("pdffonts がありません(poppler)。飛ばします")
    raise SystemExit(0)

for name in [
    "Noto Sans CJK JP",
    "Noto Serif CJK JP",
    "BIZ UDPゴシック",
    "BIZ UDP明朝",
    "BIZ UDゴシック",
    "IPAexゴシック",
    "存在しない書体XYZ",
]:
    d = doc.Doc()
    d.font = name
    d.add_heading("見出し " + name, 1)
    d.add_paragraph(HON)
    p = os.path.join(tmp, name.replace(" ", "_") + ".pdf")
    try:
        d.save(p)
    except Exception as e:
        print("{:20} 落ちた: {}".format(name, str(e)[:60]))
        continue
    # PDF に埋まった書体の名前を見る
    o = subprocess.run(["pdffonts", p], capture_output=True, text=True).stdout
    umeta = [ln.split()[0] for ln in o.splitlines()[2:] if ln.strip()]
    # サブセットの名前は "ABCDEF+BIZUDPGothic" の形。+ の後ろが元の名前
    moto = [u.split("+", 1)[-1] for u in umeta]
    # 字がちゃんと出ているか(豆腐なら取り出せない)
    t = subprocess.run(["pdftotext", p, "-"], capture_output=True, text=True).stdout
    deta = "四月の売上" in t
    got = moto[0] if moto else "(無し)"
    machi = EXPECT.get(name)
    ok = deta and (machi is None or got == machi)
    if not ok:
        warui += 1
    print("{}  {:20} {:7} 埋まった書体: {}  字: {}".format(
        "OK" if ok else "NG",
        name,
        "{}KB".format(os.path.getsize(p) // 1024),
        got[:46],
        "出る" if deta else "出ない",
    ))

print("OK" if warui == 0 else "{} 件おかしい".format(warui))
sys.exit(1 if warui else 0)
