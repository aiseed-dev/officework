#!/usr/bin/env bash
# macOS の署名と公証。**Developer ID の直配布**用(Mac App Store ではない)。
#
#   packaging/macos/sign.sh keychain          署名に使う証明書を決める
#   packaging/macos/sign.sh app <path.app>    .app を中から外へ署名する
#   packaging/macos/sign.sh notarize <file>   公証して staple する(.zip / .dmg)
#   packaging/macos/sign.sh dmg <file.dmg>    .dmg 自身に署名する
#   packaging/macos/sign.sh verify <file.dmg> 出来上がりを確かめる
#
# **2つの機械で同じ物が走る。** 要る物が違うだけ。
#
# ## 手元の Mac(簡単な方)— 渡す物はほぼ無い
#
#   鍵は既にその Mac の鍵束に居るので、**.p12 に書き出す必要が無い**。
#   書き出しが要るのは、鍵の無い機械(CI)へ持っていくときだけ。
#   公証の資格だけ一度貯めておく:
#
#     xcrun notarytool store-credentials officework \
#       --key AuthKey_XXXX.p8 --key-id XXXX --issuer XXXX-…
#
#   以後は `export MAC_NOTARY_PROFILE=officework` だけでよい。
#
# ## CI(自動にしたくなったら)— Secrets で渡す
#
#   MAC_CERT_P12         Developer ID Application の .p12 を base64 にした物
#   MAC_CERT_PASSWORD    その .p12 の合言葉
#   MAC_API_KEY_P8       App Store Connect API キー(.p8)を base64 にした物
#   MAC_API_KEY_ID       その Key ID
#   MAC_API_ISSUER_ID    その Issuer ID
#
# ## どちらでも、要るとは限らない物
#
#   MAC_SIGN_IDENTITY    使う証明書の SHA-1。鍵束に Developer ID Application が
#                        2枚以上あるときだけ要る(1枚なら自分で見つける)
#
# 手順書は docs/mac-signing.ja.md。
#
# **なぜ台本にするか**: ワークフローに直書きすると読む人が居なくなる。
# ここなら手元の Mac でも同じ物が走る。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ENTITLEMENTS="$ROOT/packaging/macos/entitlements.plist"
# 使い捨ての鍵束(CI で .p12 を入れるときだけ作る)
TMP_KEYCHAIN="${RUNNER_TEMP:-/tmp}/officework-sign.keychain-db"
# 決めた証明書の置き場(段をまたいで渡すため)
IDFILE="${TMPDIR:-/tmp}/officework-sign-identity"

die() { echo "❌ $*" >&2; exit 1; }

# codesign に渡す鍵束の指定。手元の Mac では**空**(既定の鍵束を使う)
keychain_args() {
  if [ -f "$TMP_KEYCHAIN" ]; then printf '%s\n%s\n' --keychain "$TMP_KEYCHAIN"; fi
}

# ---- 署名に使う証明書を決める -----------------------------------------------

cmd_keychain() {
  local where=""
  if [ -n "${MAC_CERT_P12:-}" ]; then
    # **CI の道**: 渡された .p12 を使い捨ての鍵束に入れる。
    # login.keychain を触らないので、手元の Mac で走らせても汚さない
    [ -n "${MAC_CERT_PASSWORD:-}" ] || die "MAC_CERT_PASSWORD がありません"
    local kp
    kp="$(uuidgen)"
    security delete-keychain "$TMP_KEYCHAIN" 2>/dev/null || true
    security create-keychain -p "$kp" "$TMP_KEYCHAIN"
    security set-keychain-settings -lut 21600 "$TMP_KEYCHAIN"
    security unlock-keychain -p "$kp" "$TMP_KEYCHAIN"

    # **`-d` を使う**(`--decode` ではない)。macOS の base64 は BSD 系で、
    # GNU 風の長い綴りが通るとは限らない。-d はどちらの機械でも通る
    local p12="${RUNNER_TEMP:-/tmp}/cert.p12"
    printf '%s' "$MAC_CERT_P12" | base64 -d > "$p12"
    security import "$p12" -k "$TMP_KEYCHAIN" -P "$MAC_CERT_PASSWORD" \
      -T /usr/bin/codesign -T /usr/bin/security
    rm -f "$p12"
    # codesign が合言葉を聞かずに鍵を使えるようにする(聞かれると CI が止まる)
    security set-key-partition-list -S apple-tool:,apple:,codesign: \
      -s -k "$kp" "$TMP_KEYCHAIN" > /dev/null
    security list-keychain -d user -s "$TMP_KEYCHAIN" \
      "$(security default-keychain | tr -d ' "')"
    where="$TMP_KEYCHAIN"
  else
    # **手元の Mac の道**: 鍵は既にこの機械の鍵束に居る。
    # **.p12 に書き出す必要は無い**
    rm -f "$TMP_KEYCHAIN" 2>/dev/null || true
    echo "この機械の鍵束を使います(.p12 は要りません)"
  fi

  local found n ident
  # shellcheck disable=SC2086  # $where は「無指定」も表したいので括らない
  found="$(security find-identity -v -p codesigning $where \
           | grep "Developer ID Application" || true)"
  n="$(printf '%s' "$found" | grep -c . || true)"

  # **`-v` で出ないときは、`-v` 無しでもう一度見る。**
  #
  # `-v` は「いま有効な物」だけを出す。有効かどうかには**証明書の鎖が
  # 辿れること**が要り、Apple の中間証明書(Developer ID CA)が鍵束に
  # 無いと、証明書自体は入っているのに1件も出ない。まっさらな CI の
  # 機械で起きる(2026-08-17、`1 identity imported` の直後に
  # 「証明書がありません」と言った)。
  #
  # 中間証明書は誰でも取れる公開の物なので、入れて見直す。
  if [ "$n" -eq 0 ] && [ -n "$where" ]; then
    echo "有効な物として出ませんでした。Apple の中間証明書を入れて見直します…" >&2
    local ca="${RUNNER_TEMP:-/tmp}/DeveloperIDG2CA.cer"
    if curl -fsSL -o "$ca" https://www.apple.com/certificateauthority/DeveloperIDG2CA.cer; then
      security import "$ca" -k "$where" 2>/dev/null || true
      rm -f "$ca"
    fi
    found="$(security find-identity -v -p codesigning $where \
             | grep "Developer ID Application" || true)"
    n="$(printf '%s' "$found" | grep -c . || true)"
  fi

  if [ -n "${MAC_SIGN_IDENTITY:-}" ]; then
    # 名指しがあればそれに従う(2枚以上ある鍵束で、どれを使うか決めた場合)
    ident="$MAC_SIGN_IDENTITY"
  elif [ "$n" -eq 0 ]; then
    # **何が入っているのかを見せてから止める。** 「ありません」だけでは
    # 直しようがない。証明書の名前は秘密ではない(署名した物から誰でも読める)
    echo "鍵束の中身(名前だけ。中身は出しません):" >&2
    # shellcheck disable=SC2086
    security find-identity -p codesigning $where >&2 || true
    echo >&2
    echo "**Developer ID Application が1つも見つかりません。** よくある原因:" >&2
    echo "  - .p12 に入っているのが別の証明書(Apple Development など)" >&2
    echo "  - 証明書の鎖が辿れない(上で中間証明書を入れても駄目だった)" >&2
    echo "上の一覧に Developer ID Application が出ているなら、その SHA-1 を" >&2
    echo "MAC_SIGN_IDENTITY に入れれば先へ進めます。" >&2
    die "docs/mac-signing.ja.md の A を見てください"
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
  printf '%s' "$ident" > "$IDFILE"
}

identity() {
  if [ -n "${SIGN_IDENTITY:-}" ]; then
    printf '%s' "$SIGN_IDENTITY"
  elif [ -s "$IDFILE" ]; then
    cat "$IDFILE"
  else
    die "先に sign.sh keychain を走らせてください"
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
  local app="$1" id
  [ -d "$app" ] || die "$app がありません"
  id="$(identity)"
  local kc=()
  while IFS= read -r a; do kc+=("$a"); done < <(keychain_args)

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
        ${kc[@]+"${kc[@]}"} -s "$id" "$f" \
        2> >(grep -v "replacing existing signature" >&2)
    else
      # ライブラリ(.dylib / .so)は自分では走らないので権利は要らない
      codesign --force --timestamp --options runtime \
        ${kc[@]+"${kc[@]}"} -s "$id" "$f" \
        2> >(grep -v "replacing existing signature" >&2)
    fi
    n=$((n + 1))
  done < <(find "$app/Contents" -type f)

  # 最後に包みそのもの
  codesign --force --timestamp --options runtime \
    --entitlements "$ENTITLEMENTS" \
    ${kc[@]+"${kc[@]}"} -s "$id" "$app"

  codesign --verify --strict --verbose=2 "$app"
  echo "署名しました: $app(中の Mach-O $n 個 + 包み)"
}

# ---- 公証 -------------------------------------------------------------------

P8="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/asc.p8"

# notarytool に渡す資格。手元は鍵束に貯めた名前1つ、CI は .p8 の3つ組
notary_args() {
  if [ -n "${MAC_NOTARY_PROFILE:-}" ]; then
    printf '%s\n%s\n' --keychain-profile "$MAC_NOTARY_PROFILE"
    return
  fi
  for v in MAC_API_KEY_P8 MAC_API_KEY_ID MAC_API_ISSUER_ID; do
    [ -n "${!v:-}" ] || die "MAC_NOTARY_PROFILE も $v もありません(docs/mac-signing.ja.md の 3)"
  done
  printf '%s' "$MAC_API_KEY_P8" | base64 -d > "$P8"
  printf '%s\n%s\n%s\n%s\n%s\n%s\n' \
    --key "$P8" --key-id "$MAC_API_KEY_ID" --issuer "$MAC_API_ISSUER_ID"
}

cmd_notarize() {
  local f="$1"
  [ -e "$f" ] || die "$f がありません"
  local na=()
  while IFS= read -r a; do na+=("$a"); done < <(notary_args)

  # **--wait で待つ。** 待たずに次へ進むと staple が「まだ通っていない」で
  # 落ちる。落ちたら理由を出す — 「失敗しました」だけでは直せない
  echo "公証に出しています(数分かかります)…"
  if ! xcrun notarytool submit "$f" "${na[@]}" --wait --timeout 30m; then
    echo "公証が通りませんでした。直近の記録:" >&2
    xcrun notarytool history "${na[@]}" 2>&1 | head -20 >&2 || true
    rm -f "$P8"
    exit 1
  fi
  rm -f "$P8"

  # .zip には staple できない(中身に貼る物なので)。中の .app へ貼る
  case "$f" in
    *.zip) echo "(.zip は staple の対象外 — 中の .app に貼ります)" ;;
    *) xcrun stapler staple "$f" ;;
  esac
}

cmd_dmg() {
  local f="$1" id
  id="$(identity)"
  local kc=()
  while IFS= read -r a; do kc+=("$a"); done < <(keychain_args)
  codesign --force --timestamp ${kc[@]+"${kc[@]}"} -s "$id" "$f"
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
