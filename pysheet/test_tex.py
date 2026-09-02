# 数式を LaTeX で受けて絵に組む口の検査。
#
# 組むのは Rust のエンジン(typst + mitex)なので(2026-09-02)、Python の側に
# 要る物は無い。matplotlib も TeX も見ない。
#
# 手で回すなら:
#   .venv/bin/python pysheet/test_tex.py
import sys

from officework import tex


def check(cond, msg):
    if not cond:
        print(f"NG: {msg}", file=sys.stderr)
        sys.exit(1)


def muri(src, msg):
    try:
        tex.to_svg(src)
    except tex.Muri:
        return
    check(False, msg)


check(tex.kumi_kata() == "typst", f"組み方が変: {tex.kumi_kata()}")

# ── 素直な数式は組める ────────────────────────────────────────
for src in [r"\frac{a+b}{2}", r"\sqrt{x^2+y^2}", r"\sum_{n=1}^{\infty}\frac{1}{n^2}",
            r"\lim_{x \to 0}\frac{\sin x}{x}", r"\alpha\beta\gamma\pm\times\approx",
            r"\bar{x}\hat{y}\vec{v}", r"\sqrt[3]{x}", r"\left(\frac{a}{b}\right)^{n}",
            r"\int\limits_{1}^{\infty}\frac{1}{x}dx", r"a \le b \ge c"]:
    svg = tex.to_svg(src)
    check(svg.startswith(b"<?xml") or b"<svg" in svg[:400], f"SVG でない: {src}")
    check(len(svg) > 500, f"中身の無い SVG: {src}")
    # **字は輪郭に落ちている** — 受け手にその書体が無くても化けない
    check(b"<text" not in svg, f"字が輪郭になっていない(書体に依存する): {src}")

# ── 環境(行列・場合分け)はそのまま組める。列が揃う ──────────
for name in ["matrix", "pmatrix", "bmatrix", "Bmatrix", "vmatrix"]:
    src = r"\begin{%s}1 & 2\\ 3 & 4\end{%s}" % (name, name)
    check(len(tex.to_svg(src)) > 500, f"{name} が組めない")
check(len(tex.to_svg(r"f(x)=\begin{cases}1 & x>0\\ 0 & x\le 0\end{cases}")) > 500,
      "場合分けが組めない")

# ── 組めないものは**黙らずに断る** ───────────────────────────
muri(r"\begin{tikzpicture}\draw (0,0);\end{tikzpicture}", "知らない環境を黙って受けた")
muri(r"\frac{1}{", "壊れた式を黙って受けた")
muri(r"\foo{1}", "知らない命令を黙って受けた")
muri("", "空を黙って受けた")
muri("   ", "空白だけを黙って受けた")
# 断りの文言に**理由が入っている**(「何が駄目か」を言わずに断らない)
try:
    tex.to_svg(r"\frac{1}{")
except tex.Muri as e:
    check("{" in str(e) or "括弧" in str(e), f"理由を言っていない: {e}")

# ── $ で囲んであっても受ける(LaTeX を貼る人の癖)──────────────
check(tex.fit(r"$\frac{1}{2}$") == r"\frac{1}{2}", "$ を外していない")
check(len(tex.to_svg(r"$\frac{1}{2}$")) > 500, "$ 付きが組めない")

# ── 文書に入れる PNG(模型の InlineImage は png/jpeg を持つ)────
png, w_mm, h_mm = tex.to_png(r"\frac{a+b}{2}")
check(png[:8] == b"\x89PNG\r\n\x1a\n", "PNG になっていない")
check(0 < w_mm < 50 and 0 < h_mm < 30, f"寸法が変: {w_mm} x {h_mm} mm")
# 字を大きくすれば絵も大きくなる
check(tex.to_png(r"\frac{a}{b}", size_pt=22)[2]
      > tex.to_png(r"\frac{a}{b}", size_pt=11)[2] * 1.5, "字の大きさが効かない")
# 色が効く(SVG に色が出る)。読めない色は黒で組む
check(b"1b6e3c" in tex.to_svg(r"x", color="#1B6E3C").lower(), "色が効かない")
check(len(tex.to_svg(r"x", color="赤")) > 100, "読めない色で落ちた")
# 組めないものは PNG の口でも断る(SVG の口と同じ規則)
try:
    tex.to_png(r"\frac{1}{")
    check(False, "PNG の口が壊れた式を黙って受けた")
except tex.Muri:
    pass

# ── SymPy から起こす道(入っていれば)──────────────────────────
try:
    import sympy  # noqa: F401
except ImportError:
    print("sympy が無いのでその節は飛ばした", file=sys.stderr)
else:
    t = tex.from_sympy("sqrt(x**2 + y**2)")
    check(r"\sqrt" in t, f"sympy の LaTeX: {t}")
    check(len(tex.to_svg(t)) > 500, "sympy 由来の式が組めない")
    # **式は書き直される** — 書いたとおりの見た目が要るなら LaTeX を直に。
    # これは欠陥ではなく SymPy の性質なので、検査で明示して固定しておく
    check(tex.from_sympy("(a+b)/2") != r"\frac{a+b}{2}",
          "SymPy が正規化しなくなった(台帳の注記を見直すこと)")

print("OK")
