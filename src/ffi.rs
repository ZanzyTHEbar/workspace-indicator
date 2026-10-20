#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_double, c_int, c_uint, c_ulong, c_void};

pub type gboolean = c_int;
pub type guint = c_uint;
pub type gulong = c_ulong;
pub type gpointer = *mut c_void;
pub type GCallback = unsafe extern "C" fn();
pub type GSourceFunc = unsafe extern "C" fn(user_data: gpointer) -> gboolean;

pub const FALSE: gboolean = 0;
pub const TRUE: gboolean = 1;
pub const G_SOURCE_REMOVE: gboolean = FALSE;
pub const G_SOURCE_CONTINUE: gboolean = TRUE;

pub const GTK_WINDOW_TOPLEVEL: c_int = 0;

pub const GTK_LAYER_SHELL_LAYER_OVERLAY: c_int = 3;

pub const GTK_LAYER_SHELL_EDGE_LEFT: c_int = 0;
pub const GTK_LAYER_SHELL_EDGE_RIGHT: c_int = 1;
pub const GTK_LAYER_SHELL_EDGE_TOP: c_int = 2;
pub const GTK_LAYER_SHELL_EDGE_BOTTOM: c_int = 3;

pub const GTK_LAYER_SHELL_KEYBOARD_MODE_NONE: c_int = 0;

pub const LOCK_EX: c_int = 2;
pub const LOCK_NB: c_int = 4;

pub const SIGINT: c_int = 2;
pub const SIGUSR1: c_int = 10;
pub const SIGUSR2: c_int = 12;
pub const SIGTERM: c_int = 15;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct GdkRectangle {
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
}

pub type GtkAllocation = GdkRectangle;

#[repr(C)]
pub struct GtkWidget {
    _private: [u8; 0],
}

#[repr(C)]
pub struct GtkWindow {
    _private: [u8; 0],
}

#[repr(C)]
pub struct GtkContainer {
    _private: [u8; 0],
}

#[repr(C)]
pub struct GdkScreen {
    _private: [u8; 0],
}

#[repr(C)]
pub struct GdkVisual {
    _private: [u8; 0],
}

#[repr(C)]
pub struct GdkDisplay {
    _private: [u8; 0],
}

#[repr(C)]
pub struct GdkMonitor {
    _private: [u8; 0],
}

#[repr(C)]
pub struct cairo_t {
    _private: [u8; 0],
}

#[repr(C)]
pub struct cairo_region_t {
    _private: [u8; 0],
}

extern "C" {
    // libc
    pub fn flock(fd: c_int, operation: c_int) -> c_int;
    pub fn kill(pid: c_int, sig: c_int) -> c_int;

    // GLib
    pub fn g_idle_add(function: GSourceFunc, data: gpointer) -> guint;
    pub fn g_timeout_add(interval: guint, function: GSourceFunc, data: gpointer) -> guint;
    pub fn g_source_remove(tag: guint) -> gboolean;
    pub fn g_unix_signal_add(signum: c_int, handler: GSourceFunc, user_data: gpointer) -> guint;
    pub fn g_signal_connect_data(
        instance: gpointer,
        detailed_signal: *const c_char,
        c_handler: GCallback,
        data: gpointer,
        destroy_data: gpointer,
        connect_flags: c_int,
    ) -> gulong;

    // GDK
    pub fn gdk_display_get_default() -> *mut GdkDisplay;
    pub fn gdk_display_get_n_monitors(display: *mut GdkDisplay) -> c_int;
    pub fn gdk_display_get_monitor(display: *mut GdkDisplay, monitor_num: c_int) -> *mut GdkMonitor;
    pub fn gdk_display_get_monitor_at_point(
        display: *mut GdkDisplay,
        x: c_int,
        y: c_int,
    ) -> *mut GdkMonitor;
    pub fn gdk_monitor_get_geometry(monitor: *mut GdkMonitor, geometry: *mut GdkRectangle);
    pub fn gdk_monitor_get_manufacturer(monitor: *mut GdkMonitor) -> *const c_char;
    pub fn gdk_monitor_get_model(monitor: *mut GdkMonitor) -> *const c_char;
    pub fn gdk_screen_get_rgba_visual(screen: *mut GdkScreen) -> *mut GdkVisual;

    // GTK
    pub fn gtk_init(argc: *mut c_int, argv: *mut *mut *mut c_char);
    pub fn gtk_main();
    pub fn gtk_main_quit();
    pub fn gtk_window_new(window_type: c_int) -> *mut GtkWidget;
    pub fn gtk_window_resize(window: *mut GtkWindow, width: c_int, height: c_int);
    pub fn gtk_drawing_area_new() -> *mut GtkWidget;
    pub fn gtk_container_add(container: *mut GtkContainer, widget: *mut GtkWidget);
    pub fn gtk_widget_get_screen(widget: *mut GtkWidget) -> *mut GdkScreen;
    pub fn gtk_widget_set_visual(widget: *mut GtkWidget, visual: *mut GdkVisual);
    pub fn gtk_widget_set_app_paintable(widget: *mut GtkWidget, app_paintable: gboolean);
    pub fn gtk_widget_set_size_request(widget: *mut GtkWidget, width: c_int, height: c_int);
    pub fn gtk_widget_get_allocation(widget: *mut GtkWidget, allocation: *mut GtkAllocation);
    pub fn gtk_widget_queue_draw(widget: *mut GtkWidget);
    pub fn gtk_widget_set_opacity(widget: *mut GtkWidget, opacity: c_double);
    pub fn gtk_widget_show_all(widget: *mut GtkWidget);
    pub fn gtk_widget_get_visible(widget: *mut GtkWidget) -> gboolean;
    pub fn gtk_widget_input_shape_combine_region(
        widget: *mut GtkWidget,
        region: *mut cairo_region_t,
    );

    // gtk-layer-shell
    pub fn gtk_layer_init_for_window(window: *mut GtkWindow);
    pub fn gtk_layer_set_layer(window: *mut GtkWindow, layer: c_int);
    pub fn gtk_layer_set_anchor(window: *mut GtkWindow, edge: c_int, anchor_to_edge: gboolean);
    pub fn gtk_layer_set_margin(window: *mut GtkWindow, edge: c_int, margin_size: c_int);
    pub fn gtk_layer_set_namespace(window: *mut GtkWindow, name_space: *const c_char);
    pub fn gtk_layer_set_keyboard_mode(window: *mut GtkWindow, mode: c_int);
    pub fn gtk_layer_set_monitor(window: *mut GtkWindow, monitor: *mut GdkMonitor);

    // Cairo
    pub fn cairo_new_sub_path(cr: *mut cairo_t);
    pub fn cairo_close_path(cr: *mut cairo_t);
    pub fn cairo_arc(
        cr: *mut cairo_t,
        xc: c_double,
        yc: c_double,
        radius: c_double,
        angle1: c_double,
        angle2: c_double,
    );
    pub fn cairo_set_source_rgba(
        cr: *mut cairo_t,
        red: c_double,
        green: c_double,
        blue: c_double,
        alpha: c_double,
    );
    pub fn cairo_fill(cr: *mut cairo_t);
    pub fn cairo_region_create() -> *mut cairo_region_t;
    pub fn cairo_region_destroy(region: *mut cairo_region_t);
}
