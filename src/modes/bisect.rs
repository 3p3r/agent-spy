use super::{MIN_SUB_AREA_SIZE, Mode, Rect, Subdivision};

const DIVIDE_8_RATIO: f64 = 1.8;

pub struct BisectMode;

#[derive(Debug, Clone, Copy)]
enum Division {
    Eight,
    Four,
    Horizontal,
    Vertical,
    Undividable,
}

fn determine_division(area: &Rect) -> Division {
    if area.w <= 1 && area.h <= 1 {
        return Division::Undividable;
    }
    if area.w <= 1 {
        return Division::Vertical;
    }
    if area.h <= 1 {
        return Division::Horizontal;
    }
    if area.area() < MIN_SUB_AREA_SIZE {
        return Division::Undividable;
    }
    if (area.w as f64) > (area.h as f64) * DIVIDE_8_RATIO {
        Division::Eight
    } else {
        Division::Four
    }
}

/// Labels for Division4: nw=0, ne=1, sw=2, se=3
const LABELS_4: [&str; 4] = ["nw", "ne", "sw", "se"];

/// Labels for Division8: top row nw..ne4, bottom row sw..se4
const LABELS_8: [&str; 8] = ["nw", "nw2", "ne2", "ne", "sw", "sw2", "se2", "se"];

const LABELS_H: [&str; 2] = ["w", "e"];
const LABELS_V: [&str; 2] = ["n", "s"];

fn subdivide_4_or_8(area: &Rect, cols: u32) -> Vec<Subdivision> {
    let rows = 2u32;
    let sub_w = area.w / cols;
    let sub_w_off = area.w % cols;
    let sub_h = area.h / rows;
    let sub_h_off = area.h % rows;

    let labels = if cols == 4 {
        &LABELS_8[..]
    } else {
        &LABELS_4[..]
    };

    let mut subs = Vec::with_capacity((cols * rows) as usize);
    for j in 0..rows {
        for i in 0..cols {
            let idx = (j * cols + i) as usize;
            let x = area.x + (i * sub_w + i.min(sub_w_off)) as i32;
            let y = area.y + (j * sub_h + j.min(sub_h_off)) as i32;
            let w = sub_w + if i < sub_w_off { 1 } else { 0 };
            let h = sub_h + if j < sub_h_off { 1 } else { 0 };
            subs.push(Subdivision {
                label: labels[idx].to_string(),
                rect: Rect { x, y, w, h },
            });
        }
    }
    subs
}

fn subdivide_horizontal(area: &Rect) -> Vec<Subdivision> {
    let half = area.w / 2;
    vec![
        Subdivision {
            label: LABELS_H[0].to_string(),
            rect: Rect {
                x: area.x,
                y: area.y,
                w: half,
                h: area.h,
            },
        },
        Subdivision {
            label: LABELS_H[1].to_string(),
            rect: Rect {
                x: area.x + half as i32,
                y: area.y,
                w: area.w - half,
                h: area.h,
            },
        },
    ]
}

fn subdivide_vertical(area: &Rect) -> Vec<Subdivision> {
    let half = area.h / 2;
    vec![
        Subdivision {
            label: LABELS_V[0].to_string(),
            rect: Rect {
                x: area.x,
                y: area.y,
                w: area.w,
                h: half,
            },
        },
        Subdivision {
            label: LABELS_V[1].to_string(),
            rect: Rect {
                x: area.x,
                y: area.y + half as i32,
                w: area.w,
                h: area.h - half,
            },
        },
    ]
}

impl Mode for BisectMode {
    fn subdivisions(&self, area: Rect) -> Vec<Subdivision> {
        match determine_division(&area) {
            Division::Eight => subdivide_4_or_8(&area, 4),
            Division::Four => subdivide_4_or_8(&area, 2),
            Division::Horizontal => subdivide_horizontal(&area),
            Division::Vertical => subdivide_vertical(&area),
            Division::Undividable => vec![],
        }
    }

    fn resolve(&self, area: Rect, selection: &str) -> Option<Rect> {
        let subs = self.subdivisions(area);
        let sel = selection.to_lowercase();
        subs.into_iter().find(|s| s.label == sel).map(|s| s.rect)
    }
}
