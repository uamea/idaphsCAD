use slint::language::KeyboardModifiers;
use slint::wgpu_27::wgpu;

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
