"""A Flet component that shows a form filled by officework and edits its
fields (draft; docs/sekkei/drawlist.ja.adoc).

The lower layer is a Canvas that draws the draw list (Book.draw_list())
as it is: every position, size and font comes from the list. The upper
layer holds an input only for the field being edited. A click on the page
finds the field under it; when the input is confirmed, the value goes into
the data book, the form is filled again, and the new draw list is drawn.

    view = FormView(page, form, data, assets_dir="assets")
    page.add(view.control)

The fonts named in the draw list are copied under `assets_dir/fonts` and
registered in `page.fonts`, so pass the same folder to `ft.run(...,
assets_dir=...)`.

What Flet's canvas cannot draw: a path's even-odd fill and its clip
(`even_odd`, `clip`) are drawn as a plain fill without the clip.
"""
import base64
import math
import os
import shutil

import flet as ft
import flet.canvas as cv

from officework import sheet


def _color(c, alpha=1.0):
    return c if alpha >= 1 else ft.Colors.with_opacity(alpha, c)


def _family(font):
    """The family name the font is registered under in page.fonts."""
    return f"officework-{font['id']}"


def install_fonts(page, draw_list, assets_dir):
    """Copies the draw list's fonts under assets_dir/fonts and registers them."""
    os.makedirs(os.path.join(assets_dir, "fonts"), exist_ok=True)
    fonts = dict(page.fonts or {})
    for f in draw_list["fonts"]:
        name = os.path.basename(f["file"])
        dst = os.path.join(assets_dir, "fonts", name)
        if not os.path.exists(dst):
            shutil.copyfile(f["file"], dst)
        fonts[_family(f)] = f"fonts/{name}"
    page.fonts = fonts


def _elements(d, z):
    P = cv.Path
    out = []
    for seg in d:
        op = seg[0]
        if op == "M":
            out.append(P.MoveTo(seg[1] * z, seg[2] * z))
        elif op == "L":
            out.append(P.LineTo(seg[1] * z, seg[2] * z))
        elif op == "C":
            out.append(P.CubicTo(*[v * z for v in seg[1:]]))
        elif op == "Z":
            out.append(P.Close())
    return out


def shapes(page_list, fonts, z=1.0):
    """The canvas shapes of one page of a draw list, at zoom `z`
    (1 = one logical pixel per point)."""
    out = []
    for it in page_list["items"]:
        t = it["type"]
        a = it.get("alpha", 1)
        if t == "fill":
            x, y, w, h = it["rect"]
            out.append(cv.Rect(x * z, y * z, w * z, h * z,
                               paint=ft.Paint(color=_color(it["color"], a), style=ft.PaintingStyle.FILL)))
        elif t == "line":
            dash = [v * z for v in it.get("dash", [])] or None
            out.append(cv.Line(it["from"][0] * z, it["from"][1] * z, it["to"][0] * z, it["to"][1] * z,
                               paint=ft.Paint(color=_color(it["color"], a), stroke_width=it["width"] * z,
                                              style=ft.PaintingStyle.STROKE, stroke_dash_pattern=dash)))
        elif t == "path":
            if it.get("fill"):
                out.append(cv.Path(_elements(it["d"], z),
                                   paint=ft.Paint(color=_color(it["fill"], a), style=ft.PaintingStyle.FILL)))
            if it.get("stroke"):
                dash = [v * z for v in it.get("dash", [])] or None
                out.append(cv.Path(_elements(it["d"], z),
                                   paint=ft.Paint(color=_color(it["stroke"], a), stroke_width=it["width"] * z,
                                                  style=ft.PaintingStyle.STROKE, stroke_dash_pattern=dash)))
        elif t == "image":
            x, y, w, h = it["rect"]
            out.append(cv.Image(src=base64.b64decode(it["base64"]), x=x * z, y=y * z, width=w * z, height=h * z))
        elif t == "text":
            style = ft.TextStyle(size=it["size"] * z, font_family=_family(fonts[it["font"]]),
                                 color=it["color"],
                                 weight=ft.FontWeight.BOLD if it.get("bold") else None,
                                 italic=bool(it.get("italic")),
                                 letter_spacing=it["letter_spacing"] * z if "letter_spacing" in it else None)
            # The canvas places text by its top; the list gives that top.
            # The list turns left in degrees; the canvas turns right in radians
            out.append(cv.Text(it["x"] * z, it["top"] * z, it["text"], style=style,
                               rotate=-math.radians(it.get("rotation", 0))))
    return out


def field_at(page_list, x, y):
    """The field whose rectangle holds the point (points), or None."""
    for f in page_list["fields"]:
        fx, fy, fw, fh = f["rect"]
        if fx <= x <= fx + fw and fy <= y <= fy + fh:
            return f
    return None


class FormView:
    """A form filled from data, drawn page by page, with its fields editable."""

    def __init__(self, page, form, data, assets_dir="assets", zoom=1.0, on_change=None):
        self.page = page
        self.form = form
        self.data = data
        self.assets_dir = assets_dir
        self.zoom = zoom
        self.on_change = on_change
        self.editing = None  # (page index, field)
        self.draw_list = None
        self.control = ft.Column(spacing=16)
        self.refresh()

    def refresh(self):
        """Fills the form again and redraws every page."""
        self.draw_list = sheet.Book.fill(self.form, self.data).draw_list()
        install_fonts(self.page, self.draw_list, self.assets_dir)
        fonts = {f["id"]: f for f in self.draw_list["fonts"]}
        z = self.zoom
        pages = []
        for i, p in enumerate(self.draw_list["pages"]):
            w, h = p["size"]
            layers = [
                ft.Container(width=w * z, height=h * z, bgcolor=ft.Colors.WHITE),
                cv.Canvas(shapes=shapes(p, fonts, z), width=w * z, height=h * z),
                ft.GestureDetector(content=ft.Container(width=w * z, height=h * z),
                                   on_tap_down=lambda e, i=i: self._tap(i, e)),
            ]
            if self.editing and self.editing[0] == i:
                layers.append(self._editor(self.editing[1]))
            pages.append(ft.Stack(layers, width=w * z, height=h * z))
        self.control.controls = pages
        self.page.update()

    def _tap(self, i, e):
        pos = e.local_position
        f = field_at(self.draw_list["pages"][i], pos.x / self.zoom, pos.y / self.zoom)
        self.editing = (i, f) if f else None
        self.refresh()

    def _editor(self, f):
        z = self.zoom
        x, y, w, h = f["rect"]
        multi = f["kind"] == "multiline"
        hint = {"date": "2026-09-24", "image": "写真のファイル名"}.get(f["kind"])
        if f["kind"] == "choice":
            # One of the options, as the form writes them
            hint = "・".join(f.get("options", []))
        box = ft.TextField(value=f["value"], left=x * z, top=y * z, width=w * z,
                           height=None if multi else max(h * z, 32),
                           multiline=multi, min_lines=max(1, round(h / 16)) if multi else None,
                           text_size=10.5 * z, dense=True, autofocus=True, hint_text=hint,
                           bgcolor=ft.Colors.WHITE, content_padding=4)

        def done(e):
            self._set(f["name"], box.value)

        box.on_submit = done
        box.on_blur = done
        return box

    def _set(self, name, value):
        if self.editing is None:
            return
        self.editing = None
        self.data.set_field(name, value or "")
        if self.on_change:
            self.on_change(name, value)
        self.refresh()
