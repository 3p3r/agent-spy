use super::{Mode, Rect, Subdivision};

const MIN_SUB_AREA_SIZE: u32 = 1250; // 25 * 50, matching wl-kbptr
const SYMBOLS: &[u8] = b"abcdefghijklmnopqrstuvwxyz";

pub struct TileMode;

struct GridLayout {
    rows: u32,
    cols: u32,
    sub_w: u32,
    sub_w_off: u32,
    sub_h: u32,
    sub_h_off: u32,
}

fn compute_grid(area: &Rect) -> GridLayout {
    let total = area.w as u64 * area.h as u64;
    let max_cells = (SYMBOLS.len() * SYMBOLS.len()) as u64; // 676
    let sub_area_size = (total / max_cells).max(MIN_SUB_AREA_SIZE as u64);

    let sub_h = ((sub_area_size as f64 / 2.0).sqrt()) as u32;
    let rows = if sub_h == 0 {
        1
    } else {
        (area.h / sub_h).max(1)
    };
    let sub_h = area.h / rows;
    let sub_h_off = area.h % rows;

    let sub_w = ((sub_area_size as f64 * 2.0).sqrt()) as u32;
    let cols = if sub_w == 0 {
        1
    } else {
        (area.w / sub_w).max(1)
    };
    let sub_w = area.w / cols;
    let sub_w_off = area.w % cols;

    GridLayout {
        rows,
        cols,
        sub_w,
        sub_w_off,
        sub_h,
        sub_h_off,
    }
}

fn index_to_label(idx: u32, total: u32) -> String {
    if total <= SYMBOLS.len() as u32 {
        return String::from(SYMBOLS[idx as usize] as char);
    }
    // Multi-character: column-major encoding
    let base = SYMBOLS.len() as u32;
    let first = idx / base;
    let second = idx % base;
    let mut s = String::with_capacity(2);
    s.push(SYMBOLS[first as usize % SYMBOLS.len()] as char);
    s.push(SYMBOLS[second as usize] as char);
    s
}

fn label_to_index(label: &str, total: u32) -> Option<u32> {
    let bytes = label.as_bytes();
    if total <= SYMBOLS.len() as u32 {
        if bytes.len() != 1 {
            return None;
        }
        let pos = SYMBOLS
            .iter()
            .position(|&c| c == bytes[0].to_ascii_lowercase())?;
        let idx = pos as u32;
        if idx < total { Some(idx) } else { None }
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
        let base = SYMBOLS.len() as u32;
        let idx = first as u32 * base + second as u32;
        if idx < total { Some(idx) } else { None }
    }
}

fn idx_to_rect(grid: &GridLayout, area: &Rect, idx: u32) -> Rect {
    // Column-major: column = idx / rows, row = idx % rows
    let col = idx / grid.rows;
    let row = idx % grid.rows;

    let x = area.x + (col * grid.sub_w + col.min(grid.sub_w_off)) as i32;
    let y = area.y + (row * grid.sub_h + row.min(grid.sub_h_off)) as i32;
    let w = grid.sub_w + if col < grid.sub_w_off { 1 } else { 0 };
    let h = grid.sub_h + if row < grid.sub_h_off { 1 } else { 0 };

    Rect { x, y, w, h }
}

impl Mode for TileMode {
    fn subdivisions(&self, area: Rect) -> Vec<Subdivision> {
        let grid = compute_grid(&area);
        let total = grid.rows * grid.cols;

        (0..total)
            .map(|idx| {
                let rect = idx_to_rect(&grid, &area, idx);
                Subdivision {
                    label: index_to_label(idx, total),
                    rect,
                }
            })
            .collect()
    }

    fn resolve(&self, area: Rect, selection: &str) -> Option<Rect> {
        let grid = compute_grid(&area);
        let total = grid.rows * grid.cols;
        let idx = label_to_index(selection, total)?;
        Some(idx_to_rect(&grid, &area, idx))
    }
}
