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

# ── 文書に入れる PNG(模型の InlineImage は png/jpeg を持つ)────
png, w_mm, h_mm = tex.to_png(r"\frac{a+b}{2}")
check(png[:8] == b"\x89PNG\r\n\x1a\n", "PNG になっていない")
check(0 < w_mm < 50 and 0 < h_mm < 30, f"寸法が変: {w_mm} x {h_mm} mm")
# **倍率を変えても置く寸法は動かない。** 数式は本文の行に置くので、
# ここが動くと行送りが変わる(tight で切り出すと dpi の丸めで 3% 動いた)
sun = [tex.to_png(r"\frac{a+b}{2}", bai=b)[1:] for b in (1, 2, 4, 8)]
check(all(abs(s[0] - sun[0][0]) < 0.01 and abs(s[1] - sun[0][1]) < 0.01 for s in sun),
      f"倍率で寸法が動いた: {sun}")
# 倍率を上げたぶんは**画素の細かさ**になる(拡大しても粗くならない)
check(len(tex.to_png(r"\frac{a+b}{2}", bai=8)[0])
      > len(tex.to_png(r"\frac{a+b}{2}", bai=1)[0]) * 3, "倍率が画素に効いていない")
# 字を大きくすれば絵も大きくなる
check(tex.to_png(r"\frac{a}{b}", size_pt=22)[2]
      > tex.to_png(r"\frac{a}{b}", size_pt=11)[2] * 1.5, "字の大きさが効かない")
# 組めないものは PNG の口でも断る(SVG の口と同じ規則)
try:
    tex.to_png(r"\begin{tikzpicture}x\end{tikzpicture}")
    check(False, "PNG の口が知らない環境を黙って受けた")
except tex.Muri:
    pass

# ── TeX があればそちらで組む(無ければ mathtext)────────────────
kata = tex.kumi_kata()
check(kata in ("tex", "mathtext"), f"組み方が変: {kata}")
# mathtext は**必ず**通る道。TeX の有無に関わらず同じ約束を守る
png_m, w_m, h_m = tex.to_png(r"\frac{a+b}{2}", tex_wo_tsukau=False)
check(png_m[:8] == b"\x89PNG\r\n\x1a\n" and w_m > 0, "mathtext の道が壊れた")
if kata == "tex":
    png_t, w_t, h_t = tex.to_png(r"\frac{a+b}{2}", tex_wo_tsukau=True)
    check(png_t[:8] == b"\x89PNG\r\n\x1a\n", "TeX の道が PNG を返さない")
    check(0 < w_t < 60 and 0 < h_t < 40, f"TeX の寸法が変: {w_t} x {h_t} mm")
    # 倍率を変えても置く寸法は動かない(mathtext の側と同じ約束)
    s1 = tex.to_png(r"\frac{a}{b}", bai=2, tex_wo_tsukau=True)[1:]
    s2 = tex.to_png(r"\frac{a}{b}", bai=6, tex_wo_tsukau=True)[1:]
    check(abs(s1[0] - s2[0]) < 0.05 and abs(s1[1] - s2[1]) < 0.05,
          f"TeX の側で倍率が寸法に効いた: {s1} {s2}")
    # **TeX は寄せない。** 行列は \begin{matrix} のまま渡す(寄せると列が崩れる)
    check(len(tex.to_png(r"\begin{bmatrix}1 & 200\\ 30000 & 4\end{bmatrix}",
                         tex_wo_tsukau=True)[0]) > 500, "TeX で行列が組めない")
    # 壊れた式は TeX の側でも断る。**理由を言う**(TeX の言い分を拾う)
    try:
        tex.to_png(r"\frac{1}{", tex_wo_tsukau=True)
        check(False, "TeX の道が壊れた式を黙って受けた")
    except tex.Muri as e:
        check("TeX" in str(e) or "組めない" in str(e), f"理由を言っていない: {e}")
    # 字の大きさが効く
    check(tex.to_png(r"\frac{a}{b}", size_pt=22, tex_wo_tsukau=True)[2]
          > tex.to_png(r"\frac{a}{b}", size_pt=11, tex_wo_tsukau=True)[2] * 1.5,
          "TeX の側で字の大きさが効かない")
else:
    print("TeX が無いので TeX の節は飛ばした", file=sys.stderr)

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
