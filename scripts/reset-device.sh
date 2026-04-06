#!/usr/bin/env bash
set -euo pipefail

# Reset the Matter-ONVIF bridge to factory state.
# This removes commissioning data so the device can be re-commissioned.

STORAGE_PATH="${MATTER_STORAGE_PATH:-.matter-storage}"

echo "=== Matter-ONVIF Bridge Reset ==="

# Stop the service if running via systemd
if systemctl is-active --quiet matter-onvif-bridge 2>/dev/null; then
  echo "Stopping service..."
  sudo systemctl stop matter-onvif-bridge
  RESTART=1
else
  # Kill any running instance
  if pkill -f matter-onvif-bridge 2>/dev/null; then
    echo "Stopped running bridge process"
    sleep 1
  fi
  RESTART=0
fi

# Also stop go2rtc if it was spawned by the bridge
pkill -f go2rtc 2>/dev/null && echo "Stopped go2rtc" || true

# Remove Matter commissioning state
if [ -d "$STORAGE_PATH" ]; then
  rm -rf "$STORAGE_PATH"
  echo "Removed Matter storage: $STORAGE_PATH"
else
  echo "No Matter storage found at $STORAGE_PATH (already clean)"
fi

echo ""
echo "Device reset complete."
echo "On next start, the bridge will open a fresh commissioning window."

if [ "$RESTART" = "1" ]; then
  echo ""
  read -p "Restart the service now? [y/N] " -n 1 -r
  echo
  if [[ $REPLY =~ ^[Yy]$ ]]; then
    sudo systemctl start matter-onvif-bridge
    echo "Service restarted. Check logs: journalctl -u matter-onvif-bridge -f"
  fi
fi
