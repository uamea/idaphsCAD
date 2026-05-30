use crate::MyTruckRenderer;
use crate::*;
use messages::{ControlMessage, ExtrusionMessage};

impl MyTruckRenderer<AppMessage> {
    pub fn handle_extrusion_controls(&mut self, extrusion_msg: ExtrusionMessage) {
        use ExtrusionMessage::*;
        match extrusion_msg {
            ExtrusionModeClicked => {
                println!("Extrusion mode clicked");
            }
            ExtrusionCutModeClicked => {
                println!("Extrusion cut mode clicked");
            }
        }
    }
}
