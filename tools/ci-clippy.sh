#!/usr/bin/env bash
# cargo clippy を回し、落ちたら**指摘の中身**を走行ページの要約
# (GITHUB_STEP_SUMMARY)に書き出す。ci-test.sh と同じ理由 —
# ログは認証が要り、注釈(annotations)には「exit code 101」しか
# 乗らない(2026-08-13 に実測)。名前の無い赤は一往復を無駄にする。
set -uo pipefail
log="${RUNNER_TEMP:-/tmp}/cargo-clippy.log"
cargo clippy "$@" 2>&1 | tee "$log"
code=${PIPESTATUS[0]}
if [ "$code" -ne 0 ] && [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  {
    echo "### clippy の指摘 — \`cargo clippy $*\`"
    echo '```'
    grep -B 2 -A 10 '^error' "$log" | head -200
    echo '```'
  } >> "$GITHUB_STEP_SUMMARY"
fi
exit "$code"
