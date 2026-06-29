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

impl SceneEventHandler {
    fn pan_scene(&mut self, dir2d: Vector2) {
        let light_cam_layout = self.ctx.read_cam_light_layout();

        // Move both the camera eye and the pivot by the same vector.
        let eye = light_cam_layout.cam_pos();
        let dist = (eye - light_cam_layout.pivot).magnitude();

        // Use fov_rad from projection method if possible, here using default PI/4
        let fov_rad = std::f64::consts::PI / 4.0;
        let window_height: f64 = self.read_config().window_height.try_into().unwrap();

        let world_height_at_dist = 2.0 * dist * (fov_rad / 2.0).tan();
        let unit_per_px = world_height_at_dist / window_height;

        let right = light_cam_layout.cam_orit[0].truncate().normalize();
        let up = light_cam_layout.cam_orit[1].truncate().normalize();

        let translation_vec = right * (-dir2d[0] * unit_per_px) + up * (dir2d[1] * unit_per_px);
        let trans_mat = Matrix4::from_translation(translation_vec);
        self.ctx.write_cam_light_layout().cam_orit = trans_mat * light_cam_layout.cam_orit;

        // Re-orthogonalize to prevent drift
        let r = light_cam_layout.cam_orit[0].truncate().normalize();
        let u = light_cam_layout.cam_orit[1].truncate().normalize();
        let b = r.cross(u).normalize();
        let cam_light_layout_write = &mut self.ctx.write_cam_light_layout();
        cam_light_layout_write.cam_orit[0] = r.extend(0.0);
        cam_light_layout_write.cam_orit[1] = u.extend(0.0);
        cam_light_layout_write.cam_orit[2] = b.extend(0.0);

        cam_light_layout_write.sync_light_with_camera();
        cam_light_layout_write.pivot += translation_vec;
    }

    pub fn rot_camera(&mut self, dir2d: Vector2) {
        let light_cam_layout = self.ctx.read_cam_light_layout();
        // Calculate sensitivity
        let sensitivity = self.read_config().cam_light_config.rot_sensitivity;
        let angle_x = -dir2d[0] * sensitivity; // Left/Right mouse move -> Rotate around world Y
        let angle_y = -dir2d[1] * sensitivity; // Up/Down mouse move -> Rotate around camera Right

        let inv_mat = light_cam_layout.cam_orit;

        let right = inv_mat[0].truncate().normalize();
        let rot_y = Matrix4::from_axis_angle(right, Rad(angle_y));
        let rot_x = Matrix4::from_axis_angle(Vector3::unit_y(), Rad(angle_x));

        // Rotate camera around the pivot point
        let transform = Matrix4::from_translation(light_cam_layout.pivot.to_vec())
            * rot_x
            * rot_y
            * Matrix4::from_translation(-light_cam_layout.pivot.to_vec());

        self.ctx.write_cam_light_layout().cam_orit = transform * light_cam_layout.cam_orit;

        // Re-orthogonalize to prevent drift
        let r = light_cam_layout.cam_orit[0].truncate().normalize();
        let u = light_cam_layout.cam_orit[1].truncate().normalize();
        let b = r.cross(u).normalize();
        self.ctx.write_cam_light_layout().cam_orit[0] = r.extend(0.0);
        self.ctx.write_cam_light_layout().cam_orit[1] = u.extend(0.0);
        self.ctx.write_cam_light_layout().cam_orit[2] = b.extend(0.0);

        self.ctx.write_cam_light_layout().sync_light_with_camera();
    }

    // pub fn expand_camera(&mut self, delta: f32) {
    //     let config = READ_CONFIG();
    //
    //     match config.cam_light_config.cam_perspective {
    //         ProjectionMethod::Parallel {
    //             screen_size: _screen_size,
    //         } => {
    //             if let ProjectionMethod::Parallel { screen_size } = camera.method {
    //                 let zoom_amount = 1.0 + 0.05 * (delta as f64 / 35.0);
    //                 let new_screen_size = screen_size * zoom_amount;
    //                 camera.method = ProjectionMethod::Parallel {
    //                     screen_size: new_screen_size,
    //                 };
    //             }
    //         }
    //         ProjectionMethod::Perspective { fov: _fov } => {
    //             let inv_mat = camera.matrix;
    //             let eye = Point3::from_vec(inv_mat[3].truncate());
    //             let dist = (eye - self.pivot).magnitude();
    //
    //             let move_amount = dist * 0.05 * (delta as f64 / 35.0);
    //
    //             if dist - move_amount > 0.1 {
    //                 let trans = Matrix4::from_translation(camera.eye_direction() * move_amount);
    //                 camera.matrix = trans * camera.matrix;
    //             }
    //         }
    //     }
    //
    //     if !lights.is_empty() {
    //         lights[0].position = camera.position();
    //     }
    // }
}
