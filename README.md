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

## Quick Install (Pre-Built Release)

See [distribution packaging](packaging/README.md) for the AUR recipe,
Omarchy Quattro service plugin, and Debian package builds and validation status.

```sh
ws_tmp="$(mktemp -d)"
curl -fsSL https://github.com/ZanzyTHEbar/workspace-indicator/releases/latest/download/workspace-indicator-linux-x86_64.tar.gz | tar -xz -C "$ws_tmp"
"$ws_tmp/install.sh"
rm -rf "$ws_tmp"
systemctl --user daemon-reload && systemctl --user enable --now workspace-indicator.service
```

## Build from Source

Requires only `rustc`/`cargo` (no system `-dev` headers needed):

```sh
make
make install
```

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
