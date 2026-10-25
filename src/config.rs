use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Position {
    Bottom,
    Top,
    Left,
    Right,
}

impl Position {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().trim_matches(['"', '\'']).to_ascii_lowercase().as_str() {
            "bottom" | "bot" | "b" => Some(Self::Bottom),
            "top" | "t" => Some(Self::Top),
            "left" | "l" => Some(Self::Left),
            "right" | "r" => Some(Self::Right),
            _ => None,
        }
    }

    pub const fn is_vertical(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Rgba {
    pub const fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }

    pub const fn with_alpha(self, a: f64) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a,
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim().trim_matches(['"', '\'']).trim();
        if s.is_empty() {
            return None;
        }

        // Handle Hyprland/Omarchy gradient strings by taking the first color stop
        if s.contains("rgba(") && s.contains(' ') && !s.contains(',') {
            if let Some(first) = s.split_whitespace().next() {
                return Self::parse(first);
            }
        }

        let inner = if let Some(rest) = s.strip_prefix("rgba(").and_then(|v| v.strip_suffix(')')) {
            rest.trim()
        } else if let Some(rest) = s.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
            rest.trim()
        } else {
            s
        };

        // CSS decimal rgba(r, g, b, a)
        if inner.contains(',') {
            let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
            if parts.len() == 3 || parts.len() == 4 {
                let r = parts[0].parse::<f64>().ok()?;
                let g = parts[1].parse::<f64>().ok()?;
                let b = parts[2].parse::<f64>().ok()?;
                let a = if parts.len() == 4 {
                    parts[3].parse::<f64>().ok()?
                } else {
                    1.0
                };
                let scale = if r > 1.0 || g > 1.0 || b > 1.0 {
                    255.0
                } else {
                    1.0
                };
                return Some(Self::new(
                    (r / scale).clamp(0.0, 1.0),
                    (g / scale).clamp(0.0, 1.0),
                    (b / scale).clamp(0.0, 1.0),
                    a.clamp(0.0, 1.0),
                ));
            }
            return None;
        }

        let hex = inner
            .strip_prefix('#')
            .or_else(|| inner.strip_prefix("0x"))
            .or_else(|| inner.strip_prefix("0X"))
            .unwrap_or(inner);

        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::new(
                    f64::from(r) / 255.0,
                    f64::from(g) / 255.0,
                    f64::from(b) / 255.0,
                    1.0,
                ))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::new(
                    f64::from(r) / 255.0,
                    f64::from(g) / 255.0,
                    f64::from(b) / 255.0,
                    f64::from(a) / 255.0,
                ))
            }
            _ => None,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Palette {
    pub bg: Rgba,
    pub active: Rgba,
    pub occupied: Rgba,
    pub empty: Rgba,
    pub urgent: Rgba,
    pub border_alpha: f64,
}

impl Default for Palette {
    fn default() -> Self {
        Self::preset("catppuccin-mocha").unwrap()
    }
}

impl Palette {
    pub fn preset(name: &str) -> Option<Self> {
        let slug = name.trim().to_ascii_lowercase();
        let (bg, active, fg, dim, red) = match slug.as_str() {
            "catppuccin" | "catppuccin-mocha" | "mocha" => (
                "#1e1e2e", "#89b4fa", "#cdd6f4", "#9399b2", "#f38ba8",
            ),
            "catppuccin-latte" | "latte" => (
                "#eff1f5", "#1e66f5", "#4c4f69", "#8c8fa1", "#d20f39",
            ),
            "tokyo-night" | "tokyonight" => (
                "#1a1b26", "#7aa2f7", "#a9b1d6", "#565f89", "#f7768e",
            ),
            "gruvbox" | "gruvbox-dark" => (
                "#282828", "#d79921", "#ebdbb2", "#928374", "#fb4934",
            ),
            "nord" => (
                "#2e3440", "#88c0d0", "#eceff4", "#4c566a", "#bf616a",
            ),
            "rose-pine" | "rosepine" => (
                "#191724", "#c4a7e7", "#e0def4", "#6e6a86", "#eb6f92",
            ),
            "kanagawa" => (
                "#1f1f28", "#7e9cd8", "#dcd7ba", "#727169", "#c34043",
            ),
            "everforest" => (
                "#2d353b", "#a7c080", "#d3c6aa", "#859289", "#e67e80",
            ),
            "matte-black" | "matteblack" => (
                "#121212", "#e68e0d", "#bebebe", "#666666", "#d35f5f",
            ),
            "ristretto" => (
                "#2c2525", "#f38d70", "#e6d9db", "#72696a", "#fd6883",
            ),
            _ => return None,
        };

        Some(Self {
            bg: Rgba::parse(bg)?.with_alpha(0.75),
            active: Rgba::parse(active)?.with_alpha(1.0),
            occupied: Rgba::parse(fg)?.with_alpha(0.55),
            empty: Rgba::parse(dim)?.with_alpha(0.25),
            urgent: Rgba::parse(red)?.with_alpha(1.0),
            border_alpha: 0.18,
        })
    }

    pub fn apply_theme_str(&mut self, content: &str, bg_alpha: f64, occ_alpha: f64, emp_alpha: f64) {
        let mut blue_fallback: Option<Rgba> = None;
        let mut accent_explicit = false;

        for (raw_key, raw_val) in parse_kv_pairs(content) {
            let key = raw_key
                .trim_start_matches('$')
                .replace('-', "_")
                .to_ascii_lowercase();
            let Some(color) = Rgba::parse(&raw_val) else {
                continue;
            };

            match key.as_str() {
                "background" | "bg" | "dark_background" | "dark_bg" => {
                    if key == "background" || key == "bg" {
                        self.bg = color.with_alpha(bg_alpha);
                    }
                }
                "accent" | "active" => {
                    self.active = color.with_alpha(1.0);
                    accent_explicit = true;
                }
                "blue" | "color4" => {
                    blue_fallback = Some(color.with_alpha(1.0));
                }
                "foreground" | "fg" | "occupied" => {
                    self.occupied = color.with_alpha(occ_alpha);
                }
                "comment" | "muted" | "dim" | "empty" | "color8" | "dark_foreground" => {
                    self.empty = color.with_alpha(emp_alpha);
                }
                "border_alpha" | "border-alpha" => {
                    if let Ok(v) = raw_val.trim().parse::<f64>() {
                        self.border_alpha = v.clamp(0.0, 1.0);
                    }
                }
                "red" | "color1" | "urgent" => {
                    self.urgent = color.with_alpha(1.0);
                }
                _ => {}
            }
        }

        if !accent_explicit {
            if let Some(blue) = blue_fallback {
                self.active = blue;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub position: Position,
    pub margin: i32,
    pub display_ms: u32,
    pub fade_in_ms: u32,
    pub fade_out_ms: u32,
    pub debounce_ms: u32,
    pub dot_spacing: i32,
    pub pad_h: i32,
    pub pad_v: i32,
    pub dot_radius: f64,
    pub active_radius: f64,
    pub persistent_workspaces: usize,
    pub max_workspaces: usize,
    pub per_monitor: bool,
    pub theme: String,
    pub keybind: String,
    pub background_alpha: f64,
    pub occupied_alpha: f64,
    pub empty_alpha: f64,
    pub custom_bg: Option<Rgba>,
    pub custom_active: Option<Rgba>,
    pub custom_occupied: Option<Rgba>,
    pub custom_empty: Option<Rgba>,
    pub custom_urgent: Option<Rgba>,
    pub border_alpha: f64,
    pub palette: Palette,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            position: Position::Bottom,
            margin: 60,
            display_ms: 1200,
            fade_in_ms: 150,
            fade_out_ms: 300,
            debounce_ms: 80,
            dot_spacing: 20,
            pad_h: 24,
            pad_v: 14,
            dot_radius: 4.0,
            active_radius: 5.5,
            persistent_workspaces: 5,
            max_workspaces: 32,
            per_monitor: false,
            theme: "auto".to_string(),
            keybind: "$mainMod, grave".to_string(),
            background_alpha: 0.75,
            occupied_alpha: 0.55,
            empty_alpha: 0.25,
            custom_bg: None,
            custom_active: None,
            custom_occupied: None,
            custom_empty: None,
            custom_urgent: None,
            border_alpha: 0.18,
            palette: Palette::default(),
        }
    }
}

impl Config {
    pub fn apply_str(&mut self, content: &str) {
        for (raw_key, raw_val) in parse_kv_pairs(content) {
            let key = raw_key
                .trim_start_matches('$')
                .replace(['-', '.'], "_")
                .to_ascii_lowercase();
            let val = raw_val.trim().trim_matches(['"', '\'']);

            match key.as_str() {
                "position" | "location" | "edge" | "anchor" => {
                    if let Some(pos) = Position::parse(val) {
                        self.position = pos;
                    }
                }
                "margin" | "margin_bottom" | "margin_top" | "margin_left" | "margin_right" => {
                    if let Ok(v) = val.parse::<i32>() {
                        self.margin = v.clamp(0, 1000);
                    }
                }
                "display_ms" | "hold_ms" => {
                    if let Ok(v) = val.parse::<u32>() {
                        self.display_ms = v.clamp(50, 30_000);
                    }
                }
                "fade_in_ms" => {
                    if let Ok(v) = val.parse::<u32>() {
                        self.fade_in_ms = v.clamp(0, 5_000);
                    }
                }
                "fade_out_ms" => {
                    if let Ok(v) = val.parse::<u32>() {
                        self.fade_out_ms = v.clamp(0, 5_000);
                    }
                }
                "debounce_ms" => {
                    if let Ok(v) = val.parse::<u32>() {
                        self.debounce_ms = v.clamp(0, 2_000);
                    }
                }
                "dot_spacing" | "spacing" => {
                    if let Ok(v) = val.parse::<i32>() {
                        self.dot_spacing = v.clamp(4, 200);
                    }
                }
                "pad_h" => {
                    if let Ok(v) = val.parse::<i32>() {
                        self.pad_h = v.clamp(2, 200);
                    }
                }
                "pad_v" => {
                    if let Ok(v) = val.parse::<i32>() {
                        self.pad_v = v.clamp(2, 200);
                    }
                }
                "dot_radius" | "dot_r" => {
                    if let Ok(v) = val.parse::<f64>() {
                        self.dot_radius = v.clamp(1.0, 64.0);
                    }
                }
                "active_radius" | "active_r" => {
                    if let Ok(v) = val.parse::<f64>() {
                        self.active_radius = v.clamp(1.0, 64.0);
                    }
                }
                "persistent_workspaces" | "persistent_ws" => {
                    if let Ok(v) = val.parse::<usize>() {
                        self.persistent_workspaces = v.clamp(1, 32);
                    }
                }
                "max_workspaces" | "max_ws" => {
                    if let Ok(v) = val.parse::<usize>() {
                        self.max_workspaces = v.clamp(1, 32);
                    }
                }
                "per_monitor" | "per_monitor_workspaces" => {
                    self.per_monitor = matches!(
                        val.to_ascii_lowercase().as_str(),
                        "true" | "1" | "yes" | "on"
                    );
                }
                "theme" | "palette" => {
                    if !val.is_empty() {
                        self.theme = val.to_string();
                    }
                }
                "keybind" | "bind" | "hotkey" | "shortcut" => {
                    if !val.is_empty() {
                        self.keybind = val.to_string();
                    }
                }
                "background_alpha" | "bg_alpha" => {
                    if let Ok(v) = val.parse::<f64>() {
                        self.background_alpha = v.clamp(0.0, 1.0);
                    }
                }
                "occupied_alpha" | "fg_alpha" | "foreground_alpha" => {
                    if let Ok(v) = val.parse::<f64>() {
                        self.occupied_alpha = v.clamp(0.0, 1.0);
                    }
                }
                "empty_alpha" | "dim_alpha" | "muted_alpha" => {
                    if let Ok(v) = val.parse::<f64>() {
                        self.empty_alpha = v.clamp(0.0, 1.0);
                    }
                }
                "background" | "bg" | "colors_background" | "colors_bg" => {
                    self.custom_bg = Rgba::parse(val);
                }
                "active" | "accent" | "colors_active" | "colors_accent" => {
                    self.custom_active = Rgba::parse(val);
                }
                "occupied" | "foreground" | "fg" | "colors_occupied" | "colors_foreground" => {
                    self.custom_occupied = Rgba::parse(val);
                }
                "empty" | "dim" | "muted" | "comment" | "colors_empty" | "colors_dim" => {
                    self.custom_empty = Rgba::parse(val);
                }
                "urgent" | "red" | "colors_urgent" | "colors_red" => {
                    self.custom_urgent = Rgba::parse(val);
                }
                _ => {}
            }
        }
        if self.persistent_workspaces > self.max_workspaces {
            self.persistent_workspaces = self.max_workspaces;
        }
    }

    pub fn resolve_palette(&mut self, theme_file_content: Option<&str>) {
        // 1. Start from named preset or Catppuccin Mocha fallback
        let mut pal = Palette::preset(&self.theme).unwrap_or_default();
        pal.bg = pal.bg.with_alpha(self.background_alpha);
        pal.occupied = pal.occupied.with_alpha(self.occupied_alpha);
        pal.empty = pal.empty.with_alpha(self.empty_alpha);

        // 2. If theme is auto / omarchy / dragonarchy, layer active theme palette file
        let slug = self.theme.trim().to_ascii_lowercase();
        if matches!(slug.as_str(), "auto" | "omarchy" | "dragonarchy" | "current") {
            if let Some(content) = theme_file_content {
                pal.apply_theme_str(
                    content,
                    self.background_alpha,
                    self.occupied_alpha,
                    self.empty_alpha,
                );
            }
        }

        // 3. Apply explicit user color overrides
        if let Some(c) = self.custom_bg {
            let a = if c.a < 0.999 { c.a } else { self.background_alpha };
            pal.bg = c.with_alpha(a);
        }
        if let Some(c) = self.custom_active {
            pal.active = c;
        }
        if let Some(c) = self.custom_occupied {
            let a = if c.a < 0.999 { c.a } else { self.occupied_alpha };
            pal.occupied = c.with_alpha(a);
        }
        if let Some(c) = self.custom_empty {
            let a = if c.a < 0.999 { c.a } else { self.empty_alpha };
            pal.empty = c.with_alpha(a);
        }
        if let Some(c) = self.custom_urgent {
            pal.urgent = c;
        }
        pal.border_alpha = self.border_alpha;

        self.palette = pal;
    }

    /// Normalize keybind for Hyprland `.conf` (`MODS, KEY`, e.g. `"$mainMod, grave"` or `"SUPER, grave"`).
    pub fn hyprland_conf_bind(&self) -> String {
        let raw = self.keybind.trim();
        if raw.contains(',') {
            return raw.to_string();
        }
        // Convert Lua style "SUPER + SHIFT + grave" -> "SUPER SHIFT, grave"
        let parts: Vec<&str> = raw.split('+').map(str::trim).filter(|s| !s.is_empty()).collect();
        if parts.len() >= 2 {
            let key = parts.last().unwrap();
            let mods = parts[..parts.len() - 1].join(" ");
            return format!("{mods}, {key}");
        }
        "$mainMod, grave".to_string()
    }

    /// Normalize keybind for Hyprland 0.56+ / Omarchy 4 `.lua` (`"SUPER + grave"`).
    pub fn hyprland_lua_bind(&self) -> String {
        let raw = self.keybind.trim().replace("$mainMod", "SUPER");
        if let Some((mods, key)) = raw.split_once(',') {
            let mod_list: Vec<&str> = mods.split_whitespace().filter(|s| !s.is_empty()).collect();
            let k = key.trim();
            if !mod_list.is_empty() && !k.is_empty() {
                return format!("{} + {k}", mod_list.join(" + "));
            }
        }
        let parts: Vec<&str> = raw.split('+').map(str::trim).filter(|s| !s.is_empty()).collect();
        if parts.len() >= 2 {
            return parts.join(" + ");
        }
        "SUPER + grave".to_string()
    }

    /// Format a `hyprctl keyword bindd` command for live keybind registration over `.socket.sock`.
    pub fn hyprland_keyword_bindd(&self) -> String {
        format!(
            "keyword bindd {}, Show workspace indicator, exec, workspace-indicator --peek",
            self.hyprland_conf_bind()
        )
    }

    /// Format a `hyprctl keyword unbind` command for removing a previous keybind over `.socket.sock`.
    pub fn hyprland_keyword_unbind(&self) -> String {
        format!("keyword unbind {}", self.hyprland_conf_bind())
    }

    pub fn render_hyprland_conf(&self) -> String {
        let bind = self.hyprland_conf_bind();
        format!(
            "# Workspace indicator OSD — frosted-glass pill\n\
             layerrule = blur on, match:namespace workspace-indicator\n\
             layerrule = ignore_alpha 0.3, match:namespace workspace-indicator\n\
             \n\
             # Manual peek keybind\n\
             bindd = {bind}, Show workspace indicator, exec, workspace-indicator --peek\n\
             \n\
             # Session autostart\n\
             exec-once = bash -lc 'command -v systemctl >/dev/null 2>&1 && systemctl --user start workspace-indicator.service' &\n"
        )
    }

    pub fn render_hyprland_lua(&self) -> String {
        let bind = self.hyprland_lua_bind();
        format!(
            "-- Workspace indicator OSD — Hyprland 0.56+ & Omarchy Quattro Lua integration\n\
             hl.layer_rule({{\n\
             \x20 name         = \"workspace-indicator\",\n\
             \x20 match        = {{ namespace = \"workspace-indicator\" }},\n\
             \x20 blur         = true,\n\
             \x20 ignore_alpha = 0.3,\n\
             }})\n\
             \n\
             local peek_cmd = \"workspace-indicator --peek\"\n\
             if type(o) == \"table\" and type(o.bind) == \"function\" then\n\
             \x20 o.bind(\"{bind}\", \"Show workspace indicator\", peek_cmd)\n\
             else\n\
             \x20 hl.bind(\"{bind}\", hl.dsp.exec_cmd(peek_cmd), {{ description = \"Show workspace indicator\" }})\n\
             end\n\
             \n\
             hl.on(\"hyprland.start\", function()\n\
             \x20 hl.exec_cmd(\"command -v systemctl >/dev/null 2>&1 && systemctl --user start workspace-indicator.service\")\n\
             end)\n"
        )
    }
}

/// Parse TOML, INI, Hyprland `.conf`, and `.lua` table key-value assignments.
pub fn parse_kv_pairs(content: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut section = String::new();

    for raw_line in content.lines() {
        let line = strip_comments(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        // TOML section header: [colors]
        if let Some(sec) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = sec.trim().to_ascii_lowercase();
            continue;
        }

        // Track Lua nested table scope: `colors = {` or `}`
        if line == "}" || line == "}," || line == "})" {
            section.clear();
            continue;
        }
        if let Some((k, rhs)) = line.split_once('=') {
            let k_trim = k
                .trim()
                .trim_start_matches("local ")
                .trim()
                .trim_matches(['"', '\'', '[', ']']);
            let rhs_trim = rhs.trim();

            if rhs_trim == "{" {
                section = k_trim.to_ascii_lowercase();
                continue;
            }

            // Inline Lua table: `colors = { active = "#89b4fa", bg = "#1e1e2e" }`
            if let Some(inner) = rhs_trim
                .strip_prefix('{')
                .and_then(|s| s.trim_end_matches(',').strip_suffix('}'))
            {
                let sub_sec = k_trim.to_ascii_lowercase();
                for part in split_top_level_commas(inner) {
                    if let Some((sk, sv)) = part.split_once('=') {
                        let sk_clean = sk.trim().trim_matches(['"', '\'']);
                        let sv_clean = sv.trim().trim_end_matches([',', ';']).trim();
                        let full_key = if sub_sec.is_empty() || sub_sec == "return" || sub_sec == "config" || sub_sec == "indicator" {
                            sk_clean.to_string()
                        } else {
                            format!("{sub_sec}_{sk_clean}")
                        };
                        out.push((full_key, sv_clean.to_string()));
                    }
                }
                continue;
            }

            let val_clean = rhs_trim.trim_end_matches([',', ';']).trim();
            let full_key = if section.is_empty() || section == "general" || section == "indicator" {
                k_trim.to_string()
            } else {
                format!("{section}_{k_trim}")
            };
            out.push((full_key, val_clean.to_string()));
        }
    }

    out
}

fn split_top_level_commas(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0usize;

    while i < bytes.len() {
        match bytes[i] {
            b'\\' if in_single || in_double => {
                i += 2;
                continue;
            }
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b',' if !in_single && !in_double => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start <= bytes.len() {
        out.push(&s[start..]);
    }
    out
}

fn strip_comments(line: &str) -> &str {
    let trimmed_start = line.bytes().position(|b| !b.is_ascii_whitespace()).unwrap_or(0);
    let bytes = line.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'\\' if in_single || in_double => {
                i += 2;
                continue;
            }
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'#' if !in_single && !in_double => {
                // '#' at start of trimmed line is always a comment (even `#facade`).
                if i == trimmed_start {
                    return &line[..i];
                }
                if bytes[i - 1].is_ascii_whitespace() || bytes[i - 1] == b'=' {
                    let after = &line[i + 1..];
                    let hex_len = after.bytes().take_while(|b| b.is_ascii_hexdigit()).count();
                    if !(hex_len == 6 || hex_len == 8) {
                        return &line[..i];
                    }
                }
            }
            b'-' if !in_single && !in_double && i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                return &line[..i];
            }
            b'/' if !in_single && !in_double && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                return &line[..i];
            }
            _ => {}
        }
        i += 1;
    }
    line
}

pub struct ConfigLoader {
    explicit_config: Option<PathBuf>,
    cli_position: Option<Position>,
    last_config_mtime: Option<SystemTime>,
    last_theme_mtime: Option<SystemTime>,
    last_config_path: Option<PathBuf>,
    last_theme_path: Option<PathBuf>,
}

impl ConfigLoader {
    pub fn new(explicit_config: Option<PathBuf>, cli_position: Option<Position>) -> Self {
        Self {
            explicit_config,
            cli_position,
            last_config_mtime: None,
            last_theme_mtime: None,
            last_config_path: None,
            last_theme_path: None,
        }
    }

    pub fn load_force(&mut self) -> Config {
        let cfg_path = self.find_config_path();
        let theme_path = Self::find_active_theme_path();

        self.last_config_mtime = cfg_path.as_deref().and_then(file_mtime);
        self.last_theme_mtime = theme_path.as_deref().and_then(file_mtime);
        self.last_config_path = cfg_path.clone();
        self.last_theme_path = theme_path.clone();

        let mut cfg = Config::default();
        if let Some(path) = cfg_path {
            if let Ok(content) = fs::read_to_string(path) {
                cfg.apply_str(&content);
            }
        }
        if let Some(pos) = self.cli_position {
            cfg.position = pos;
        }

        let theme_content = theme_path.and_then(|p| fs::read_to_string(p).ok());
        cfg.resolve_palette(theme_content.as_deref());
        cfg
    }

    /// Returns `Some(Config)` only if the config file or active theme palette changed on disk.
    pub fn reload_if_changed(&mut self) -> Option<Config> {
        let cfg_path = self.find_config_path();
        let theme_path = Self::find_active_theme_path();

        let cfg_mtime = cfg_path.as_deref().and_then(file_mtime);
        let theme_mtime = theme_path.as_deref().and_then(file_mtime);

        if cfg_path != self.last_config_path
            || theme_path != self.last_theme_path
            || cfg_mtime != self.last_config_mtime
            || theme_mtime != self.last_theme_mtime
        {
            Some(self.load_force())
        } else {
            None
        }
    }

    fn find_config_path(&self) -> Option<PathBuf> {
        if let Some(p) = &self.explicit_config {
            if p.is_file() {
                return Some(p.clone());
            }
        }
        if let Ok(env_path) = env::var("WORKSPACE_INDICATOR_CONFIG") {
            let p = PathBuf::from(env_path);
            if p.is_file() {
                return Some(p);
            }
        }

        let xdg = xdg_config_home();
        let candidates = [
            xdg.join("workspace-indicator/config.lua"),
            xdg.join("workspace-indicator/config.toml"),
            xdg.join("workspace-indicator/config.conf"),
        ];
        candidates.into_iter().find(|p| p.is_file())
    }

    /// Discovers active theme palette across Omarchy 4 (Quattro), Omarchy 3, and Dragonarchy.
    pub fn find_active_theme_path() -> Option<PathBuf> {
        let xdg_state = xdg_state_home();
        let xdg_config = xdg_config_home();

        let candidates = [
            // Omarchy 4 (Quattro) active staged theme
            xdg_state.join("omarchy/current/theme/colors.toml"),
            // Omarchy 3 active theme
            xdg_config.join("omarchy/current/theme/colors.toml"),
            // Dragonarchy active theme palette
            xdg_config.join("current/theme/hyprland-palette.conf"),
            // Omarchy 3 fallback hyprland.conf
            xdg_config.join("omarchy/current/theme/hyprland.conf"),
        ];
        candidates.into_iter().find(|p| p.is_file())
    }
}

pub fn xdg_config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn xdg_state_home() -> PathBuf {
    env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn xdg_cache_home() -> PathBuf {
    env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_positions_and_orientations() {
        assert_eq!(Position::parse("bottom"), Some(Position::Bottom));
        assert_eq!(Position::parse("TOP"), Some(Position::Top));
        assert_eq!(Position::parse("left"), Some(Position::Left));
        assert_eq!(Position::parse("right"), Some(Position::Right));
        assert!(!Position::Bottom.is_vertical());
        assert!(!Position::Top.is_vertical());
        assert!(Position::Left.is_vertical());
        assert!(Position::Right.is_vertical());
    }

    #[test]
    fn parses_hex_hyprland_and_gradient_colors() {
        let c1 = Rgba::parse("#7aa2f7").unwrap();
        assert!((c1.r - 122.0 / 255.0).abs() < 1e-4);
        assert!((c1.a - 1.0).abs() < 1e-4);

        let c2 = Rgba::parse("rgba(89b4faff)").unwrap();
        assert!((c2.r - 137.0 / 255.0).abs() < 1e-4);

        let c3 = Rgba::parse("rgba(33ccffee) rgba(00ff99ee) 45deg").unwrap();
        assert!((c3.r - 0x33 as f64 / 255.0).abs() < 1e-4);
    }

    #[test]
    fn parses_omarchy_colors_toml_and_dragonarchy_palette() {
        let omarchy_toml = r##"
mode = "dark"
accent = "#7aa2f7"
muted = "#414868"
background = "#1a1b26"
foreground = "#a9b1d6"
"##;
        let mut cfg = Config::default();
        cfg.resolve_palette(Some(omarchy_toml));
        assert!((cfg.palette.active.r - 0x7a as f64 / 255.0).abs() < 1e-4);
        assert!((cfg.palette.bg.r - 0x1a as f64 / 255.0).abs() < 1e-4);
        assert!((cfg.palette.empty.r - 0x41 as f64 / 255.0).abs() < 1e-4);

        let dragon_conf = r##"
$background = rgba(282828ff)
$accent = rgba(d79921ff)
$foreground = rgba(ebdbb2ff)
$comment = rgba(928374ff)
"##;
        cfg.resolve_palette(Some(dragon_conf));
        assert!((cfg.palette.active.r - 0xd7 as f64 / 255.0).abs() < 1e-4);
        assert!((cfg.palette.bg.r - 0x28 as f64 / 255.0).abs() < 1e-4);
    }

    #[test]
    fn parses_lua_and_toml_user_config() {
        let lua_cfg = r##"
-- Lua configuration for workspace-indicator
#facade = "ignored comment starting with 6 hex chars"
config = { keybind = "SUPER, TAB", position = "right", margin = 48 }
theme = "tokyo-night"
colors = {
  active = "#ff9e64",
  background = "#1a1b26",
}
"##;
        let mut cfg = Config::default();
        cfg.apply_str(lua_cfg);
        cfg.resolve_palette(None);
        assert_eq!(cfg.position, Position::Right);
        assert_eq!(cfg.margin, 48);
        assert_eq!(cfg.hyprland_conf_bind(), "SUPER, TAB");
        assert_eq!(cfg.hyprland_lua_bind(), "SUPER + TAB");
        assert!((cfg.palette.active.r - 0xff as f64 / 255.0).abs() < 1e-4);
    }
}
