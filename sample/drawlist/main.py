"""The MHLW resume in the Flet component (draft).

    flet run main.py

Click a field to edit it; Enter or clicking elsewhere puts the value in the
data and draws the page again. "PDF に書き出す" saves the same pages as a
PDF next to this file.
"""
import asyncio
import pathlib

import flet as ft
from officework import sheet

from flet_form import FormView

HERE = pathlib.Path(__file__).resolve().parent
SAMPLE = HERE.parent / "rirekisho"
ASSETS = HERE / "assets"


def main(page: ft.Page):
    page.title = "履歴書"
    page.scroll = ft.ScrollMode.AUTO
    page.window.width = 820
    page.window.height = 1000
    form = sheet.Book.open(str(SAMPLE / "履歴書-厚労省.form.adoc"))
    data = sheet.Book.open(str(SAMPLE / "履歴書.sheet.adoc"))
    status = ft.Text("")
    view = FormView(page, form, data, assets_dir=str(ASSETS), zoom=1.2,
                    on_change=lambda name, value: setattr(status, "value", f"{name} を直しました"))

    def to_pdf(e):
        out = HERE / "履歴書.pdf"
        sheet.Book.fill(form, data).save(str(out))
        status.value = f"{out.name} に書き出しました"
        page.update()

    page.add(ft.Row([ft.Button("PDF に書き出す", on_click=to_pdf), status]), view.control)
    # Start at the top of the first page, once the pages are laid out
    async def to_top():
        await asyncio.sleep(0.5)
        await page.scroll_to(offset=0)

    page.run_task(to_top)


if __name__ == "__main__":
    # The fonts are copied in here when the form is drawn; Flet serves only
    # an assets folder that is there when it starts
    (ASSETS / "fonts").mkdir(parents=True, exist_ok=True)
    ft.run(main, assets_dir=str(ASSETS))
