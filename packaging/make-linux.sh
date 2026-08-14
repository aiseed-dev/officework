#!/usr/bin/env bash
# officework を Linux 向けに包む。**Python を同梱する**(発注者 2026-08-14)。
#
#   packaging/make-linux.sh            tar.gz と .deb を作る
#   packaging/make-linux.sh --no-py    Python を同梱しない(小さい・機械の python3 を使う)
#
# 出来上がりは packaging/out/ に置く。
#
# 同梱する Python は python-build-standalone(astral-sh。PSF ライセンスで
# 再配布できる)の **3.14 系** — 手元の miniforge3 と揃える(3.12 では
# スマホの的に届かない)。pip も入っているので、matplotlib や polars は
# 利用者が後から入れられる。
#
# 探し方の順は JO_PYTHON → **同梱** → .venv → python3(pyrun/src/lib.rs)。
# 同梱を .venv より先に見るので、配った物が開発機の環境に引っ張られない。
set -euo pipefail
cd "$(dirname "$0")/.."

VER=$(grep -m1 '^version' calc/Cargo.toml | cut -d'"' -f2)
PY_VER="3.14.6"
PY_TAG="20260610"
ARCH="x86_64"
OUT="packaging/out"
NAME="officework-${VER}-linux-${ARCH}"
WITH_PY=1
[ "${1:-}" = "--no-py" ] && WITH_PY=0

echo "== officework ${VER} を包みます(Python 同梱: $([ $WITH_PY = 1 ] && echo あり || echo なし))"

# ---- 1. 組む ----------------------------------------------------------------
echo "-- cargo build --release"
cargo build --release -p calc -p writer

# ---- 2. 中身を並べる --------------------------------------------------------
rm -rf "$OUT/$NAME"
mkdir -p "$OUT/$NAME"/{bin,share/officework}
cp target/release/calc target/release/writer "$OUT/$NAME/bin/"
cp -r sample/plugins "$OUT/$NAME/share/officework/"
cp docs/calc-manual.ja.md docs/writer-manual.ja.md docs/python-manual.ja.md \
   "$OUT/$NAME/share/officework/" 2>/dev/null || true
cp README.ja.md LICENSE "$OUT/$NAME/" 2>/dev/null || true

# ---- 3. Python を同梱 -------------------------------------------------------
if [ $WITH_PY = 1 ]; then
  PY_TGZ="packaging/cache/cpython-${PY_VER}-linux.tar.gz"
  mkdir -p packaging/cache
  if [ ! -f "$PY_TGZ" ]; then
    echo "-- CPython ${PY_VER} を落とします(初回だけ)"
    curl -fsSL -o "$PY_TGZ" \
      "https://github.com/astral-sh/python-build-standalone/releases/download/${PY_TAG}/cpython-${PY_VER}%2B${PY_TAG}-${ARCH}-unknown-linux-gnu-install_only_stripped.tar.gz"
  fi
  echo "-- Python を実行ファイルの隣へ"
  tar xzf "$PY_TGZ" -C "$OUT/$NAME/bin"      # bin/python/ に展開される
  # officework パッケージ(手続きから calc を操るのに要る)を同梱の python へ
  echo "-- officework パッケージを同梱の python に入れます"
  "$OUT/$NAME/bin/python/bin/python3" -m pip install --quiet --disable-pip-version-check \
    officework || echo "   (PyPI から入れられませんでした — 手続きは動きません)"
fi

# ---- 4. 起動の台本(どこに置いても動く)-------------------------------------
for app in calc writer; do
  cat > "$OUT/$NAME/$app" <<'SH'
#!/usr/bin/env bash
# どこに置いても動くように、自分の居場所から実行ファイルを引く
here="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
exec "$here/bin/APPNAME" "$@"
SH
  sed -i "s/APPNAME/$app/" "$OUT/$NAME/$app"
  chmod +x "$OUT/$NAME/$app"
done

# ---- 5. tar.gz --------------------------------------------------------------
echo "-- tar.gz を作ります"
tar czf "$OUT/${NAME}.tar.gz" -C "$OUT" "$NAME"

# ---- 6. .deb ----------------------------------------------------------------
echo "-- .deb を作ります"
DEB="$OUT/deb"
rm -rf "$DEB"
mkdir -p "$DEB/DEBIAN" "$DEB/opt/officework" "$DEB/usr/bin" "$DEB/usr/share/applications"
cp -r "$OUT/$NAME/." "$DEB/opt/officework/"
for app in calc writer; do
  ln -sf "/opt/officework/$app" "$DEB/usr/bin/officework-$app"
done
cat > "$DEB/DEBIAN/control" <<CTRL
Package: officework
Version: ${VER}
Section: office
Priority: optional
Architecture: amd64
Depends: libxkbcommon0, libxkbcommon-x11-0, libxcb1, libxcb-xkb1, libfontconfig1
Recommends: fonts-noto-cjk
Maintainer: aiseed-dev <https://github.com/aiseed-dev/officework>
Description: officework — 表計算と文書(Python でマクロが書ける)
 xlsx と docx を読み書きする、ネイティブの表計算とワープロ。
 .py を ~/.config/office/plugins に置くと、セルから日本語の関数として
 呼べます。ブックはコードを運ばないので「開く=実行」がありません。
CTRL
for app in calc writer; do
  cat > "$DEB/usr/share/applications/officework-$app.desktop" <<DESK
[Desktop Entry]
Type=Application
Name=officework $app
Comment=$([ "$app" = calc ] && echo "表計算(xlsx)" || echo "文書(docx)")
Exec=/opt/officework/$app %f
Icon=officework-$app
Categories=Office;$([ "$app" = calc ] && echo "Spreadsheet;" || echo "WordProcessor;")
MimeType=$([ "$app" = calc ] \
  && echo "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet;" \
  || echo "application/vnd.openxmlformats-officedocument.wordprocessingml.document;")
DESK
done
dpkg-deb --build --root-owner-group "$DEB" "$OUT/officework_${VER}_amd64.deb" > /dev/null

# ---- 7. 報せ ----------------------------------------------------------------
echo
echo "== できました"
ls -lh "$OUT"/*.tar.gz "$OUT"/*.deb | awk '{print "  ", $9, $5}'
echo
echo "試し方(どちらでも):"
echo "  tar xzf $OUT/${NAME}.tar.gz && ./${NAME}/calc"
echo "  sudo dpkg -i $OUT/officework_${VER}_amd64.deb && officework-calc"
