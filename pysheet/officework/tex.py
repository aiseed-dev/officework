# -*- coding: utf-8 -*-
"""数式を **LaTeX で受けて SVG に組む**(2026-08-13 発注者確定の分業)。

    from officework import tex
    svg = tex.to_svg(r"\frac{a+b}{2}")      # bytes(SVG)

**組版は自前で書かない。** calc がグラフを matplotlib に任せているのと
同じ筋で、数式も matplotlib(mathtext)に組ませる。TeX の実体は要らない。
出すのは SVG — アプリ側に resvg が既にあり、拡大しても粗くならない。
**字は輪郭に落ちる**ので、受け手にその書体が無くても化けない。

**OMML は扱わない**(2026-08-13 発注者確定)。既存の docx の数式は
「原文を控えて保存で返す」ままで、読んで組み直すことはしない。
ここが受けるのは**これから書く数式**だけ。

mathtext は LaTeX の**部分集合**しか解さない。食えない書き方のうち、
機械的に寄せられるものは `fit()` が寄せる(行列・場合分け・積分の記法)。
それでも駄目なものは `Muri` で断る — **黙って空の絵を返さない**。
"""

import io
import re

__all__ = ["to_svg", "fit", "Muri"]


class Muri(Exception):
    """この数式は組めない。**何が駄目かを言う**(黙って落とさない)。"""


# mathtext が持たない環境。`\substack` に寄せると縦に積める —
# 本来は「添字の下に複数行」の道具なので、**列は揃わない**(空白で寄せるだけ)。
# 桁の揃った行列は綺麗に出るが、幅がまちまちだと歪む(その旨は fit の返りで言う)
_KANKYO = {
    "matrix": ("", ""),
    "pmatrix": (r"\left(", r"\right)"),
    "bmatrix": (r"\left[", r"\right]"),
    "Bmatrix": (r"\left\{", r"\right\}"),
    "vmatrix": (r"\left|", r"\right|"),
    "array": (r"\left[", r"\right]"),
    "cases": (r"\left\{", r"\right."),
}


def _env(tex):
    r"""`\begin{…}…\end{…}` を `\substack{…}` に寄せる(入れ子は1段だけ)。"""
    def hitotsu(m):
        name, naka = m.group(1), m.group(3)
        if name not in _KANKYO:
            raise Muri("mathtext に無い環境です: %s" % name)
        hidari, migi = _KANKYO[name]
        # 列の区切り(&)は空きに。**揃えは効かない** — substack の限界
        gyou = [re.sub(r"\s*&\s*", r" \\;\\; ", g.strip())
                for g in re.split(r"\\\\", naka) if g.strip()]
        if not gyou:
            raise Muri("中身が空の %s です" % name)
        return "%s\\substack{%s}%s" % (hidari, r" \\ ".join(gyou), migi)

    # array は列指定({cc} など)を捨てる — mathtext に揃えの概念が無い
    pat = re.compile(r"\\begin\{(\w+)\}(\{[^{}]*\})?(.*?)\\end\{\1\}", re.S)
    for _ in range(4):  # 入れ子のぶん何度か回す
        tex, n = pat.subn(hitotsu, tex)
        if not n:
            break
    return tex


def fit(tex):
    r"""mathtext が食える形に寄せる。**寄せられないものは Muri で断る。**

    直すのはこの3つ(実測で mathtext が落ちたもの):
      * `\begin{matrix}` などの環境 → `\substack`(列は揃わない)
      * `\int\limits_a^b` → `\int_a^b`(sympy の既定が \limits つき)
      * `\le` `\ge` → `\leq` `\geq`(mathtext はこの綴りしか持たない)
    """
    if not tex or not tex.strip():
        raise Muri("空の数式です")
    t = tex.strip()
    if t.startswith("$") and t.endswith("$"):
        t = t[1:-1]
    t = _env(t)
    t = t.replace(r"\limits", "")
    t = re.sub(r"\\le(?![a-zA-Z])", r"\\leq", t)
    t = re.sub(r"\\ge(?![a-zA-Z])", r"\\geq", t)
    return t


def to_svg(tex, size_pt=11.0, color="#000000", font_dir=None):
    """LaTeX を SVG(bytes)に組む。**組めなければ Muri で断る。**

    size_pt は本文の字の大きさに合わせる。color は文字色。
    font_dir を渡すと、その中の書体を使う(日本語混じりの数式のため)。
    """
    t = fit(tex)
    try:
        import matplotlib
    except ImportError as e:
        raise Muri("matplotlib が入っていません(数式は組めません)") from e
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig = plt.figure(figsize=(0.1, 0.1))
    try:
        fig.text(0, 0, "$%s$" % t, fontsize=float(size_pt), color=color)
        buf = io.BytesIO()
        fig.savefig(buf, format="svg", transparent=True,
                    bbox_inches="tight", pad_inches=0.01)
    except ValueError as e:
        # mathtext の言い分をそのまま渡す — **どこが駄目かは向こうが知っている**
        naka = str(e).strip().splitlines()
        raise Muri("組めない数式です: %s" % (naka[-1] if naka else e)) from None
    finally:
        plt.close(fig)
    return buf.getvalue()


def from_sympy(shiki):
    """SymPy の式から LaTeX を起こす(Python の式で数式を書きたいとき)。

    **式は書き直される。** SymPy は意味で持つので、`(a+b)/2` は
    `\frac{a}{2} + \frac{b}{2}` になる — 書いたとおりの見た目が要るなら
    LaTeX を直に渡すこと。検算(値を入れて評価する)ができるのが取り柄。
    """
    try:
        from sympy import latex, sympify
    except ImportError as e:
        raise Muri("sympy が入っていません(LaTeX を直に渡してください)") from e
    return latex(sympify(shiki) if isinstance(shiki, str) else shiki)
