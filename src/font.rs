use std::ffi::{c_char, c_int, c_long, CString};

// Lato Medium, Łukasz Dziedzic, SIL Open Font License 1.1.
// Unmodified font and full license/provenance: assets/fonts/.
const EMBEDDED: &[u8] = include_bytes!("../assets/fonts/Lato-Medium.ttf");

#[repr(C)]
#[derive(Default)]
struct NativeGlyph {
    pixels: *mut u8,
    width: c_int,
    height: c_int,
    left: c_int,
    top: c_int,
    advance: c_int,
}

extern "C" {
    fn wi_prepare_digits(
        embedded: *const u8,
        length: c_long,
        family: *const c_char,
        size: u32,
        glyphs: *mut NativeGlyph,
    ) -> c_int;
    fn wi_free_glyphs(glyphs: *mut NativeGlyph);
}

struct Glyph {
    pixels: Vec<u8>,
    width: i32,
    height: i32,
    left: i32,
    top: i32,
    advance: i32,
}

pub struct Digits {
    glyphs: [Glyph; 10],
    scale: f64,
    top: i32,
    bottom: i32,
    pub half_width: f64,
    pub half_height: f64,
}

impl Digits {
    pub fn load(family: &str, size: f64, scale: f64) -> Option<Self> {
        let family_c = CString::new(family).unwrap_or_default();
        let mut native: [NativeGlyph; 10] = std::array::from_fn(|_| NativeGlyph::default());
        // SAFETY: embedded data and the NUL-terminated family live through the call;
        // native is exactly ten writable WiGlyph records matching our C shim.
        let status = unsafe {
            wi_prepare_digits(
                EMBEDDED.as_ptr(),
                EMBEDDED.len() as c_long,
                family_c.as_ptr(),
                (size * scale * 64.0).round() as u32,
                native.as_mut_ptr(),
            )
        };
        if status == 0 {
            eprintln!("workspace-indicator: text initialization failed; using dots");
            return None;
        }
        if status == 3 || (family_c.as_bytes().is_empty() && !family.is_empty()) {
            eprintln!("workspace-indicator: font family {family:?} unavailable; using embedded Lato Medium");
        }
        let glyphs = std::array::from_fn(|i| {
            let glyph = &native[i];
            // SAFETY: a successful shim call returns ten allocated, tightly packed
            // grayscale bitmaps; they remain valid until wi_free_glyphs below.
            let pixels = unsafe {
                std::slice::from_raw_parts(glyph.pixels, (glyph.width * glyph.height) as usize)
                    .to_vec()
            };
            Glyph {
                pixels,
                width: glyph.width,
                height: glyph.height,
                left: glyph.left,
                top: glyph.top,
                advance: glyph.advance,
            }
        });
        // SAFETY: these records were allocated by wi_prepare_digits and are freed
        // only after copying their bitmaps, using the same C allocator.
        unsafe { wi_free_glyphs(native.as_mut_ptr()) };
        let top = glyphs.iter().map(|g| g.top).max().unwrap_or(0);
        let bottom = glyphs.iter().map(|g| g.height - g.top).max().unwrap_or(0);
        let mut digits = Self {
            glyphs,
            scale,
            top,
            bottom,
            half_width: 0.0,
            half_height: 0.0,
        };
        let width = (1..=32)
            .map(|n| {
                let (left, right) = digits.bounds(n);
                right - left
            })
            .max()
            .unwrap_or(0);
        // Reserve one physical pixel for independently rounded origins/baselines.
        digits.half_width = (f64::from(width) * 0.5 + 1.0) / scale;
        digits.half_height = (f64::from(top + bottom) * 0.5 + 1.0) / scale;
        Some(digits)
    }

    fn bounds(&self, number: usize) -> (i32, i32) {
        let last = &self.glyphs[number % 10];
        if number < 10 {
            (last.left.min(0), (last.left + last.width).max(last.advance))
        } else {
            let first = &self.glyphs[number / 10];
            (
                first.left.min(0).min(first.advance + last.left),
                (first.left + first.width)
                    .max(first.advance + last.left + last.width)
                    .max(first.advance + last.advance),
            )
        }
    }

    pub fn coverage(&self, number: usize, x: u32, y: u32, cx: f64, cy: f64) -> f64 {
        let (left, right) = self.bounds(number);
        let origin = (cx * self.scale - f64::from(left + right) * 0.5).round() as i32;
        let baseline = (cy * self.scale + f64::from(self.top - self.bottom) * 0.5).round() as i32;
        let sample = |digit: usize, offset: i32| {
            let glyph = &self.glyphs[digit];
            let gx = x as i32 - origin - offset - glyph.left;
            let gy = y as i32 - baseline + glyph.top;
            if gx >= 0 && gx < glyph.width && gy >= 0 && gy < glyph.height {
                f64::from(glyph.pixels[(gy * glyph.width + gx) as usize]) / 255.0
            } else {
                0.0
            }
        };
        if number < 10 {
            sample(number, 0)
        } else {
            let first = number / 10;
            // Alpha union also handles fonts with overlapping digit ink.
            let a = sample(first, 0);
            a + sample(number % 10, self.glyphs[first].advance) * (1.0 - a)
        }
    }
}
