#!/usr/bin/env python3
"""GitHub Actions の書き方を、押す前に見る。

    python3 tools/workflow_check.py

CI は**押してからでないと動きません**。書き間違いに気づくのが
「タグを押した後」になり、PyPI のようにやり直しの効かない相手だと
そこで詰まります。

2026-08-28 に2回踏みました。

1. `guard` の仕事に `actions/checkout` を足し忘れ、木が無いのに
   `pysheet/Cargo.toml` を読もうとして止まりました
2. まっさらな環境の検査が `--find-links` だけだったので PyPI からも
   取りに行き、組んだ wheel ではなく古い版を検査していました

どちらも**中身を読めば分かる**ことなので、ここで見ます。
"""
import pathlib
import re
import sys

try:
    import yaml
except ImportError:
    print("PyYAML が要ります: pip install pyyaml", file=sys.stderr)
    sys.exit(2)

ROOT = pathlib.Path(__file__).resolve().parent.parent
warui = []


def ng(where, msg):
    warui.append(f"{where}: {msg}")


def steps_of(job):
    return job.get("steps") or []


def uses(job, name):
    return any(str(s.get("uses", "")).startswith(name) for s in steps_of(job))


def run_text(job):
    """その仕事が走らせる字。**行の折り返しを繋いでから返します。**

    シェルは長い行を `\\` で折れます。折れたままだと
    `pip install --quiet --no-index --pre \\` と次の行の `--find-links`
    が別の字になり、「--find-links があるのに --no-index が無い」と
    見えてしまいます(2026-08-28 に、わざと壊して素通りしたので気づきました)。
    """
    nama = "\n".join(str(s.get("run", "")) for s in steps_of(job))
    # 行の折り返しを繋ぐ
    nama = re.sub(r"\\\s*\n\s*", " ", nama)
    # **注釈は落とします。** 説明の中に `--find-links` と書いてあるのを
    # 命令だと数えて、直っているのに「直っていない」、壊れているのに
    # 「大丈夫」と言っていました(2026-08-28)
    return "\n".join(
        re.sub(r"(^|\s)#.*$", "", ln) for ln in nama.split("\n")
    )


for f in sorted((ROOT / ".github/workflows").glob("*.yml")):
    d = yaml.safe_load(f.read_text(encoding="utf-8"))
    for name, job in (d.get("jobs") or {}).items():
        doko = f"{f.name} / {name}"
        text = run_text(job)

        # ① 木の中のファイルを読むなら checkout が要る
        yomu = re.findall(r"[\w./-]*(?:Cargo\.toml|pyproject\.toml|\.py|\.sh)\b", text)
        yomu = [y for y in yomu if "/" in y and not y.startswith(("$", "~"))]
        if yomu and not uses(job, "actions/checkout"):
            ng(doko, f"木のファイルを読むのに checkout がありません: {sorted(set(yomu))[:3]}")

        # ② 組んだ物を検査するなら、外から取ってこない
        if "--find-links" in text and "--no-index" not in text:
            ng(doko, "--find-links に --no-index がありません(PyPI から古い版が入ります)")
        # 前触れの版(β)は既定で飛ばされる
        if "--find-links" in text and "--pre" not in text:
            ng(doko, "--find-links に --pre がありません(β が入りません)")

for line in warui:
    print("NG:", line)
print("OK" if not warui else f"{len(warui)} 件おかしい")
sys.exit(1 if warui else 0)
