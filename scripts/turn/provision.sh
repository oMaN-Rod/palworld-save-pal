#!/usr/bin/env bash
# Provision coturn for PSP Signal on AlmaLinux/EL9 (EPEL packaging). Idempotent; safe to re-run.
# Run from the directory holding turnserver.conf.tmpl (e.g. `cd ~/turn && bash provision.sh`).
set -euo pipefail

DOMAIN=turn.palworldsavepal.app
SVCUSER=coturn
SVCGROUP=coturn
CONF=/etc/coturn/turnserver.conf
CERT_DIR=/etc/coturn/certs
SECRET_FILE=/etc/turnserver.secret
DEPLOY_HOOK=/etc/letsencrypt/renewal-hooks/deploy/coturn

sudo dnf -y install epel-release >/dev/null
sudo dnf -y install coturn certbot >/dev/null

if [ ! -s "$SECRET_FILE" ]; then
	sudo install -m 600 -o root -g root /dev/null "$SECRET_FILE"
	openssl rand -hex 32 | sudo tee "$SECRET_FILE" >/dev/null
fi

PUBLIC_IP=$(curl -fsS ifconfig.me)

sudo mkdir -p "$CERT_DIR"
sudo chown root:"$SVCGROUP" "$CERT_DIR"
sudo chmod 750 "$CERT_DIR"

# Secret is passed via the environment, never as a sed/awk argv, so it can't leak through `ps`.
TMP_CONF=$(mktemp)
SECRET=$(sudo cat "$SECRET_FILE") PIP="$PUBLIC_IP" awk '
	{ gsub(/__PUBLIC_IP__/, ENVIRON["PIP"]); gsub(/__STATIC_AUTH_SECRET__/, ENVIRON["SECRET"]); print }
' turnserver.conf.tmpl > "$TMP_CONF"

if [ ! -f "$CERT_DIR/fullchain.pem" ]; then
	sed -i '/^cert=/d; /^pkey=/d' "$TMP_CONF"
fi

sudo install -m 640 -o root -g "$SVCGROUP" "$TMP_CONF" "$CONF"
rm -f "$TMP_CONF"

sudo mkdir -p "$(dirname "$DEPLOY_HOOK")"
sudo tee "$DEPLOY_HOOK" >/dev/null <<HOOK
#!/bin/sh
set -e
install -m 640 -o root -g $SVCGROUP "/etc/letsencrypt/live/$DOMAIN/fullchain.pem" $CERT_DIR/fullchain.pem
install -m 640 -o root -g $SVCGROUP "/etc/letsencrypt/live/$DOMAIN/privkey.pem" $CERT_DIR/privkey.pem
command -v restorecon >/dev/null && restorecon -R /etc/coturn
systemctl restart coturn
HOOK
sudo chmod 750 "$DEPLOY_HOOK"

if command -v restorecon >/dev/null; then
	sudo restorecon -R /etc/coturn
fi

if command -v firewall-cmd >/dev/null && sudo firewall-cmd --state >/dev/null 2>&1; then
	sudo firewall-cmd --permanent --add-port=3478/udp --add-port=3478/tcp --add-port=443/tcp --add-port=80/tcp --add-port=49160-49960/udp
	sudo firewall-cmd --reload
else
	echo 'firewalld not active — skipping firewall rules (open ports manually if one is in front of this host).'
fi

sudo systemctl enable --now coturn
sudo systemctl restart coturn
sudo systemctl --no-pager --lines=5 status coturn
