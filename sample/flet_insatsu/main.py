# -*- coding: utf-8 -*-
"""Print a docx or xlsx from a Flet app: officework makes the PDF,
flet-printing opens the machine's print dialog.

    flet run --web main.py   # the page and the PDF; the print dialog needs a build
    flet build macos         # (or windows / linux) the print dialog as well;
                             # flet-printing is a Flutter extension and is
                             # only there in a built app

Without the built extension the "印刷…" button opens the PDF in the
browser instead, which can print it from there.
"""
import os
import pathlib
import webbrowser

import flet as ft
from officework import doc, sheet

try:
    from flet_printing import PdfPreview, Printing
except ImportError:  # not installed; the app still makes the PDF
    PdfPreview = Printing = None

# flet build sets FLET_PLATFORM; flet run does not. Only a built app carries
# the Flutter extension, so the dialog and the preview are used only there
BUILT = os.getenv("FLET_PLATFORM") is not None

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE / "assets" / "out"          # served by Flet as out/<name>.pdf
SAMPLE = HERE.parent                   # ../ has 報告書.docx, 見積書.xlsx and so on


def to_pdf(path):
    """docx or xlsx -> PDF under assets/out. Returns the PDF path."""
    path = pathlib.Path(path).expanduser()
    if not path.is_absolute():
        path = SAMPLE / path
    OUT.mkdir(parents=True, exist_ok=True)
    out = OUT / (path.stem + ".pdf")
    ext = path.suffix.lower()
    if ext == ".docx":
        return pathlib.Path(doc.Doc.open(str(path)).to_pdf(str(out)))
    if ext in (".xlsx", ".xlsm"):
        b = sheet.Book.open(str(path))
        b.recalc()
        return pathlib.Path(b.to_pdf(str(out)))
    raise ValueError(f"docx か xlsx を指定してください: {path.name}")


def main(page: ft.Page):
    page.title = "officework + flet-printing"
    page.padding = 24

    printing = None
    if BUILT and Printing is not None:
        printing = Printing()
        page.services.append(printing)
    launcher = ft.UrlLauncher()  # Flet 1.0: opening a URL is a service too
    page.services.append(launcher)

    path = ft.TextField(label="docx か xlsx", value="報告書.docx", width=420)
    status = ft.Text("")
    preview = ft.Column(expand=True)
    pdf = {"path": None}

    def show(msg):
        status.value = msg
        page.update()

    def make(_):
        try:
            pdf["path"] = to_pdf(path.value)
        except Exception as e:
            show(f"PDF にできませんでした: {e}")
            return
        preview.controls.clear()
        if printing is not None and PdfPreview is not None:
            preview.controls.append(
                PdfPreview(src=pdf["path"].read_bytes(), pdf_file_name=pdf["path"].name, expand=True)
            )
        print_btn.disabled = False
        show(f"PDF にしました: {pdf['path']}")

    async def print_(_):
        if pdf["path"] is None:
            return
        if printing is not None:
            ok = await printing.print_pdf(pdf["path"].read_bytes(), name=pdf["path"].stem)
            show("印刷しました" if ok else "印刷を取りやめました")
            return
        # no print dialog in this runtime: hand the PDF to the browser
        if page.web:
            await launcher.launch_url(f"out/{pdf['path'].name}", web_only_window_name="_blank")
        else:
            webbrowser.open(pdf["path"].as_uri())
        show("印刷ダイアログは組んだアプリで出ます。代わりに PDF を開きました。")

    print_btn = ft.FilledButton("印刷…", on_click=print_, disabled=True)

    page.add(
        ft.Row([path, ft.FilledButton("PDF にする", on_click=make), print_btn], wrap=True),
        status,
        ft.Text(
            "印刷ダイアログ(プリンターの選択・部数・両面)は flet-printing が開きます。"
            + ("" if printing is not None else " flet run では拡張が無いので、代わりに PDF を開きます。flet build で組むと出ます。"),
            size=13,
            color=ft.Colors.BLUE_GREY_600,
        ),
        preview,
    )


if __name__ == "__main__":
    ft.run(main, assets_dir="assets")
