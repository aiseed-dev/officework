#!/usr/bin/env bash
# macOS の署名と公証。**Developer ID の直配布**用(Mac App Store ではない)。
#
#   packaging/macos/sign.sh keychain          鍵を一時の keychain に入れる
#   packaging/macos/sign.sh app <path.app>    .app を中から外へ署名する
#   packaging/macos/sign.sh notarize <file>   公証して staple する(.zip / .dmg)
#   packaging/macos/sign.sh dmg <file.dmg>    .dmg 自身に署名する
#   packaging/macos/sign.sh verify <file.dmg> 出来上がりを確かめる
#
# **要る秘密**(GitHub の Secrets。中身はどこにも出さない):
#   MAC_CERT_P12         Developer ID Application の .p12 を base64 にした物
#   MAC_CERT_PASSWORD    その .p12 の合言葉
#   MAC_API_KEY_P8       App Store Connect API キー(.p8)を base64 にした物
#   MAC_API_KEY_ID       その Key ID
#   MAC_API_ISSUER_ID    その Issuer ID
#
# **要るとは限らない物**:
#   MAC_SIGN_IDENTITY    使う証明書の SHA-1。鍵束に Developer ID Application が
#                        2枚以上あるときだけ要る(1枚なら自分で見つける)
#
# 手順書は docs/mac-signing.ja.md。
#
# **なぜ台本にするか**: ワークフローに直書きすると読む人が居なくなる。
# ここなら手元の Mac でも同じ物が走る(発注者は実機を持っている)。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ENTITLEMENTS="$ROOT/packaging/macos/entitlements.plist"
KEYCHAIN="${RUNNER_TEMP:-/tmp}/officework-sign.keychain-db"

die() { echo "❌ $*" >&2; exit 1; }

# ---- 鍵を入れる -------------------------------------------------------------

cmd_keychain() {
  [ -n "${MAC_CERT_P12:-}" ] || die "MAC_CERT_P12 がありません(docs/mac-signing.ja.md)"
  [ -n "${MAC_CERT_PASSWORD:-}" ] || die "MAC_CERT_PASSWORD がありません"

  # **使い捨ての keychain。** login.keychain を触らない(手元の Mac で
  # 走らせても、その機械の鍵束を汚さない)。合言葉はその場で作って捨てる
  local kp
  kp="$(uuidgen)"
  security delete-keychain "$KEYCHAIN" 2>/dev/null || true
  security create-keychain -p "$kp" "$KEYCHAIN"
  security set-keychain-settings -lut 21600 "$KEYCHAIN"
  security unlock-keychain -p "$kp" "$KEYCHAIN"

  # **`-d` を使う**(`--decode` ではない)。macOS の base64 は BSD 系で、
  # GNU 風の長い綴りが通るとは限らない。-d はどちらの機械でも通る
  local p12="${RUNNER_TEMP:-/tmp}/cert.p12"
  printf '%s' "$MAC_CERT_P12" | base64 -d > "$p12"
  security import "$p12" -k "$KEYCHAIN" -P "$MAC_CERT_PASSWORD" \
    -T /usr/bin/codesign -T /usr/bin/security
  rm -f "$p12"

  # codesign が合言葉を聞かずに鍵を使えるようにする(聞かれると CI が止まる)
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$kp" "$KEYCHAIN" > /dev/null
  security list-keychain -d user -s "$KEYCHAIN" "$(security default-keychain | tr -d ' "')"

  # 識別子(SHA-1)。**これは秘密ではない** — 署名した物から誰でも読める
  local found n ident
  found="$(security find-identity -v -p codesigning "$KEYCHAIN" \
           | grep "Developer ID Application" || true)"
  n="$(printf '%s' "$found" | grep -c . || true)"

  if [ -n "${MAC_SIGN_IDENTITY:-}" ]; then
    # 名指しがあればそれに従う(2枚以上ある鍵束で、どれを使うか決めた場合)
    ident="$MAC_SIGN_IDENTITY"
  elif [ "$n" -eq 0 ]; then
    die "Developer ID Application の証明書が鍵束にありません(docs/mac-signing.ja.md の 0)"
  elif [ "$n" -gt 1 ]; then
    # **黙って選ばない。** 更新して古い物が残っている・別チームの物が
    # 混ざっている、のどちらでも「どちらで署名したか分からない物」が
    # 出来てしまう。どれを使うかは人が決める(docs の 0-A ③)
    echo "Developer ID Application の証明書が $n 枚あります:" >&2
    printf '%s\n' "$found" >&2
    die "MAC_SIGN_IDENTITY に使う方の SHA-1(40 桁)を入れてください"
  else
    ident="$(printf '%s' "$found" | awk '{print $2}')"
  fi
  echo "署名に使う証明書: $ident"
  if [ -n "${GITHUB_ENV:-}" ]; then
    echo "SIGN_IDENTITY=$ident" >> "$GITHUB_ENV"
  fi
}

# ---- .app を署名する --------------------------------------------------------

# Mach-O か。**拡張子で決めない** — 同梱 Python の中は .so も拡張子なしの
# 実行ファイルも混ざっている
is_macho() { file -b "$1" | grep -q "Mach-O"; }
# 走る物(= 別の処理になる物)か。同梱の python3 がこれに当たるので、
# **権利は実行ファイルにも要る**(親の権利は子に継がれない)
is_exec() { file -b "$1" | grep -q "Mach-O.*executable"; }

cmd_app() {
  local app="$1"
  [ -d "$app" ] || die "$app がありません"
  [ -n "${SIGN_IDENTITY:-}" ] || die "SIGN_IDENTITY がありません(先に keychain を)"

  # **中から外へ。** 包みを先に署名すると、後から中を触った時点で壊れる
  local n=0
  while IFS= read -r f; do
    is_macho "$f" || continue
    # **誤りを握り潰さない。** codesign は1つ署名するたびに
    # 「replacing existing signature」を stderr に出すので、**その1行だけ**
    # 捨てる。まとめて /dev/null に流すと、本当の失敗が見えなくなる
    if is_exec "$f"; then
      codesign --force --timestamp --options runtime \
        --entitlements "$ENTITLEMENTS" \
        --keychain "$KEYCHAIN" -s "$SIGN_IDENTITY" "$f" \
        2> >(grep -v "replacing existing signature" >&2)
    else
      # ライブラリ(.dylib / .so)は自分では走らないので権利は要らない
      codesign --force --timestamp --options runtime \
        --keychain "$KEYCHAIN" -s "$SIGN_IDENTITY" "$f" \
        2> >(grep -v "replacing existing signature" >&2)
    fi
    n=$((n + 1))
  done < <(find "$app/Contents" -type f)

  # 最後に包みそのもの
  codesign --force --timestamp --options runtime \
    --entitlements "$ENTITLEMENTS" \
    --keychain "$KEYCHAIN" -s "$SIGN_IDENTITY" "$app"

  codesign --verify --strict --verbose=2 "$app"
  echo "署名しました: $app(中の Mach-O $n 個 + 包み)"
}

# ---- 公証 -------------------------------------------------------------------

cmd_notarize() {
  local f="$1"
  [ -e "$f" ] || die "$f がありません"
  for v in MAC_API_KEY_P8 MAC_API_KEY_ID MAC_API_ISSUER_ID; do
    [ -n "${!v:-}" ] || die "$v がありません(docs/mac-signing.ja.md)"
  done

  local p8="${RUNNER_TEMP:-/tmp}/asc.p8"
  printf '%s' "$MAC_API_KEY_P8" | base64 -d > "$p8"
  # **--wait で待つ。** 待たずに次へ進むと staple が「まだ通っていない」で
  # 落ちる。落ちたら log を取って理由を出す — 「失敗しました」だけでは直せない
  if ! xcrun notarytool submit "$f" \
        --key "$p8" --key-id "$MAC_API_KEY_ID" --issuer "$MAC_API_ISSUER_ID" \
        --wait --timeout 30m; then
    echo "公証が通りませんでした。直近の記録を出します:" >&2
    xcrun notarytool history --key "$p8" --key-id "$MAC_API_KEY_ID" \
      --issuer "$MAC_API_ISSUER_ID" 2>&1 | head -20 >&2 || true
    rm -f "$p8"
    exit 1
  fi
  rm -f "$p8"

  # .zip には staple できない(中身に貼る物なので)。中の .app へ貼る
  case "$f" in
    *.zip) echo "(.zip は staple の対象外 — 中の .app に貼ります)" ;;
    *) xcrun stapler staple "$f" ;;
  esac
}

cmd_dmg() {
  local f="$1"
  [ -n "${SIGN_IDENTITY:-}" ] || die "SIGN_IDENTITY がありません"
  codesign --force --timestamp --keychain "$KEYCHAIN" -s "$SIGN_IDENTITY" "$f"
  echo "署名しました: $f"
}

# ---- 出来上がりを確かめる ---------------------------------------------------

cmd_verify() {
  local f="$1"
  echo "== 貼り付いた公証の券"
  xcrun stapler validate "$f"
  echo "== Gatekeeper の目"
  # **これが利用者の目と同じ物差し。** 通れば「初回に右クリック→開く」は要らない
  spctl -a -t open --context context:primary-signature -vv "$f"
  echo "== 中の .app も見る"
  local mnt
  mnt="$(mktemp -d)"
  hdiutil attach "$f" -nobrowse -readonly -mountpoint "$mnt" > /dev/null
  local ok=0
  for app in "$mnt"/*.app; do
    [ -d "$app" ] || continue
    codesign --verify --strict --verbose=2 "$app"
    spctl -a -vv "$app"
    xcrun stapler validate "$app"
    ok=$((ok + 1))
  done
  hdiutil detach "$mnt" > /dev/null
  rmdir "$mnt" 2>/dev/null || true
  [ "$ok" -gt 0 ] || die "中に .app が1つもありません"
  echo "✅ $f は署名・公証とも通っています(.app $ok 個)"
}

case "${1:-}" in
  keychain) cmd_keychain ;;
  app)      cmd_app "${2:?使い方: sign.sh app <path.app>}" ;;
  notarize) cmd_notarize "${2:?使い方: sign.sh notarize <file>}" ;;
  dmg)      cmd_dmg "${2:?使い方: sign.sh dmg <file.dmg>}" ;;
  verify)   cmd_verify "${2:?使い方: sign.sh verify <file.dmg>}" ;;
  *) die "使い方: sign.sh keychain | app <app> | notarize <f> | dmg <f> | verify <f>" ;;
esac
