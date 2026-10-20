#!/usr/bin/env bash
# Install workspace-indicator binary, focus watcher, systemd unit, and Hyprland drop-in.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
BINDIR="${BINDIR:-$PREFIX/bin}"
XDG_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
SYSTEMD_USER_DIR="${SYSTEMD_USER_DIR:-$XDG_CONFIG/systemd/user}"
HYPR_CONF_DIR="${HYPR_CONF_DIR:-$XDG_CONFIG/hypr/config}"

/usr/bin/install -Dm755 "$ROOT_DIR/workspace-indicator" "$BINDIR/workspace-indicator"
/usr/bin/install -Dm755 "$ROOT_DIR/scripts/workspace-indicator-focusd" "$BINDIR/workspace-indicator-focusd"
/usr/bin/install -Dm644 "$ROOT_DIR/systemd/workspace-indicator.service" "$SYSTEMD_USER_DIR/workspace-indicator.service"

if [[ -d "$HYPR_CONF_DIR" ]]; then
  /usr/bin/install -Dm644 "$ROOT_DIR/hyprland/workspace-indicator.conf" "$HYPR_CONF_DIR/workspace-indicator.local.conf"
fi
