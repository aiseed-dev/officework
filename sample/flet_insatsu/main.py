# -*- coding: utf-8 -*-
"""Print a docx or xlsx from a Flet app: officework makes the PDF and the
page pictures for the preview, flet-printing opens the machine's print
dialog.

    flet run --web main.py   # preview and PDF; the print dialog needs a build
    flet build macos         # (or windows / linux) the print dialog as well;
                             # flet-printing is a Flutter extension and is
                             # only there in a built app

The preview is the engine's own rendering (save("x.png")), so what is on
the screen is what goes to the printer. No extension is needed for it.
"""
import os
import pathlib

import flet as ft
from officework import doc, sheet

try:
    from flet_printing import Printing
except ImportError:  # not installed; the app still previews and makes the PDF
    Printing = None

# flet build sets FLET_PLATFORM; flet run does not. Only a built app carries
# the Flutter extension, so the print dialog is offered only there
BUILT = os.getenv("FLET_PLATFORM") is not None

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE / "assets" / "out"          # served by Flet as out/<name>.pdf
SAMPLE = HERE.parent                   # ../ has 報告書.docx, 見積書.xlsx and so on
DPI = 96                               # preview pictures; print uses the PDF


def open_file(path):
    path = pathlib.Path(path).expanduser()
    if not path.is_absolute():
        path = SAMPLE / path
    ext = path.suffix.lower()
    if ext == ".docx":
        return path, doc.Doc.open(str(path))
    if ext in (".xlsx", ".xlsm"):
        b = sheet.Book.open(str(path))
        b.recalc()
        return path, b
    raise ValueError(f"docx か xlsx を指定してください: {path.name}")


def render(path):
    """docx or xlsx -> PDF and one PNG per page under assets/out.
    Returns (pdf_path, [png_path, ...]) in page order."""
    path, f = open_file(path)
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob(path.stem + "*.png"):
        old.unlink()
    pdf = OUT / (path.stem + ".pdf")
    f.save(str(pdf))
    f.save(str(OUT / (path.stem + ".png")), dpi=DPI)
    # page 1 is <stem>.png, then <stem>-2.png, <stem>-3.png ...
    pages = sorted(OUT.glob(path.stem + "*.png"),
                   key=lambda q: 1 if q.stem == path.stem else int(q.stem.rsplit("-", 1)[1]))
    return pdf, pages


def main(page: ft.Page):
    page.title = "officework + flet-printing"
    page.padding = 24
    page.scroll = ft.ScrollMode.AUTO

    printing = None
    if BUILT and Printing is not None:
        printing = Printing()
        page.services.append(printing)

    path = ft.TextField(label="docx か xlsx", value="報告書.docx", width=420)
    status = ft.Text("")
    open_btn = ft.TextButton("PDF を開く", visible=False)
    preview = ft.Row(wrap=True, spacing=16, run_spacing=16)
    state = {"pdf": None}

    def show(msg):
        status.value = msg
        page.update()

    def make(_):
        try:
            pdf, pages = render(path.value)
        except Exception as e:
            show(f"PDF にできませんでした: {e}")
            return
        state["pdf"] = pdf
        preview.controls = [
            ft.Container(
                ft.Image(src=p.read_bytes(), width=420, fit=ft.BoxFit.CONTAIN),
                border=ft.Border.all(1, ft.Colors.BLUE_GREY_200),
                bgcolor=ft.Colors.WHITE,
            )
            for p in pages
        ]
        # a link, not a service call: works in the browser and on the desktop
        open_btn.url = f"out/{pdf.name}" if page.web else pdf.as_uri()
        open_btn.visible = True
        print_btn.disabled = printing is None
        show(f"{len(pages)} 頁。PDF: {pdf}")

    async def print_(_):
        if state["pdf"] is None or printing is None:
            return
        ok = await printing.print_pdf(state["pdf"].read_bytes(), name=state["pdf"].stem)
        show("印刷しました" if ok else "印刷を取りやめました")

    print_btn = ft.FilledButton("印刷…", on_click=print_, disabled=True)

    note = ("印刷ダイアログ(プリンターの選択・部数・両面)は flet-printing が開きます。"
            if printing is not None else
            "印刷ダイアログは flet build で組んだアプリで出ます(flet-printing は Flutter の拡張です)。"
            "ここでは「PDF を開く」でブラウザから刷れます。")

    page.add(
        ft.Row([path, ft.FilledButton("PDF にする", on_click=make), print_btn, open_btn], wrap=True),
        status,
        ft.Text(note, size=13, color=ft.Colors.BLUE_GREY_600),
        preview,
    )


if __name__ == "__main__":
    ft.run(main, assets_dir="assets")
