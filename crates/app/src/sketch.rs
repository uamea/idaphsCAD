use crate::MyTruckRenderer;
use crate::*;
use messages::{ControlMessage, ExtrusionMessage, SketchMessage};

impl MyTruckRenderer<AppMessage> {
    pub fn handle_sketch_controls(&mut self, sketch_msg: SketchMessage) {
        match sketch_msg {
            SketchMessage::SketchModeClicked => {
                println!("Sketch mode clicked");
                self.look_at_origin(
                    Point3::new(0.0, 0.0, 15.0),
                    Point3::new(0.0, 0.0, 0.0),
                    Vector3::unit_y(),
                )
            }
            SketchMessage::SketchToolLineclicked => {
                println!("Sketch tool line clicked");
            }
            SketchMessage::SketchToolRectangleClicked => {
                println!("Sketch tool rectangle clicked");
            }
        }
    }
}
