use crate::messages::{AppMessage, ControlMessage, InputMessage};
use crate::scene::SceneRenderer;
use crate::{AppWindow, bind_callbacks};
use slint::Weak;
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

impl SceneRenderer {
    pub fn handle_baseops_controls(&mut self, input_msg: InputMessage) {
        use InputMessage::*;
        match input_msg {
            Click { x, y } => {
                println!("Clicked at ({}, {})", x, y);
                crate::selection::handle_selection(self, x as f64, y as f64);
            }
            MouseMove { x, y, modifiers } => {
                let position = Vector2::new(x as f64, y as f64);
                let dir2d = position - self.last_mouse_pos;

                if dir2d.so_small() {
                    self.last_mouse_pos = position;
                    return;
                }

                let is_ctrl_pressed = modifiers.control;
                self.ctrl_pressed = is_ctrl_pressed;

                if self.is_middle_button_pressed {
                    let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
                        let studio = self.scene.studio_config_mut();
                        (&mut studio.camera, &mut studio.lights)
                    };

                    if is_ctrl_pressed {
                        // --- Pan (Translate) ---
                        // Move both the camera eye and the pivot by the same vector.
                        let eye = Point3::from_vec(camera.matrix[3].truncate());
                        let dist = (eye - self.pivot).magnitude();

                        // Use fov_rad from projection method if possible, here using default PI/4
                        let fov_rad = std::f64::consts::PI / 4.0;
                        let window_height = 480.0; // Hardcoded default, can be fetched dynamically

                        let world_height_at_dist = 2.0 * dist * (fov_rad / 2.0).tan();
                        let unit_per_px = world_height_at_dist / window_height;

                        let right = camera.matrix[0].truncate().normalize();
                        let up = camera.matrix[1].truncate().normalize();

                        let translation_vec =
                            right * (-dir2d[0] * unit_per_px) + up * (dir2d[1] * unit_per_px);

                        let trans_mat = Matrix4::from_translation(translation_vec);
                        camera.matrix = trans_mat * camera.matrix;

                        // Re-orthogonalize to prevent drift
                        let r = camera.matrix[0].truncate().normalize();
                        let u = camera.matrix[1].truncate().normalize();
                        let b = r.cross(u).normalize();
                        camera.matrix[0] = r.extend(0.0);
                        camera.matrix[1] = u.extend(0.0);
                        camera.matrix[2] = b.extend(0.0);

                        lights[0].position = camera.position();
                        self.pivot += translation_vec;
                    } else {
                        // --- Rotate ---
                        let inv_mat = camera.matrix;

                        // Calculate sensitivity
                        let sensitivity = 0.015;
                        let angle_x = -dir2d[0] * sensitivity; // Left/Right mouse move -> Rotate around world Y
                        let angle_y = -dir2d[1] * sensitivity; // Up/Down mouse move -> Rotate around camera Right

                        let right = inv_mat[0].truncate().normalize();
                        let rot_y = Matrix4::from_axis_angle(right, Rad(angle_y));
                        let rot_x = Matrix4::from_axis_angle(Vector3::unit_y(), Rad(angle_x));

                        // Rotate camera around the pivot point
                        let transform = Matrix4::from_translation(self.pivot.to_vec())
                            * rot_x
                            * rot_y
                            * Matrix4::from_translation(-self.pivot.to_vec());

                        camera.matrix = transform * camera.matrix;

                        // Re-orthogonalize to prevent drift
                        let r = camera.matrix[0].truncate().normalize();
                        let u = camera.matrix[1].truncate().normalize();
                        let b = r.cross(u).normalize();
                        camera.matrix[0] = r.extend(0.0);
                        camera.matrix[1] = u.extend(0.0);
                        camera.matrix[2] = b.extend(0.0);

                        lights[0].position = camera.position();
                    }
                }

                self.last_mouse_pos = position;
            }
            Wheel { delta } => {
                let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
                    let studio = self.scene.studio_config_mut();
                    (&mut studio.camera, &mut studio.lights)
                };

                let inv_mat = camera.matrix;
                let eye = Point3::from_vec(inv_mat[3].truncate());
                let dist = (eye - self.pivot).magnitude();

                // Zoom amount proportional to distance
                let move_amount = dist * 0.05 * (delta as f64 / 35.0); // Assuming typical delta is 120

                // Prevent moving past the pivot
                if dist - move_amount > 0.1 {
                    let trans = Matrix4::from_translation(camera.eye_direction() * move_amount);
                    camera.matrix = trans * camera.matrix;
                }

                lights[0].position = camera.position();
            }
            MiddleClickUp => {
                self.is_middle_button_pressed = false;
            }
            MiddleClickDown { x, y } => {
                self.is_middle_button_pressed = true;
                self.last_mouse_pos = Vector2::new(x as f64, y as f64);
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

        if self.ctrl_pressed && self.pressed_keys.contains("1") {
            self.look_at_origin(
                Point3::new(5.0, 0.0, 0.0),
                Point3::new(0.0, 0.0, 0.0),
                Vector3::unit_y(),
            );
        }
    }
}
