"""サイドカーに直に喋って、再計算の前後を並べる。

**画面に出す前に、期待値を数字で確定させる。** 実機で見て「合っている
気がする」では試験にならない。
"""
import json
import subprocess
import sys

SIDECAR = sys.argv[1] if len(sys.argv) > 1 else "/home/dev/dev/officework/target/release/xlsx-sidecar"
PATH = "/home/dev/xlsx-corpus/recalc_test.xlsx"

p = subprocess.Popen([SIDECAR], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
seq = [0]


def call(command, **body):
    seq[0] += 1
    req = {"version": 1, "requestId": f"probe-{seq[0]:08x}", "command": command, **body}
    p.stdin.write(json.dumps(req, ensure_ascii=False) + "\n")
    p.stdin.flush()
    r = json.loads(p.stdout.readline())
    if not r.get("ok"):
        raise SystemExit(f"{command} が失敗: {r.get('error')}")
    return r["result"]


RANGE = {"startRow": 0, "endRow": 8, "startColumn": 0, "endColumn": 3}


def grid(cells):
    """(行,列) → 表示文字列。"""
    return {(c["row"], c["column"]): c.get("formatted", "") for c in cells}


opened = call("open", path=PATH)
print("シート:", [s["name"] for s in opened.get("sheets", [])])

before = grid(call("recalc_cells", path=PATH, edits=[],
                   reads=[{"sheet": "再計算", "range": RANGE}])["cells"])

after = grid(call("recalc_cells", path=PATH,
                  edits=[{"sheet": "再計算", "row": 1, "column": 1, "input": "1500"}],
                  reads=[{"sheet": "再計算", "range": RANGE}])["cells"])

cross = call("recalc_cells", path=PATH,
             edits=[{"sheet": "再計算", "row": 1, "column": 1, "input": "1500"}],
             reads=[{"sheet": "別シート", "range": {"startRow": 1, "endRow": 2,
                                                 "startColumn": 1, "endColumn": 1}}])

names = {1: "単価", 2: "数量", 3: "小計", 4: "消費税", 5: "合計",
         6: "判定", 7: "桁区切り", 8: "平均"}
print(f"\n{'':10} {'before (単価1200)':>16} {'after (単価1500)':>16}   追随")
for r, label in names.items():
    b, a = before.get((r, 1), ""), after.get((r, 1), "")
    mark = "→ 変わった" if b != a else ("(変わらないのが正)" if r == 2 else "**動いていない**")
    print(f"{label:<10} {b:>18} {a:>18}   {mark}")

print("\n別シート(跨ぐ参照、単価1500のとき):")
for c in cross["cells"]:
    print(f"  行{c['row']}: {c.get('formatted')}")
print("\n居座り(cached):", cross.get("cached"))
p.stdin.close()
