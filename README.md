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

## Build, inspect, install, uninstall

GNU Make is the public interface for every distribution format. Run commands
from the repository root as your normal user; package targets request `sudo`
only for APT/pacman operations. Choose one binary installation method.

```sh
git clone https://github.com/ZanzyTHEbar/workspace-indicator.git
cd workspace-indicator
make help
```

| Format | Build | Inspect | Install | Uninstall |
| --- | --- | --- | --- | --- |
| Local source | `make build` | `make inspect` | `make install` | `make uninstall` |
| Release tarball | `make build-tar` | `make inspect-tar` | `make install-tar` | `make uninstall-tar` |
| Debian / Ubuntu | `make build-deb` | `make inspect-deb` | `make install-deb` | `make uninstall-deb` |
| Arch / AUR recipe | `make build-aur` | `make inspect-aur` | `make install-aur` | `make uninstall-aur` |
| Omarchy plugin | `make build-omarchy` | `make inspect-omarchy` | `make install-omarchy` | `make uninstall-omarchy` |

Local and Debian installs build automatically. Build a tarball before installing
it, or pass a downloaded archive. AUR installs build the pinned binary package
through `makepkg`. Omarchy installs validate, stop the competing user service,
copy the plugin, rescan the shell, and enable it; removing uses Omarchy's own
removal command. Existing plugin directories are never overwritten.

### Prerequisites and architectures

- **Source/tarball builds:** Rust/Cargo via rustup, GNU Make, Bash, and GNU tar.
  `make toolchain` installs the selected musl target. No GTK/Cairo headers required.
- **Debian builds:** also Node.js, xz, and binutils (`ar`). Debian inspection
  requires only `ar` and tar.
- **AUR builds:** Arch `base-devel`/`makepkg`. No Rust compiler needed: the recipe
  downloads checksum-verified release binaries.
- **Omarchy:** Quattro's `omarchy` and `omarchy-shell` commands, plus a separately
  installed `workspace-indicator` binary on the shell's `PATH`.

The host architecture is selected automatically (`x86_64` or `aarch64`). To cross-build:

```sh
make toolchain ARCH=aarch64
make build-tar build-deb ARCH=aarch64
```

Cross-compilation also requires a suitable linker configured through Cargo, e.g.
`CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER`. Never cross-install onto a
different host architecture. `RUST_TARGET` remains available for custom toolchains.

### Use downloaded packages without compiling

Download your matching artifact from [Releases](https://github.com/ZanzyTHEbar/workspace-indicator/releases).
amd64 and arm64 `.deb` packages are available since v0.4.2. From this checkout,
replace the `PACKAGE` path below with the exact downloaded filename for your
version and architecture:

```sh
make inspect-deb install-deb PACKAGE="/path/to/downloaded-package.deb"
# Or a downloaded x86_64 / aarch64 release tarball:
make inspect-tar install-tar ARCHIVE="$HOME/Downloads/workspace-indicator-linux-x86_64.tar.gz"
```

No Rust or Node is required to install downloaded artifacts. Newly built tarballs
also include a Makefile: after extraction, `make install` uses the bundled binary
without compiling, and `make uninstall` removes the user install. Older release
archives can be installed through this checkout's `make install-tar`.

Local/tarball installation uses `~/.local/bin` and `~/.config`, including the user
unit, Hyprland snippets, and Omarchy theme hooks. It may append an optional Lua
include; back up existing config before installation. Uninstall preserves your
main Hyprland config (including the harmless optional include) and personal settings.
APT/pacman installations use `/usr/bin` and the distribution's user-unit directory.
There is no APT repository or official Debian archive listing.

### Session management

Install targets enable/start a **systemd user service** when run from a terminal
inside your Wayland session. If you installed from a Linux console or over SSH,
open a terminal inside Sway, Hyprland, or Niri and run these commands from the checkout:

```sh
make start
systemctl --user status workspace-indicator.service
```

`make start` reloads user units and imports the compositor environment before
starting the service; no `sudo` is needed. `make stop` disables/stops it.
To stage without changing session state:

```sh
make install MANAGE_SERVICE=0
```

Use either systemd or the Omarchy plugin to own the daemon—not both. Before using
`make install-omarchy`, install the binary through one of the other Make targets
and disable any Hyprland `exec-once` snippet that starts the service. The plugin
does not own the separate binary, so `make uninstall-omarchy` leaves it installed.

For Hyprland, source the generated `.conf` snippet or load the Lua snippet—not both.
For Sway, add an optional shortcut to `~/.config/sway/config` and reload the config:

```sway
bindsym $mod+grave exec workspace-indicator --peek
```

For Niri, bind the same `workspace-indicator --peek` command in its configuration.
The shortcut reveals the OSD; the service must already be running.

See [distribution packaging](packaging/README.md) for release and publication details.

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
  max_workspaces = 32,       -- supported numeric workspace range: 1–32
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

Numeric workspaces 1–32 are supported; `max_workspaces` defaults to 32 and can
be lowered. Five slots remain visible by default (`persistent_workspaces = 5`);
additional slots appear as needed. Sway workspaces without a positive numeric
`num` are not mapped to indicator slots.

## CLI

```sh
workspace-indicator --peek                  # Manually reveal the OSD (SIGUSR1)
workspace-indicator --reload                # Force reload config & theme palette (SIGUSR2)
workspace-indicator --position right        # Run anchored to right edge (90° vertical pill)
workspace-indicator --sync-hyprland         # Write Hyprland .conf and .lua integration files
workspace-indicator --print-hyprland-conf   # Print Hyprland .conf snippet
workspace-indicator --print-hyprland-lua    # Print Hyprland 0.56+ / Omarchy Quattro Lua snippet
```
