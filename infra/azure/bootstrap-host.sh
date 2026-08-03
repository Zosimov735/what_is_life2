#!/usr/bin/env bash
set -euo pipefail

TASK_ADMIN_USERNAME="${1:?Pass the administrator username as argument 1}"
TASK_MANAGEMENT_CIDR="${2:?Pass the management IPv4 CIDR as argument 2}"
TASK_BACKUP_STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
TASK_BACKUP_DIR="/var/backups/what-is-life"

id "${TASK_ADMIN_USERNAME}" >/dev/null
sudo mkdir -p "${TASK_BACKUP_DIR}"
sudo tar -czf "${TASK_BACKUP_DIR}/host-config-${TASK_BACKUP_STAMP}.tar.gz" \
  /etc/nginx /etc/ssh /etc/ufw /etc/fail2ban /etc/systemd/system \
  2>/dev/null || true

sudo install -d -m 0755 /srv/what-is-life/releases/bootstrap
sudo tee /etc/nginx/sites-available/what-is-life >/dev/null <<'NGINX'
server {
    listen 127.0.0.1:8080 default_server;
    server_name _;
    root /srv/what-is-life/current;
    index index.html;

    location = /index.html {
        add_header Cache-Control "no-store" always;
    }

    location ~* \.wasm$ {
        default_type application/wasm;
        add_header Cache-Control "public, max-age=31536000, immutable" always;
    }

    location / {
        try_files $uri $uri/ /index.html;
        add_header X-Content-Type-Options "nosniff" always;
        add_header Referrer-Policy "no-referrer" always;
    }
}
NGINX

sudo tee /srv/what-is-life/releases/bootstrap/index.html >/dev/null <<'HTML'
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>What Is Life test host</title>
  </head>
  <body>
    <main>
      <h1>What Is Life test host is ready.</h1>
      <p>No project build has been deployed yet.</p>
    </main>
  </body>
</html>
HTML

sudo tee /etc/fail2ban/jail.d/sshd.local >/dev/null <<'FAIL2BAN'
[sshd]
enabled = true
maxretry = 5
findtime = 10m
bantime = 1h
FAIL2BAN

sudo tee /etc/ssh/sshd_config.d/60-what-is-life.conf >/dev/null <<SSH
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitRootLogin no
X11Forwarding no
AllowUsers ${TASK_ADMIN_USERNAME}
SSH

sudo chown -R "${TASK_ADMIN_USERNAME}:www-data" /srv/what-is-life
sudo ln -sfn /srv/what-is-life/releases/bootstrap /srv/what-is-life/current
sudo rm -f /etc/nginx/sites-enabled/default
sudo ln -sfn /etc/nginx/sites-available/what-is-life /etc/nginx/sites-enabled/what-is-life
sudo nginx -t
sudo sshd -t
sudo systemctl enable --now nginx fail2ban unattended-upgrades
sudo systemctl reload ssh nginx
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow from "${TASK_MANAGEMENT_CIDR}" to any port 22 proto tcp
sudo ufw --force enable
curl --fail --silent --show-error http://127.0.0.1:8080/ >/dev/null

printf 'Backup: %s/host-config-%s.tar.gz\n' "${TASK_BACKUP_DIR}" "${TASK_BACKUP_STAMP}"
