# 橋(officework.calc)の実機検査 — 動いている calc に繋いで、xlwings 互換の
# 口をひととおり動かす。画面まわりの直しの ribbon_sweep.py と対になる道具。
#
# 使い方(起動と**同じ XDG_RUNTIME_DIR** で回すこと — ソケットの径路を揃える):
#   env -u WAYLAND_DISPLAY XDG_RUNTIME_DIR=$SP/xdg DISPLAY=:0 ./target/release/calc &
#   XDG_RUNTIME_DIR=$SP/xdg .venv/bin/python tools/hashi_check.py
#
# 検査はいま出ているブックに書き込む(未保存の変更が残っていても attach で続ける)。
import os
import sys
import tempfile

from officework import calc as xw


def check(cond, msg):
    if not cond:
        print(f"NG: {msg}", file=sys.stderr)
        sys.exit(1)


pong = xw.ping()
check(pong.get("app") == "calc", f"ping の名乗り: {pong}")
check(pong.get("version", "") != "", "ping に version が無い")

# 新しいブックに付く(未保存の変更が残っていたら、そのまま付く)
try:
    wb = xw.Book()
except xw.OfficeworkError:
    wb = xw.Book.attach()
check(wb.name != "", "Book の名前が空")
check(isinstance(wb.sheet_names, list) and len(wb.sheet_names) >= 1,
      f"sheet_names: {wb.sheet_names}")
check(wb.app.books.active.name == wb.name, "wb.app.books.active が自分に戻らない")
check(wb.app.version == pong["version"], "app.version が ping と食い違う")

sh = wb.sheets.active
check(sh.index == 1, f"先頭シートの index が 1 でない: {sh.index}")
check(sh.book.name == wb.name, "Sheet.book がブックに戻らない")

# まっさらから始める(前の残りがあっても、この2行で盤面が決まる)
sh.clear()
check(sh.used_range.shape == (1, 1), "clear の後に何か残っている")

# 値の書き・読み(2次元 → 表)
sh["A1"].value = [["品名", "数", "単価", "金額"],
                  ["ザボガードF", 4, 125000, None]]
check(sh["B2"].value == 4, f"書いた値が読めない: {sh['B2'].value}")

# 式: 書いて、計算された値が返る
sh["D2"].value = "=B2*C2"
check(sh["D2"].value == 500000, f"式が実機で計算されない: {sh['D2'].value}")
check(sh["D2"].formula == "=B2*C2", f"式が式で戻らない: {sh['D2'].formula}")
check(sh["D2"].formula2 == "=B2*C2", "formula2(別名)が食い違う")
check(sh["D2"].raw_value == 500000, "raw_value が食い違う")
check(sh["D2"].get_value() == 500000, "get_value が食い違う")

# expand / used_range / current_region(実機の盤面で)
tbl = sh["A1"].expand("table")
check(tbl.shape == (2, 4), f"expand の形: {tbl.shape}")
check(sh.used_range.shape == (2, 4), f"used_range の形: {sh.used_range.shape}")
check(sh["A1"].current_region.shape == (2, 4),
      f"current_region の形: {sh['A1'].current_region.shape}")

# 参照の算術と実機の読みが噛み合う
check([r.value for r in tbl.rows[0].columns] == ["品名", "数", "単価", "金額"],
      "rows/columns の刻みと実機の値が食い違う")
check(tbl.last_cell.value == 500000, f"last_cell の値: {tbl.last_cell.value}")
check(sh["A1"].offset(1, 0).value == "ザボガードF", "offset の先の値")
check(tbl.address == "$A$1:$D$2", f"address: {tbl.address}")
check(tbl.sheet.name == sh.name, "Range.sheet がシートに戻らない")

# select / selection: 選択を動かし、読み戻す
sh["B1:C2"].select()
sel = wb.selection
check(sel.get_address(False, False) == "B1:C2",
      f"select → selection の往復: {sel.get_address(False, False)}")
check(wb.app.selection.get_address(False, False) == "B1:C2",
      "app.selection が食い違う")

# load: 選んで DataFrame(1マスなら表に広げる)
sh["A1"].select()
df = xw.load()
check(df.shape == (1, 4) or df.shape == (2, 4),
      f"load の形(polars は見出し1行+中身): {df.shape}")

# end: Ctrl+矢印相当(端は使っている範囲まで)
check(sh["A1"].end("right").address == "$D$1", f"end(right): {sh['A1'].end('right').address}")
check(sh["A1"].end("down").address == "$A$2", f"end(down): {sh['A1'].end('down').address}")
check(sh["D2"].end("up").address == "$D$1", f"end(up): {sh['D2'].end('up').address}")

# merge / merge_area / merge_cells / unmerge(家の作法どおり)
sh["A4:B5"].merge()
check(sh["A4"].merge_cells, "merge の後に merge_cells が False")
check(sh["A4"].merge_area.get_address(False, False) == "A4:B5",
      f"merge_area: {sh['A4'].merge_area.get_address(False, False)}")
check(not sh["D4"].merge_cells, "関係ない範囲まで merge_cells が True")
sh["A4:B5"].unmerge()
check(not sh["A4"].merge_cells, "unmerge の後も merge_cells が True")

# clear_contents: 値は消え、書式は据え置き(書式の検査は書式の口が来てから)
sh["A2:D2"].clear_contents()
check(sh["A2"].value is None and sh["D2"].value is None, "clear_contents で消えない")
check(sh.used_range.shape == (1, 4), f"clear_contents 後の used_range: {sh.used_range.shape}")

# status_bar: アプリの状態行に文言を出す
wb.app.status_bar = "橋の検査中(hashi_check.py)"
check(wb.app.status_bar == "橋の検査中(hashi_check.py)", "status_bar の覚えが違う")

# calculate: 呼べて、値が保たれる
wb.app.calculate()
check(sh["A1"].value == "品名", "calculate の後に値が変わった")

# activate: シートを切り替えて見せる(2枚目が無ければ作らない — 見るだけ)
names = wb.sheet_names
if len(names) > 1:
    wb.sheets[names[1]].activate()
    check(wb.sheets.active.name == names[1], "activate が効かない")
    wb.sheets[names[0]].activate()

# 書式(xlwings の形): 塗り・font の性質ごとの書き・表示形式・折り返し・列幅・行高
sh["A1"].color = (255, 255, 0)
check(sh["A1"].color == (255, 255, 0), f"color の往復: {sh['A1'].color}")
sh["A1"].font.bold = True
sh["A1"].font.size = 14
check(sh["A1"].font.bold and sh["A1"].font.size == 14,
      f"font の性質ごとの書き: {sh['A1'].font.bold},{sh['A1'].font.size}")
sh["A1"].font.color = "#C00000"
check(sh["A1"].font.color == (192, 0, 0), f"font.color: {sh['A1'].font.color}")
sh["B1:C1"].number_format = "#,##0"
check(sh["B1"].number_format == "#,##0" and sh["C1"].number_format == "#,##0",
      "number_format が範囲に効かない")
sh["A1"].wrap_text = True
check(sh["A1"].wrap_text, "wrap_text が効かない")
sh["A1:B1"].column_width = 20
check(sh["A1"].column_width == 20 and sh["B1"].column_width == 20,
      f"column_width: {sh['A1'].column_width}")
sh["A2"].row_height = 30
check(sh["A2"].row_height == 30, f"row_height: {sh['A2'].row_height}")
sh["A1"].clear_formats()
check(sh["A1"].color is None and not sh["A1"].font.bold,
      "clear_formats で書式が残っている")
check(sh["B1"].number_format == "#,##0", "clear_formats が隣まで消した")

# copy / delete: 複製は中身ごと・右隣に。削除で元に戻る(undo の束は消える)
names0 = wb.sheet_names
c1 = sh.copy()
check(c1.name in wb.sheet_names and len(wb.sheet_names) == len(names0) + 1,
      f"copy でシートが増えない: {wb.sheet_names}")
check(wb.sheets[c1.name]["A1"].value == "品名", "複製に中身が写っていない")
c2 = sh.copy(name="写し")
check("写し" in wb.sheet_names, "名前つき copy が効かない")
try:
    sh.copy(name=c1.name)
    check(False, "同じ名前の copy が黙って通った")
except xw.OfficeworkError:
    pass
c2.delete()
c1.delete()
check(wb.sheet_names == names0, f"delete で戻らない: {wb.sheet_names}")
try:
    while len(wb.sheet_names) > 1:  # 最後の1枚まで消して、断られ方を見る
        wb.sheets[wb.sheet_names[-1]].delete()
    wb.sheets[wb.sheet_names[0]].delete()
    check(False, "最後の1枚の削除が黙って通った")
except xw.OfficeworkError:
    pass

# 行・列の出し入れ: **残った式の参照が付いて動く**(明細の行を増やす操作)
sh["H1"].value = 10
sh["H2"].value = "=H1*3"
check(sh["H2"].value == 30, f"下ごしらえ: {sh['H2'].value}")
sh.insert_rows(1)                    # 1行目に挿す — H1→H2、式は H3 へ
check(sh["H2"].value == 10, f"挿入で値が動かない: {sh['H2'].value}")
check(sh["H3"].formula == "=H2*3",
      f"挿入で式の参照が追随しない: {sh['H3'].formula}")
check(sh["H3"].value == 30, "追随した式の答えが違う")
sh.delete_rows(1)                    # 戻す
check(sh["H1"].value == 10 and sh["H2"].formula == "=H1*3", "削除で戻らない")
sh["G1:G2"].insert()                 # Range から2行(shape の行数ぶん)
check(sh["H3"].value == 10, f"Range.insert が2行入っていない: {sh['H3'].value}")
sh["G1:G2"].delete()
check(sh["H1"].value == 10, "Range.delete で戻らない")
sh["H1:H2"].clear()

# pictures: Python の絵が実機のシートに浮かぶ(SEKKEI「calc の分業」の筋)
png1x1 = (b"\x89PNG\r\n\x1a\n"
          b"\x00\x00\x00\rIHDR\x00\x00\x00\x02\x00\x00\x00\x02"
          b"\x08\x02\x00\x00\x00\xfd\xd4\x9as"
          b"\x00\x00\x00\x0cIDATx\x9cc\xf8\xff\xff?\x00\x05\xfe\x02\xfe"
          b"\xa75\x81\x84\x00\x00\x00\x00IEND\xaeB`\x82")
n0 = len(sh.pictures)
p1 = sh.pictures.add(png1x1, anchor="F4", width=120)  # 片方 → 縦横比を保つ
check(len(sh.pictures) == n0 + 1, "add で画像が増えない")
check(p1.width == 120 and p1.height == 120, f"縦横比: {p1.width}×{p1.height}")
check(any(p.anchor == "F4" for p in sh.pictures), f"留めたセル: {list(sh.pictures)}")
try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    fig, ax = plt.subplots(figsize=(3, 2))
    # 札は ASCII(検査は日本語フォントを matplotlib に登録しない — 豆腐を出さない)
    ax.bar(["A", "B", "C"], [4, 7, 2])
    ax.set_title("bar")
    sh.pictures.add(fig, anchor="F12", width=240)  # figure をそのまま
    plt.close(fig)
    check(len(sh.pictures) == n0 + 2, "matplotlib の figure が貼れない")
except ImportError:
    print("matplotlib が無いので figure の検査は飛ばした", file=sys.stderr)

# 名前付き範囲: add・refers_to・Range.name・式の追随・delete
sh["C2"].value = 125000   # 上の clear_contents で消えているので置き直す
wb.names.add("単価", "=%s!$C$2" % sh.name)
check("単価" in wb.names and len(wb.names) == 1, f"names.add: {wb.names}")
check(wb.names["単価"].refers_to == "=%s!$C$2" % sh.name,
      f"refers_to: {wb.names['単価'].refers_to}")
sh["E2"].value = "=単価*2"
check(sh["E2"].value == 250000, f"名前が実機の式で効かない: {sh['E2'].value}")
check(sh["C2"].name.name == "単価", "Range.name が引けない")
sh["F1:F2"].name = "対象"
check(wb.names["対象"].refers_to.endswith("$F$1:$F$2"), "Range.name の代入")
wb.names["対象"].delete()
wb.names["単価"].delete()
check(len(wb.names) == 0, "delete で名前が消えない")
sh["E2"].clear_contents()  # #NAME? を残さない

# freeze_panes: 固定して・読めて・解ける(xlwings の freeze_at の定義どおり)
fp = sh.freeze_panes
fp.freeze_at("B2")  # 上2行・左2列
r = xw._call("freeze", sheet=sh.name)
check((r["rows"], r["cols"]) == (2, 2), f"freeze_at('B2'): {r}")
fp.freeze_at("1:1")  # 上1行だけ
r = xw._call("freeze", sheet=sh.name)
check((r["rows"], r["cols"]) == (1, 0), f"freeze_at('1:1'): {r}")
fp.freeze_at("A:A")  # 左1列だけ
r = xw._call("freeze", sheet=sh.name)
check((r["rows"], r["cols"]) == (0, 1), f"freeze_at('A:A'): {r}")
fp.unfreeze()
r = xw._call("freeze", sheet=sh.name)
check((r["rows"], r["cols"]) == (0, 0), "unfreeze が解けていない")

# visible: 隠して・戻せて、最後の1枚は断られる
cv = sh.copy()
sv = wb.sheets[cv.name]
sv.visible = False
check(not sv.visible, "visible=False が効かない")
check(wb.sheets.active.name != sv.name, "隠したシートが画面に出たまま")
sv.visible = True
check(sv.visible, "visible=True で戻らない")
sv.delete()
if len(wb.sheet_names) == 1:
    try:
        wb.sheets.active.visible = False
        check(False, "最後の1枚の非表示が黙って通った")
    except xw.OfficeworkError:
        pass

# to_pdf: シートを PDF に(帳票の印刷設定に従う)
sh = wb.sheets.active  # 削除で札が古びたかもしれないので引き直す
with tempfile.TemporaryDirectory() as t:
    out = os.path.join(t, "hashi.pdf")
    got = sh.to_pdf(out)
    check(got == out and os.path.exists(out) and os.path.getsize(out) > 0,
          "to_pdf が PDF を書かない")

print("OK")
