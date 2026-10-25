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

## Omarchy Quattro quick start (manual dependency install)

The plugin keeps the native renderer as a **separate dependency**. Omarchy's
installer clones and validates Git plugins; it does not run install hooks, request
sudo, resolve dependencies, or build Rust. Review the repository before enabling
its unsandboxed code. Omarchy 3 users should use the standalone install below.

The source-build route uses this checkout's native implementation. Prerequisites: Git, GNU Make,
Bash, and Rust/Cargo through an existing rustup installation. `make toolchain`
adds the host's musl target; no GTK/Cairo headers are needed. If migrating an
existing installation, complete the ownership migration below before enabling
the plugin.

```sh
git clone https://github.com/ZanzyTHEbar/workspace-indicator.git
cd workspace-indicator
make toolchain
make install INSTALL_MODE=plugin
# Ensure ~/.local/bin is on the Omarchy shell's PATH.
omarchy plugin add https://github.com/ZanzyTHEbar/workspace-indicator.git --enable
```

`INSTALL_MODE=plugin` installs only the native binary for local/tar installs and
suppresses service activation for package targets. It doesn't migrate an existing
standalone installation. Packages still ship an inactive systemd unit and example
integration under `/usr/share`; no user configuration is changed by their payload.
Ensure the binary is on the **shell's** PATH, not only your terminal's PATH.
`make install-omarchy` is a convenience wrapper around the official add command,
with binary, existing-directory, service, and generated-autostart preflight checks.
It does not stop services or edit main configuration; use the migration steps
below for existing setups. Direct official add does not perform these preflights.

### No-Rust Arch package route (check native compatibility first)

**Use this route for the plugin only with a compatible native release:** its help
must list `--plugin-managed`. Older releases lack this flag; if the checkout's
recipe still pins an older binary, use the source-build quick start above instead.
The plugin refuses incompatible binaries to prevent standalone config writes.

With that prerequisite met, the local package route requires Git, GNU Make,
and Arch `base-devel` (including `makepkg`), but no Rust compiler. From the checkout:

```sh
make build-aur inspect-aur
make install-aur INSTALL_MODE=plugin
workspace-indicator --help  # Must list --plugin-managed before adding the plugin.
omarchy plugin add https://github.com/ZanzyTHEbar/workspace-indicator.git --enable
```

The recipe downloads checksum-pinned release binaries and builds a package with
`makepkg`; installation asks `sudo pacman -U` to install it. There is currently no
published AUR listing for `workspace-indicator` or `workspace-indicator-bin`:
`yay -S` is not an available installation route. AUR publication remains a
separate maintainer action.

### Existing-user migration (explicit consent required)

1. Back up your Hyprland main config and the generated
   `~/.config/hypr/config/workspace-indicator.local.conf` and
   `~/.config/hypr/workspace-indicator.lua`, if present. Review all custom
   `exec-once` entries or other launchers for this binary/service.
2. When you're ready to transfer ownership, run
   `systemctl --user disable --now workspace-indicator.service`. Stop any other
   standalone launcher. Merely setting `MANAGE_SERVICE=0` doesn't stop an existing
   service, disable autostart, or suppress standalone config writes.
3. With your consent, edit the main Hyprland config to stop loading generated
   standalone integration and remove any other autostart of the service/binary.
   Move the old generated files outside their loaded directories (and outside
   the two generated paths checked by `make install-omarchy`). To keep blur and
   peek, copy the desired layer rules and `workspace-indicator --peek` binding
   into your own maintained snippet, without `exec-once`/systemd activation, and
   load that snippet once. Inspect `hyprland/` for syntax appropriate to `.conf`
   or Lua; don't load both. Reload Hyprland after reviewing your changes.
4. Install/update the native dependency in plugin mode, then add the plugin.
   Plugin-managed startup and config reload never write Hyprland snippets or
   change keybindings. **Standalone startup and `--sync-hyprland` still regenerate
   autostart integration**: deleting generated autostart lines once is not a
   durable migration if you later run the binary without `--plugin-managed`.

If a previous `make install-omarchy` copied a non-Git directory, inspect and back
it up before consenting to `omarchy plugin remove zanzythebar.workspace-indicator`.
Official removal disables it and moves a handmade non-Git folder to a timestamped
backup inside the plugins directory. Inspect/preserve that backup; don't overwrite
the old directory or run update on it. Then use official add to create a Git checkout.
Git-managed removal deletes the checkout, so save local edits separately first.

### Update, retry, and remove

```sh
omarchy plugin update zanzythebar.workspace-indicator
# After installing/updating the native dependency, retry a failed launch:
omarchy plugin disable zanzythebar.workspace-indicator
omarchy plugin enable zanzythebar.workspace-indicator
# Remove the plugin first; optionally remove the binary package afterward:
omarchy plugin remove zanzythebar.workspace-indicator
# Optional, for the source-built user install:
make uninstall INSTALL_MODE=plugin
```

For a local Arch package instead, optional binary removal is
`make uninstall-aur INSTALL_MODE=plugin`.

Native updates are separate from Git plugin updates: update the binary package
or repeat your chosen binary install in plugin mode, then disable/re-enable the
plugin to run it. Removing the plugin leaves the dependency and personal config.
Quickshell tracks the native child; it is not detached or automatically restarted.
Missing/old binary and native exit diagnostics go to the shell log:
`qs log -p "$OMARCHY_PATH/shell" --tail 100`.

See the official [plugin lifecycle](https://github.com/omacom/omarchy/blob/quattro/manual/32-shell-plugins.md)
and [development guide](https://omarchyplugins.com/develop.html).

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
through `makepkg`. Omarchy installs use official Git add/enable after preflight;
update/remove remain official operations with confirmation preserved.
Build/inspect-omarchy only stage and validate developer files under `dist/`.

### Prerequisites and architectures

- **Source/tarball builds:** Rust/Cargo via rustup, GNU Make, Bash, and GNU tar.
  `make toolchain` installs the selected musl target. No GTK/Cairo headers required.
- **Debian builds:** also Node.js, xz, and binutils (`ar`). Debian inspection
  requires only `ar` and tar.
- **AUR builds:** Arch `base-devel`/`makepkg`. No Rust compiler needed: the recipe
  downloads checksum-verified release binaries.
- **Omarchy:** Quattro's `omarchy` and `omarchy-shell` commands, plus a separately
  installed `workspace-indicator` binary supporting `--plugin-managed` on the shell's `PATH`.

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
# For plugin ownership, add INSTALL_MODE=plugin to either install command.
```

No Rust or Node is required to install downloaded artifacts. Newly built tarballs
also include a Makefile: after extraction, `make install` uses the bundled binary
without compiling, and `make uninstall` removes the user install. Older release
archives can be installed through this checkout's `make install-tar` in standalone
mode. Plugin mode requires an archive with the updated installer and native flag;
do not use older archives for plugin setup.

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
To suppress service actions (this still writes standalone integration):

```sh
make install MANAGE_SERVICE=0
```

Use either systemd or the Omarchy plugin to own the daemon—not both. Use
`INSTALL_MODE=plugin` for dependency-only installation, including `install-tar`,
`install-deb`, and `install-aur`; follow the explicit migration above first if
standalone integration already exists. `make uninstall-omarchy` leaves the binary.

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
workspace-indicator --plugin-managed        # Run without integration writes or keybinding changes
workspace-indicator --print-hyprland-conf   # Print Hyprland .conf snippet
workspace-indicator --print-hyprland-lua    # Print Hyprland 0.56+ / Omarchy Quattro Lua snippet
```
