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


def _hadaka(tex):
    """前後の空白と `$` を落とす(LaTeX を貼る人は $ ごと写すことが多い)。"""
    if not tex or not tex.strip():
        raise Muri("空の数式です")
    t = tex.strip()
    if t.startswith("$") and t.endswith("$") and len(t) > 1:
        t = t[1:-1].strip()
    if not t:
        raise Muri("空の数式です")
    return t


def fit(tex):
    r"""mathtext が食える形に寄せる。**寄せられないものは Muri で断る。**

    直すのはこの3つ(実測で mathtext が落ちたもの):
      * `\begin{matrix}` などの環境 → `\substack`(列は揃わない)
      * `\int\limits_a^b` → `\int_a^b`(sympy の既定が \limits つき)
      * `\le` `\ge` → `\leq` `\geq`(mathtext はこの綴りしか持たない)
    """
    t = _hadaka(tex)
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


def kumi_kata():
    """いま数式を何で組めるか。`"tex"` / `"mathtext"` / `None`。

    **TeX があればそちらで組む**(発注者確定 2026-08-13)。本物の TeX は
    LaTeX の全部を解し、行列の列も正しく揃う。無ければ mathtext に落ちる —
    こちらは TeX の実体が要らない代わりに**部分集合**しか解さない。
    どちらで組んだかで**文書は変わらない**(数式は絵として保存されるので、
    渡した先では誰が見ても同じ)。差は作ったときの見た目に閉じる。
    """
    from shutil import which
    if which("pdflatex") and which("pdftoppm"):
        return "tex"
    try:
        import matplotlib  # noqa: F401
        return "mathtext"
    except ImportError:
        return None


# standalone は数式ぴったりの紙を作る(余白の切り出しが要らない)
_TEX_HINAGATA = r"""\documentclass[preview,border=%(fuchi)spt]{standalone}
\usepackage{amsmath}
\usepackage{amssymb}
\begin{document}
$\displaystyle %(shiki)s$
\end{document}
"""


def _tex_png(t, size_pt, bai):
    """本物の TeX で組む。返りは `(bytes, w_mm, h_mm)`。

    pdflatex で紙にし、pdftoppm で画素にする。**寸法は紙から取る** —
    画素数と解像度から逆に出すと丸めで動く(mathtext の側で踏んだ穴と同じ)。
    """
    import os
    import subprocess
    import tempfile

    with tempfile.TemporaryDirectory() as d:
        src = os.path.join(d, "s.tex")
        with open(src, "w") as f:
            f.write(_TEX_HINAGATA % {"fuchi": 1, "shiki": t})
        r = subprocess.run(
            ["pdflatex", "-interaction=nonstopmode", "-halt-on-error",
             "-output-directory", d, src],
            capture_output=True, text=True, timeout=30)
        pdf = os.path.join(d, "s.pdf")
        if r.returncode != 0 or not os.path.exists(pdf):
            # TeX の言い分から**最初の ! の行**を拾う(そこに理由が書いてある)
            wake = [l for l in r.stdout.splitlines() if l.startswith("!")]
            raise Muri("TeX が組めない数式です: %s"
                       % (wake[0][2:] if wake else "(理由が読めません)"))
        # 紙の寸法(pt)。standalone なので数式ぴったり
        info = subprocess.run(["pdfinfo", pdf], capture_output=True, text=True)
        w_pt = h_pt = None
        for line in info.stdout.splitlines():
            if line.startswith("Page size:"):
                nums = [x for x in line.replace("x", " ").split() if _kazu(x)]
                if len(nums) >= 2:
                    w_pt, h_pt = float(nums[0]), float(nums[1])
        if not w_pt or not h_pt:
            raise Muri("紙の寸法が読めません")
        # standalone の紙は 10pt の本文で組まれている。指定の大きさへ縮める
        k = float(size_pt) / 10.0
        w_pt, h_pt = w_pt * k, h_pt * k
        dpi = int(round(72.0 * float(bai) / k))
        subprocess.run(["pdftoppm", "-png", "-r", str(dpi), "-singlefile",
                        pdf, os.path.join(d, "out")],
                       capture_output=True, timeout=30)
        png = os.path.join(d, "out.png")
        if not os.path.exists(png):
            raise Muri("紙を画素にできません(pdftoppm)")
        with open(png, "rb") as f:
            data = f.read()
    mm = 25.4 / 72.0
    return data, w_pt * mm, h_pt * mm


def _kazu(s):
    try:
        float(s)
        return True
    except ValueError:
        return False


def to_png(tex, size_pt=11.0, color="#000000", bai=4.0, tex_wo_tsukau=None):
    """LaTeX を PNG に組む。返りは `(bytes, w_mm, h_mm)`。

    **TeX があればそちらで組む**(行列の列まで揃う)。無ければ mathtext。
    `tex_wo_tsukau` に True / False を渡せば選べる(検査で両方を見るため)。

    **文書に入れるのはこちら。** docx の画像は png / jpeg で、模型の
    InlineImage も「png/jpeg のまま」を持つ — SVG に寄り道すると、
    画面のために作った物を保存でまた作り直すことになる。

    `bai` は**紙の寸法に対する画素の倍率**。画面を拡大しても粗くならない
    ように大きめに作り、置く大きさ(w_mm/h_mm)は等倍で返す。
    """
    if tex_wo_tsukau is None:
        tex_wo_tsukau = kumi_kata() == "tex"
    if tex_wo_tsukau:
        # **原文をそのまま渡す。** TeX は LaTeX の全部を解すので、
        # mathtext のために寄せる(fit)必要がない — 寄せると行列の列が崩れる
        return _tex_png(_hadaka(tex), size_pt, bai)
    t = fit(tex)
    try:
        import matplotlib
    except ImportError as e:
        raise Muri("matplotlib が入っていません(数式は組めません)") from e
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    # **先に寸法を測り、その大きさの紙に焼く。** bbox_inches="tight" で
    # 切り出すと切り口が dpi で丸められ、**倍率を変えると寸法が動く**
    # (実測で 3% ほど)。数式は本文の行に置くので、それでは行送りが変わる
    w_pt, h_pt = _hakaru(plt, t, size_pt, color)
    dpi = 72.0 * float(bai)
    fig = plt.figure(figsize=(w_pt / 72.0, h_pt / 72.0), dpi=dpi)
    try:
        # 紙いっぱいに置く(測った枠と同じ大きさなので、余白は出ない)
        fig.text(0, 0, "$%s$" % t, fontsize=float(size_pt), color=color,
                 va="bottom", ha="left")
        buf = io.BytesIO()
        fig.savefig(buf, format="png", dpi=dpi, transparent=True)
    finally:
        plt.close(fig)
    mm = 25.4 / 72.0
    return buf.getvalue(), w_pt * mm, h_pt * mm


def _hakaru(plt, t, size_pt, color):
    """組んだ字が占める大きさ(pt)。**dpi 72 で測る** — 画素と pt が
    1対1になるので、焼くときの倍率に左右されない。"""
    fig = plt.figure(figsize=(1, 1), dpi=72)
    try:
        txt = fig.text(0, 0, "$%s$" % t, fontsize=float(size_pt), color=color)
        fig.canvas.draw()
        bb = txt.get_window_extent(fig.canvas.get_renderer())
    except ValueError as e:
        naka = str(e).strip().splitlines()
        raise Muri("組めない数式です: %s" % (naka[-1] if naka else e)) from None
    finally:
        plt.close(fig)
    return max(bb.width, 1.0), max(bb.height, 1.0)


def _png_size(data):
    """PNG の幅・高さ(画素)。**頭の 24 バイトを読むだけ** — 画像の
    ライブラリを引き込まない(この一つのために依存を増やさない)。"""
    if data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
        raise Muri("PNG になっていません")
    w = int.from_bytes(data[16:20], "big")
    h = int.from_bytes(data[20:24], "big")
    return w, h


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
