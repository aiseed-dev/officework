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

VER=$(grep -m1 '^version' officework/Cargo.toml | cut -d'"' -f2)
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
cargo build --release -p officework

# ---- 2. 中身を並べる --------------------------------------------------------
rm -rf "$OUT/$NAME"
mkdir -p "$OUT/$NAME"/{bin,share/officework}
# **配るのは officework 1本**(2026-08-19 発注者確定。SEKKEI 段11)。
# calc と writer の単体は開発と試験の道具として残しますが、包みません
cp target/release/officework "$OUT/$NAME/bin/"
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
cat > "$OUT/$NAME/officework" <<'SH'
#!/usr/bin/env bash
# どこに置いても動くように、自分の居場所から実行ファイルを引く
here="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
exec "$here/bin/officework" "$@"
SH
chmod +x "$OUT/$NAME/officework"

# ---- 5. tar.gz --------------------------------------------------------------
echo "-- tar.gz を作ります"
tar czf "$OUT/${NAME}.tar.gz" -C "$OUT" "$NAME"

# ---- 6. .deb ----------------------------------------------------------------
echo "-- .deb を作ります"
DEB="$OUT/deb"
rm -rf "$DEB"
mkdir -p "$DEB/DEBIAN" "$DEB/opt/officework" "$DEB/usr/bin" "$DEB/usr/share/applications"
cp -r "$OUT/$NAME/." "$DEB/opt/officework/"
ln -sf "/opt/officework/officework" "$DEB/usr/bin/officework"
# **bubblewrap は Recommends ではなく Depends です**(2026-08-24 発注者)。
# マクロのサンドボックスがこれを使います。入っていない機械では、他所から
# 来たコードは実行を断る作りなので、機能が1つ丸ごと使えません。
# 「入っていれば効く」だと、守りが効くかどうかが機械任せになります。
cat > "$DEB/DEBIAN/control" <<CTRL
Package: officework
Version: ${VER}
Section: office
Priority: optional
Architecture: amd64
Depends: libxkbcommon0, libxkbcommon-x11-0, libxcb1, libxcb-xkb1, libfontconfig1, bubblewrap
Recommends: fonts-noto-cjk
Maintainer: aiseed-dev <https://github.com/aiseed-dev/officework>
Description: officework — 表計算と文書(Python でマクロが書ける)
 xlsx と docx を読み書きする、ネイティブの表計算とワープロ。
 .py を ~/.config/officework/funcs に置くと、セルから日本語の関数として
 呼べます。ブックはコードを運ばないので「開く=実行」がありません。
CTRL
# ---- .desktop は1枚 ---------------------------------------------------------
#
# **開ける物を全部並べます**(SEKKEI 段11)。うちの形(`.adoc` `.sheet.adoc`)と、
# 受け渡しの2つ(`.docx` `.xlsx`)です。`.adoc` には決まった MIME 型が無いので
# `text/x-asciidoc` を使います(asciidoctor の界隈で通っている名前)。
#
# 表か文章かは**名前で決まります**(`.sheet.adoc` は表)。窓は1つで、
# ファイルはタブとして開きます — 2枚目を渡されても窓は増えません
cat > "$DEB/usr/share/applications/officework.desktop" <<DESK
[Desktop Entry]
Type=Application
Name=officework
Comment=表計算と文書
Exec=/opt/officework/officework %f
Icon=officework
Terminal=false
Categories=Office;Spreadsheet;WordProcessor;
MimeType=application/vnd.openxmlformats-officedocument.spreadsheetml.sheet;application/vnd.openxmlformats-officedocument.wordprocessingml.document;text/x-asciidoc;
DESK

# **うちの形の MIME 型を機械に教えます。** `text/x-asciidoc` を知らない
# 機械では `.adoc` を渡しても officework が候補に出ません
mkdir -p "$DEB/usr/share/mime/packages"
cat > "$DEB/usr/share/mime/packages/officework.xml" <<MIME
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="text/x-asciidoc">
    <comment>AsciiDoc</comment>
    <comment xml:lang="ja">AsciiDoc の文書</comment>
    <glob pattern="*.adoc"/>
    <glob pattern="*.sheet.adoc"/>
    <glob pattern="*.tmpl.adoc"/>
    <glob pattern="*.form.adoc"/>
    <sub-class-of type="text/plain"/>
  </mime-type>
</mime-info>
MIME

# **絵も一緒に入れる。** `.desktop` が Icon= で名指ししているのに絵が
# 無ければ、ランチャーで無地の四角になる(2026-08-17 のアルファの
# 棚卸しまで、まさにその状態だった)。正本は packaging/icons の SVG 1枚で、
# 配る形は tools/make_icons.py が起こしてコミットしてある
for s in 16 22 24 32 48 64 128 256 512; do
  d="$DEB/usr/share/icons/hicolor/${s}x${s}/apps"
  mkdir -p "$d"
  cp "packaging/icons/hicolor/${s}x${s}/officework.png" "$d/"
done
mkdir -p "$DEB/usr/share/icons/hicolor/scalable/apps"
cp packaging/icons/officework.svg "$DEB/usr/share/icons/hicolor/scalable/apps/"

# **入れた後に機械へ知らせます。** これが無いと、絵も関連付けも
# 次のログインまで効きません
mkdir -p "$DEB/DEBIAN"
cat > "$DEB/DEBIAN/postinst" <<'POST'
#!/bin/sh
set -e
update-mime-database /usr/share/mime >/dev/null 2>&1 || true
update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
gtk-update-icon-cache -f -t /usr/share/icons/hicolor >/dev/null 2>&1 || true
POST
chmod 755 "$DEB/DEBIAN/postinst"
cp "$DEB/DEBIAN/postinst" "$DEB/DEBIAN/postrm"

dpkg-deb --build --root-owner-group "$DEB" "$OUT/officework_${VER}_amd64.deb" > /dev/null

# ---- 7. 報せ ----------------------------------------------------------------
echo
echo "== できました"
ls -lh "$OUT"/*.tar.gz "$OUT"/*.deb | awk '{print "  ", $9, $5}'
echo
echo "試し方(どちらでも):"
echo "  tar xzf $OUT/${NAME}.tar.gz && ./${NAME}/officework"
echo "  sudo dpkg -i $OUT/officework_${VER}_amd64.deb && officework"
