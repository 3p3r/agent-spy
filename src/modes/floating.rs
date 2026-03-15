use image::{GrayImage, Luma};
use imageproc::edges::canny;
use imageproc::region_labelling::{Connectivity, connected_components};

use super::{Mode, Rect, Subdivision};

const SYMBOLS: &[u8] = b"abcdefghijklmnopqrstuvwxyz";

pub struct FloatingMode;

/// Dilate a binary image with a rectangular kernel (kw × kh).
fn dilate_binary(img: &GrayImage, kw: u32, kh: u32) -> GrayImage {
    let (w, h) = img.dimensions();
    let hw = kw / 2;
    let hh = kh / 2;
    let mut out = GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let y0 = y.saturating_sub(hh);
            let y1 = (y + hh).min(h - 1);
            let x0 = x.saturating_sub(hw);
            let x1 = (x + hw).min(w - 1);
            let mut hit = false;
            'outer: for ny in y0..=y1 {
                for nx in x0..=x1 {
                    if img.get_pixel(nx, ny).0[0] > 0 {
                        hit = true;
                        break 'outer;
                    }
                }
            }
            if hit {
                out.put_pixel(x, y, Luma([255]));
            }
        }
    }
    out
}

/// Check if `inner`'s center lies within `outer` and `inner` is smaller.
fn rect_contains(outer: &Rect, inner: &Rect) -> bool {
    let icx = inner.x + inner.w as i32 / 2;
    let icy = inner.y + inner.h as i32 / 2;
    icx >= outer.x
        && icx < outer.x + outer.w as i32
        && icy >= outer.y
        && icy < outer.y + outer.h as i32
        && (inner.w as u64 * inner.h as u64) < (outer.w as u64 * outer.h as u64)
}

/// Detect clickable UI targets in a screen region using edge detection.
/// Pipeline: capture → grayscale → Canny → dilate → connected components → bounding rects → filter.
pub fn detect_targets(area: Rect) -> Vec<Rect> {
    let capture = match screenshots::Screen::from_point(area.x, area.y) {
        Ok(screen) => {
            let rel_x = area.x - screen.display_info.x;
            let rel_y = area.y - screen.display_info.y;
            screen.capture_area(rel_x, rel_y, area.w.max(1), area.h.max(1))
        }
        Err(_) => return vec![],
    };

    let image = match capture {
        Ok(img) => img,
        Err(_) => return vec![],
    };

    let width = image.width();
    let height = image.height();
    let rgba_data = image.into_raw();

    let gray = GrayImage::from_fn(width, height, |x, y| {
        let idx = (y * width + x) as usize * 4;
        let r = rgba_data[idx] as u32;
        let g = rgba_data[idx + 1] as u32;
        let b = rgba_data[idx + 2] as u32;
        Luma([((r * 299 + g * 587 + b * 114) / 1000) as u8])
    });

    let edges = canny(&gray, 70.0, 220.0);

    let scale = height as f64 / area.h as f64;
    let kernel_cols = (3.5 * scale).round().max(1.0) as u32;
    let kernel_rows = (2.5 * scale).round().max(1.0) as u32;
    let dilated = dilate_binary(&edges, kernel_cols, kernel_rows);

    let labels = connected_components(&dilated, Connectivity::Eight, Luma([0]));

    // Find max label
    let max_label = labels.pixels().map(|p| p.0[0]).max().unwrap_or(0);
    if max_label == 0 {
        return vec![];
    }

    // Bounding rect per component
    let count = max_label as usize;
    let mut min_x = vec![u32::MAX; count];
    let mut min_y = vec![u32::MAX; count];
    let mut max_x = vec![0u32; count];
    let mut max_y = vec![0u32; count];

    for (x, y, p) in labels.enumerate_pixels() {
        let l = p.0[0];
        if l == 0 {
            continue;
        }
        let i = (l - 1) as usize;
        min_x[i] = min_x[i].min(x);
        min_y[i] = min_y[i].min(y);
        max_x[i] = max_x[i].max(x);
        max_y[i] = max_y[i].max(y);
    }

    // Build rects in screen coords, filter by size
    let mut rects: Vec<Rect> = Vec::new();
    for i in 0..count {
        if min_x[i] == u32::MAX {
            continue;
        }
        let rw = (max_x[i] - min_x[i] + 1) as f64 / scale;
        let rh = (max_y[i] - min_y[i] + 1) as f64 / scale;
        if rh >= 50.0 || rw >= 500.0 || rh <= 3.0 || rw <= 7.0 {
            continue;
        }
        rects.push(Rect {
            x: (min_x[i] as f64 / scale).round() as i32 + area.x,
            y: (min_y[i] as f64 / scale).round() as i32 + area.y,
            w: rw.round() as u32,
            h: rh.round() as u32,
        });
    }

    // Suppress nested rects (keep the larger enclosing one)
    let mut keep = vec![true; rects.len()];
    for i in 0..rects.len() {
        if !keep[i] {
            continue;
        }
        for j in (i + 1)..rects.len() {
            if !keep[j] {
                continue;
            }
            if rect_contains(&rects[i], &rects[j]) {
                keep[j] = false;
            } else if rect_contains(&rects[j], &rects[i]) {
                keep[i] = false;
                break;
            }
        }
    }

    rects
        .into_iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(r, _)| r)
        .collect()
}

fn index_to_label(idx: usize, total: usize) -> String {
    if total <= SYMBOLS.len() {
        return String::from(SYMBOLS[idx] as char);
    }
    let base = SYMBOLS.len();
    let first = idx / base;
    let second = idx % base;
    let mut s = String::with_capacity(2);
    s.push(SYMBOLS[first % SYMBOLS.len()] as char);
    s.push(SYMBOLS[second] as char);
    s
}

fn label_to_index(label: &str, total: usize) -> Option<usize> {
    let bytes = label.as_bytes();
    if total <= SYMBOLS.len() {
        if bytes.len() != 1 {
            return None;
        }
        let pos = SYMBOLS
            .iter()
            .position(|&c| c == bytes[0].to_ascii_lowercase())?;
        if pos < total { Some(pos) } else { None }
    } else {
        if bytes.len() != 2 {
            return None;
        }
        let first = SYMBOLS
            .iter()
            .position(|&c| c == bytes[0].to_ascii_lowercase())?;
        let second = SYMBOLS
            .iter()
            .position(|&c| c == bytes[1].to_ascii_lowercase())?;
        let idx = first * SYMBOLS.len() + second;
        if idx < total { Some(idx) } else { None }
    }
}

impl Mode for FloatingMode {
    fn subdivisions(&self, area: Rect) -> Vec<Subdivision> {
        let targets = detect_targets(area);
        let total = targets.len();
        targets
            .into_iter()
            .enumerate()
            .map(|(i, rect)| Subdivision {
                label: index_to_label(i, total),
                rect,
            })
            .collect()
    }

    fn resolve(&self, area: Rect, selection: &str) -> Option<Rect> {
        let targets = detect_targets(area);
        let total = targets.len();
        let idx = label_to_index(selection, total)?;
        targets.into_iter().nth(idx)
    }
}
