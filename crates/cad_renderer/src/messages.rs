use slint::wgpu_27::wgpu;

/// This enum describes the two kinds of message the Slint application send to the bevy integration thread.
#[derive(Clone)]
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
    /// Send this message to initially kick off the rendering process
    KickOff,
    AppMsg(T),
}

impl<T> std::fmt::Debug for ControlMessage<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlMessage::ReleaseFrontBufferTexture { .. } => {
                write!(f, "ControlMessage::ReleaseFrontBufferTexture")
            }
            ControlMessage::ResizeBuffers { width, height } => {
                write!(
                    f,
                    "ControlMessage::ResizeBuffers {{ width: {}, height: {} }}",
                    width, height
                )
            }
            ControlMessage::AppMsg(_) => {
                write!(f, "ControlMessage::AppMsg")
            }
            ControlMessage::KickOff => {
                write!(f, "ControlMessage::KickOff")
            }
        }
    }
}
