use crate::messages::{AppMessage, InputMessage};
use crate::scene::SceneRenderer;
use crate::{AppWindow, bind_callbacks};
use cad_renderer::*;
use slint::Weak;
use std::collections::HashSet;
use truck_meshalgo::prelude::*;
use truck_platform::{Camera, Light};
use truck_rendimpl::*;

pub fn setup_baseops_controls(
    ui_weak: Weak<AppWindow>,
    sender: smol::channel::Sender<ControlMessage<AppMessage>>,
) {
    let Some(ui) = ui_weak.upgrade() else {
        eprintln!("Failed to upgrade Weak reference to AppWindow");
        return;
    };
    bind_callbacks!(ui, sender, {
        on_image_clicked(x, y) => AppMessage::InputMsg{ msg: InputMessage::Click { x, y } },
        on_mouse_moved(x, y, modifiers) => AppMessage::InputMsg { msg: InputMessage::MouseMove { x, y, modifiers } },
        on_wheel_scrolled(delta) => AppMessage::InputMsg { msg: InputMessage::Wheel { delta } },
        on_middle_click_down(x, y) => AppMessage::InputMsg { msg: InputMessage::MiddleClickDown { x, y } },
        on_middle_click_up => AppMessage::InputMsg { msg: InputMessage::MiddleClickUp },
        on_key_event_pressed_received(key_pressed, modifiers) => AppMessage::InputMsg { msg: InputMessage::KeyEventPressed { key: String::from(&key_pressed), modifiers } },
        on_key_event_released_received(key_released, modifiers) => AppMessage::InputMsg { msg: InputMessage::KeyEventReleased { key: String::from(&key_released), modifiers } },
    });
}

#[derive(Debug, Default)]
pub struct InputEventHandler {
    pub last_input_msg: InputMessage,
    pub is_middle_click_pressed: bool,
    pub middle_click_start_pos: Option<(f32, f32)>,
    pub pressed_keys: HashSet<String>,
    pub ctrl_pressed: bool,
}

impl InputEventHandler {
    pub fn new() -> Self {
        Self {
            last_input_msg: InputMessage::Click { x: 0.0, y: 0.0 },
            is_middle_click_pressed: false,
            middle_click_start_pos: None,
            pressed_keys: HashSet::new(),
            ctrl_pressed: false,
        }
    }

    pub fn handle_baseops_controls(&mut self, input_msg: InputMessage) {
        use InputMessage::*;
        self.last_input_msg = input_msg.clone();

        match input_msg {
            Click { x, y } => {}
            MouseMove { x, y, modifiers } => {}
            Wheel { delta } => {}
            MiddleClickUp => {
                self.is_middle_click_pressed = false;
            }
            MiddleClickDown { x, y } => {
                self.is_middle_click_pressed = true;
                self.middle_click_start_pos = Some((x, y));
            }
            KeyEventPressed { key, modifiers } => {
                self.ctrl_pressed = modifiers.control;
                if !key.is_empty() {
                    self.pressed_keys.insert(key.clone());
                }
            }
            KeyEventReleased { key, modifiers } => {
                self.ctrl_pressed = modifiers.control;
                if !key.is_empty() {
                    self.pressed_keys.remove(&key);
                }
            }
        }
    }
}
