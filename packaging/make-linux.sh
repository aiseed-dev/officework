#!/usr/bin/env bash
# officework を Linux 向けに包む。**Python は同梱しません**(発注者 2026-08-24
# 「Python を同梱する必要もなくて、zed と同じように作業ディレクトリー内の
# 仮想環境を優先でいいでしょう」)。
#
#   packaging/make-linux.sh            tar.gz と .deb を作る
#
# 出来上がりは packaging/out/ に置く。
#
# **なぜやめたか。** 同梱した Python は読むだけの物で、matplotlib も polars も
# 入っていませんでした。その2つは結局あとから `.venv` に入れる必要があり、
# *同梱があってもなくても利用者の手順は同じ*でした。荷物だけが大きくなります。
#
# **同梱する道そのものを外しました**(2026-09-04 発注者「Python の同梱は
# なくてもいいのでは」「自由に環境が選択できるのがいい」)。mac と Windows の
# 包みからも外し、3つの OS で同じ形になりました。
#
# 探し方の順は JO_PYTHON → **開いている綴りの .venv** → 開発機の .venv →
# 利用者の venv → python3(pyrun/src/lib.rs)。
# エディタや JupyterLab と同じフォルダを見ているとき、3つとも同じ Python を使います。
set -euo pipefail
cd "$(dirname "$0")/.."

VER=$(grep -m1 '^version' officework/Cargo.toml | cut -d'"' -f2)
ARCH="x86_64"
OUT="packaging/out"
NAME="officework-${VER}-linux-${ARCH}"

echo "== officework ${VER} を包みます"

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
# **組んだ跡は配りません。** `__pycache__` は走らせた機械の物で、
# 配る意味がないうえ、別の版の Python では読めません
find "$OUT/$NAME/share/officework/plugins" -name __pycache__ -type d -exec rm -rf {} + 2>/dev/null || true
# **手引きは docs/ja の adoc です**(2026-09-04 に直しました)。
# 前は `docs/calc-manual.ja.md` という**もう無い径路**を見ていて、
# `|| true` が付いていたので**黙って手引き無しの包み**が出来ていました
for m in calc-manual writer-manual python-manual genkou-manual; do
  cp "docs/ja/$m.adoc" "$OUT/$NAME/share/officework/"
done
cp LICENSE "$OUT/$NAME/"
cp packaging/README.ja.md "$OUT/$NAME/はじめに.md"

# **Python は同梱しません**(2026-09-04 発注者「Python の同梱はなくてもいい」
# 「自由に環境が選択できるのがいい」)。実行の側は 2026-08-24 から同梱を
# 見ておらず、同梱の物には matplotlib も polars も入っていないので、
# 利用者は結局自分の環境を用意します。使う Python は設定で選べます

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
