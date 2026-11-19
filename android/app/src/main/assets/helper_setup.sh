$(command -v sudo) bash -c "$(cat <<'BOARD_SETUP'
# -board power helper setup (Debian, Ubuntu, Fedora and other systemd Linux)
set -e
if ! command -v python3 >/dev/null; then
  if command -v apt-get >/dev/null; then apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq python3 </dev/null
  elif command -v dnf >/dev/null; then dnf install -y -q python3 </dev/null
  else echo "Please install python3 first, then paste this again."; exit 1; fi
fi
(umask 077; printf '%s\n' '{{KEY}}' > /etc/board-power.key)
cat > /usr/local/bin/board-power <<'BOARD_PY'
{{HELPER_PY}}BOARD_PY
chmod 755 /usr/local/bin/board-power
cat > /etc/systemd/system/board-power.service <<'BOARD_UNIT'
[Unit]
Description=-board power helper (reboot / shut down from the -board app)
After=network-online.target
Wants=network-online.target

[Service]
Environment=BOARD_POWER_PORT={{PORT}}
ExecStart=/usr/bin/env python3 /usr/local/bin/board-power
Restart=on-failure

[Install]
WantedBy=multi-user.target
BOARD_UNIT
systemctl daemon-reload
systemctl enable board-power >/dev/null 2>&1
systemctl restart board-power
if command -v ufw >/dev/null && ufw status 2>/dev/null | grep -q "Status: active"; then ufw allow {{PORT}}/tcp >/dev/null; fi
if command -v firewall-cmd >/dev/null && firewall-cmd --state >/dev/null 2>&1; then firewall-cmd -q --permanent --add-port={{PORT}}/tcp && firewall-cmd -q --reload; fi
echo "-board power helper is running on port {{PORT}}. Go back to -board and tap Test connection."
BOARD_SETUP
)"
