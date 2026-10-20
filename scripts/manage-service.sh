#!/usr/bin/env bash
set -euo pipefail
[[ "${MANAGE_SERVICE:-1}" == 1 ]] || exit 0
if ! command -v systemctl >/dev/null || [[ -z "${XDG_RUNTIME_DIR:-}" ]]; then
  echo 'No user service session; use make start inside your Wayland session.'
  exit 0
fi
case "${1:-}" in
  start)
    if [[ -z "${WAYLAND_DISPLAY:-}" ]]; then
      echo 'No Wayland session; use make start after login.'
      exit 0
    fi
    systemctl --user daemon-reload
    # Import only variables present in this session.
    for name in WAYLAND_DISPLAY XDG_RUNTIME_DIR HYPRLAND_INSTANCE_SIGNATURE SWAYSOCK NIRI_SOCKET; do
      if [[ -n "${!name:-}" ]]; then systemctl --user import-environment "$name"; fi
    done
    systemctl --user enable --now workspace-indicator.service
    ;;
  stop)
    if systemctl --user cat workspace-indicator.service >/dev/null 2>&1; then
      systemctl --user disable --now workspace-indicator.service
    fi
    ;;
  *) echo 'Expected start or stop' >&2; exit 1 ;;
esac
