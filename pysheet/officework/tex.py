# -*- coding: utf-8 -*-
"""数式を **LaTeX で受けて、エンジンが組む**。

    from officework import tex
    png, w_mm, h_mm = tex.to_png(r"\\frac{a+b}{2}", size_pt=11)   # 文書に入れるのはこちら
    svg = tex.to_svg(r"\\frac{a+b}{2}")                          # bytes(SVG)。字は輪郭

組むのは Rust のエンジン(typst と mitex)です。Python の側には何も要りません
(2026-09-02 の決め。前は TeX か matplotlib に組ませていました)。
出来上がりは TeX と同じ書体(New Computer Modern Math)です。
数式の中の日本語は、`font` で指定した書体(省略すると機械の既定)で出ます。

組めない式は `Muri` で断ります。理由も付きます。**黙って空の絵を返しません。**
"""

from . import _sheet

__all__ = ["to_svg", "to_png", "fit", "from_sympy", "kumi_kata", "Muri"]


class Muri(Exception):
    """組めない数式。message に理由が入る。"""


def kumi_kata():
    """いま何で組むか。エンジン(typst)で固定なので、いつも "typst"。"""
    return "typst"


def fit(tex):
    """`$…$` や `\\[…\\]` で囲んであっても受ける(LaTeX を貼る人の癖)。
    前は matplotlib が読める形に寄せていましたが、いまは寄せる必要が無いので、
    囲いを外して返すだけです。"""
    t = (tex or "").strip()
    for a, b in (("$$", "$$"), ("\\[", "\\]"), ("$", "$"), ("\\(", "\\)")):
        if t.startswith(a) and t.endswith(b) and len(t) >= len(a) + len(b):
            t = t[len(a):len(t) - len(b)].strip()
            break
    return t


def from_sympy(shiki):
    """SymPy の式から LaTeX を起こす(Python の式で数式を書きたいとき)。

    **式は書き直される。** SymPy は意味で持つので、`(a+b)/2` は
    `\\frac{a}{2} + \\frac{b}{2}` になる — 書いたとおりの見た目が要るなら
    LaTeX を直に渡すこと。検算(値を入れて評価する)ができるのが取り柄。
    """
    try:
        from sympy import latex, sympify
    except ImportError as e:
        raise Muri("sympy が入っていません(LaTeX を直に渡してください)") from e
    return latex(sympify(shiki) if isinstance(shiki, str) else shiki)


def to_svg(tex, size_pt=11.0, color="#000000", font=None):
    """LaTeX を SVG(bytes)に組む。**組めなければ Muri で断る。**"""
    try:
        return _sheet.suushiki_svg(fit(tex), float(size_pt), color, font).encode("utf-8")
    except ValueError as e:
        raise Muri(str(e)) from None


def to_png(tex, size_pt=11.0, color="#000000", font=None):
    """LaTeX を PNG に組み、(bytes, 幅 mm, 高さ mm) を返す。
    **組めなければ Muri で断る。**"""
    try:
        png, w, h = _sheet.suushiki_png(fit(tex), float(size_pt), color, font)
    except ValueError as e:
        raise Muri(str(e)) from None
    return bytes(png), float(w), float(h)
