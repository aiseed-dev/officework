"""`openpyxl.drawing.image` と同じ名前。

`Image(径路か開いたファイルか PIL の絵)` を作り、`ws.add_image(img, "B2")` に
渡します。`officework.sheet` の `add_image` は、この形(`ref`・`anchor`・
`width`・`height`)を openpyxl の `Image` として読みます。
"""


class Image:
    def __init__(self, img):
        self.ref = img
        self.anchor = None
        self.width = None
        self.height = None
        self.format = "png"


__all__ = ["Image"]
