#!/bin/sh
# Put the introduction page on the server. Run on the server as root, once.
#   sh web/deploy/install.sh
# Afterwards: certbot --nginx -d office.aiseed.dev
set -eu
SRC=$(cd "$(dirname "$0")/.." && pwd)
DEST=/srv/officework/web
mkdir -p "$DEST"
cp -R "$SRC"/main.py "$SRC"/assets "$SRC"/requirements.txt "$DEST"/
python3 -m venv "$DEST/.venv"
"$DEST/.venv/bin/pip" install -q -r "$DEST/requirements.txt"
chown -R www-data:www-data "$DEST"
cp "$SRC/deploy/aiseed-office-web.service" /etc/systemd/system/
cp "$SRC/deploy/office.aiseed.dev.nginx.conf" /etc/nginx/sites-available/office.aiseed.dev
ln -sf /etc/nginx/sites-available/office.aiseed.dev /etc/nginx/sites-enabled/office.aiseed.dev
systemctl daemon-reload
systemctl enable --now aiseed-office-web
nginx -t && systemctl reload nginx
echo "done: http://office.aiseed.dev (then run certbot --nginx -d office.aiseed.dev)"
