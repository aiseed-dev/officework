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

# to_pdf: シートを PDF に(帳票の印刷設定に従う)
sh = wb.sheets.active  # 削除で札が古びたかもしれないので引き直す
with tempfile.TemporaryDirectory() as t:
    out = os.path.join(t, "hashi.pdf")
    got = sh.to_pdf(out)
    check(got == out and os.path.exists(out) and os.path.getsize(out) > 0,
          "to_pdf が PDF を書かない")

print("OK")
