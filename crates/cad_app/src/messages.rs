use slint::language::KeyboardModifiers;

#[derive(Clone, Debug)]
pub enum AppMessage {
    CameraMsg { msg: CameraMessage },
    InputMsg { msg: InputMessage },
    ToolbarMsg { msg: ToolbarMessage },
    FileIOMsg { msg: FileIOMessage },
}

#[derive(Clone, Debug)]
pub enum ToolbarMessage {
    ExtrusionMsg(ExtrusionMessage),
    SketchMsg(SketchMessage),
}

#[derive(Clone, Debug)]
pub enum CameraMessage {
    Reset,
}

#[derive(Clone, Debug)]
pub enum ExtrusionMessage {
    ExtrusionModeClicked,
    ExtrusionCutModeClicked,
}

#[derive(Clone, Debug)]
pub enum SketchMessage {
    SketchModeClicked,
    SketchToolLineclicked,
    SketchToolRectangleClicked,
}

#[derive(Clone, Debug)]
pub enum InputMessage {
    Click {
        x: f32,
        y: f32,
    },
    MouseMove {
        x: f32,
        y: f32,
        modifiers: KeyboardModifiers,
    },
    Wheel {
        delta: f32,
    },
    MiddleClickUp,
    MiddleClickDown {
        x: f32,
        y: f32,
    },
    KeyEventPressed {
        key: String,
        modifiers: KeyboardModifiers,
    },
    KeyEventReleased {
        key: String,
        modifiers: KeyboardModifiers,
    },
}

#[derive(Clone, Debug)]
pub enum FileIOMessage {
    NewFile,
    OpenFile,
    ExportFile,
}
