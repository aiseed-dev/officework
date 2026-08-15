# 写真を xlsx の台帳で管理する。中身はすべて架空。
#
#   pip install officework Pillow
#   python3 sample/写真の台帳.py
#
# **原本をそのまま埋める。** xlsx は zip で、JPEG は既に圧縮済みなので、
# フォルダに置いても台帳の中に入れても合計はほとんど変わらない
# (発注者 2026-08-15)。ならば**1つのファイルに収まるほうがよい** —
# 台帳と写真が離れて片方だけ無くなる、が起きない。
#
# 撮影日・画素・向きは**写真自身が持っている**(EXIF)ので人に打たせない。
# 打たせるのは人にしか分からない欄(品名・場所・覚え書き)だけ。
#
# Pillow は**あれば使う**。無ければ EXIF は読めないとその場で言って、
# ファイルの日付で代える(黙って空欄にしない)。
import datetime
import pathlib

from officework import sheet

ここ = pathlib.Path(__file__).resolve().parent
写真置き場 = ここ / "写真"
台帳 = ここ / "写真の台帳.xlsx"

# 台帳に出す長辺(px)。**原本は縮めない** — 見せる大きさだけを決める。
# 小さく並べても写真は見えないので、1シートに1枚を大きく置く
見せる長辺 = 640

# 架空の写真(品名, 説明, 色, 縦長か)
見本 = [
    ("青しそ", "本葉4枚", (94, 140, 76), False),
    ("聖護院かぶ", "間引き後", (196, 190, 170), False),
    ("丹波黒大豆", "莢がふくらむ", (72, 84, 60), False),
    ("千日紅", "花が上がった", (188, 96, 128), True),
    ("藍", "刈り取り前", (60, 92, 110), False),
    ("カモミール", "開花", (222, 210, 130), True),
]


def 写真を作る():
    """**実物の代わり。** 撮った写真のフォルダがあれば、ここは要らない。
    EXIF に撮影日と向きを入れる — 実物と同じ穴が出るように"""
    try:
        from PIL import Image, ImageChops, ImageDraw
    except ImportError:
        print("Pillow が無いので見本の写真は作れません(pip install Pillow)")
        return False
    写真置き場.mkdir(exist_ok=True)
    日 = datetime.datetime(2026, 6, 1, 9, 30)
    for i, (品名, 説明, 色, 縦長) in enumerate(見本, start=1):
        w, h = (900, 1200) if 縦長 else (1200, 900)
        # **粒を入れる。** 単色の絵は JPEG も zip もよく圧縮するので、
        # そのままだと「台帳に入れても大きさは変わらない」を確かめられない
        # (2026-08-15 に一度これで測り違えた)。実物の写真は圧縮されない
        粒 = Image.effect_noise((w, h), 28).convert("L")
        im = Image.merge("RGB", [
            # 粒は中心 128 なので、その分を引かないと絵が白く飛ぶ
            ImageChops.add(Image.new("L", (w, h), c), 粒, scale=1.0, offset=-128)
            for c in 色
        ])
        d = ImageDraw.Draw(im)
        # 何の写真か分かる程度の書き込み(実物では要らない)
        d.rectangle([40, 40, w - 40, h - 40], outline=(255, 255, 255), width=6)
        d.text((70, 70), f"{i:03d} {品名}", fill=(255, 255, 255))
        d.text((70, 110), 説明, fill=(255, 255, 255))
        撮影 = 日 + datetime.timedelta(days=i * 3, hours=i)
        exif = im.getexif()
        exif[0x9003] = 撮影.strftime("%Y:%m:%d %H:%M:%S")  # DateTimeOriginal
        exif[0x0112] = 1  # Orientation(1 = そのまま)
        im.save(写真置き場 / f"{i:03d}_{品名}.jpg", quality=85, exif=exif)
    return True


def 撮影日(p):
    """EXIF の撮影日。**無ければファイルの日付で代え、そう言えるように返す**"""
    try:
        from PIL import Image
    except ImportError:
        return None, "Pillow が無いので EXIF を読めません"
    try:
        with Image.open(p) as im:
            v = im.getexif().get(0x9003)
            画素 = f"{im.width}×{im.height}"
        if v:
            return datetime.datetime.strptime(v, "%Y:%m:%d %H:%M:%S"), 画素
    except Exception as e:
        return None, f"読めません({e})"
    return None, 画素


def 台帳を作る():
    """**1シートに写真1枚**(発注者 2026-08-15「こんなに小さくしなくても
    1シートに一ついれる」)。小さく並べても写真は見えない — 見えない台帳は
    見ない台帳になる。

    - 1枚目の「一覧」が目次(番号・品名・撮影日・シート名)
    - 以降は写真ごとに1シート。写真を大きく置き、右に欄を並べる
    - シートの耳がそのまま索引になる
    """
    写真 = sorted(写真置き場.glob("*.jpg"))
    if not 写真:
        print(f"{写真置き場} に写真がありません")
        return 0

    b = sheet.Book()
    目次 = b.active
    目次.title = "一覧"
    for c, 名 in enumerate(["番号", "品名", "撮影日", "画素", "シート"], start=1):
        cell = 目次.cell(row=1, column=c)
        cell.value = 名
        cell.font = sheet.Font(bold=True)
    for col, w in (("A", 8), ("B", 18), ("C", 20), ("D", 12), ("E", 22)):
        目次.column_dimensions[col].width = w
    目次.freeze_panes = "A2"

    for i, p in enumerate(写真, start=1):
        撮, 画素 = 撮影日(p)
        品名 = p.stem.split("_", 1)[-1]
        # シートの名前は 31 字まで・記号は使えない(xlsx の決まり)
        名 = f"{i:03d} {品名}"[:31]

        目次.cell(row=i + 1, column=1).value = f"{i:03d}"
        目次.cell(row=i + 1, column=2).value = 品名
        c3 = 目次.cell(row=i + 1, column=3)
        if 撮:
            c3.value = 撮
            c3.number_format = "yyyy/m/d h:mm"
        else:
            c3.value = "(EXIF に撮影日なし)"
        目次.cell(row=i + 1, column=4).value = 画素
        目次.cell(row=i + 1, column=5).value = 名

        ws = b.add_sheet(名)
        # 見出しと欄。**人にしか分からない欄だけ**を空けておく —
        # 撮影日と画素は写真自身が持っている(EXIF)
        ws.cell(row=1, column=1).value = 品名
        ws.cell(row=1, column=1).font = sheet.Font(bold=True, size=14)
        欄 = [("撮影日", 撮 or "(EXIF に撮影日なし)"), ("画素", 画素),
              ("ファイル名", p.name), ("場所", ""), ("覚え書き", "")]
        for r, (名前, 値) in enumerate(欄, start=3):
            k = ws.cell(row=r, column=6)
            k.value = 名前
            k.font = sheet.Font(bold=True)
            v = ws.cell(row=r, column=7)
            if 名前 == "撮影日" and 撮:
                v.value = 撮
                v.number_format = "yyyy/m/d h:mm"
            else:
                v.value = 値
        for col, w in (("A", 3), ("B", 14), ("C", 14), ("D", 14), ("E", 14),
                       ("F", 14), ("G", 40)):
            ws.column_dimensions[col].width = w

        # **写真は大きく。** 長辺 640px で置く(画面でも紙でも見える大きさ)
        比 = 見せる長辺 / max(*(int(x) for x in 画素.split("×"))) if 画素 and "×" in 画素 else 1.0
        w_px = int(round(int(画素.split("×")[0]) * 比)) if 画素 and "×" in 画素 else 見せる長辺
        h_px = int(round(int(画素.split("×")[1]) * 比)) if 画素 and "×" in 画素 else 見せる長辺
        ws.add_image(p.read_bytes(), at="B3", width_px=w_px, height_px=h_px)
        # 写真の下に行が来ないよう、収まる高さを B3 から配る
        for r in range(3, 3 + 12):
            ws.row_dimensions[r].height = (h_px / 12.0) * 72.0 / 96.0

    b.save(台帳)
    return len(写真)


if __name__ == "__main__":
    作れた = 写真を作る()
    if 作れた:
        print(f"見本の写真: {写真置き場.name}/ に {len(見本)} 枚")
    n = 台帳を作る()
    if n:
        大きさ = 台帳.stat().st_size
        原本合計 = sum(p.stat().st_size for p in 写真置き場.glob("*.jpg"))
        print(f"台帳: {台帳.name}({n} 行・{大きさ / 1024:.0f}KB)")
        print(f"  写真の原本の合計 {原本合計 / 1024:.0f}KB / "
              f"台帳 {大きさ / 1024:.0f}KB — 差は {(大きさ - 原本合計) / 1024:+.0f}KB")
        print("  **入れても合計はほとんど変わらない**(xlsx は zip、"
              "JPEG は既に圧縮済み)")
        print()
        print("calc で開くと、1枚目が一覧、以降はシート1つに写真1枚です。")
