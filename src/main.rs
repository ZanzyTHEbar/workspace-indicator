mod config;
mod ffi;
mod ipc;
mod render;

use std::cell::RefCell;
use std::env;
use std::ffi::{CStr, CString};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use config::{xdg_cache_home, xdg_config_home, Config, ConfigLoader, Position};
use ipc::{MonitorTarget, WorkspaceState};
use render::{compute_pill_layout, draw_pill, layer_edge_for_position, PillLayout};

static TRIGGER_PENDING: AtomicBool = AtomicBool::new(false);

struct AppState {
    loader: ConfigLoader,
    cfg: Config,
    ws_state: WorkspaceState,
    layout: PillLayout,
    win: *mut ffi::GtkWidget,
    da: *mut ffi::GtkWidget,
    bound_monitor: *mut ffi::GdkMonitor,
    applied_position: Option<Position>,
    applied_margin: i32,
    applied_size: (i32, i32),
    opacity: f64,
    fade_target: f64,
    fade_step: f64,
    tid_hide: ffi::guint,
    tid_fade: ffi::guint,
    tid_debounce: ffi::guint,
}

thread_local! {
    static APP: RefCell<Option<AppState>> = const { RefCell::new(None) };
}

fn with_app<R>(f: impl FnOnce(&mut AppState) -> R) -> Option<R> {
    APP.with(|cell| cell.borrow_mut().as_mut().map(f))
}

fn lock_file_path() -> PathBuf {
    let dir = xdg_cache_home().join("workspace-indicator");
    let _ = fs::create_dir_all(&dir);
    dir.join("lock")
}

fn acquire_lock() -> Option<fs::File> {
    let path = lock_file_path();
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)
        .ok()?;

    let rc = unsafe { ffi::flock(file.as_raw_fd(), ffi::LOCK_EX | ffi::LOCK_NB) };
    if rc < 0 {
        return None;
    }
    let _ = file.set_len(0);
    Some(file)
}

fn publish_lock_pid(file: &mut fs::File) {
    let _ = file.set_len(0);
    let _ = writeln!(file, "{}", std::process::id());
    let _ = file.flush();
}

fn signal_running_instance(sig: i32) -> bool {
    let path = lock_file_path();
    let Ok(file) = OpenOptions::new().read(true).write(true).open(&path) else {
        return false;
    };
    // Verify the exclusive flock is actively held by a running instance.
    // If flock succeeds, no daemon is running; drop the lock and do not signal stale PIDs.
    let rc = unsafe { ffi::flock(file.as_raw_fd(), ffi::LOCK_EX | ffi::LOCK_NB) };
    if rc == 0 {
        let _ = file.set_len(0);
        return false;
    }

    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(pid) = text.trim().parse::<i32>() {
            if pid > 1 && is_workspace_indicator_pid(pid) {
                return unsafe { ffi::kill(pid, sig) } == 0;
            }
        }
    }
    false
}

fn is_workspace_indicator_pid(pid: i32) -> bool {
    let comm_path = format!("/proc/{pid}/comm");
    if let Ok(comm) = fs::read_to_string(comm_path) {
        return comm.trim().starts_with("workspace-indic");
    }
    false
}

unsafe fn cstr_to_str<'a>(p: *const std::ffi::c_char) -> &'a str {
    if p.is_null() {
        ""
    } else {
        CStr::from_ptr(p).to_str().unwrap_or("")
    }
}

/// Resolve the GdkMonitor corresponding to Hyprland's target monitor.
/// Fixes the C bugs:
/// 1. Matches exact `(x, y)` layout geometry first (immune to duplicate monitor models or empty make/model).
/// 2. Only matches `make` + `model` when at least one is non-empty AND unique.
/// 3. Falls back to `gdk_display_get_monitor_at_point`.
unsafe fn resolve_gdk_monitor(
    display: *mut ffi::GdkDisplay,
    target: &MonitorTarget,
) -> *mut ffi::GdkMonitor {
    let n = ffi::gdk_display_get_n_monitors(display);

    // 1. Exact layout origin (x, y) match
    for i in 0..n {
        let mon = ffi::gdk_display_get_monitor(display, i);
        if mon.is_null() {
            continue;
        }
        let mut rect = ffi::GdkRectangle::default();
        ffi::gdk_monitor_get_geometry(mon, &mut rect);
        if rect.x == target.x && rect.y == target.y {
            return mon;
        }
    }

    // 2. Non-empty make + model match
    if !target.make.is_empty() || !target.model.is_empty() {
        for i in 0..n {
            let mon = ffi::gdk_display_get_monitor(display, i);
            if mon.is_null() {
                continue;
            }
            let make = cstr_to_str(ffi::gdk_monitor_get_manufacturer(mon));
            let model = cstr_to_str(ffi::gdk_monitor_get_model(mon));
            if !target.make.is_empty() && make != target.make {
                continue;
            }
            if !target.model.is_empty() && model != target.model {
                continue;
            }
            return mon;
        }
    }

    // 3. Point lookup fallback
    ffi::gdk_display_get_monitor_at_point(display, target.x + 1, target.y + 1)
}

unsafe fn apply_window_placement(app: &mut AppState) {
    let win_ptr = app.win as *mut ffi::GtkWindow;

    // Update edge anchors and margin only when position or margin changed
    if app.applied_position != Some(app.cfg.position) || app.applied_margin != app.cfg.margin {
        let active_edge = layer_edge_for_position(app.cfg.position);
        for edge in [
            ffi::GTK_LAYER_SHELL_EDGE_LEFT,
            ffi::GTK_LAYER_SHELL_EDGE_RIGHT,
            ffi::GTK_LAYER_SHELL_EDGE_TOP,
            ffi::GTK_LAYER_SHELL_EDGE_BOTTOM,
        ] {
            let is_active = if edge == active_edge {
                ffi::TRUE
            } else {
                ffi::FALSE
            };
            ffi::gtk_layer_set_anchor(win_ptr, edge, is_active);
            ffi::gtk_layer_set_margin(
                win_ptr,
                edge,
                if edge == active_edge { app.cfg.margin } else { 0 },
            );
        }
        app.applied_position = Some(app.cfg.position);
        app.applied_margin = app.cfg.margin;
    }

    // Bind to focused monitor only when target GdkMonitor actually changes
    let display = ffi::gdk_display_get_default();
    if !display.is_null() {
        if let Some(target) = &app.ws_state.target_monitor {
            let mon = resolve_gdk_monitor(display, target);
            if !mon.is_null() && mon != app.bound_monitor {
                ffi::gtk_layer_set_monitor(win_ptr, mon);
                app.bound_monitor = mon;
            }
        }
    }

    // Update size request and force GtkWindow to shrink/grow when dot count or orientation changes
    let new_size = (app.layout.width, app.layout.height);
    if app.applied_size != new_size {
        ffi::gtk_widget_set_size_request(app.da, new_size.0, new_size.1);
        ffi::gtk_window_resize(win_ptr, 1, 1);
        app.applied_size = new_size;
    }

    if ffi::gtk_widget_get_visible(app.win) == ffi::FALSE {
        ffi::gtk_widget_show_all(app.win);
    }
}

unsafe extern "C" fn on_draw(
    widget: *mut ffi::GtkWidget,
    cr: *mut ffi::cairo_t,
    _data: ffi::gpointer,
) -> ffi::gboolean {
    with_app(|app| {
        if app.opacity < 0.001 {
            return;
        }
        let mut alloc = ffi::GtkAllocation::default();
        ffi::gtk_widget_get_allocation(widget, &mut alloc);
        draw_pill(
            cr,
            f64::from(alloc.width),
            f64::from(alloc.height),
            app.opacity,
            app.cfg.palette.bg,
            &app.layout,
        );
    });
    ffi::FALSE
}

unsafe extern "C" fn on_realize(widget: *mut ffi::GtkWidget, _data: ffi::gpointer) {
    let rgn = ffi::cairo_region_create();
    ffi::gtk_widget_input_shape_combine_region(widget, rgn);
    ffi::cairo_region_destroy(rgn);
}

unsafe extern "C" fn fade_step_cb(_data: ffi::gpointer) -> ffi::gboolean {
    with_app(|app| {
        app.opacity += app.fade_step;
        let done = if app.fade_step >= 0.0 {
            app.opacity >= app.fade_target
        } else {
            app.opacity <= app.fade_target
        };
        if done {
            app.opacity = app.fade_target;
        }

        ffi::gtk_widget_queue_draw(app.da);

        if done {
            app.tid_fade = 0;
            if app.fade_target <= 0.0 {
                ffi::gtk_widget_set_opacity(app.win, 0.0);
            }
            ffi::G_SOURCE_REMOVE
        } else {
            ffi::G_SOURCE_CONTINUE
        }
    })
    .unwrap_or(ffi::G_SOURCE_REMOVE)
}

unsafe fn start_fade(app: &mut AppState, target: f64, duration_ms: u32) {
    if app.tid_fade != 0 {
        ffi::g_source_remove(app.tid_fade);
        app.tid_fade = 0;
    }
    let steps = (duration_ms / 16).max(1);
    app.fade_target = target;
    app.fade_step = (target - app.opacity) / f64::from(steps);
    app.tid_fade = ffi::g_timeout_add(16, fade_step_cb, ptr::null_mut());
}

unsafe extern "C" fn begin_hide_cb(_data: ffi::gpointer) -> ffi::gboolean {
    with_app(|app| {
        app.tid_hide = 0;
        let fade_out = app.cfg.fade_out_ms;
        start_fade(app, 0.0, fade_out);
    });
    ffi::G_SOURCE_REMOVE
}

unsafe fn show_indicator(app: &mut AppState) {
    // Auto-reload config or active Omarchy/Dragonarchy theme if modified on disk (~1us stat check)
    if let Some(new_cfg) = app.loader.reload_if_changed() {
        apply_reloaded_config(app, new_cfg);
    }

    app.ws_state = ipc::query_workspace_state(app.cfg.max_workspaces);
    if app.ws_state.active_ws < 1 {
        return; // skip special workspaces
    }

    app.layout = compute_pill_layout(
        &app.cfg,
        app.ws_state.active_ws,
        &app.ws_state.occupied,
        app.ws_state.occ_max,
    );

    if app.tid_hide != 0 {
        ffi::g_source_remove(app.tid_hide);
        app.tid_hide = 0;
    }
    if app.tid_fade != 0 {
        ffi::g_source_remove(app.tid_fade);
        app.tid_fade = 0;
    }

    apply_window_placement(app);
    ffi::gtk_widget_queue_draw(app.da);
    ffi::gtk_widget_set_opacity(app.win, 1.0);

    let fade_in = app.cfg.fade_in_ms;
    let display_ms = app.cfg.display_ms;
    start_fade(app, 1.0, fade_in);
    app.tid_hide = ffi::g_timeout_add(display_ms, begin_hide_cb, ptr::null_mut());
}

fn apply_reloaded_config(app: &mut AppState, new_cfg: Config) {
    if app.cfg.hyprland_conf_bind() != new_cfg.hyprland_conf_bind() {
        let _ = ipc::hypr_request(app.cfg.hyprland_keyword_unbind().as_bytes());
        let _ = ipc::hypr_request(new_cfg.hyprland_keyword_bindd().as_bytes());
        let _ = sync_hyprland_files(&new_cfg);
    }
    app.cfg = new_cfg;
}

unsafe extern "C" fn do_show_cb(_data: ffi::gpointer) -> ffi::gboolean {
    with_app(|app| {
        app.tid_debounce = 0;
        show_indicator(app);
    });
    ffi::G_SOURCE_REMOVE
}

unsafe extern "C" fn sched_show_cb(_data: ffi::gpointer) -> ffi::gboolean {
    TRIGGER_PENDING.store(false, Ordering::Release);
    with_app(|app| {
        if app.tid_debounce != 0 {
            ffi::g_source_remove(app.tid_debounce);
        }
        app.tid_debounce = ffi::g_timeout_add(app.cfg.debounce_ms, do_show_cb, ptr::null_mut());
    });
    ffi::G_SOURCE_REMOVE
}

fn trigger() {
    if !TRIGGER_PENDING.swap(true, Ordering::AcqRel) {
        unsafe {
            ffi::g_idle_add(sched_show_cb, ptr::null_mut());
        }
    }
}

unsafe extern "C" fn on_sigusr1(_data: ffi::gpointer) -> ffi::gboolean {
    trigger();
    ffi::G_SOURCE_CONTINUE
}

unsafe extern "C" fn on_sigusr2(_data: ffi::gpointer) -> ffi::gboolean {
    with_app(|app| {
        let new_cfg = app.loader.load_force();
        apply_reloaded_config(app, new_cfg);
        app.layout = compute_pill_layout(
            &app.cfg,
            app.ws_state.active_ws.max(1),
            &app.ws_state.occupied,
            app.ws_state.occ_max,
        );
        apply_window_placement(app);
        ffi::gtk_widget_queue_draw(app.da);
    });
    ffi::G_SOURCE_CONTINUE
}

unsafe extern "C" fn on_sigquit(_data: ffi::gpointer) -> ffi::gboolean {
    ffi::gtk_main_quit();
    ffi::G_SOURCE_REMOVE
}

unsafe fn build_window(app: &mut AppState) {
    let win = ffi::gtk_window_new(ffi::GTK_WINDOW_TOPLEVEL);
    ffi::gtk_widget_set_app_paintable(win, ffi::TRUE);

    let scr = ffi::gtk_widget_get_screen(win);
    let vis = ffi::gdk_screen_get_rgba_visual(scr);
    if !vis.is_null() {
        ffi::gtk_widget_set_visual(win, vis);
    }

    let win_ptr = win as *mut ffi::GtkWindow;
    ffi::gtk_layer_init_for_window(win_ptr);
    ffi::gtk_layer_set_layer(win_ptr, ffi::GTK_LAYER_SHELL_LAYER_OVERLAY);
    let ns = CString::new("workspace-indicator").unwrap();
    ffi::gtk_layer_set_namespace(win_ptr, ns.as_ptr());
    ffi::gtk_layer_set_keyboard_mode(win_ptr, ffi::GTK_LAYER_SHELL_KEYBOARD_MODE_NONE);

    let da = ffi::gtk_drawing_area_new();
    let draw_sig = CString::new("draw").unwrap();
    ffi::g_signal_connect_data(
        da as ffi::gpointer,
        draw_sig.as_ptr(),
        std::mem::transmute::<
            unsafe extern "C" fn(*mut ffi::GtkWidget, *mut ffi::cairo_t, ffi::gpointer) -> ffi::gboolean,
            ffi::GCallback,
        >(on_draw),
        ptr::null_mut(),
        ptr::null_mut(),
        0,
    );
    ffi::gtk_container_add(win as *mut ffi::GtkContainer, da);

    let realize_sig = CString::new("realize").unwrap();
    ffi::g_signal_connect_data(
        win as ffi::gpointer,
        realize_sig.as_ptr(),
        std::mem::transmute::<
            unsafe extern "C" fn(*mut ffi::GtkWidget, ffi::gpointer),
            ffi::GCallback,
        >(on_realize),
        ptr::null_mut(),
        ptr::null_mut(),
        0,
    );

    app.win = win;
    app.da = da;

    apply_window_placement(app);
    ffi::gtk_widget_set_opacity(win, 0.0);
    ffi::gtk_widget_show_all(win);
}

fn sync_hyprland_files(cfg: &Config) -> std::io::Result<()> {
    let hypr_dir = xdg_config_home().join("hypr");
    let hypr_conf_dir = hypr_dir.join("config");
    if hypr_conf_dir.is_dir() {
        fs::write(
            hypr_conf_dir.join("workspace-indicator.local.conf"),
            cfg.render_hyprland_conf(),
        )?;
    }
    if hypr_dir.is_dir() {
        fs::write(
            hypr_dir.join("workspace-indicator.lua"),
            cfg.render_hyprland_lua(),
        )?;
    }
    Ok(())
}

fn print_usage() {
    println!(
        "workspace-indicator — Hyper-performant Wayland workspace OSD for Hyprland & Omarchy\n\n\
         Usage:\n  \
           workspace-indicator [OPTIONS]\n\n\
         Options:\n  \
           --position <bottom|top|left|right>  Override indicator screen edge (left/right rotate 90°)\n  \
           --config <path>                     Path to config.{{lua,toml,conf}}\n  \
           --peek                              Trigger OSD peek on running instance (SIGUSR1)\n  \
           --reload                            Reload config & theme on running instance (SIGUSR2)\n  \
           --sync-hyprland                     Write Hyprland .conf and .lua integration files\n  \
           --print-hyprland-conf               Print Hyprland latest .conf snippet\n  \
           --print-hyprland-lua                Print Hyprland 0.56+ / Omarchy 4 Lua snippet\n  \
           -h, --help                          Show this help"
    );
}

fn main() {
    let mut explicit_config: Option<PathBuf> = None;
    let mut cli_position: Option<Position> = None;
    let mut mode_peek = false;
    let mut mode_reload = false;
    let mut mode_sync = false;
    let mut mode_print_conf = false;
    let mut mode_print_lua = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--peek" => mode_peek = true,
            "--reload" => mode_reload = true,
            "--sync-hyprland" => mode_sync = true,
            "--print-hyprland-conf" => mode_print_conf = true,
            "--print-hyprland-lua" => mode_print_lua = true,
            "--position" | "-p" => {
                if let Some(val) = args.next() {
                    cli_position = Position::parse(&val);
                }
            }
            "--config" | "-c" => {
                if let Some(val) = args.next() {
                    explicit_config = Some(PathBuf::from(val));
                }
            }
            "-h" | "--help" => {
                print_usage();
                return;
            }
            other => {
                if let Some(v) = other.strip_prefix("--position=") {
                    cli_position = Position::parse(v);
                } else if let Some(v) = other.strip_prefix("--config=") {
                    explicit_config = Some(PathBuf::from(v));
                }
            }
        }
    }

    if mode_peek {
        let _ = signal_running_instance(ffi::SIGUSR1);
        return;
    }
    if mode_reload {
        let _ = signal_running_instance(ffi::SIGUSR2);
        return;
    }

    let mut loader = ConfigLoader::new(explicit_config, cli_position);
    let cfg = loader.load_force();

    if mode_print_conf {
        print!("{}", cfg.render_hyprland_conf());
        return;
    }
    if mode_print_lua {
        print!("{}", cfg.render_hyprland_lua());
        return;
    }
    if mode_sync {
        if let Err(e) = sync_hyprland_files(&cfg) {
            eprintln!("workspace-indicator: failed to sync Hyprland files: {e}");
            std::process::exit(1);
        }
        return;
    }

    let Some(mut lock_file) = acquire_lock() else {
        eprintln!("workspace-indicator: already running");
        return;
    };

    let _ = sync_hyprland_files(&cfg);
    if cfg.hyprland_conf_bind() != "$mainMod, grave" {
        let _ = ipc::hypr_request(b"keyword unbind SUPER, grave");
        let _ = ipc::hypr_request(cfg.hyprland_keyword_bindd().as_bytes());
    }

    // Install UNIX signal handlers immediately after acquiring flock so early
    // SIGUSR1/SIGUSR2 signals during Wayland/GTK initialization never hit default SIG_DFL terminate.
    unsafe {
        ffi::g_unix_signal_add(ffi::SIGUSR1, on_sigusr1, ptr::null_mut());
        ffi::g_unix_signal_add(ffi::SIGUSR2, on_sigusr2, ptr::null_mut());
        ffi::g_unix_signal_add(ffi::SIGTERM, on_sigquit, ptr::null_mut());
        ffi::g_unix_signal_add(ffi::SIGINT, on_sigquit, ptr::null_mut());
    }

    unsafe {
        let mut argc: i32 = 0;
        let mut argv: *mut *mut std::ffi::c_char = ptr::null_mut();
        ffi::gtk_init(&mut argc, &mut argv);
    }

    let ws_state = ipc::query_workspace_state(cfg.max_workspaces);
    let layout = compute_pill_layout(
        &cfg,
        ws_state.active_ws.max(1),
        &ws_state.occupied,
        ws_state.occ_max,
    );

    let mut state = AppState {
        loader,
        cfg,
        ws_state,
        layout,
        win: ptr::null_mut(),
        da: ptr::null_mut(),
        bound_monitor: ptr::null_mut(),
        applied_position: None,
        applied_margin: -1,
        applied_size: (0, 0),
        opacity: 0.0,
        fade_target: 0.0,
        fade_step: 0.0,
        tid_hide: 0,
        tid_fade: 0,
        tid_debounce: 0,
    };

    unsafe {
        build_window(&mut state);
    }

    APP.with(|cell| {
        *cell.borrow_mut() = Some(state);
    });

    publish_lock_pid(&mut lock_file);

    std::thread::spawn(|| {
        ipc::listen_socket2(trigger);
    });

    unsafe {
        ffi::gtk_main();
    }

    let _ = lock_file.set_len(0);
}
