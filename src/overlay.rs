use crate::modes::{ModeType, Rect, Subdivision};

#[derive(Debug, Clone)]
pub struct OverlayState {
    pub mode: ModeType,
    pub viewport: Rect,
    pub area: Rect,
    pub history: Vec<Rect>,
    pub subdivisions: Vec<Subdivision>,
}

impl OverlayState {
    pub fn new(mode: ModeType, viewport: Rect) -> Self {
        let mut state = Self {
            mode,
            viewport,
            area: viewport,
            history: Vec::new(),
            subdivisions: Vec::new(),
        };
        state.rebuild();
        state
    }

    pub fn set_mode(&mut self, mode: ModeType) {
        self.mode = mode;
        self.history.clear();
        self.rebuild();
    }

    pub fn select_at(&mut self, x: i32, y: i32) -> Option<String> {
        let Some((label, new_area)) = self
            .subdivisions
            .iter()
            .filter(|sub| sub.rect.contains(x, y))
            .min_by_key(|sub| sub.rect.area())
            .map(|sub| (sub.label.clone(), sub.rect))
        else {
            return None;
        };

        self.history.push(self.area);
        self.area = new_area;
        self.rebuild();
        Some(label)
    }

    fn rebuild(&mut self) {
        let mode = self.mode.as_mode();
        self.subdivisions = mode.subdivisions(self.area);
    }
}
