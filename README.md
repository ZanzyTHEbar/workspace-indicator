# workspace-indicator

Minimal macOS-style frosted pill workspace OSD for Hyprland (`gtk+-3.0` + `gtk-layer-shell-0` + Cairo).

## Components

- `workspace-indicator` — GTK3 layer-shell OSD daemon (`SIGUSR1` peek, `SIGUSR2` palette reload)
- `scripts/workspace-indicator-focusd` — focused-monitor rebind helper
- `systemd/workspace-indicator.service` — user systemd unit
- `hyprland/workspace-indicator.conf` — optional Hyprland drop-in (`layerrule`, `bindd`, `exec-once`)

## Build & Install

```sh
make
make install
```

## Pre-built Tarball Install

```sh
tar -xzf workspace-indicator-linux-x86_64.tar.gz
./install.sh
```
