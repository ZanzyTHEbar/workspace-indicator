#!/usr/bin/env bash
set -euo pipefail
archive="${1:?Expected release archive}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
tar -xzf "$archive" -C "$tmp"
if [[ "${INSTALL_MODE:-standalone}" == plugin ]] && ! grep -q 'INSTALL_MODE' "$tmp/install.sh"; then
  echo 'This archive predates plugin install mode; use a newer release.' >&2
  exit 1
fi
bash "$tmp/install.sh"
