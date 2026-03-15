#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSection {
    Overview,
    Window,
    Capture,
    Input,
}

impl AppSection {
    pub const ALL: [AppSection; 4] = [
        AppSection::Overview,
        AppSection::Window,
        AppSection::Capture,
        AppSection::Input,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AppSection::Overview => "Overview",
            AppSection::Window => "Window",
            AppSection::Capture => "Capture",
            AppSection::Input => "Input",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButtonChoice {
    Left,
    Right,
    Middle,
}

impl MouseButtonChoice {
    pub const ALL: [MouseButtonChoice; 3] = [
        MouseButtonChoice::Left,
        MouseButtonChoice::Right,
        MouseButtonChoice::Middle,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MouseButtonChoice::Left => "Left",
            MouseButtonChoice::Right => "Right",
            MouseButtonChoice::Middle => "Middle",
        }
    }
}
