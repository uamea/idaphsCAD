use crate::bind_callbacks;
use crate::messages::{AppMessage, InputMessage};
use crate::{AppWindow, ControlMessage, SceneEventHandler};
use slint::Weak;
use truck_meshalgo::prelude::*;

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
