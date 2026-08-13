#!/usr/bin/env bash
# cargo test を回し、落ちたら**どの試験が落ちたか**を走行ページの要約
# (GITHUB_STEP_SUMMARY)に書き出す。
#
# なぜ要るか: CI のログは認証が要り、貼ってもらえるのは大抵ログの尻尾 —
# そこには「1 failed」までしか出ず、落ちた試験の**名前**が届かない。
# 3 OS 対応の週(2026-08-13)にこの往復を4回やった。名前さえあれば
# 1回で済んでいた。以後、機械が自分で名乗る。
#
# Windows の runner でも Git Bash で動く(step に shell: bash と書く)。
set -uo pipefail
log="${RUNNER_TEMP:-/tmp}/cargo-test.log"
cargo test "$@" 2>&1 | tee "$log"
code=${PIPESTATUS[0]}
if [ "$code" -ne 0 ] && [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  {
    echo "### 落ちた試験 — \`cargo test $*\`"
    echo '```'
    # failures: の塊(名前の一覧)。--no-fail-fast だと複数回出る
    grep -A 40 '^failures:$' "$log" | head -160
    echo '```'
    echo "落ちた場所(panicked の前後):"
    echo '```'
    grep -B 2 -A 8 'panicked at' "$log" | head -120
    echo '```'
  } >> "$GITHUB_STEP_SUMMARY"
fi
exit "$code"
