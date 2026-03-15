use super::{MIN_SUB_AREA_SIZE, Mode, Rect, Subdivision};

pub struct SplitXMode;
pub struct SplitYMode;

impl Mode for SplitXMode {
    fn subdivisions(&self, area: Rect) -> Vec<Subdivision> {
        if area.area() < MIN_SUB_AREA_SIZE || (area.w <= 1 && area.h <= 1) {
            return vec![];
        }

        if area.w <= 1 {
            return vec![];
        }

        let half_w = area.w / 2;
        vec![
            Subdivision {
                label: "w".to_string(),
                rect: Rect {
                    x: area.x,
                    y: area.y,
                    w: half_w,
                    h: area.h,
                },
            },
            Subdivision {
                label: "e".to_string(),
                rect: Rect {
                    x: area.x + half_w as i32,
                    y: area.y,
                    w: area.w - half_w,
                    h: area.h,
                },
            },
        ]
    }

    fn resolve(&self, area: Rect, selection: &str) -> Option<Rect> {
        let subs = self.subdivisions(area);
        let sel = selection.to_lowercase();
        subs.into_iter().find(|s| s.label == sel).map(|s| s.rect)
    }
}

impl Mode for SplitYMode {
    fn subdivisions(&self, area: Rect) -> Vec<Subdivision> {
        if area.area() < MIN_SUB_AREA_SIZE || (area.w <= 1 && area.h <= 1) {
            return vec![];
        }

        if area.h <= 1 {
            return vec![];
        }

        let half_h = area.h / 2;
        vec![
            Subdivision {
                label: "n".to_string(),
                rect: Rect {
                    x: area.x,
                    y: area.y,
                    w: area.w,
                    h: half_h,
                },
            },
            Subdivision {
                label: "s".to_string(),
                rect: Rect {
                    x: area.x,
                    y: area.y + half_h as i32,
                    w: area.w,
                    h: area.h - half_h,
                },
            },
        ]
    }

    fn resolve(&self, area: Rect, selection: &str) -> Option<Rect> {
        let subs = self.subdivisions(area);
        let sel = selection.to_lowercase();
        subs.into_iter().find(|s| s.label == sel).map(|s| s.rect)
    }
}
