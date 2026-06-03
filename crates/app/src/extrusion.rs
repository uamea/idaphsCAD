use crate::scene::SceneRenderer;
use crate::messages::ExtrusionMessage;

impl SceneRenderer {
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
