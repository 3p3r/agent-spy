#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSection {
    Overview,
    Window,
    Browser,
    Capture,
    Input,
}

impl AppSection {
    pub const CORE: [AppSection; 4] = [
        AppSection::Overview,
        AppSection::Window,
        AppSection::Capture,
        AppSection::Input,
    ];

    pub const WITH_BROWSER: [AppSection; 5] = [
        AppSection::Overview,
        AppSection::Window,
        AppSection::Browser,
        AppSection::Capture,
        AppSection::Input,
    ];

    pub fn visible_sections(has_browser: bool) -> &'static [AppSection] {
        if has_browser {
            &Self::WITH_BROWSER
        } else {
            &Self::CORE
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AppSection::Overview => "Overview",
            AppSection::Window => "Window",
            AppSection::Browser => "Browser",
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
