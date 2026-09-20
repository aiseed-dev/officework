# -*- coding: utf-8 -*-
# Make 事業のご報告.docx: a short business report with two charts and a table.
# Everything in it is made up. The charts are drawn here with Pillow so the
# sample needs nothing but officework and Pillow.
#
#   .venv/bin/python sample/gen_hokoku.py
import pathlib

from PIL import Image, ImageDraw, ImageFont
from officework import doc

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE / "事業のご報告.docx"
FONT = "/System/Library/Fonts/ヒラギノ角ゴシック W6.ttc"
FONT_R = "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc"
S = 3  # supersampling for smooth edges

NAVY = (28, 52, 92)
BLUE = (66, 133, 244)
TEAL = (52, 168, 130)
AMBER = (244, 180, 0)
CORAL = (234, 88, 66)
GREY = (120, 120, 120)
LIGHT = (235, 238, 243)


def font(size, regular=False):
    return ImageFont.truetype(FONT_R if regular else FONT, size * S)


def bar_chart(path):
    w, h = 1600, 900
    im = Image.new("RGB", (w * S, h * S), "white")
    d = ImageDraw.Draw(im)
    months = ["4月", "5月", "6月", "7月", "8月", "9月"]
    sales = [42, 48, 45, 57, 61, 68]
    plan = [45, 47, 50, 52, 55, 58]
    left, top, right, bottom = 160 * S, 120 * S, (w - 60) * S, (h - 120) * S
    d.text((60 * S, 30 * S), "売上の推移(百万円)", font=font(40), fill=NAVY)
    for i in range(0, 5):
        y = bottom - (bottom - top) * i // 4
        d.line([(left, y), (right, y)], fill=LIGHT, width=2 * S)
        d.text((60 * S, y - 18 * S), f"{i * 20}", font=font(28, True), fill=GREY)
    n = len(months)
    slot = (right - left) // n
    bw = slot * 0.5
    for i, (m, v, p) in enumerate(zip(months, sales, plan)):
        x0 = left + slot * i + (slot - bw) / 2
        y0 = bottom - (bottom - top) * v / 80
        d.rectangle([x0, y0, x0 + bw, bottom], fill=BLUE)
        d.text((left + slot * i + slot / 2 - 24 * S, bottom + 20 * S), m, font=font(28, True), fill=GREY)
        d.text((x0 + bw / 2 - 22 * S, y0 - 44 * S), str(v), font=font(28), fill=NAVY)
    pts = [(left + slot * i + slot / 2, bottom - (bottom - top) * p / 80) for i, p in enumerate(plan)]
    d.line(pts, fill=CORAL, width=6 * S, joint="curve")
    for x, y in pts:
        d.ellipse([x - 10 * S, y - 10 * S, x + 10 * S, y + 10 * S], fill="white", outline=CORAL, width=5 * S)
    d.rectangle([right - 330 * S, top - 70 * S, right - 300 * S, top - 40 * S], fill=BLUE)
    d.text((right - 285 * S, top - 76 * S), "実績", font=font(28, True), fill=GREY)
    d.line([(right - 180 * S, top - 55 * S), (right - 140 * S, top - 55 * S)], fill=CORAL, width=6 * S)
    d.text((right - 125 * S, top - 76 * S), "計画", font=font(28, True), fill=GREY)
    im.resize((w, h), Image.LANCZOS).save(path)


def pie_chart(path):
    w, h = 1600, 900
    im = Image.new("RGB", (w * S, h * S), "white")
    d = ImageDraw.Draw(im)
    d.text((60 * S, 30 * S), "地域の内訳", font=font(40), fill=NAVY)
    parts = [("関東", 46, BLUE), ("関西", 27, TEAL), ("中部", 17, AMBER), ("その他", 10, CORAL)]
    cx, cy, r = 520 * S, 490 * S, 320 * S
    a = -90
    for name, v, c in parts:
        b = a + 360 * v / 100
        d.pieslice([cx - r, cy - r, cx + r, cy + r], a, b, fill=c, outline="white", width=6 * S)
        a = b
    d.ellipse([cx - r * 0.55, cy - r * 0.55, cx + r * 0.55, cy + r * 0.55], fill="white")
    d.text((cx - 70 * S, cy - 28 * S), "上期", font=font(44), fill=NAVY)
    y = 260 * S
    for name, v, c in parts:
        d.rectangle([980 * S, y, 1030 * S, y + 50 * S], fill=c)
        d.text((1060 * S, y - 2 * S), f"{name}  {v}%", font=font(40, True), fill=NAVY)
        y += 110 * S
    im.resize((w, h), Image.LANCZOS).save(path)


def main():
    tmp = HERE / ".hokoku"
    tmp.mkdir(exist_ok=True)
    bar = tmp / "bar.png"
    pie = tmp / "pie.png"
    bar_chart(bar)
    pie_chart(pie)

    d = doc.Document()
    d.add_heading("事業のご報告", 0)
    p = d.add_paragraph("2026 年度 上期(4 月〜9 月)  株式会社みほん商事")
    d.add_paragraph("上期の売上は計画を上回り、6 か月のうち 5 か月で計画を超えました。"
                    "関東の伸びが全体を引き上げ、中部は新しい取引先が 3 社増えました。")
    d.add_heading("売上の推移", 1)
    d.add_picture(str(bar), width=160)
    d.add_paragraph("7 月以降、実績(青)が計画(赤)を上回っています。9 月は 68 百万円で、上期で最も高い月になりました。")
    d.add_heading("地域の内訳", 1)
    d.add_picture(str(pie), width=160)
    d.add_paragraph("関東が半分近くを占めます。その他の地域は 10% ですが、前年の 6% から伸びています。")
    d.add_heading("主な取引先", 1)
    rows = [("取引先", "地域", "上期の売上(百万円)", "前年比"),
            ("例示工務店", "関東", "82", "+12%"),
            ("みほん建設", "関西", "64", "+8%"),
            ("架空設備", "中部", "41", "+21%"),
            ("仮名商会", "関東", "35", "-3%")]
    t = d.add_table(len(rows), 4, style="Table Grid")
    for i, row in enumerate(rows):
        for j, v in enumerate(row):
            t.cell(i, j).text = v
    d.add_heading("下期に向けて", 1)
    d.add_paragraph("中部の新しい取引先への納品を 10 月から始めます。関西は担当を 1 人増やします。")
    d.save(str(OUT))
    print(OUT)


if __name__ == "__main__":
    main()
