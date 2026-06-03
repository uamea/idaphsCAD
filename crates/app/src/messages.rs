use slint::language::KeyboardModifiers;
use slint::wgpu_27::wgpu;
use truck_rendimpl::*;

/// This enum describes the two kinds of message the Slint application send to the bevy integration thread.
pub enum ControlMessage<T> {
    /// Send this message when you don't need a previously received texture anymore.
    ReleaseFrontBufferTexture {
        texture: wgpu::Texture,
    },
    /// Send this message to adjust the size of the scene textures.
    ResizeBuffers {
        width: u32,
        height: u32,
    },
    AppMsg(T),
}

#[derive(Clone, Debug)]
pub enum AppMessage {
    Cameramsg { msg: CameraMessage },
    InputMsg { msg: InputMessage },
    ToolbarMsg { msg: ToolbarMessage },
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
