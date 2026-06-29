use crate::{UserIOUtils, messages::AppMessage};

pub trait CadTool {
    fn handle_event(&mut self, event: AppMessage, user_io_utils: &mut UserIOUtils) -> ToolResult;
}

pub enum ToolMode {
    Selection,
    Extrusion,
    Sketch,
}

pub enum ToolResult {
    Continue(bool),
    MoveTo(bool, ToolMode),
}
