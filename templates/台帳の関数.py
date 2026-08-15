# 台帳テンプレ集の関数(UDF)。
# ~/.config/officework/plugins/台帳の関数.py に置くと、式から呼べます。
#   =状態集計(E2:E501) / =要発注(A2:D100) / =実働(C2, D2)
# 引数が変われば裏で計算し直します(押すボタンはありません)。


def 状態集計(r):
    from collections import Counter
    c = Counter(v for row in r for v in row if v)
    return [[k, n] for k, n in sorted(c.items())] or [["(まだ無い)", 0]]


def 要発注(r):
    out = [[row[0], row[2], row[3]] for row in r
           if row[0] and row[2] is not None and row[3] is not None and row[2] < row[3]]
    return out or [["(無し)", "", ""]]


def 実働(a, b):
    # "9:00"〜"18:00" → 休憩1時間を引いた時間数。空なら空
    if not a or not b:
        return ""
    def m(t):
        h, mm = str(t).split(":")
        return int(h) * 60 + int(mm)
    return round((m(b) - m(a) - 60) / 60, 2)
