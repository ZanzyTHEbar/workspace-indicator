use std::f64::consts::PI;

use crate::config::{Config, Position, Rgba};
use crate::ffi;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DotGeometry {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
    pub color: Rgba,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PillLayout {
    pub width: i32,
    pub height: i32,
    pub vertical: bool,
    pub dots: Vec<DotGeometry>,
}

pub fn dot_count(cfg: &Config, active_ws: usize, occ_max: usize) -> usize {
    let hi = occ_max.max(active_ws);
    hi.clamp(cfg.persistent_workspaces, cfg.max_workspaces)
}

pub fn compute_pill_layout(
    cfg: &Config,
    active_ws: usize,
    occupied: &[bool],
    occ_max: usize,
) -> PillLayout {
    let n = dot_count(cfg, active_ws, occ_max);
    let long_axis =
        cfg.pad_h * 2 + (n.saturating_sub(1) as i32) * cfg.dot_spacing + (cfg.active_radius * 2.0) as i32;
    let short_axis = cfg.pad_v * 2 + (cfg.active_radius * 2.0) as i32;

    let vertical = cfg.position.is_vertical();
    let (width, height) = if vertical {
        (short_axis, long_axis)
    } else {
        (long_axis, short_axis)
    };

    let span = (n.saturating_sub(1) as f64) * f64::from(cfg.dot_spacing);
    let start = ((if vertical { f64::from(height) } else { f64::from(width) }) - span) * 0.5;
    let center_cross = (if vertical { f64::from(width) } else { f64::from(height) }) * 0.5;

    let mut dots = Vec::with_capacity(n);
    for i in 0..n {
        let ws = i + 1;
        let offset = start + (i as f64) * f64::from(cfg.dot_spacing);
        let (cx, cy) = if vertical {
            (center_cross, offset)
        } else {
            (offset, center_cross)
        };

        let (color, radius) = if ws == active_ws {
            (cfg.palette.active, cfg.active_radius)
        } else if occupied.get(ws).copied().unwrap_or(false) {
            (cfg.palette.occupied, cfg.dot_radius)
        } else {
            (cfg.palette.empty, (cfg.dot_radius - 1.0).max(1.5))
        };

        dots.push(DotGeometry {
            cx,
            cy,
            radius,
            color,
        });
    }

    PillLayout {
        width,
        height,
        vertical,
        dots,
    }
}

/// Draw the pill capsule (horizontal or 90°-rotated vertical) and workspace dots via Cairo.
pub unsafe fn draw_pill(
    cr: *mut ffi::cairo_t,
    alloc_w: f64,
    alloc_h: f64,
    opacity: f64,
    bg: Rgba,
    layout: &PillLayout,
) {
    if opacity < 0.001 || alloc_w <= 1.0 || alloc_h <= 1.0 {
        return;
    }

    ffi::cairo_new_sub_path(cr);
    if layout.vertical {
        // 90-degree rotated vertical capsule: semicircular caps at top and bottom
        let r = (alloc_w * 0.5).min(alloc_h * 0.5);
        ffi::cairo_arc(cr, r, r, r, PI, 2.0 * PI);
        ffi::cairo_arc(cr, r, alloc_h - r, r, 0.0, PI);
    } else {
        // Horizontal capsule: semicircular caps at left and right
        let r = (alloc_h * 0.5).min(alloc_w * 0.5);
        ffi::cairo_arc(cr, r, r, r, PI * 0.5, PI * 1.5);
        ffi::cairo_arc(cr, alloc_w - r, r, r, PI * 1.5, PI * 0.5);
    }
    ffi::cairo_close_path(cr);
    ffi::cairo_set_source_rgba(cr, bg.r, bg.g, bg.b, bg.a * opacity);
    ffi::cairo_fill(cr);

    for dot in &layout.dots {
        ffi::cairo_set_source_rgba(
            cr,
            dot.color.r,
            dot.color.g,
            dot.color.b,
            dot.color.a * opacity,
        );
        ffi::cairo_arc(cr, dot.cx, dot.cy, dot.radius, 0.0, PI * 2.0);
        ffi::cairo_fill(cr);
    }
}

pub const fn layer_edge_for_position(pos: Position) -> i32 {
    match pos {
        Position::Left => ffi::GTK_LAYER_SHELL_EDGE_LEFT,
        Position::Right => ffi::GTK_LAYER_SHELL_EDGE_RIGHT,
        Position::Top => ffi::GTK_LAYER_SHELL_EDGE_TOP,
        Position::Bottom => ffi::GTK_LAYER_SHELL_EDGE_BOTTOM,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_and_vertical_pill_rotation_swaps_axes_and_dot_coordinates() {
        let mut cfg = Config::default();
        let occ = vec![false, true, true, false, false, false];

        cfg.position = Position::Bottom;
        let horiz = compute_pill_layout(&cfg, 2, &occ, 2);
        assert!(!horiz.vertical);
        assert!(horiz.width > horiz.height);
        assert_eq!(horiz.dots.len(), 5);
        // Horizontal: all dots share the same Y center, X increases
        assert!((horiz.dots[0].cy - horiz.dots[4].cy).abs() < 1e-6);
        assert!(horiz.dots[4].cx > horiz.dots[0].cx);

        cfg.position = Position::Right;
        let vert = compute_pill_layout(&cfg, 2, &occ, 2);
        assert!(vert.vertical);
        assert_eq!(vert.width, horiz.height);
        assert_eq!(vert.height, horiz.width);
        // Vertical (rotated 90 deg): all dots share the same X center, Y increases
        assert!((vert.dots[0].cx - vert.dots[4].cx).abs() < 1e-6);
        assert!(vert.dots[4].cy > vert.dots[0].cy);
    }
}
