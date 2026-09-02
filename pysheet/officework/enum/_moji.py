# -*- coding: utf-8 -*-
"""**本家と同じ整数。名前で語も引ける値。**

python-docx の列挙は整数です(`WD_ALIGN_PARAGRAPH.CENTER == 1`)。
本家の口へそのまま渡せるよう、こちらも整数を継ぎます。

こちらは揃えなどを `"center"` のような語で持つので、`.name` に語を
置きます。`officework` 側の受け口は `.name` を見るので、どちらの
ライブラリへ渡しても通ります(2026-09-01。本家の見本が
`docx.Document()` で作りながら列挙だけこちらから取っていて、
語を渡すと本家が「知らない値」で止まりました)。
"""


class Ne(int):
    """決まった値1つ。数は本家の番号、`.name` と `str()` は officework の語。"""

    # `int` を継ぐと `__slots__` は使えません(名前を後から入れるため)

    def __new__(cls, na, kazu):
        self = super().__new__(cls, kazu)
        self.name = na
        return self

    def __str__(self):
        return self.name

    def __repr__(self):
        return "{}({!r}, {})".format(type(self).__name__, self.name, int(self))


class Hyou:
    """名前 → 値の集まり。本家の列挙クラスの役です。"""

    def __init__(self, na, **kw):
        self._na = na
        for k, v in kw.items():
            setattr(self, k, v)

    def __repr__(self):
        return "<officework.enum {}>".format(self._na)
