#!/usr/bin/env bash
# officework を macOS 向けに包む。**署名と公証まで1回で**。
#
#   packaging/make-macos.sh              組んで・包んで・署名して・公証する
#   packaging/make-macos.sh --no-sign    包むだけ(署名の下ごしらえの前に試す用)
#
# 出来上がりは packaging/out/ に置く。
#
# ## 使う前に一度だけ
#
#   1. Developer ID Application の証明書がこの Mac にあること
#        security find-identity -v -p codesigning
#      無ければ docs/mac-signing.ja.md の 1 へ(Xcode から数クリックで作れる)
#
#   2. 公証の資格を鍵束に貯める(**一度きり**)
#        xcrun notarytool store-credentials officework \
#          --key ~/Downloads/AuthKey_XXXX.p8 --key-id XXXX --issuer XXXX-…
#
#   以後は毎回この1行で済む:
#        MAC_NOTARY_PROFILE=officework packaging/make-macos.sh
#
# **.p12 への書き出しは要らない。** 鍵は既にこの Mac の中にあり、codesign が
# そのまま使う。書き出しが要るのは、鍵の無い機械(CI)へ持っていくときだけ。
#
# 同梱する Python は python-build-standalone の 3.14 系(make-linux.sh と同じ)。
set -euo pipefail
cd "$(dirname "$0")/.."

VER=$(grep -m1 '^version' calc/Cargo.toml | cut -d'"' -f2)
PY_VER="3.14.6"
PY_TAG="20260610"
OUT="packaging/out"
DIST="$OUT/macos"
SIGN=1
[ "${1:-}" = "--no-sign" ] && SIGN=0

# 的は走っている Mac に合わせる(Apple Silicon なら arm64)
case "$(uname -m)" in
  arm64) ARCH="arm64"; PY_ARCH="aarch64-apple-darwin" ;;
  x86_64) ARCH="x86_64"; PY_ARCH="x86_64-apple-darwin" ;;
  *) echo "知らない的です: $(uname -m)" >&2; exit 1 ;;
esac
# **署名しない物は名前で分かるようにする。** 出来上がりを取り違えて
# 「署名つき」と言って配ることが起きないように
SUFFIX=""; [ $SIGN = 0 ] && SUFFIX="-unsigned"
DMG="$OUT/officework-${VER}-macos-${ARCH}${SUFFIX}.dmg"

echo "== officework ${VER}(${ARCH})を包みます(署名: $([ $SIGN = 1 ] && echo あり || echo なし))"

# ---- 1. 組む ----------------------------------------------------------------
cargo build --release -p calc -p writer

# ---- 2. .app を作る ---------------------------------------------------------
rm -rf "$DIST"
mkdir -p "$DIST"
for app in calc writer; do
  APP="$DIST/officework $app.app"
  mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
  cp "target/release/$app" "$APP/Contents/MacOS/"
  cp "packaging/icons/officework-$app.icns" "$APP/Contents/Resources/"
  cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>officework $app</string>
  <key>CFBundleExecutable</key><string>$app</string>
  <key>CFBundleIdentifier</key><string>io.github.aiseed-dev.officework.$app</string>
  <key>CFBundleShortVersionString</key><string>$VER</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleIconFile</key><string>officework-$app.icns</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
done

# ---- 3. Python を同梱する ---------------------------------------------------
# calc の中に1つだけ置き、writer からも見えるようにする(pyrun が隣を見る)
PY_URL="https://github.com/astral-sh/python-build-standalone/releases/download/${PY_TAG}/cpython-${PY_VER}%2B${PY_TAG}-${PY_ARCH}-install_only_stripped.tar.gz"
curl -fsSL -o "$OUT/py.tar.gz" "$PY_URL"
tar xzf "$OUT/py.tar.gz" -C "$DIST/officework calc.app/Contents/MacOS"
rm -f "$OUT/py.tar.gz"
"$DIST/officework calc.app/Contents/MacOS/python/bin/python3" -m pip install --quiet officework || true

cp -r sample/plugins "$DIST/"
cp packaging/README.ja.md "$DIST/はじめに.md"

# ---- 4. 署名・公証 ----------------------------------------------------------
# **中身を全部そろえてから署名する。** 後から1つでも足すと署名が壊れる
if [ $SIGN = 1 ]; then
  packaging/macos/sign.sh keychain
  for app in "$DIST"/*.app; do
    packaging/macos/sign.sh app "$app"
  done
  # **.app にも券を貼る。** .dmg から出した後、繋がっていなくても開けるように
  ditto -c -k --keepParent --sequesterRsrc "$DIST" "$OUT/apps.zip"
  packaging/macos/sign.sh notarize "$OUT/apps.zip"
  for app in "$DIST"/*.app; do xcrun stapler staple "$app"; done
  rm -f "$OUT/apps.zip"
fi

# ---- 5. .dmg -----------------------------------------------------------------
hdiutil create -volname "officework $VER" -srcfolder "$DIST" -ov -format UDZO "$DMG"
if [ $SIGN = 1 ]; then
  packaging/macos/sign.sh dmg "$DMG"
  packaging/macos/sign.sh notarize "$DMG"
  packaging/macos/sign.sh verify "$DMG"
fi

# ---- 6. 報せ ----------------------------------------------------------------
echo
echo "== できました"
ls -lh "$DMG" | awk '{print "  ", $9, $5}'
if [ $SIGN = 1 ]; then
  echo
  echo "そのまま配れます(Releases に上げてください)。"
else
  echo
  echo "**署名していません。** 配る物には --no-sign を付けないこと。"
fi
