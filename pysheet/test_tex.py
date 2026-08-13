# 数式を LaTeX で受けて SVG に組む口の検査(2026-08-13)。
#
# **組版は自前で書かない**という分業(calc のグラフと同じ筋)の合否を見る。
# matplotlib が無い機械では飛ばす — 無いのに失敗と言わない。
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


try:
    import matplotlib  # noqa: F401
except ImportError:
    print("matplotlib が無いので飛ばした", file=sys.stderr)
    sys.exit(0)

# ── 素直な数式は組める ────────────────────────────────────────
for src in [r"\frac{a+b}{2}", r"\sqrt{x^2+y^2}", r"\sum_{n=1}^{\infty}\frac{1}{n^2}",
            r"\lim_{x \to 0}\frac{\sin x}{x}", r"\alpha\beta\gamma\pm\times\approx",
            r"\bar{x}\hat{y}\vec{v}", r"\sqrt[3]{x}", r"\left(\frac{a}{b}\right)^{n}"]:
    svg = tex.to_svg(src)
    check(svg.startswith(b"<?xml") or b"<svg" in svg[:400], f"SVG でない: {src}")
    check(len(svg) > 500, f"中身の無い SVG: {src}")
    # **字は輪郭に落ちている** — 受け手にその書体が無くても化けない
    check(b"<text" not in svg, f"字が輪郭になっていない(書体に依存する): {src}")

# ── mathtext が持たない書き方を寄せる ────────────────────────
# 環境(\begin{…})はそのままでは通らない。substack へ寄せて組む
for name in ["matrix", "pmatrix", "bmatrix", "Bmatrix", "vmatrix"]:
    src = r"\begin{%s}1 & 2\\ 3 & 4\end{%s}" % (name, name)
    check(len(tex.to_svg(src)) > 500, f"{name} が組めない")
check(r"\substack" in tex.fit(r"\begin{matrix}1 & 2\\ 3 & 4\end{matrix}"),
      "環境が substack に寄っていない")
# 場合分けも同じ道(左の中括弧だけが伸びる)
check(len(tex.to_svg(r"f(x)=\begin{cases}1 & x>0\\ 0 & x\le 0\end{cases}")) > 500,
      "場合分けが組めない")
# sympy の既定は \int\limits — mathtext は \limits を知らないので落とす
check(r"\limits" not in tex.fit(r"\int\limits_{1}^{2}x\,dx"), "\\limits が残っている")
check(len(tex.to_svg(r"\int\limits_{1}^{\infty}\frac{1}{x}dx")) > 500, "積分が組めない")
# \le / \ge の綴りは mathtext だと \leq / \geq だけ
check(tex.fit(r"a \le b \ge c") == r"a \leq b \geq c", f"綴り: {tex.fit(r'a \le b \ge c')}")
# \leq をさらに書き換えない(\le の後ろに字が続くものは触らない)
check(tex.fit(r"a \leq b") == r"a \leq b", "\\leq を壊した")

# ── 組めないものは**黙らずに断る** ───────────────────────────
muri(r"\begin{tikzpicture}\draw (0,0);\end{tikzpicture}", "知らない環境を黙って受けた")
muri(r"\frac{1}{", "壊れた式を黙って受けた")
muri("", "空を黙って受けた")
muri("   ", "空白だけを黙って受けた")
# 断りの文言に**理由が入っている**(「何が駄目か」を言わずに断らない)
try:
    tex.to_svg(r"\begin{tikzpicture}x\end{tikzpicture}")
except tex.Muri as e:
    check("tikzpicture" in str(e), f"理由を言っていない: {e}")

# ── $ で囲んであっても受ける(LaTeX を貼る人の癖)──────────────
check(tex.fit(r"$\frac{1}{2}$") == r"\frac{1}{2}", "$ を外していない")

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
