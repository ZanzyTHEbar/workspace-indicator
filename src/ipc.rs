use std::env;
use std::io::{BufRead, BufReader, Read, Write};
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
    pub occ_max: usize,
    pub target_monitor: Option<MonitorTarget>,
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

pub fn query_workspace_state(max_ws: usize) -> WorkspaceState {
    let active_json = hypr_request(b"j/activeworkspace").unwrap_or_default();
    let workspaces_json = hypr_request(b"j/workspaces").unwrap_or_default();
    let monitors_json = hypr_request(b"j/monitors").unwrap_or_default();

    parse_workspace_state(&active_json, &workspaces_json, &monitors_json, max_ws)
}

pub fn parse_workspace_state(
    active_json: &str,
    workspaces_json: &str,
    monitors_json: &str,
    max_ws: usize,
) -> WorkspaceState {
    let raw_active_id = json_key_int(active_json, "id").unwrap_or(1);
    let active_ws = if raw_active_id < 1 {
        0 // special workspace (e.g. scratchpad -99)
    } else {
        (raw_active_id as usize).min(max_ws)
    };

    let active_mon_name = json_key_string(active_json, "monitor");

    let mut occupied = vec![false; max_ws + 1];
    let mut occ_max = 0usize;

    for obj in split_top_level_objects(workspaces_json) {
        let Some(id) = json_key_int(obj, "id") else {
            continue;
        };
        if id < 1 || (id as usize) > max_ws {
            continue;
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

    let monitors = parse_monitors(monitors_json);
    let target_monitor = active_mon_name
        .as_deref()
        .and_then(|name| monitors.iter().find(|m| m.name == name).cloned())
        .or_else(|| monitors.into_iter().find(|m| m.focused));

    WorkspaceState {
        active_ws,
        occupied,
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
    )
}

/// Blocking event loop for `.socket2.sock` with automatic reconnect.
pub fn listen_socket2<F: Fn()>(on_event: F) {
    loop {
        let Some(path) = find_hypr_socket(".socket2.sock") else {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };

        let Ok(stream) = UnixStream::connect(&path) else {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };

        let mut reader = BufReader::with_capacity(4096, stream);
        let mut line = String::with_capacity(256);

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if is_relevant_ipc_event(line.trim_end()) {
                        on_event();
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_secs(1));
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

/// Replaces nested `{ ... }` blocks at depth >= 2 with `{}` while preserving strings.
fn strip_nested_objects(obj: &str) -> String {
    let bytes = obj.as_bytes();
    let mut out = String::with_capacity(obj.len());
    let mut depth = 0i32;
    let mut in_str = false;
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        if in_str {
            if b == b'\\' && i + 1 < bytes.len() {
                if depth <= 1 {
                    out.push(b as char);
                    out.push(bytes[i + 1] as char);
                }
                i += 2;
                continue;
            }
            if b == b'"' {
                in_str = false;
            }
            if depth <= 1 {
                out.push(b as char);
            }
            i += 1;
            continue;
        }

        match b {
            b'"' => {
                in_str = true;
                if depth <= 1 {
                    out.push('"');
                }
            }
            b'{' => {
                depth += 1;
                if depth <= 2 {
                    out.push('{');
                }
            }
            b'}' => {
                if depth <= 2 {
                    out.push('}');
                }
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ => {
                if depth <= 1 {
                    out.push(b as char);
                }
            }
        }
        i += 1;
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
            "make": "Dell Inc.",
            "model": "U2723QE",
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
        assert_eq!(mon.x, 1920);
        assert_eq!(mon.width, 3840);
    }

    #[test]
    fn matches_v1_and_v2_ipc_events() {
        assert!(is_relevant_ipc_event("workspace>>2"));
        assert!(is_relevant_ipc_event("workspacev2>>2,2"));
        assert!(is_relevant_ipc_event("focusedmon>>DP-1,2"));
        assert!(is_relevant_ipc_event("focusedmonv2>>DP-1,2"));
        assert!(is_relevant_ipc_event("destroyworkspacev2>>8,8"));
        assert!(!is_relevant_ipc_event("activewindow>>kitty,zsh"));
    }
}
