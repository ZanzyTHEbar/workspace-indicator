# workspace-indicator

<p align="center">
  <img src="assets/demo.gif" alt="workspace-indicator demo" width="840" />
</p>

Hyper-performant Wayland workspace OSD for **Hyprland** (`0.56+` `.conf` & `.lua`), **Sway**, **Niri**, and **Omarchy** (Quattro / v4 & v3) written in pure Rust (`0` crate dependencies, 100% static `musl` binary, `~700 KB` RSS).

- **Raw Wayland Wire Protocol (`wl_shm` + `zwlr_layer_shell_v1`):** Speaks the Wayland wire protocol directly over `$WAYLAND_DISPLAY` with a double-buffered `memfd_create` shared-memory pool. Zero GTK, zero Cairo, zero GLib, zero dynamic `.so` dependencies (`static-pie linked`).
- **Native Fractional HiDPI (`wp_fractional_scale_v1` + `wp_viewporter`):** Renders SDF geometry at exact physical device pixels (`1.25x`, `1.5x`, `2.0x`) with 1-physical-pixel smoothstep anti-aliasing and zero compositor resampling blur.
- **Sub-Pixel SDF Capsule Rasterizer, Morph Animation & Urgent Dots:** Renders the frosted pill, workspace dots, a stretching active-dot capsule transition, and urgent-workspace highlights (`urgent>>` + theme `red` token) through a single analytical Signed Distance Field (SDF) function.
- **Single-Threaded `epoll` State Machine:** Multiplexes the Wayland socket, compositor event stream (`.socket2.sock` / `$SWAYSOCK` / `$NIRI_SOCKET`), `signalfd` (`SIGUSR1`/`SIGUSR2`/`SIGTERM`), `inotify` (live theme/config hot-reload), and `timerfd` in 1 thread (`~700 KB` RSS, zero child process forks).
- **4-Edge Positioning (`top`, `bottom`, `left`, `right`):** `left` and `right` automatically swap axes and rotate the pill 90° vertically.
- **Omarchy Quattro & Dragonarchy Auto-Theming:** Watches `~/.local/state/omarchy/current/theme/colors.toml` (Omarchy 4), `~/.config/omarchy/current/theme/colors.toml` (Omarchy 3), and `~/.config/current/theme/hyprland-palette.conf` (Dragonarchy) via `inotify`, plus 10 built-in presets (`catppuccin-mocha`, `catppuccin-latte`, `tokyo-night`, `gruvbox`, `nord`, `rose-pine`, `kanagawa`, `everforest`, `matte-black`, `ristretto`).
- **Dual Hyprland Integration (`.conf` + `.lua`) & Live Keybind Sync:** Emits both `workspace-indicator.local.conf` and `workspace-indicator.lua` (`hl.layer_rule` + `o.bind`/`hl.bind`), and dynamically applies custom keybinds over `.socket.sock`.

## Installation

Requires Linux, a Wayland compositor supporting layer-shell, and a running
Hyprland, Sway, or Niri session. Choose **one** binary installation method.
Use either the Omarchy plugin or systemd to own the daemon—not both.

| Method | Status |
| --- | --- |
| [AUR recipe](#1-arch-linux--aur) | Local build available; not yet published to AUR |
| [Debian package](#2-debian--ubuntu-deb) | Published amd64 and arm64 `.deb` assets since v0.4.2 |
| [Omarchy plugin](#3-omarchy-quattro-plugin) | Manual installation; marketplace submission and runtime validation pending |
| [Release `install.sh`](#4-prebuilt-release-installsh) | Published x86_64 and aarch64 tarballs |
| [Source `make`](#5-build-from-source-make) | Builds and installs locally |

### 1. Arch Linux / AUR

Until `workspace-indicator-bin` is published to AUR, build the supplied recipe
using Arch's `base-devel` tools (run `makepkg` as your normal user):

```sh
git clone https://github.com/ZanzyTHEbar/workspace-indicator.git
cd workspace-indicator/packaging/aur
makepkg -si
```

The recipe downloads checksum-verified **v0.4.0** prebuilt binaries for
`x86_64` or `aarch64`; it does not compile Rust. Once published, the intended
AUR-helper command is `paru -S workspace-indicator-bin`.

### 2. Debian / Ubuntu (`.deb`)

Download a matching `.deb` from [Releases](https://github.com/ZanzyTHEbar/workspace-indicator/releases),
then install with APT. Direct downloads for v0.4.2:

- [amd64 (x86_64)](https://github.com/ZanzyTHEbar/workspace-indicator/releases/download/v0.4.2/workspace-indicator_0.4.2-1_amd64.deb)
- [arm64 (aarch64)](https://github.com/ZanzyTHEbar/workspace-indicator/releases/download/v0.4.2/workspace-indicator_0.4.2-1_arm64.deb)

```sh
sudo apt install ./workspace-indicator_*_amd64.deb
# Use the arm64 package on ARM64 systems.
```

To build a package locally instead, install Rust via rustup,
Python 3.11+, GNU Make, and binutils (`ar`), then on x86_64:

```sh
git clone https://github.com/ZanzyTHEbar/workspace-indicator.git
cd workspace-indicator
rustup target add x86_64-unknown-linux-musl
make
python3 scripts/build-deb.py target/x86_64-unknown-linux-musl/release/workspace-indicator amd64
sudo apt install ./dist/workspace-indicator_*_amd64.deb
```

ARM64 builds require the `aarch64-unknown-linux-musl` target and a suitable
linker; pass that target to `make` and `arm64` to the package builder.
There is no APT repository or official Debian archive listing. Debian installation
testing remains pending. Package installation does not start a user service as root.

### 3. Omarchy Quattro plugin

Install the binary first using one of the other methods, and ensure
`workspace-indicator` is on the Omarchy shell's `PATH`. Then install the plugin
manually (not yet listed in the marketplace):

```sh
systemctl --user disable --now workspace-indicator.service
git clone https://github.com/ZanzyTHEbar/workspace-indicator.git \
  "$HOME/.config/omarchy/plugins/zanzythebar.workspace-indicator"
omarchy plugin validate "$HOME/.config/omarchy/plugins/zanzythebar.workspace-indicator"
omarchy-shell shell rescanPlugins
```

Enable `zanzythebar.workspace-indicator` in Omarchy's plugin manager. The service
plugin runs the existing binary; it does not install or download it. Disable any
Hyprland `exec-once` snippet that starts `workspace-indicator.service`, otherwise
it will compete with the plugin at the next login. Keep blur rules and keybinds.

Disable the plugin before removing its directory. The separately installed
binary and your configuration remain intact. This wrapper needs runtime testing
on Quattro before marketplace submission; Omarchy 3 users should use systemd instead.

### 4. Prebuilt release (`install.sh`)

No Rust compiler required. Choose `x86_64` or `aarch64` to match your machine.
Download before extracting so a failed download cannot run a partial installer:

```bash
(
  set -eu
  arch="$(uname -m)"
  case "$arch" in x86_64|aarch64) ;; *) echo "Unsupported architecture: $arch" >&2; exit 1 ;; esac
  ws_tmp="$(mktemp -d)"
  trap 'rm -rf "$ws_tmp"' EXIT
  curl -fL "https://github.com/ZanzyTHEbar/workspace-indicator/releases/latest/download/workspace-indicator-linux-${arch}.tar.gz" \
    -o "$ws_tmp/release.tar.gz"
  tar -xzf "$ws_tmp/release.tar.gz" -C "$ws_tmp"
  "$ws_tmp/install.sh"
)
```

Run as your normal user, without `sudo`. The installer places the binary in
`~/.local/bin`, the user unit in `~/.config/systemd/user`, and integration files
in existing Hyprland/Omarchy config directories. It may append an optional Lua
include to `hyprland.lua`; review/back up existing configuration before installing.
Ensure `~/.local/bin` is on your session's `PATH`.

### 5. Build from source (`make`)

Requires Rust/Cargo via rustup, GNU Make, and the matching musl target; no GTK
or Cairo development headers are needed. On x86_64:

```sh
git clone https://github.com/ZanzyTHEbar/workspace-indicator.git
cd workspace-indicator
rustup target add x86_64-unknown-linux-musl
make test
make install
```

`make install` builds the binary and invokes `install.sh` as your user. For ARM64,
use `make install RUST_TARGET=aarch64-unknown-linux-musl` with a suitable linker.

### Start and integrate

Skip systemd activation when using the Omarchy plugin. Otherwise, from a terminal
inside your Wayland session:

```sh
systemctl --user daemon-reload
systemctl --user import-environment WAYLAND_DISPLAY XDG_RUNTIME_DIR HYPRLAND_INSTANCE_SIGNATURE SWAYSOCK NIRI_SOCKET
systemctl --user enable --now workspace-indicator.service
workspace-indicator --peek
```

For Hyprland, generate the snippets with `workspace-indicator --sync-hyprland`.
Source `~/.config/hypr/config/workspace-indicator.local.conf` from `hyprland.conf`,
or load `~/.config/hypr/workspace-indicator.lua` from your Lua config—**not both**.
For Sway/Niri, configure a compositor keybinding to run `workspace-indicator --peek`;
the generated Hyprland snippets do not apply to those compositors.

See [distribution packaging](packaging/README.md) for packaging and publication details.

## Configuration (`~/.config/workspace-indicator/config.lua` or `config.toml`)

```lua
return {
  position = "bottom",      -- "bottom" | "top" | "left" | "right" (left/right rotate pill 90deg)
  margin = 60,              -- distance in px from anchored screen edge
  per_monitor = false,      -- true = only show workspaces belonging to the focused monitor
  keybind = "SUPER + grave",
  theme = "auto",           -- "auto" | "omarchy" | "dragonarchy" | "tokyo-night" | "catppuccin-mocha" | ...
  display_ms = 1200,
  fade_in_ms = 150,
  fade_out_ms = 300,
  debounce_ms = 80,
  persistent_workspaces = 5,
  max_workspaces = 10,
  colors = {
    -- Optional explicit overrides (#RRGGBB, #RRGGBBAA, or rgba(RRGGBBAA))
    -- background = "#1a1b26",
    -- active     = "#7aa2f7",
    -- occupied   = "#a9b1d6",
    -- empty      = "#414868",
    -- urgent     = "#f7768e",
  },
}
```

## CLI

```sh
workspace-indicator --peek                  # Manually reveal the OSD (SIGUSR1)
workspace-indicator --reload                # Force reload config & theme palette (SIGUSR2)
workspace-indicator --position right        # Run anchored to right edge (90° vertical pill)
workspace-indicator --sync-hyprland         # Write Hyprland .conf and .lua integration files
workspace-indicator --print-hyprland-conf   # Print Hyprland .conf snippet
workspace-indicator --print-hyprland-lua    # Print Hyprland 0.56+ / Omarchy Quattro Lua snippet
```
