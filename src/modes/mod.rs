pub mod bisect;
pub mod floating;
pub mod split;
pub mod tile;

/// Minimum sub-area size in pixels² — matches tile mode's smallest tile.
/// Bisect and split modes stop subdividing when the area would fall below this.
pub const MIN_SUB_AREA_SIZE: u32 = 1250;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn area(self) -> u32 {
        self.w * self.h
    }

    pub fn center(self) -> (i32, i32) {
        (self.x + (self.w / 2) as i32, self.y + (self.h / 2) as i32)
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w as i32 && y >= self.y && y < self.y + self.h as i32
    }
}

#[derive(Debug, Clone)]
pub struct Subdivision {
    pub label: String,
    pub rect: Rect,
}

pub trait Mode {
    fn subdivisions(&self, area: Rect) -> Vec<Subdivision>;
    fn resolve(&self, area: Rect, selection: &str) -> Option<Rect>;
}

#[derive(Debug, Clone)]
pub struct ModeStep {
    pub mode: ModeType,
    pub selection: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeType {
    Bisect,
    SplitX,
    SplitY,
    Tile,
    Floating,
}

impl ModeType {
    pub const ALL: [ModeType; 5] = [
        ModeType::Bisect,
        ModeType::SplitX,
        ModeType::SplitY,
        ModeType::Tile,
        ModeType::Floating,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ModeType::Bisect => "Bisect",
            ModeType::SplitX => "Split-X",
            ModeType::SplitY => "Split-Y",
            ModeType::Tile => "Tile",
            ModeType::Floating => "Floating",
        }
    }

    pub fn as_mode(&self) -> Box<dyn Mode> {
        match self {
            ModeType::Bisect => Box::new(bisect::BisectMode),
            ModeType::SplitX => Box::new(split::SplitXMode),
            ModeType::SplitY => Box::new(split::SplitYMode),
            ModeType::Tile => Box::new(tile::TileMode),
            ModeType::Floating => Box::new(floating::FloatingMode),
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|mode| *mode == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

/// Resolve a chain of mode+selection steps starting from an initial area.
pub fn resolve_chain(initial_area: Rect, steps: &[ModeStep]) -> Option<Rect> {
    let mut area = initial_area;
    for step in steps {
        let mode = step.mode.as_mode();
        area = mode.resolve(area, &step.selection)?;
    }
    Some(area)
}
