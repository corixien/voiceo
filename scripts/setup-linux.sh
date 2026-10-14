#!/usr/bin/env bash
# One-time Linux setup for Voiceo: lets the input group create a virtual keyboard (auto-paste).
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo: sudo bash $0"; exit 1; }

echo uinput > /etc/modules-load.d/voiceo-uinput.conf
modprobe uinput
cat > /etc/udev/rules.d/99-voiceo.rules <<'EOF'
KERNEL=="uinput", SUBSYSTEM=="misc", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"
EOF
udevadm control --reload-rules
udevadm trigger /dev/uinput
echo "Done. /dev/uinput:"; ls -l /dev/uinput
