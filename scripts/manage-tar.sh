#!/usr/bin/env bash
set -euo pipefail
archive="${1:?Expected release archive}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
tar -xzf "$archive" -C "$tmp"
bash "$tmp/install.sh"
