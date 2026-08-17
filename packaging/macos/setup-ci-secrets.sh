#!/usr/bin/env bash
# GitHub Actions で mac の署名をするための秘密を、**Mac から直に入れる**。
#
#   packaging/macos/setup-ci-secrets.sh <AuthKey_XXXX.p8> <Key ID> <Issuer ID>
#
# 一度きりの作業。以後はタグを押すだけで署名つきの .dmg が出来る。
#
# ## なぜこれがあるか
#
# CI は鍵を持たない機械なので、証明書と秘密鍵を `.p12` にして渡すしかない。
# その手作業(書き出す → base64 にする → 画面から貼る)が長く、
# **秘密が画面とクリップボードを何度も通る**。この台本は全部を繋いで、
# 値をどこにも表示せずに `gh` へ流す。
#
# ## 要る物
#
#   - Developer ID Application の証明書がこの Mac にあること
#       security find-identity -v -p codesigning
#     無ければ docs/mac-signing.ja.md の A(Xcode から数クリック)
#   - GitHub CLI が入って認証済みであること
#       brew install gh && gh auth login
#   - App Store Connect の API キー(.p8 / Key ID / Issuer ID)
#     作り方は docs/mac-signing.ja.md の B-1
set -euo pipefail
cd "$(dirname "$0")/../.."

die() { echo "❌ $*" >&2; exit 1; }

P8="${1:-}"; KEY_ID="${2:-}"; ISSUER="${3:-}"
[ -n "$P8" ] && [ -n "$KEY_ID" ] && [ -n "$ISSUER" ] || die \
  "使い方: $0 <AuthKey_XXXX.p8> <Key ID> <Issuer ID>(docs/mac-signing.ja.md の B-1)"
[ -f "$P8" ] || die "$P8 がありません"

command -v gh > /dev/null || die "GitHub CLI(gh)がありません: brew install gh && gh auth login"
gh auth status > /dev/null 2>&1 || die "gh が認証されていません: gh auth login"

# ---- 1. 証明書を確かめる ----------------------------------------------------
found="$(security find-identity -v -p codesigning | grep "Developer ID Application" || true)"
n="$(printf '%s' "$found" | grep -c . || true)"
[ "$n" -gt 0 ] || die "Developer ID Application の証明書がありません(docs/mac-signing.ja.md の A)"
if [ "$n" -gt 1 ]; then
  # **黙って選ばない。** どちらで署名したか分からない物を作らないため
  echo "Developer ID Application が $n 枚あります:"
  printf '%s\n' "$found"
  echo
  read -r -p "使う方の SHA-1(行頭の 40 桁): " ident
  [ -n "$ident" ] || die "選ばれませんでした"
else
  ident="$(printf '%s' "$found" | awk '{print $2}')"
fi
echo "使う証明書: $ident"

# ---- 2. .p12 に書き出す -----------------------------------------------------
echo
echo "この .p12 に付ける合言葉を決めてください(**この Mac が壊れたときの"
echo "鍵の控えになる**ので、パスワード管理に残すこと)。"
read -r -s -p "合言葉: " pw; echo
read -r -s -p "もう一度: " pw2; echo
[ "$pw" = "$pw2" ] || die "合言葉が一致しません"
[ -n "$pw" ] || die "空の合言葉は使えません"

P12="$HOME/Desktop/officework-signing-$(date +%Y%m%d).p12"
# **`-P` は ps に一瞬見える。** 他人と共有している機械では、この台本では
# なく docs の C-1(手で書き出す)を使うこと
security export -t identities -f pkcs12 -P "$pw" -o "$P12"
[ -s "$P12" ] || die "書き出せませんでした"
echo "書き出しました: $P12"

# ---- 3. GitHub に入れる(値は画面に出さない)---------------------------------
repo="$(gh repo view --json nameWithOwner -q .nameWithOwner)"
echo
echo "$repo に入れます…"

base64 -i "$P12" | gh secret set MAC_CERT_P12
printf '%s' "$pw"   | gh secret set MAC_CERT_PASSWORD
base64 -i "$P8"     | gh secret set MAC_API_KEY_P8
printf '%s' "$KEY_ID"  | gh secret set MAC_API_KEY_ID
printf '%s' "$ISSUER"  | gh secret set MAC_API_ISSUER_ID
if [ "$n" -gt 1 ]; then
  printf '%s' "$ident" | gh secret set MAC_SIGN_IDENTITY
fi

echo
echo "== 入れた物(名前だけ。中身は見せません)"
gh secret list | grep -E "^MAC_" || true

echo
echo "== できました"
echo "  タグを押すと署名つきの .dmg が出来ます:"
echo "    git tag app-v<版> && git push origin app-v<版>"
echo
echo "  **$P12 は消さないでください。** この Mac が壊れたとき、証明書を"
echo "  作り直さずに済む唯一の控えです(安全な所へ移す)。"
