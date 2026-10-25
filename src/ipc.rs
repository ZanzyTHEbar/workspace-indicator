use std::env;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MonitorTarget {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub make: String,
    pub model: String,
    pub focused: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceState {
    pub active_ws: usize,
    pub occupied: Vec<bool>,
    pub urgent: Vec<bool>,
    pub occ_max: usize,
    pub target_monitor: Option<MonitorTarget>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CompositorBackend {
    Hyprland,
    Sway,
    Niri,
}

pub fn detect_backend() -> CompositorBackend {
    if env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        CompositorBackend::Hyprland
    } else if env::var_os("NIRI_SOCKET").is_some() {
        CompositorBackend::Niri
    } else if env::var_os("SWAYSOCK").is_some() {
        CompositorBackend::Sway
    } else {
        CompositorBackend::Hyprland
    }
}

/// Locate Hyprland socket (`socket.sock` or `socket2.sock`).
pub fn find_hypr_socket(socket_name: &str) -> Option<PathBuf> {
    let sig = env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    if sig.is_empty() {
        return None;
    }

    if let Ok(xdg) = env::var("XDG_RUNTIME_DIR") {
        let p = PathBuf::from(xdg).join("hypr").join(&sig).join(socket_name);
        if p.exists() {
            return Some(p);
        }
    }

    let fallback = PathBuf::from("/tmp/hypr").join(&sig).join(socket_name);
    if fallback.exists() {
        return Some(fallback);
    }
    None
}

/// Query Hyprland `.socket.sock` directly over a Unix domain socket with zero process forks.
pub fn hypr_request(cmd: &[u8]) -> Option<String> {
    let path = find_hypr_socket(".socket.sock")?;
    let mut stream = UnixStream::connect(path).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
    stream.write_all(cmd).ok()?;

    let mut out = String::with_capacity(2048);
    stream.read_to_string(&mut out).ok()?;
    Some(out)
}

pub fn query_workspace_state(max_ws: usize, per_monitor: bool) -> WorkspaceState {
    match detect_backend() {
        CompositorBackend::Hyprland => {
            let active_json = hypr_request(b"j/activeworkspace").unwrap_or_default();
            let workspaces_json = hypr_request(b"j/workspaces").unwrap_or_default();
            let monitors_json = hypr_request(b"j/monitors").unwrap_or_default();
            parse_workspace_state_filtered(
                &active_json,
                &workspaces_json,
                &monitors_json,
                max_ws,
                per_monitor,
            )
        }
        CompositorBackend::Sway => {
            let ws_json = sway_request(1, b"").unwrap_or_default();
            parse_sway_workspaces(&ws_json, max_ws, per_monitor)
        }
        CompositorBackend::Niri => {
            let ws_json = niri_request(b"\"Workspaces\"\n").unwrap_or_default();
            parse_niri_workspaces(&ws_json, max_ws, per_monitor)
        }
    }
}

fn sway_request(msg_type: u32, payload: &[u8]) -> Option<String> {
    let path = env::var_os("SWAYSOCK")?;
    let mut stream = UnixStream::connect(path).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));

    let mut hdr = [0u8; 14];
    hdr[0..6].copy_from_slice(b"i3-ipc");
    hdr[6..10].copy_from_slice(&(payload.len() as u32).to_ne_bytes());
    hdr[10..14].copy_from_slice(&msg_type.to_ne_bytes());
    stream.write_all(&hdr).ok()?;
    if !payload.is_empty() {
        stream.write_all(payload).ok()?;
    }

    let mut resp_hdr = [0u8; 14];
    stream.read_exact(&mut resp_hdr).ok()?;
    let resp_len = u32::from_ne_bytes(resp_hdr[6..10].try_into().ok()?) as usize;
    let mut buf = vec![0u8; resp_len];
    stream.read_exact(&mut buf).ok()?;
    String::from_utf8(buf).ok()
}

fn niri_request(cmd: &[u8]) -> Option<String> {
    let path = env::var_os("NIRI_SOCKET")?;
    let mut stream = UnixStream::connect(path).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
    stream.write_all(cmd).ok()?;

    let mut out = Vec::with_capacity(2048);
    let mut byte = [0u8; 1];
    while stream.read(&mut byte).ok()? == 1 {
        if byte[0] == b'\n' {
            break;
        }
        out.push(byte[0]);
    }
    String::from_utf8(out).ok()
}

pub fn parse_sway_workspaces(js: &str, max_ws: usize, per_monitor: bool) -> WorkspaceState {
    let objs = split_top_level_objects(js);
    let mut active_ws = 0usize;
    let mut focused_output = String::new();

    for &obj in &objs {
        if json_key_bool(obj, "focused").unwrap_or(false) {
            if let Some(num) = json_key_int(obj, "num") {
                if num >= 1 {
                    active_ws = num as usize;
                }
            }
            focused_output = json_key_string(obj, "output").unwrap_or_default();
            break;
        }
    }

    let mut occupied = vec![false; max_ws + 1];
    let mut urgent = vec![false; max_ws + 1];
    let mut occ_max = 0usize;

    for obj in objs {
        let Some(num) = json_key_int(obj, "num") else {
            continue;
        };
        if num < 1 || (num as usize) > max_ws {
            continue;
        };
        if per_monitor && !focused_output.is_empty() {
            if let Some(out) = json_key_string(obj, "output") {
                if out != focused_output {
                    continue;
                }
            }
        }
        let idx = num as usize;
        occupied[idx] = true;
        if json_key_bool(obj, "urgent").unwrap_or(false) && idx != active_ws {
            urgent[idx] = true;
        }
        if idx > occ_max {
            occ_max = idx;
        }
    }

    WorkspaceState {
        active_ws,
        occupied,
        urgent,
        occ_max,
        target_monitor: if focused_output.is_empty() {
            None
        } else {
            Some(MonitorTarget {
                name: focused_output,
                focused: true,
                ..Default::default()
            })
        },
    }
}

pub fn parse_niri_workspaces(js: &str, max_ws: usize, per_monitor: bool) -> WorkspaceState {
    // Extract the inner array `[...]` inside `{"Ok":{"Workspaces":[...]}}`
    let array_slice = js
        .find('[')
        .and_then(|s| js.rfind(']').map(|e| &js[s..=e]))
        .unwrap_or(js);
    let objs = split_top_level_objects(array_slice);

    let mut active_ws = 1usize;
    let mut focused_output = String::new();

    for &obj in &objs {
        if json_key_bool(obj, "is_focused").unwrap_or(false) {
            if let Some(idx) = json_key_int(obj, "idx") {
                if idx >= 1 {
                    active_ws = idx as usize;
                }
            }
            focused_output = json_key_string(obj, "output").unwrap_or_default();
            break;
        }
    }

    let mut occupied = vec![false; max_ws + 1];
    let mut urgent = vec![false; max_ws + 1];
    let mut occ_max = 0usize;

    for obj in objs {
        let Some(idx_i32) = json_key_int(obj, "idx") else {
            continue;
        };
        if idx_i32 < 1 || (idx_i32 as usize) > max_ws {
            continue;
        }
        if per_monitor && !focused_output.is_empty() {
            if let Some(out) = json_key_string(obj, "output") {
                if out != focused_output {
                    continue;
                }
            }
        }
        let idx = idx_i32 as usize;
        // Niri workspace is occupied if active_window_id is a number (not null)
        let has_win = json_key_int(obj, "active_window_id").is_some();
        if has_win {
            occupied[idx] = true;
            if idx > occ_max {
                occ_max = idx;
            }
        }
        if json_key_bool(obj, "is_urgent").unwrap_or(false) && idx != active_ws {
            urgent[idx] = true;
            if idx > occ_max {
                occ_max = idx;
            }
        }
    }

    WorkspaceState {
        active_ws,
        occupied,
        urgent,
        occ_max,
        target_monitor: if focused_output.is_empty() {
            None
        } else {
            Some(MonitorTarget {
                name: focused_output,
                focused: true,
                ..Default::default()
            })
        },
    }
}

#[cfg(test)]
pub fn parse_workspace_state(
    active_json: &str,
    workspaces_json: &str,
    monitors_json: &str,
    max_ws: usize,
) -> WorkspaceState {
    parse_workspace_state_filtered(active_json, workspaces_json, monitors_json, max_ws, false)
}

pub fn parse_workspace_state_filtered(
    active_json: &str,
    workspaces_json: &str,
    monitors_json: &str,
    max_ws: usize,
    per_monitor: bool,
) -> WorkspaceState {
    let raw_active_id = json_key_int(active_json, "id").unwrap_or(1);
    let active_ws = if raw_active_id < 1 {
        0 // special workspace (e.g. scratchpad -99)
    } else {
        raw_active_id as usize
    };

    let active_mon_name = json_key_string(active_json, "monitor");
    let monitors = parse_monitors(monitors_json);
    let target_monitor = active_mon_name
        .as_deref()
        .and_then(|name| monitors.iter().find(|m| m.name == name).cloned())
        .or_else(|| monitors.into_iter().find(|m| m.focused));

    let filter_mon_name = if per_monitor {
        target_monitor
            .as_ref()
            .map(|m| m.name.as_str())
            .or(active_mon_name.as_deref())
    } else {
        None
    };

    let mut occupied = vec![false; max_ws + 1];
    let mut occ_max = 0usize;

    for obj in split_top_level_objects(workspaces_json) {
        let Some(id) = json_key_int(obj, "id") else {
            continue;
        };
        if id < 1 || (id as usize) > max_ws {
            continue;
        }
        if let Some(req_mon) = filter_mon_name {
            if !req_mon.is_empty() {
                if let Some(ws_mon) = json_key_string(obj, "monitor") {
                    if ws_mon != req_mon {
                        continue;
                    }
                }
            }
        }
        // Only mark workspace as occupied if it actually holds windows (or windows key is absent).
        // Empty persistent workspaces ("windows": 0) should render as empty slots unless active.
        let windows = json_key_int(obj, "windows").unwrap_or(1);
        if windows > 0 {
            let idx = id as usize;
            occupied[idx] = true;
            if idx > occ_max {
                occ_max = idx;
            }
        }
    }

    WorkspaceState {
        active_ws,
        occupied,
        urgent: Vec::new(),
        occ_max,
        target_monitor,
    }
}

pub fn parse_monitors(js: &str) -> Vec<MonitorTarget> {
    let mut out = Vec::new();
    for obj in split_top_level_objects(js) {
        // Strip nested objects ("activeWorkspace": {...}, "specialWorkspace": {...})
        // so top-level "name" and "id" never collide with nested workspace fields.
        let top = strip_nested_objects(obj);
        let Some(x) = json_key_int(&top, "x") else {
            continue;
        };
        let Some(y) = json_key_int(&top, "y") else {
            continue;
        };
        let Some(width) = json_key_int(&top, "width") else {
            continue;
        };
        let Some(height) = json_key_int(&top, "height") else {
            continue;
        };

        out.push(MonitorTarget {
            name: json_key_string(&top, "name").unwrap_or_default(),
            x,
            y,
            width,
            height,
            make: json_key_string(&top, "make").unwrap_or_default(),
            model: json_key_string(&top, "model").unwrap_or_default(),
            focused: json_key_bool(&top, "focused").unwrap_or(false),
        });
    }
    out
}

/// Returns true if a Hyprland socket2 event line should trigger the indicator.
pub fn is_relevant_ipc_event(line: &str) -> bool {
    let Some((event, _payload)) = line.split_once(">>") else {
        return false;
    };
    matches!(
        event,
        "workspace"
            | "workspacev2"
            | "focusedmon"
            | "focusedmonv2"
            | "createworkspace"
            | "createworkspacev2"
            | "destroyworkspace"
            | "destroyworkspacev2"
            | "moveworkspace"
            | "moveworkspacev2"
            | "urgent"
    )
}

pub fn parse_urgent_address(line: &str) -> Option<&str> {
    let addr = line.strip_prefix("urgent>>")?.trim();
    if addr.is_empty() {
        None
    } else {
        Some(addr.trim_start_matches("0x"))
    }
}

/// Resolve the workspace ID (1..=max_ws) that owns the window with hex address `raw_addr`.
pub fn resolve_urgent_workspace(raw_addr: &str, max_ws: usize) -> Option<usize> {
    let clients_json = hypr_request(b"j/clients")?;
    resolve_urgent_workspace_from_clients(&clients_json, raw_addr, max_ws)
}

pub fn resolve_urgent_workspace_from_clients(
    clients_json: &str,
    raw_addr: &str,
    max_ws: usize,
) -> Option<usize> {
    let needle = raw_addr.trim().trim_start_matches("0x");
    if needle.is_empty() {
        return None;
    }

    for obj in split_top_level_objects(clients_json) {
        let Some(addr) = json_key_string(obj, "address") else {
            continue;
        };
        if addr.trim_start_matches("0x").eq_ignore_ascii_case(needle) {
            // Find nested `"workspace": { "id": N }`
            if let Some(ws_pos) = obj.find("\"workspace\"") {
                if let Some(id) = json_key_int(&obj[ws_pos..], "id") {
                    if id >= 1 && (id as usize) <= max_ws {
                        return Some(id as usize);
                    }
                }
            }
        }
    }
    None
}

#[derive(Default, Debug)]
pub struct Socket2Events {
    pub matched: bool,
    pub urgent_addresses: Vec<String>,
    pub closed: bool,
}

/// Non-blocking compositor event stream reader (Hyprland `.socket2.sock`, Sway `SUBSCRIBE`, Niri `EventStream`).
pub struct Socket2Stream {
    backend: CompositorBackend,
    stream: UnixStream,
    buf: Vec<u8>,
}

impl Socket2Stream {
    pub fn connect_nonblocking() -> Option<Self> {
        let backend = detect_backend();
        match backend {
            CompositorBackend::Hyprland => {
                let path = find_hypr_socket(".socket2.sock")?;
                let stream = UnixStream::connect(path).ok()?;
                stream.set_nonblocking(true).ok()?;
                Some(Self {
                    backend,
                    stream,
                    buf: Vec::with_capacity(4096),
                })
            }
            CompositorBackend::Sway => {
                let path = env::var_os("SWAYSOCK")?;
                let mut stream = UnixStream::connect(path).ok()?;
                let payload = b"[\"workspace\"]";
                let mut hdr = [0u8; 14];
                hdr[0..6].copy_from_slice(b"i3-ipc");
                hdr[6..10].copy_from_slice(&(payload.len() as u32).to_ne_bytes());
                hdr[10..14].copy_from_slice(&2u32.to_ne_bytes()); // SUBSCRIBE = 2
                stream.write_all(&hdr).ok()?;
                stream.write_all(payload).ok()?;
                stream.set_nonblocking(true).ok()?;
                Some(Self {
                    backend,
                    stream,
                    buf: Vec::with_capacity(4096),
                })
            }
            CompositorBackend::Niri => {
                let path = env::var_os("NIRI_SOCKET")?;
                let mut stream = UnixStream::connect(path).ok()?;
                stream.write_all(b"\"EventStream\"\n").ok()?;
                stream.set_nonblocking(true).ok()?;
                Some(Self {
                    backend,
                    stream,
                    buf: Vec::with_capacity(4096),
                })
            }
        }
    }

    pub fn fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    /// Drains available bytes; returns parsed compositor notification batch.
    pub fn drain_events(&mut self) -> Socket2Events {
        let mut events = Socket2Events::default();
        let mut tmp = [0u8; 4096];

        loop {
            match self.stream.read(&mut tmp) {
                Ok(0) => {
                    events.closed = true;
                    break;
                }
                Ok(n) => {
                    self.buf.extend_from_slice(&tmp[..n]);
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => {
                    events.closed = true;
                    break;
                }
            }
        }

        match self.backend {
            CompositorBackend::Hyprland => {
                while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
                    if let Ok(line) = std::str::from_utf8(&self.buf[..pos]) {
                        let trimmed = line.trim_end();
                        if is_relevant_ipc_event(trimmed) {
                            events.matched = true;
                            if let Some(addr) = parse_urgent_address(trimmed) {
                                events.urgent_addresses.push(addr.to_string());
                            }
                        }
                    }
                    self.buf.drain(..=pos);
                }
            }
            CompositorBackend::Sway => {
                while self.buf.len() >= 14 {
                    if &self.buf[0..6] != b"i3-ipc" {
                        self.buf.clear();
                        break;
                    }
                    let plen = u32::from_ne_bytes(self.buf[6..10].try_into().unwrap()) as usize;
                    let mtype = u32::from_ne_bytes(self.buf[10..14].try_into().unwrap());
                    if self.buf.len() < 14 + plen {
                        break;
                    }
                    if (mtype & 0x8000_0000) != 0 {
                        events.matched = true;
                    }
                    self.buf.drain(..14 + plen);
                }
            }
            CompositorBackend::Niri => {
                while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
                    if let Ok(line) = std::str::from_utf8(&self.buf[..pos]) {
                        if line.contains("Workspace") {
                            events.matched = true;
                        }
                    }
                    self.buf.drain(..=pos);
                }
            }
        }

        if self.buf.len() > 16384 {
            self.buf.clear();
        }

        events
    }
}

/// Split a JSON array/stream into top-level `{ ... }` object slices, respecting string literals.
pub fn split_top_level_objects(js: &str) -> Vec<&str> {
    let bytes = js.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut obj_start = 0usize;
    let mut in_str = false;
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        if in_str {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }

        match b {
            b'"' => in_str = true,
            b'{' => {
                if depth == 0 {
                    obj_start = i;
                }
                depth += 1;
            }
            b'}' => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        out.push(&js[obj_start..=i]);
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }

    out
}

/// Replaces nested `{ ... }` blocks at depth >= 2 with `{}` while preserving UTF-8 and strings.
fn strip_nested_objects(obj: &str) -> String {
    let mut out = String::with_capacity(obj.len());
    let mut depth = 0i32;
    let mut in_str = false;
    let mut chars = obj.chars();

    while let Some(ch) = chars.next() {
        if in_str {
            if ch == '\\' {
                if let Some(escaped) = chars.next() {
                    if depth <= 1 {
                        out.push(ch);
                        out.push(escaped);
                    }
                }
                continue;
            }
            if ch == '"' {
                in_str = false;
            }
            if depth <= 1 {
                out.push(ch);
            }
            continue;
        }

        match ch {
            '"' => {
                in_str = true;
                if depth <= 1 {
                    out.push('"');
                }
            }
            '{' => {
                depth += 1;
                if depth <= 2 {
                    out.push('{');
                }
            }
            '}' => {
                if depth <= 2 {
                    out.push('}');
                }
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ => {
                if depth <= 1 {
                    out.push(ch);
                }
            }
        }
    }
    out
}

/// String-aware JSON key value locator: finds `"<key>":` outside of JSON string values.
fn find_json_value<'a>(js: &'a str, key: &str) -> Option<&'a str> {
    let bytes = js.as_bytes();
    let mut in_str = false;
    let mut str_start = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        if in_str {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_str = false;
                let token = &js[str_start..i];
                if token == key {
                    let mut j = i + 1;
                    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if j < bytes.len() && bytes[j] == b':' {
                        j += 1;
                        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                            j += 1;
                        }
                        return Some(&js[j..]);
                    }
                }
            }
            i += 1;
            continue;
        }

        if b == b'"' {
            in_str = true;
            str_start = i + 1;
        }
        i += 1;
    }
    None
}

pub fn json_key_int(js: &str, key: &str) -> Option<i32> {
    let rest = find_json_value(js, key)?;
    let end = rest
        .bytes()
        .position(|b| !(b.is_ascii_digit() || b == b'-'))
        .unwrap_or(rest.len());
    rest[..end].parse::<i32>().ok()
}

pub fn json_key_bool(js: &str, key: &str) -> Option<bool> {
    let rest = find_json_value(js, key)?;
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

pub fn json_key_string(js: &str, key: &str) -> Option<String> {
    let rest = find_json_value(js, key)?;
    let rest = rest.strip_prefix('"')?;
    let bytes = rest.as_bytes();
    let mut out = String::new();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            out.push(bytes[i + 1] as char);
            i += 2;
            continue;
        }
        if bytes[i] == b'"' {
            return Some(out);
        }
        if let Some(ch) = rest[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_parser_ignores_braces_and_keys_inside_window_titles() {
        let active = r#"{
            "id": 3,
            "name": "3",
            "monitor": "DP-2",
            "lastwindowtitle": "vim { \"id\": 9 } }"
        }"#;
        let workspaces = r#"[{
            "id": 1,
            "windows": 2,
            "lastwindowtitle": "fake \"id\": 8 and {braces}"
        },{
            "id": 3,
            "windows": 1,
            "lastwindowtitle": "active"
        },{
            "id": 7,
            "windows": 0,
            "lastwindowtitle": ""
        }]"#;
        let monitors = r#"[{
            "id": 0,
            "name": "eDP-1",
            "make": "",
            "model": "",
            "x": 0,
            "y": 0,
            "width": 1920,
            "height": 1080,
            "activeWorkspace": { "id": 1, "name": "1" },
            "focused": false
        },{
            "id": 1,
            "name": "DP-2",
            "make": "Dell Inc.®",
            "model": "U2723QE™",
            "x": 1920,
            "y": 0,
            "width": 3840,
            "height": 2160,
            "activeWorkspace": { "id": 3, "name": "3" },
            "focused": true
        }]"#;

        let state = parse_workspace_state(active, workspaces, monitors, 10);
        assert_eq!(state.active_ws, 3);
        assert!(state.occupied[1]);
        assert!(state.occupied[3]);
        assert!(!state.occupied[7]); // empty persistent workspace ("windows": 0) is not occupied
        assert!(!state.occupied[8]); // fake "id": 8 in window title ignored
        assert_eq!(state.occ_max, 3);
        let mon = state.target_monitor.unwrap();
        assert_eq!(mon.name, "DP-2");
        assert_eq!(mon.make, "Dell Inc.®");
        assert_eq!(mon.model, "U2723QE™");
        assert_eq!(mon.x, 1920);
        assert_eq!(mon.width, 3840);

        // Workspace > max_workspaces must not clamp active_ws to max_workspaces
        let high_active = r#"{ "id": 12, "name": "12", "monitor": "DP-2" }"#;
        let high_state = parse_workspace_state(high_active, workspaces, monitors, 10);
        assert_eq!(high_state.active_ws, 12);

        // per_monitor filtering excludes workspaces belonging to other monitors
        let multi_ws = r#"[{
            "id": 1,
            "monitor": "eDP-1",
            "windows": 2
        },{
            "id": 3,
            "monitor": "DP-2",
            "windows": 1
        }]"#;
        let filtered = parse_workspace_state_filtered(active, multi_ws, monitors, 10, true);
        assert!(!filtered.occupied[1]);
        assert!(filtered.occupied[3]);
    }

    #[test]
    fn matches_v1_and_v2_ipc_events() {
        assert!(is_relevant_ipc_event("workspace>>2"));
        assert!(is_relevant_ipc_event("workspacev2>>2,2"));
        assert!(is_relevant_ipc_event("focusedmon>>DP-1,2"));
        assert!(is_relevant_ipc_event("focusedmonv2>>DP-1,2"));
        assert!(is_relevant_ipc_event("destroyworkspacev2>>8,8"));
        assert!(is_relevant_ipc_event("urgent>>559c47964e20"));
        assert!(!is_relevant_ipc_event("activewindow>>kitty,zsh"));

        let clients = r#"[{
            "address": "0x559c47964e20",
            "workspace": { "id": 4, "name": "4" },
            "title": "Alert"
        }]"#;
        assert_eq!(
            resolve_urgent_workspace_from_clients(clients, "559c47964e20", 10),
            Some(4)
        );
    }

    #[test]
    fn parses_sway_and_niri_workspaces() {
        let sway_js = r#"[{
            "num": 1,
            "name": "1",
            "focused": false,
            "urgent": true,
            "output": "eDP-1"
        },{
            "num": 3,
            "name": "3",
            "focused": true,
            "urgent": false,
            "output": "eDP-1"
        }]"#;
        let sw = parse_sway_workspaces(sway_js, 10, true);
        assert_eq!(sw.active_ws, 3);
        assert!(sw.occupied[1]);
        assert!(sw.urgent[1]);
        assert!(sw.occupied[3]);
        assert_eq!(sw.target_monitor.unwrap().name, "eDP-1");

        let niri_js = r#"{"Ok":{"Workspaces":[{
            "id": 10,
            "idx": 1,
            "output": "DP-1",
            "is_urgent": false,
            "is_focused": false,
            "active_window_id": 42
        },{
            "id": 11,
            "idx": 2,
            "output": "DP-1",
            "is_urgent": false,
            "is_focused": true,
            "active_window_id": null
        },{
            "id": 12,
            "idx": 4,
            "output": "DP-1",
            "is_urgent": true,
            "is_focused": false,
            "active_window_id": 99
        }]}}"#;
        let nw = parse_niri_workspaces(niri_js, 10, true);
        assert_eq!(nw.active_ws, 2);
        assert!(nw.occupied[1]);
        assert!(!nw.occupied[2]);
        assert!(nw.occupied[4]);
        assert!(nw.urgent[4]);
        assert_eq!(nw.target_monitor.unwrap().name, "DP-1");
    }
}
