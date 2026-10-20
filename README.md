# workspace-indicator

![workspace-indicator demo](assets/demo.gif)

Hyper-performant Wayland workspace OSD for **Hyprland** (0.56+ `.conf` & `.lua`) and **Omarchy** (Quattro / v4 & v3) written in pure Rust (`std` + raw Wayland `wl_shm` + `zwlr_layer_shell_v1` + CPU SDF rasterizer).

## Features

- **Zero-fork Hyprland IPC:** Queries `.socket.sock` (`j/activeworkspace`, `j/workspaces`, `j/monitors`) directly over Unix domain sockets and listens on `.socket2.sock` (`workspacev2`, `focusedmonv2`, `createworkspacev2`, `destroyworkspacev2`, `moveworkspacev2`) with atomic event coalescing.
- **4-Edge Positioning & 90° Pill Rotation:** Set `position = "bottom" | "top" | "left" | "right"`. `left` and `right` automatically rotate the pill 90° vertically.
- **Omarchy & Dragonarchy Auto-Theming:** Automatically discovers `~/.local/state/omarchy/current/theme/colors.toml` (Omarchy 4 Quattro), `~/.config/omarchy/current/theme/colors.toml` (Omarchy 3), or `~/.config/current/theme/hyprland-palette.conf` (Dragonarchy), plus built-in presets (`catppuccin-mocha`, `catppuccin-latte`, `tokyo-night`, `gruvbox`, `nord`, `rose-pine`, `kanagawa`, `everforest`, `matte-black`, `ristretto`) and custom color overrides.
- **Lua, TOML, and `.conf` Configuration:** Configure via `~/.config/workspace-indicator/config.lua`, `config.toml`, or `config.conf` with automatic `mtime` hot-reload.
- **Custom Keybinds & Dual Hyprland Output:** Generates both `workspace-indicator.local.conf` and `workspace-indicator.lua` (`hl.layer_rule` + `o.bind`/`hl.bind`).

## Example `~/.config/workspace-indicator/config.lua`

```lua
return {
  position = "bottom", -- "bottom" | "top" | "left" | "right" (left/right rotate pill 90deg)
  margin = 60,
  keybind = "SUPER + grave",
  theme = "auto",      -- "auto" | "omarchy" | "dragonarchy" | "tokyo-night" | "catppuccin-mocha" | ...
  colors = {
    -- Optional explicit overrides (#RRGGBB, #RRGGBBAA, or rgba(RRGGBBAA))
    -- background = "#1a1b26",
    -- active     = "#7aa2f7",
    -- occupied   = "#a9b1d6",
    -- empty      = "#414868",
  },
}
```
