#[derive(Debug, Clone)]
pub enum Message {
    RefreshWindows,
    SearchChanged(String),
    SelectWindow(u64),
    ToggleTrackMouse,
    Tick,
    FocusSelected,
    MoveXChanged(String),
    MoveYChanged(String),
    ApplyMove,
    SizeWChanged(String),
    SizeHChanged(String),
    ApplySize,
    ToggleAlwaysOnTop,
    CaptureSelectedWindow,
    CaptureScreen,
    InputTextChanged(String),
    SendInputText,
    ClickXChanged(String),
    ClickYChanged(String),
    SelectMouseButton(MouseButtonChoice),
    SendMouseClick,
    MoveMouseToPoint,
    ClearStatus,
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
