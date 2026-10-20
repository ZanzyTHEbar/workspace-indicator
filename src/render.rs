use crate::config::{Config, Rgba};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DotGeometry {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
    pub color: Rgba,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CapsuleGeometry {
    pub ax: f64,
    pub ay: f64,
    pub bx: f64,
    pub by: f64,
    pub radius: f64,
    pub color: Rgba,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PillLayout {
    pub width: i32,
    pub height: i32,
    pub vertical: bool,
    pub dots: Vec<DotGeometry>,
    pub active_capsule: Option<CapsuleGeometry>,
}

pub fn dot_count(cfg: &Config, active_ws: usize, occ_max: usize) -> usize {
    let capped_active = if active_ws <= cfg.max_workspaces {
        active_ws
    } else {
        0
    };
    let hi = occ_max.max(capped_active);
    hi.clamp(cfg.persistent_workspaces, cfg.max_workspaces)
}

#[cfg(test)]
pub fn compute_pill_layout(
    cfg: &Config,
    active_ws: usize,
    occupied: &[bool],
    occ_max: usize,
) -> PillLayout {
    compute_pill_layout_morphed(cfg, active_ws, active_ws, 1.0, occupied, &[], occ_max)
}

pub fn compute_pill_layout_morphed(
    cfg: &Config,
    active_ws: usize,
    prev_ws: usize,
    morph_t: f64,
    occupied: &[bool],
    urgent: &[bool],
    occ_max: usize,
) -> PillLayout {
    let urgent_max = urgent
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, &u)| if u { Some(i) } else { None })
        .unwrap_or(0);
    let n = dot_count(cfg, active_ws, occ_max.max(urgent_max));
    let long_axis = cfg.pad_h * 2
        + (n.saturating_sub(1) as i32) * cfg.dot_spacing
        + (cfg.active_radius * 2.0) as i32;
    let short_axis = cfg.pad_v * 2 + (cfg.active_radius * 2.0) as i32;

    let vertical = cfg.position.is_vertical();
    let (width, height) = if vertical {
        (short_axis, long_axis)
    } else {
        (long_axis, short_axis)
    };

    let span = (n.saturating_sub(1) as f64) * f64::from(cfg.dot_spacing);
    let start = ((if vertical {
        f64::from(height)
    } else {
        f64::from(width)
    }) - span)
        * 0.5;
    let center_cross = (if vertical {
        f64::from(width)
    } else {
        f64::from(height)
    }) * 0.5;

    let dot_center = |ws: usize| -> (f64, f64) {
        let i = ws.saturating_sub(1);
        let offset = start + (i as f64) * f64::from(cfg.dot_spacing);
        if vertical {
            (center_cross, offset)
        } else {
            (offset, center_cross)
        }
    };

    let mut dots = Vec::with_capacity(n);
    for i in 0..n {
        let ws = i + 1;
        let (cx, cy) = dot_center(ws);

        let (color, radius) = if ws == active_ws {
            (cfg.palette.active, cfg.active_radius)
        } else if urgent.get(ws).copied().unwrap_or(false) {
            (cfg.palette.urgent, cfg.dot_radius + 0.75)
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

    // Active-dot stretching capsule interpolation between prev_ws and active_ws
    let active_capsule = if active_ws >= 1 && active_ws <= n {
        let (to_x, to_y) = dot_center(active_ws);
        let (from_x, from_y) = if prev_ws >= 1 && prev_ws <= n {
            dot_center(prev_ws)
        } else {
            (to_x, to_y)
        };
        let t = morph_t.clamp(0.0, 1.0);
        // Leading edge eases out fast; trailing edge eases in slower -> capsule stretch
        let inv = 1.0 - t;
        let t_head = 1.0 - inv * inv * inv;
        let t_tail = t * t * t;

        Some(CapsuleGeometry {
            ax: from_x + (to_x - from_x) * t_tail,
            ay: from_y + (to_y - from_y) * t_tail,
            bx: from_x + (to_x - from_x) * t_head,
            by: from_y + (to_y - from_y) * t_head,
            radius: cfg.active_radius,
            color: cfg.palette.active,
        })
    } else {
        None
    };

    PillLayout {
        width,
        height,
        vertical,
        dots,
        active_capsule,
    }
}

/// Signed-distance-field coverage `[0.0, 1.0]` for a 2D capsule `(ax, ay) -> (bx, by)` with radius `r`.
#[inline]
pub fn sdf_capsule_coverage_scaled(
    px: f64,
    py: f64,
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
    r: f64,
    scale: f64,
) -> f64 {
    let pax = px - ax;
    let pay = py - ay;
    let bax = bx - ax;
    let bay = by - ay;
    let ba_dot = bax * bax + bay * bay;
    let h = if ba_dot > 1e-9 {
        ((pax * bax + pay * bay) / ba_dot).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let dx = pax - bax * h;
    let dy = pay - bay * h;
    let dist = (dx * dx + dy * dy).sqrt() - r;
    (0.5 - dist * scale).clamp(0.0, 1.0)
}

/// Rasterizes the frosted pill background, workspace dots, and morphing active capsule
/// directly into a Wayland `ARGB8888` shared-memory pixel buffer at fractional `scale`.
pub fn rasterize_pill_argb8888_scaled(
    pixels: &mut [u32],
    phys_width: u32,
    phys_height: u32,
    scale: f64,
    opacity: f64,
    bg: Rgba,
    layout: &PillLayout,
    active_ws: usize,
    border_alpha: f64,
) {
    let total = (phys_width * phys_height) as usize;
    if pixels.len() < total {
        return;
    }
    if opacity < 0.001 || phys_width <= 1 || phys_height <= 1 {
        pixels[..total].fill(0);
        return;
    }

    let inv_scale = 1.0 / scale.max(0.25);
    let w = f64::from(layout.width);
    let h = f64::from(layout.height);
    let (pill_ax, pill_ay, pill_bx, pill_by, pill_r) = if layout.vertical {
        let r = (w * 0.5).min(h * 0.5);
        (r, r, r, h - r, r)
    } else {
        let r = (h * 0.5).min(w * 0.5);
        (r, r, w - r, r, r)
    };
    let aa_pad = inv_scale;

    for y in 0..phys_height {
        let py = (f64::from(y) + 0.5) * inv_scale;
        let row_off = (y * phys_width) as usize;

        for x in 0..phys_width {
            let px = (f64::from(x) + 0.5) * inv_scale;

            // Compute signed distance to pill boundary for rim calculation
            let pax = px - pill_ax;
            let pay = py - pill_ay;
            let bax = pill_bx - pill_ax;
            let bay = pill_by - pill_ay;
            let ba_dot = bax * bax + bay * bay;
            let h_param = if ba_dot > 1e-9 {
                ((pax * bax + pay * bay) / ba_dot).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let dx = pax - bax * h_param;
            let dy = pay - bay * h_param;
            let dist = (dx * dx + dy * dy).sqrt() - pill_r;

            let bg_cov = (0.5 - dist * scale).clamp(0.0, 1.0);
            if bg_cov <= 0.0 {
                pixels[row_off + x as usize] = 0;
                continue;
            }

            let mut out_a = bg.a * opacity * bg_cov;
            let mut out_r = bg.r * out_a;
            let mut out_g = bg.g * out_a;
            let mut out_b = bg.b * out_a;

            // Composite 1-px specular glass rim (inner edge highlight, tinted by active accent)
            // Rim is brightest at the inner 1px boundary of the pill capsule.
            // dist < 0 means inside the pill; the rim sits at dist in [-1/scale, 0].
            let rim = (1.0 + dist * scale).clamp(0.0, 1.0) * opacity * border_alpha * 2.5;
            if rim > 0.005 {
                // Tint rim with a subtle white/active blend (inner glass highlight)
                let inv_rim = 1.0 - rim.clamp(0.0, 0.7);
                out_r = 1.0 * rim + out_r * inv_rim;
                out_g = 1.0 * rim + out_g * inv_rim;
                out_b = 1.0 * rim + out_b * inv_rim;
                out_a = rim + out_a * inv_rim;
            }

            // Composite non-active dots
            for (idx, dot) in layout.dots.iter().enumerate() {
                if idx + 1 == active_ws && layout.active_capsule.is_some() {
                    continue;
                }
                if (px - dot.cx).abs() > dot.radius + aa_pad
                    || (py - dot.cy).abs() > dot.radius + aa_pad
                {
                    continue;
                }
                let cov = sdf_capsule_coverage_scaled(
                    px, py, dot.cx, dot.cy, dot.cx, dot.cy, dot.radius, scale,
                );
                if cov > 0.0 {
                    let src_a = dot.color.a * opacity * cov;
                    let inv_a = 1.0 - src_a;
                    out_r = dot.color.r * src_a + out_r * inv_a;
                    out_g = dot.color.g * src_a + out_g * inv_a;
                    out_b = dot.color.b * src_a + out_b * inv_a;
                    out_a = src_a + out_a * inv_a;
                }
            }

            // Composite morphing active-dot capsule
            if let Some(cap) = &layout.active_capsule {
                let min_x = cap.ax.min(cap.bx) - cap.radius - aa_pad;
                let max_x = cap.ax.max(cap.bx) + cap.radius + aa_pad;
                let min_y = cap.ay.min(cap.by) - cap.radius - aa_pad;
                let max_y = cap.ay.max(cap.by) + cap.radius + aa_pad;

                if px >= min_x && px <= max_x && py >= min_y && py <= max_y {
                    let cov = sdf_capsule_coverage_scaled(
                        px, py, cap.ax, cap.ay, cap.bx, cap.by, cap.radius, scale,
                    );
                    if cov > 0.0 {
                        let src_a = cap.color.a * opacity * cov;
                        let inv_a = 1.0 - src_a;
                        out_r = cap.color.r * src_a + out_r * inv_a;
                        out_g = cap.color.g * src_a + out_g * inv_a;
                        out_b = cap.color.b * src_a + out_b * inv_a;
                        out_a = src_a + out_a * inv_a;
                    }
                }
            }

            let a8 = (out_a.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
            let r8 = (out_r.clamp(0.0, 1.0) * 255.0 + 0.5).min(a8 as f64) as u32;
            let g8 = (out_g.clamp(0.0, 1.0) * 255.0 + 0.5).min(a8 as f64) as u32;
            let b8 = (out_b.clamp(0.0, 1.0) * 255.0 + 0.5).min(a8 as f64) as u32;
            pixels[row_off + x as usize] = (a8 << 24) | (r8 << 16) | (g8 << 8) | b8;
        }
    }
}

#[cfg(test)]
pub fn rasterize_pill_argb8888(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    opacity: f64,
    bg: Rgba,
    layout: &PillLayout,
    active_ws: usize,
) {
    rasterize_pill_argb8888_scaled(pixels, width, height, 1.0, opacity, bg, layout, active_ws, 0.18);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Position;

    #[test]
    fn horizontal_and_vertical_pill_rotation_swaps_axes_and_dot_coordinates() {
        let mut cfg = Config::default();
        let occ = vec![false, true, true, false, false, false];

        cfg.position = Position::Bottom;
        let horiz = compute_pill_layout(&cfg, 2, &occ, 2);
        assert!(!horiz.vertical);
        assert!(horiz.width > horiz.height);
        assert_eq!(horiz.dots.len(), 5);
        assert!((horiz.dots[0].cy - horiz.dots[4].cy).abs() < 1e-6);
        assert!(horiz.dots[4].cx > horiz.dots[0].cx);

        cfg.position = Position::Right;
        let vert = compute_pill_layout(&cfg, 2, &occ, 2);
        assert!(vert.vertical);
        assert_eq!(vert.width, horiz.height);
        assert_eq!(vert.height, horiz.width);
        assert!((vert.dots[0].cx - vert.dots[4].cx).abs() < 1e-6);
        assert!(vert.dots[4].cy > vert.dots[0].cy);
    }

    #[test]
    fn active_dot_capsule_stretches_during_morph_and_converges_at_end() {
        let cfg = Config::default();
        let occ = vec![false, true, false, true, false, false];

        let mid = compute_pill_layout_morphed(&cfg, 4, 1, 0.5, &occ, &[], 4);
        let cap_mid = mid.active_capsule.unwrap();
        assert!(cap_mid.bx > cap_mid.ax + 10.0); // stretched capsule between dot 1 and dot 4

        let done = compute_pill_layout_morphed(&cfg, 4, 1, 1.0, &occ, &[], 4);
        let cap_done = done.active_capsule.unwrap();
        assert!((cap_done.bx - cap_done.ax).abs() < 1e-6);
        assert!((cap_done.bx - done.dots[3].cx).abs() < 1e-6);

        let mut buf = vec![0u32; (mid.width * mid.height) as usize];
        rasterize_pill_argb8888(
            &mut buf,
            mid.width as u32,
            mid.height as u32,
            1.0,
            cfg.palette.bg,
            &mid,
            4,
        );
        // Center pixel of the pill must be non-transparent
        let center_idx =
            ((mid.height as usize / 2) * mid.width as usize) + (mid.width as usize / 2);
        assert!((buf[center_idx] >> 24) > 0);
    }
}
