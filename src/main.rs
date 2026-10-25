mod config;
mod ffi;
mod ipc;
mod render;
mod wayland;

use std::env;
use std::ffi::CString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::PathBuf;
use std::ptr;

use config::{xdg_cache_home, xdg_config_home, xdg_state_home, Config, ConfigLoader, Position};
use ipc::{Socket2Stream, WorkspaceState};
use render::{compute_pill_layout_morphed, rasterize_pill_argb8888_scaled, PillLayout};
use wayland::WaylandClient;

const TAG_WAYLAND: u64 = 1;
const TAG_SOCKET2: u64 = 2;
const TAG_SIGNAL: u64 = 3;
const TAG_INOTIFY: u64 = 4;
const TAG_DEBOUNCE: u64 = 5;
const TAG_FRAME: u64 = 6;
const TAG_HOLD: u64 = 7;

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

unsafe fn create_timerfd() -> RawFd {
    ffi::timerfd_create(
        ffi::CLOCK_MONOTONIC,
        ffi::TFD_NONBLOCK | ffi::TFD_CLOEXEC,
    )
}

unsafe fn arm_timer_ms(tfd: RawFd, initial_ms: u32, interval_ms: u32) {
    let to_ts = |ms: u32| ffi::timespec {
        tv_sec: i64::from(ms / 1000),
        tv_nsec: i64::from(ms % 1000) * 1_000_000,
    };
    let spec = ffi::itimerspec {
        it_value: to_ts(initial_ms),
        it_interval: to_ts(interval_ms),
    };
    ffi::timerfd_settime(tfd, 0, &spec, ptr::null_mut());
}

unsafe fn disarm_timer(tfd: RawFd) {
    let spec = ffi::itimerspec::default();
    ffi::timerfd_settime(tfd, 0, &spec, ptr::null_mut());
}

unsafe fn drain_timerfd(tfd: RawFd) {
    let mut val = 0u64;
    while ffi::read(
        tfd,
        &mut val as *mut u64 as *mut std::ffi::c_void,
        std::mem::size_of::<u64>(),
    ) > 0
    {}
}

unsafe fn create_signalfd() -> RawFd {
    let mut mask: ffi::sigset_t = std::mem::zeroed();
    ffi::sigemptyset(&mut mask);
    ffi::sigaddset(&mut mask, ffi::SIGUSR1);
    ffi::sigaddset(&mut mask, ffi::SIGUSR2);
    ffi::sigaddset(&mut mask, ffi::SIGTERM);
    ffi::sigaddset(&mut mask, ffi::SIGINT);
    ffi::sigprocmask(ffi::SIG_BLOCK, &mask, ptr::null_mut());
    ffi::signalfd(-1, &mask, ffi::SFD_NONBLOCK | ffi::SFD_CLOEXEC)
}

unsafe fn create_theme_inotify() -> RawFd {
    let ifd = ffi::inotify_init1(ffi::IN_NONBLOCK | ffi::IN_CLOEXEC);
    if ifd < 0 {
        return -1;
    }
    add_theme_watches(ifd);
    ifd
}

unsafe fn add_theme_watches(ifd: RawFd) {
    if ifd < 0 {
        return;
    }
    let xdg_cfg = xdg_config_home();
    let xdg_st = xdg_state_home();
    let dirs = [
        xdg_cfg.join("workspace-indicator"),
        xdg_st.join("omarchy/current"),
        xdg_st.join("omarchy/current/theme"),
        xdg_cfg.join("omarchy/current"),
        xdg_cfg.join("omarchy/current/theme"),
        xdg_cfg.join("current"),
        xdg_cfg.join("current/theme"),
    ];
    let mask =
        ffi::IN_MOVED_TO | ffi::IN_CREATE | ffi::IN_CLOSE_WRITE | ffi::IN_ATTRIB | ffi::IN_MODIFY;
    for dir in dirs {
        if dir.is_dir() {
            if let Ok(c_path) = CString::new(dir.to_string_lossy().as_bytes()) {
                ffi::inotify_add_watch(ifd, c_path.as_ptr(), mask);
            }
        }
    }
}

unsafe fn drain_inotify(ifd: RawFd) -> bool {
    let mut buf = [0u8; 1024];
    let mut got = false;
    while ffi::read(
        ifd,
        buf.as_mut_ptr() as *mut std::ffi::c_void,
        buf.len(),
    ) > 0
    {
        got = true;
    }
    if got {
        add_theme_watches(ifd);
    }
    got
}

unsafe fn epoll_add(epfd: RawFd, fd: RawFd, tag: u64) {
    if fd < 0 {
        return;
    }
    let mut ev = ffi::epoll_event {
        events: ffi::EPOLLIN,
        u64: tag,
    };
    ffi::epoll_ctl(epfd, ffi::EPOLL_CTL_ADD, fd, &mut ev);
}

unsafe fn epoll_del(epfd: RawFd, fd: RawFd) {
    if fd < 0 {
        return;
    }
    ffi::epoll_ctl(epfd, ffi::EPOLL_CTL_DEL, fd, ptr::null_mut());
}

struct Daemon {
    plugin_managed: bool,
    wl: WaylandClient,
    loader: ConfigLoader,
    cfg: Config,
    ws_state: WorkspaceState,
    urgent: Vec<bool>,
    layout: PillLayout,
    prev_ws: usize,
    morph_t: f64,
    morph_step: f64,
    opacity: f64,
    fade_target: f64,
    fade_step: f64,
    tfd_debounce: RawFd,
    tfd_frame: RawFd,
    tfd_hold: RawFd,
}

impl Daemon {
    fn recompute_layout(&mut self) {
        self.layout = compute_pill_layout_morphed(
            &self.cfg,
            self.ws_state.active_ws,
            self.prev_ws,
            self.morph_t,
            &self.ws_state.occupied,
            &self.urgent,
            self.ws_state.occ_max,
        );
    }

    fn sync_surface(&mut self) {
        let out_id = self
            .wl
            .resolve_output_id(self.ws_state.target_monitor.as_ref());
        let _ = self.wl.ensure_surface(
            out_id,
            self.cfg.position,
            self.cfg.margin,
            self.layout.width as u32,
            self.layout.height as u32,
        );
    }

    fn render_and_present(&mut self) {
        if !self.wl.configured {
            return;
        }
        let logical_w = self.layout.width as u32;
        let logical_h = self.layout.height as u32;
        let (phys_w, phys_h) = self.wl.physical_size(logical_w, logical_h);
        let scale = self.wl.scale_factor();
        let opacity = self.opacity;
        let bg = self.cfg.palette.bg;
        let active_ws = self.ws_state.active_ws;
        let layout = &self.layout;
        let _ = self.wl.present_frame(phys_w, phys_h, opacity, |pixels| {
            rasterize_pill_argb8888_scaled(
                pixels, phys_w, phys_h, scale, opacity, bg, layout, active_ws,
                self.cfg.border_alpha,
            );
        });
    }

    fn apply_reloaded_config(&mut self, new_cfg: Config) {
        if !self.plugin_managed && self.cfg.hyprland_conf_bind() != new_cfg.hyprland_conf_bind() {
            let _ = ipc::hypr_request(self.cfg.hyprland_keyword_unbind().as_bytes());
            let _ = ipc::hypr_request(new_cfg.hyprland_keyword_bindd().as_bytes());
            let _ = sync_hyprland_files(&new_cfg);
        }
        self.cfg = new_cfg;
        self.recompute_layout();
        self.sync_surface();
        if self.opacity > 0.001 {
            self.render_and_present();
        }
    }

    unsafe fn trigger_show(&mut self) {
        if let Some(new_cfg) = self.loader.reload_if_changed() {
            self.apply_reloaded_config(new_cfg);
        }

        let old_active = self.ws_state.active_ws;
        self.ws_state = ipc::query_workspace_state(self.cfg.max_workspaces, self.cfg.per_monitor);
        if self.ws_state.active_ws < 1 {
            return;
        }
        if self.urgent.len() <= self.cfg.max_workspaces {
            self.urgent.resize(self.cfg.max_workspaces + 1, false);
        }
        for (i, &u) in self.ws_state.urgent.iter().enumerate() {
            if u && i < self.urgent.len() {
                self.urgent[i] = true;
            }
        }
        if self.ws_state.active_ws < self.urgent.len() {
            self.urgent[self.ws_state.active_ws] = false;
        }

        if old_active >= 1 && old_active != self.ws_state.active_ws {
            self.prev_ws = old_active;
            self.morph_t = 0.0;
        } else {
            self.prev_ws = self.ws_state.active_ws;
            self.morph_t = 1.0;
        }

        let fade_steps = (self.cfg.fade_in_ms / 16).max(1) as f64;
        self.morph_step = 1.0 / fade_steps;
        self.fade_target = 1.0;
        self.fade_step = (1.0 - self.opacity) / fade_steps;

        self.recompute_layout();
        self.sync_surface();
        self.render_and_present();

        arm_timer_ms(self.tfd_frame, 16, 16);
        arm_timer_ms(self.tfd_hold, self.cfg.display_ms, 0);
    }

    unsafe fn on_frame_tick(&mut self) {
        self.opacity += self.fade_step;
        let fade_done = if self.fade_step >= 0.0 {
            self.opacity >= self.fade_target
        } else {
            self.opacity <= self.fade_target
        };
        if fade_done {
            self.opacity = self.fade_target;
        }

        if self.morph_t < 1.0 {
            self.morph_t = (self.morph_t + self.morph_step).min(1.0);
        }

        self.recompute_layout();
        self.render_and_present();

        if fade_done && self.morph_t >= 1.0 {
            disarm_timer(self.tfd_frame);
        }
    }

    unsafe fn on_hold_expired(&mut self) {
        let steps = (self.cfg.fade_out_ms / 16).max(1) as f64;
        self.fade_target = 0.0;
        self.fade_step = -self.opacity / steps;
        self.morph_t = 1.0;
        arm_timer_ms(self.tfd_frame, 16, 16);
    }
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
           --plugin-managed                    Do not write integration files or manage keybindings\n  \
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
    let mut plugin_managed = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--peek" => mode_peek = true,
            "--reload" => mode_reload = true,
            "--sync-hyprland" => mode_sync = true,
            "--plugin-managed" => plugin_managed = true,
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
        if plugin_managed {
            eprintln!("workspace-indicator: --sync-hyprland conflicts with --plugin-managed");
            std::process::exit(1);
        }
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

    // Block SIGUSR1/SIGUSR2/SIGTERM/SIGINT immediately via signalfd before publishing PID
    let sfd = unsafe { create_signalfd() };

    if !plugin_managed {
        let _ = sync_hyprland_files(&cfg);
        if cfg.hyprland_conf_bind() != "$mainMod, grave" {
            let _ = ipc::hypr_request(b"keyword unbind SUPER, grave");
            let _ = ipc::hypr_request(cfg.hyprland_keyword_bindd().as_bytes());
        }
    }

    let wl = match WaylandClient::connect() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("workspace-indicator: Wayland connect failed: {e}");
            std::process::exit(1);
        }
    };

    let ws_state = ipc::query_workspace_state(cfg.max_workspaces, cfg.per_monitor);
    let active_ws = ws_state.active_ws;
    let urgent = vec![false; cfg.max_workspaces + 1];
    let layout = compute_pill_layout_morphed(
        &cfg,
        active_ws,
        active_ws,
        1.0,
        &ws_state.occupied,
        &urgent,
        ws_state.occ_max,
    );

    let (epfd, ifd, tfd_debounce, tfd_frame, tfd_hold) = unsafe {
        let epfd = ffi::epoll_create1(ffi::EPOLL_CLOEXEC);
        let ifd = create_theme_inotify();
        let tfd_debounce = create_timerfd();
        let tfd_frame = create_timerfd();
        let tfd_hold = create_timerfd();

        epoll_add(epfd, wl.fd(), TAG_WAYLAND);
        epoll_add(epfd, sfd, TAG_SIGNAL);
        epoll_add(epfd, ifd, TAG_INOTIFY);
        epoll_add(epfd, tfd_debounce, TAG_DEBOUNCE);
        epoll_add(epfd, tfd_frame, TAG_FRAME);
        epoll_add(epfd, tfd_hold, TAG_HOLD);

        (epfd, ifd, tfd_debounce, tfd_frame, tfd_hold)
    };

    let mut sock2 = Socket2Stream::connect_nonblocking();
    if let Some(s2) = &sock2 {
        unsafe {
            epoll_add(epfd, s2.fd(), TAG_SOCKET2);
        }
    }

    let mut daemon = Daemon {
        plugin_managed,
        wl,
        loader,
        cfg,
        ws_state,
        urgent,
        layout,
        prev_ws: active_ws,
        morph_t: 1.0,
        morph_step: 1.0,
        opacity: 0.0,
        fade_target: 0.0,
        fade_step: 0.0,
        tfd_debounce,
        tfd_frame,
        tfd_hold,
    };

    daemon.sync_surface();
    let _ = daemon.wl.dispatch_pending();
    if daemon.wl.needs_redraw {
        daemon.wl.needs_redraw = false;
        daemon.render_and_present();
    }

    publish_lock_pid(&mut lock_file);

    let mut events = [ffi::epoll_event::default(); 16];
    let mut running = true;

    while running {
        let timeout_ms = if sock2.is_none() { 1000 } else { -1 };
        let n = unsafe { ffi::epoll_wait(epfd, events.as_mut_ptr(), events.len() as i32, timeout_ms) };

        if sock2.is_none() {
            sock2 = Socket2Stream::connect_nonblocking();
            if let Some(s2) = &sock2 {
                unsafe {
                    epoll_add(epfd, s2.fd(), TAG_SOCKET2);
                }
            }
        }

        if n <= 0 {
            continue;
        }

        for ev in &events[..n as usize] {
            match ev.u64 {
                TAG_WAYLAND => {
                    if let Err(e) = daemon.wl.dispatch_pending() {
                        eprintln!("workspace-indicator: Wayland error: {e}");
                        running = false;
                        break;
                    }
                    if daemon.wl.needs_redraw {
                        daemon.wl.needs_redraw = false;
                        daemon.render_and_present();
                    }
                }
                TAG_SOCKET2 => {
                    if let Some(s2) = &mut sock2 {
                        let batch = s2.drain_events();
                        for addr in &batch.urgent_addresses {
                            if let Some(ws) =
                                ipc::resolve_urgent_workspace(addr, daemon.cfg.max_workspaces)
                            {
                                if ws != daemon.ws_state.active_ws {
                                    if daemon.urgent.len() <= ws {
                                        daemon.urgent.resize(ws + 1, false);
                                    }
                                    daemon.urgent[ws] = true;
                                }
                            }
                        }
                        if batch.matched {
                            unsafe {
                                arm_timer_ms(daemon.tfd_debounce, daemon.cfg.debounce_ms.max(1), 0);
                            }
                        }
                        if batch.closed || (ev.events & (ffi::EPOLLHUP | ffi::EPOLLERR)) != 0 {
                            unsafe {
                                epoll_del(epfd, s2.fd());
                            }
                            sock2 = None;
                        }
                    }
                }
                TAG_SIGNAL => unsafe {
                    let mut info: ffi::signalfd_siginfo = std::mem::zeroed();
                    while ffi::read(
                        sfd,
                        &mut info as *mut ffi::signalfd_siginfo as *mut std::ffi::c_void,
                        std::mem::size_of::<ffi::signalfd_siginfo>(),
                    ) == std::mem::size_of::<ffi::signalfd_siginfo>() as isize
                    {
                        match info.ssi_signo as i32 {
                            ffi::SIGUSR1 => {
                                arm_timer_ms(daemon.tfd_debounce, daemon.cfg.debounce_ms.max(1), 0);
                            }
                            ffi::SIGUSR2 => {
                                let new_cfg = daemon.loader.load_force();
                                daemon.apply_reloaded_config(new_cfg);
                            }
                            ffi::SIGTERM | ffi::SIGINT => {
                                running = false;
                            }
                            _ => {}
                        }
                    }
                },
                TAG_INOTIFY => unsafe {
                    if drain_inotify(ifd) {
                        let new_cfg = daemon.loader.load_force();
                        daemon.apply_reloaded_config(new_cfg);
                    }
                },
                TAG_DEBOUNCE => unsafe {
                    drain_timerfd(daemon.tfd_debounce);
                    daemon.trigger_show();
                },
                TAG_FRAME => unsafe {
                    drain_timerfd(daemon.tfd_frame);
                    daemon.on_frame_tick();
                },
                TAG_HOLD => unsafe {
                    drain_timerfd(daemon.tfd_hold);
                    daemon.on_hold_expired();
                },
                _ => {}
            }
        }
    }

    let _ = lock_file.set_len(0);
}
