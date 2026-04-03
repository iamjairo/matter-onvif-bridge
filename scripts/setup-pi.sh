#!/usr/bin/env bash
set -euo pipefail

# Setup script for Raspberry Pi deployment
# Installs the matter-onvif-bridge as a systemd service

INSTALL_DIR="${1:-/opt/matter-onvif-bridge}"
SERVICE_USER="${2:-${SUDO_USER:-$(whoami)}}"
SERVICE_NAME="matter-onvif-bridge"

echo "=== Matter-ONVIF Bridge Pi Setup ==="
echo "Install dir: $INSTALL_DIR"
echo "Service user: $SERVICE_USER"

# Check if running as root
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root (sudo)"
  exit 1
fi

# Install Node.js 20 if not present
if ! command -v node &>/dev/null || [ "$(node -v | cut -d. -f1 | tr -d v)" -lt 20 ]; then
  echo "Installing Node.js 20..."
  curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
  apt-get install -y nodejs
fi

echo "Node.js version: $(node -v)"

# Install pnpm if not present
if ! command -v pnpm &>/dev/null; then
  echo "Installing pnpm..."
  npm install -g pnpm
fi

echo "pnpm version: $(pnpm -v)"

# Install go2rtc
echo "Installing go2rtc..."
bash "$INSTALL_DIR/scripts/install-go2rtc.sh"

# Install dependencies and build
cd "$INSTALL_DIR"
pnpm install --frozen-lockfile
pnpm run build

# Create .env if it doesn't exist
if [ ! -f "$INSTALL_DIR/.env" ]; then
  cp "$INSTALL_DIR/.env.example" "$INSTALL_DIR/.env"
  echo "Created .env from .env.example — edit it with your ONVIF credentials"
fi

# Set ownership
chown -R "$SERVICE_USER:$SERVICE_USER" "$INSTALL_DIR"

# Create systemd service
cat > "/etc/systemd/system/${SERVICE_NAME}.service" <<EOF
[Unit]
Description=Matter-ONVIF Camera Bridge
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$SERVICE_USER
WorkingDirectory=$INSTALL_DIR
ExecStart=/usr/bin/node $INSTALL_DIR/dist/index.js
Restart=always
RestartSec=5
Environment=NODE_ENV=production

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable "$SERVICE_NAME"

echo ""
echo "=== Setup Complete ==="
echo "1. Edit $INSTALL_DIR/.env with your ONVIF camera credentials"
echo "2. Start the service: sudo systemctl start $SERVICE_NAME"
echo "3. View logs: journalctl -u $SERVICE_NAME -f"
echo "4. On first run, scan the QR code with your Matter controller to commission"
