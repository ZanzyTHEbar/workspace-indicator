#!/usr/bin/env bash
# Install workspace-indicator binary, systemd unit, Hyprland (.conf + .lua), and Omarchy hooks.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
BINDIR="${BINDIR:-$PREFIX/bin}"
XDG_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
SYSTEMD_USER_DIR="${SYSTEMD_USER_DIR:-$XDG_CONFIG/systemd/user}"
HYPR_DIR="${HYPR_DIR:-$XDG_CONFIG/hypr}"
HYPR_CONF_DIR="${HYPR_CONF_DIR:-$HYPR_DIR/config}"
OMARCHY_HOOKS_ROOT="${OMARCHY_HOOKS_ROOT:-$XDG_CONFIG/omarchy/hooks}"
INSTALL_MODE="${INSTALL_MODE:-standalone}"
case "$INSTALL_MODE" in
  standalone|plugin) ;;
  *) echo 'INSTALL_MODE must be standalone or plugin' >&2; exit 1 ;;
esac

if [[ "${1:-}" == --uninstall ]]; then
  if [[ "$INSTALL_MODE" == plugin ]]; then
    rm -f "$BINDIR/workspace-indicator"
    exit 0
  fi
  bash "$ROOT_DIR/scripts/manage-service.sh" stop
  rm -f "$BINDIR/workspace-indicator" "$BINDIR/workspace-indicator-focusd" \
    "$SYSTEMD_USER_DIR/workspace-indicator.service" \
    "$HYPR_CONF_DIR/workspace-indicator.local.conf" "$HYPR_DIR/workspace-indicator.lua" \
    "$OMARCHY_HOOKS_ROOT/theme-set-workspace-indicator" "$OMARCHY_HOOKS_ROOT/theme-set.d/workspace-indicator"
  # The optional pcall include is harmless without its module; preserve user config.
  exit 0
fi
[[ $# == 0 ]] || { echo 'Usage: install.sh [--uninstall]' >&2; exit 1; }

/usr/bin/install -Dm755 "$ROOT_DIR/workspace-indicator" "$BINDIR/workspace-indicator"
[[ "$INSTALL_MODE" == standalone ]] || exit 0
/usr/bin/install -Dm755 "$ROOT_DIR/scripts/workspace-indicator-focusd" "$BINDIR/workspace-indicator-focusd"
/usr/bin/install -Dm644 "$ROOT_DIR/systemd/workspace-indicator.service" "$SYSTEMD_USER_DIR/workspace-indicator.service"

if [[ -d "$HYPR_CONF_DIR" ]]; then
  /usr/bin/install -Dm644 "$ROOT_DIR/hyprland/workspace-indicator.conf" "$HYPR_CONF_DIR/workspace-indicator.local.conf"
fi

if [[ -d "$HYPR_DIR" ]]; then
  /usr/bin/install -Dm644 "$ROOT_DIR/hyprland/workspace-indicator.lua" "$HYPR_DIR/workspace-indicator.lua"
  if [[ -f "$HYPR_DIR/hyprland.lua" ]] && ! grep -q 'hypr\.workspace-indicator' "$HYPR_DIR/hyprland.lua"; then
    printf '\n-- Optional workspace-indicator OSD (no-op if uninstalled)\npcall(require, "hypr.workspace-indicator")\n' >> "$HYPR_DIR/hyprland.lua"
  fi
fi

if [[ -d "$XDG_CONFIG/omarchy" ]]; then
  # Support both Omarchy Quattro flat `hooks/theme-set*` glob and `hooks/theme-set.d/`
  /usr/bin/install -Dm755 "$ROOT_DIR/omarchy/theme-set-hook" "$OMARCHY_HOOKS_ROOT/theme-set-workspace-indicator"
  /usr/bin/install -Dm755 "$ROOT_DIR/omarchy/theme-set-hook" "$OMARCHY_HOOKS_ROOT/theme-set.d/workspace-indicator"
fi

# Re-render Hyprland .conf and .lua with any user-configured keybind from config.{lua,toml,conf}
"$BINDIR/workspace-indicator" --sync-hyprland 2>/dev/null || true
bash "$ROOT_DIR/scripts/manage-service.sh" start
