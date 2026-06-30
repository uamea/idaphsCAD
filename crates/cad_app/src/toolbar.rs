use crate::messages::*;
use crate::{AppWindow, CadActions};
use cad_renderer::ControlMessage;
use slint::{ComponentHandle, Weak};

pub fn setup_toolbar_controls(
    ui_weak: Weak<AppWindow>,
    sender: smol::channel::Sender<ControlMessage<AppMessage>>,
) {
    let Some(ui) = ui_weak.upgrade() else {
        eprintln!("Failed to upgrade Weak reference to AppWindow");
        return;
    };
    ui.global::<CadActions>().on_trigger_command({
        move |cad_mode, id| {
            let _ = sender.try_send(ControlMessage::AppMsg(AppMessage::ToolbarMsg {
                msg: match cad_mode.as_str() {
                    "extrusion_mode" => {
                        ToolbarMessage::ExtrusionMsg(setup_extrusion_controls(id.as_str()))
                    }
                    "sketch_mode" => ToolbarMessage::SketchMsg(setup_sketch_controls(id.as_str())),
                    _ => ToolbarMessage::ExtrusionMsg(setup_extrusion_controls(id.as_str())),
                },
            }));
        }
    })
}

pub fn setup_extrusion_controls(mode_id: &str) -> ExtrusionMessage {
    use ExtrusionMessage::*;
    match mode_id {
        "extrusion_mode" => ExtrusionModeClicked,
        "extrusion_cut_mode" => ExtrusionCutModeClicked,
        _ => ExtrusionModeClicked,
    }
}

pub fn setup_sketch_controls(mode_id: &str) -> SketchMessage {
    use SketchMessage::*;
    match mode_id {
        "sketch_mode" => SketchModeClicked,
        "line" => SketchToolLineclicked,
        "rect" => SketchToolRectangleClicked,
        _ => SketchModeClicked,
    }
}
