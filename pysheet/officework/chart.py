"""図を描く — データを図形に変える層。

2026-08-27 発注者「チャートは python による独自描画でいいのでは。
d3 のチャートの書き方が参考になるのでは」。

## なぜ自分で描くのか

xlsx の図(`c:chart`)は、Excel が読んで**その場で描く**指図です。
指図を書くだけなら openpyxl でもできますが、**openpyxl は描けません** —
Excel で開くまで図は見えず、PDF にも出せません。

こちらは組版のエンジンを持っているので、図形の集まりとして自分で
描きます。**画面にも紙(PDF)にも xlsx にも同じ物が出ます**。xlsx へは
図形(prstGeom / custGeom)として書くので、Excel でも図形として開けます。

*失う物。* Excel の中で「データを差し替えたら図も変わる」はしません。
描いた時のデータで固まります。作り直したいときは、もう一度描きます。

## d3 の書き方を借りる

d3 の芯は**スケール**です。データの値の範囲(domain)を、絵の上の
長さの範囲(range)へ写す関数を先に作り、印(棒・線・扇)はその関数を
通して置く。軸の目盛りも同じ関数から出ます。

    c = Chart(width=320, height=200)
    x = c.band(["4月", "5月", "6月"])        # 区分 → 位置
    y = c.linear([0, 300])                   # 値 → 高さ
    c.bars(x, y, [120, 180, 240])
    c.axis_left(y)
    c.axis_bottom(x)
    c.place(ws, "A9")

種類ごとの近道もあります([`bar`] [`line`] [`pie`])。台本を短く書きたい
ときはそちらを使ってください。
"""

import math

# 図形の座標は px で持ちます。**左上が (0, 0)**(画面と同じ向き)で、
# 紙へ出すときにエンジンが上下を直します
_PAD = {"left": 44.0, "right": 12.0, "top": 26.0, "bottom": 30.0}
# 目盛りの字と題の大きさ(pt)
_TICK_PT = 8.0
_TITLE_PT = 11.0
# 既定の色。**最初の色から順に使います**。Excel の既定の並びに寄せました
PALETTE = [
    "4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5", "70AD47",
    "264478", "9E480E", "636363", "997300",
]


class Linear:
    """値 → 長さ。d3 の `scaleLinear` です。"""

    def __init__(self, domain, range_):
        (self.d0, self.d1), (self.r0, self.r1) = domain, range_
        if self.d1 == self.d0:
            self.d1 = self.d0 + 1.0

    def __call__(self, v):
        t = (float(v) - self.d0) / (self.d1 - self.d0)
        return self.r0 + t * (self.r1 - self.r0)

    def ticks(self, n=5):
        """目盛りの値。**人が読みやすい刻み**(1 / 2 / 5 の倍数)に寄せます"""
        haba = (self.d1 - self.d0) / max(1, n)
        keta = 10.0 ** math.floor(math.log10(haba)) if haba > 0 else 1.0
        for m in (1, 2, 2.5, 5, 10):
            if haba <= keta * m:
                kizami = keta * m
                break
        else:
            kizami = keta * 10
        v = math.ceil(self.d0 / kizami) * kizami
        out = []
        while v <= self.d1 + kizami * 1e-9:
            out.append(round(v, 10))
            v += kizami
        return out


class Band:
    """区分 → 位置。d3 の `scaleBand` です。棒グラフの横方向に使います。"""

    def __init__(self, labels, range_, padding=0.2):
        self.labels = list(labels)
        self.r0, self.r1 = range_
        self.padding = padding
        n = max(1, len(self.labels))
        self.step = (self.r1 - self.r0) / n
        self.width = self.step * (1.0 - padding)

    def __call__(self, label):
        i = self.labels.index(label) if label in self.labels else int(label)
        return self.r0 + self.step * i + (self.step - self.width) / 2.0

    def center(self, label):
        return self(label) + self.width / 2.0

    def ticks(self, n=None):
        return list(self.labels)


class Chart:
    """図1つ。置いた図形を溜めて、最後にシートへ流します。"""

    def __init__(self, width=320.0, height=200.0, *, padding=None, title=None):
        self.width = float(width)
        self.height = float(height)
        self.pad = dict(_PAD)
        if padding:
            self.pad.update(padding)
        self._shapes = []          # (kind, x, y, w, h, kw)
        self._title = title
        self._iro = 0

    # ── 描く場所 ────────────────────────────────────────────────

    @property
    def plot(self):
        """絵を描く枠 (左, 上, 右, 下)。題と軸の分を除いた内側です"""
        p = self.pad
        return (p["left"], p["top"], self.width - p["right"], self.height - p["bottom"])

    def band(self, labels, padding=0.2):
        l, _, r, _ = self.plot
        return Band(labels, (l, r), padding)

    def linear(self, domain, *, vertical=True):
        l, t, r, b = self.plot
        # 縦は**下が小さい値**です(画面の y は下向きなので入れ替えます)
        return Linear(domain, (b, t) if vertical else (l, r))

    def color(self, i=None):
        """順に色を配ります"""
        if i is None:
            i, self._iro = self._iro, self._iro + 1
        return PALETTE[i % len(PALETTE)]

    # ── 印(marks)────────────────────────────────────────────────

    def rect(self, x, y, w, h, *, fill=None, line=None, line_w=1.0):
        self._shapes.append(("rect", x, y, w, h,
                             {"fill": fill, "line": line, "line_w": line_w}))

    def path(self, points, *, line=None, fill=None, line_w=1.5):
        """点の列(絵の上の px)で好きな形。左上と右下を自分で割り出します"""
        xs = [p[0] for p in points]
        ys = [p[1] for p in points]
        x0, y0, x1, y1 = min(xs), min(ys), max(xs), max(ys)
        w, h = max(x1 - x0, 0.001), max(y1 - y0, 0.001)
        # 図形の中では 0〜1 に正規化して持ちます(模型の決め)
        norm = [((px - x0) / w, (py - y0) / h) for px, py in points]
        self._shapes.append(("path", x0, y0, w, h,
                             {"points": norm, "line": line, "fill": fill,
                              "line_w": line_w}))

    def line(self, x1, y1, x2, y2, *, line="808080", line_w=0.8):
        self._shapes.append(("line", min(x1, x2), min(y1, y2),
                             abs(x2 - x1) or 0.001, abs(y2 - y1) or 0.001,
                             {"line": line, "line_w": line_w}))

    def label(self, text, x, y, w, h, *, align="center", pt=_TICK_PT):
        self._shapes.append(("rect", x, y, w, h,
                             {"text": str(text), "align": align, "font_pt": pt,
                              "fill": None, "line": None, "line_w": 0.0}))

    # ── 軸 ─────────────────────────────────────────────────────

    def axis_left(self, scale, *, n=5, fmt=None):
        """左の軸。目盛りの線と数を置きます"""
        l, t, r, b = self.plot
        self.line(l, t, l, b)
        for v in scale.ticks(n):
            y = scale(v)
            self.line(l, y, r, y, line="E0E0E0", line_w=0.5)
            self.label(fmt(v) if fmt else _mijikaku(v),
                       0.0, y - 7.0, l - 4.0, 14.0, align="right")
        return self

    def axis_bottom(self, scale, *, n=None, fmt=None):
        """下の軸。区分の名前(band)か数(linear)を並べます"""
        l, t, r, b = self.plot
        self.line(l, b, r, b)
        for v in scale.ticks(n) if n else scale.ticks():
            if isinstance(scale, Band):
                x, w = scale(v), scale.width
            else:
                x, w = scale(v) - 20.0, 40.0
            self.label(fmt(v) if fmt else v, x, b + 3.0, w, 14.0)
        return self

    # ── 種類 ────────────────────────────────────────────────────

    def bars(self, x, y, values, *, color=None, labels=None):
        """縦棒。`values` は数の列、または列の列(系列が複数)です"""
        retsu = values if values and isinstance(values[0], (list, tuple)) else [values]
        n = len(retsu)
        for si, series in enumerate(retsu):
            iro = (color[si] if isinstance(color, (list, tuple)) else color) or self.color(si)
            for i, v in enumerate(series):
                mark = x.labels[i] if i < len(x.labels) else i
                # 系列が複数なら、区分の幅を分け合います
                w = x.width / n
                px = x(mark) + w * si
                y0, y1 = y(0), y(v)
                self.rect(px, min(y0, y1), w * 0.92, abs(y1 - y0), fill=iro)
        if labels:
            for i, v in enumerate(retsu[0]):
                mark = x.labels[i] if i < len(x.labels) else i
                self.label(v, x(mark), y(v) - 15.0, x.width, 14.0)
        return self

    def lines(self, x, y, values, *, color=None):
        """折れ線。区分の真ん中を結びます"""
        retsu = values if values and isinstance(values[0], (list, tuple)) else [values]
        for si, series in enumerate(retsu):
            iro = (color[si] if isinstance(color, (list, tuple)) else color) or self.color(si)
            pts = []
            for i, v in enumerate(series):
                mark = x.labels[i] if i < len(x.labels) else i
                cx = x.center(mark) if isinstance(x, Band) else x(mark)
                pts.append((cx, y(v)))
            if len(pts) >= 2:
                self.path(pts, line=iro, line_w=1.8)
        return self

    def arcs(self, values, *, labels=None, color=None, hole=0.0):
        """円グラフ。**扇は多角形に刻んで**置きます(48 分の1周ずつ)"""
        l, t, r, b = self.plot
        cx, cy = (l + r) / 2.0, (t + b) / 2.0
        rad = min(r - l, b - t) / 2.0
        goukei = float(sum(values)) or 1.0
        kaku = -math.pi / 2.0                 # 12時から時計回り(Excel と同じ)
        for i, v in enumerate(values):
            iro = (color[i] if isinstance(color, (list, tuple)) else color) or self.color(i)
            haba = 2.0 * math.pi * float(v) / goukei
            kizami = max(2, int(48 * haba / (2.0 * math.pi)) + 1)
            soto = [(cx + rad * math.cos(kaku + haba * k / kizami),
                     cy + rad * math.sin(kaku + haba * k / kizami))
                    for k in range(kizami + 1)]
            if hole > 0:
                uchi = [(cx + rad * hole * math.cos(kaku + haba * k / kizami),
                         cy + rad * hole * math.sin(kaku + haba * k / kizami))
                        for k in range(kizami, -1, -1)]
                self.path(soto + uchi, fill=iro, line=iro, line_w=0.5)
            else:
                self.path([(cx, cy)] + soto, fill=iro, line=iro, line_w=0.5)
            if labels and i < len(labels):
                naka = kaku + haba / 2.0
                lx = cx + rad * 0.68 * math.cos(naka)
                ly = cy + rad * 0.68 * math.sin(naka)
                self.label(labels[i], lx - 28.0, ly - 7.0, 56.0, 14.0)
            kaku += haba
        return self

    # ── 置く ────────────────────────────────────────────────────

    def title(self, text, pt=_TITLE_PT):
        self._title = (text, pt)
        return self

    def place(self, ws, at, *, dx=0.0, dy=0.0, frame=True):
        """シートへ流し込む。`at` は左上を留めるセル("A9")です"""
        if frame:
            ws.add_shape("rect", at, self.width, self.height,
                         dx=dx, dy=dy, fill="FFFFFF", line="D0D0D0", line_w=0.8)
        if self._title:
            t, pt = self._title if isinstance(self._title, tuple) else (self._title, _TITLE_PT)
            ws.add_shape("rect", at, self.width, self.pad["top"], dx=dx, dy=dy,
                         text=str(t), align="center", font_pt=pt, line_w=0.0)
        for kind, x, y, w, h, kw in self._shapes:
            ws.add_shape(kind, at, w, h, dx=dx + x, dy=dy + y, **kw)
        return self


def _mijikaku(v):
    """目盛りの数を短く。1000 の位で区切り、整数は小数点を出しません"""
    f = float(v)
    if abs(f - round(f)) < 1e-9:
        return "{:,}".format(int(round(f)))
    return "{:,.1f}".format(f)


# ── 近道 ───────────────────────────────────────────────────────


def bar(ws, at, values, labels=None, *, title=None, width=320.0, height=200.0,
        color=None, value_labels=False, **kw):
    """縦棒の図を1行で。`values` は数の列か、列の列(系列が複数)です"""
    retsu = values if values and isinstance(values[0], (list, tuple)) else [values]
    c = Chart(width, height, title=title)
    x = c.band(labels or list(range(1, len(retsu[0]) + 1)))
    takai = max(max(s) for s in retsu)
    hikui = min(min(s) for s in retsu)
    y = c.linear([min(0.0, hikui), takai])
    c.axis_left(y)
    c.axis_bottom(x)
    c.bars(x, y, retsu, color=color, labels=value_labels)
    return c.place(ws, at, **kw)


def line(ws, at, values, labels=None, *, title=None, width=320.0, height=200.0,
         color=None, **kw):
    """折れ線の図を1行で"""
    retsu = values if values and isinstance(values[0], (list, tuple)) else [values]
    c = Chart(width, height, title=title)
    x = c.band(labels or list(range(1, len(retsu[0]) + 1)), padding=0.0)
    y = c.linear([min(0.0, min(min(s) for s in retsu)), max(max(s) for s in retsu)])
    c.axis_left(y)
    c.axis_bottom(x)
    c.lines(x, y, retsu, color=color)
    return c.place(ws, at, **kw)


def pie(ws, at, values, labels=None, *, title=None, width=280.0, height=200.0,
        color=None, hole=0.0, **kw):
    """円グラフを1行で。`hole` を 0.5 などにするとドーナツになります"""
    c = Chart(width, height, title=title)
    c.arcs(list(values), labels=labels, color=color, hole=hole)
    return c.place(ws, at, **kw)
