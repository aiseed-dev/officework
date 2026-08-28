#!/usr/bin/env bash
# **wheel に入る crate が Windows の的でも組めるか。**
#
#     bash tools/win_check.sh
#
# 手元は Linux なので、`cargo check` が通っても Windows で落ちることが
# あります。落ちるのは `#[cfg(unix)]` の付け忘れと付き間違いです。
#
# 2026-08-12(v0.2.0)— pysheet が ops に依存した日に、ops の unix 専用の
# API が Windows の wheel を壊しました。CI に見張りを足しましたが、
# **push してからでないと分かりません**。
#
# 2026-08-28 — `pub mod pdf;` を `#[cfg(unix)]` と関数の間へ差し込んで
# しまい、印が pdf の方に掛かりました。Windows で PDF が消え、代わりに
# ソケットが組まれて落ちました。**タグを押した後に分かった**ので、
# 手元で押す前に見られる形にします。
#
# 組み立て(リンク)はしません。`check` だけで cfg の間違いは全部出ます。
set -uo pipefail

TARGET=x86_64-pc-windows-msvc
# **wheel に入る crate だけ。** `cargo tree -p pysheet` で確かめた並びです。
# gpui を持つ物(calc / writer / ui)と `lang` は入りません — `lang` は
# C の道具(lib.exe)が要るので、手元の Linux では的を変えて見られません
CRATES=(-p book -p kumihan -p sheet -p ooxml -p paper -p ops -p pysheet)

if ! rustup target list --installed | grep -q "^$TARGET$"; then
  echo "Windows の的を足します($TARGET)"
  rustup target add "$TARGET" || exit 1
fi

echo "Windows の的で組めるか見ます: ${CRATES[*]}"
# **答えを先に受け取ってから見ます。**
#
# `cargo check | grep` と繋ぐと、`pipefail` が入っているので cargo の
# 失敗が grep の答えを隠します。誤りを見つけたのに「組めます」と言う
# 形になっていました(2026-08-28 に、わざと壊して気づきました)
out=$(cargo check "${CRATES[@]}" --target "$TARGET" --message-format=short 2>&1)
code=$?
warui=$(printf '%s\n' "$out" | grep -E ": error" || true)

if [ -n "$warui" ] || [ "$code" -ne 0 ]; then
  printf '%s\n' "$warui"
  echo
  echo "Windows で組めません。**unix だけの物を使っていないか**見てください:"
  echo "  - std::os::unix::* を #[cfg(unix)] 無しで呼んでいる"
  echo "  - #[cfg(unix)] と関数の間に、別の物(mod や use)が入り込んでいる"
  exit 1
fi

echo "Windows の的でも組めます"
