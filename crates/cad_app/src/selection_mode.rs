use crate::UserIOUtils;
use crate::messages::{AppMessage, InputMessage};
use crate::tool::{CadTool, ToolResult};
use cad_core::*;
use std::sync::Arc;
use truck_meshalgo::prelude::*;
use truck_platform::ProjectionMethod;

pub struct SelectionTool {
    ctx: Arc<AppContext>,
}

impl SelectionTool {
    pub fn new(ctx: Arc<AppContext>) -> Self {
        Self { ctx }
    }

    #[inline]
    fn read_config(&self) -> Arc<AppConfig> {
        self.ctx.config.read()
    }
    fn pan_scene(&mut self, dir2d: Vector2) {
        let (eye, pivot, cam_orit_initial) = {
            let light_cam_layout = self.ctx.read_cam_light_layout();
            (
                light_cam_layout.cam_pos(),
                light_cam_layout.pivot,
                light_cam_layout.cam_orit,
            )
        };

        let dist = (eye - pivot).magnitude();

        let fov_rad = std::f64::consts::PI / 4.0;
        let window_height: f64 = self.read_config().window_height.try_into().unwrap();

        let world_height_at_dist = 2.0 * dist * (fov_rad / 2.0).tan();
        let unit_per_px = world_height_at_dist / window_height;

        let right = cam_orit_initial[0].truncate().normalize();
        let up = cam_orit_initial[1].truncate().normalize();

        // 1. Calculate the translation vector in world space
        let translation_vec = right * (-dir2d[0] * unit_per_px) + up * (dir2d[1] * unit_per_px);

        // 2. Open write scope to update layout directly
        {
            let cam_light_layout_write = &mut self.ctx.write_cam_light_layout();

            // Apply the translation to the pivot
            cam_light_layout_write.pivot += translation_vec;

            // NOTE: If your layout calculates `cam_pos` relative to `pivot` using orientation,
            // or if it has an explicit `position` / `translation` field, you need to move it here.
            // If `cam_pos` is derived from an offset, shifting the pivot is already enough.
            // If `cam_orit` holds the camera's translation in its 4th column, un-comment the below:
            // cam_light_layout_write.cam_orit[3] += translation_vec.extend(0.0);

            // Re-orthogonalize just the rotation components to prevent drift
            let r = cam_light_layout_write.cam_orit[0].truncate().normalize();
            let u = cam_light_layout_write.cam_orit[1].truncate().normalize();
            let b = r.cross(u).normalize();

            cam_light_layout_write.cam_orit[0] = r.extend(0.0);
            cam_light_layout_write.cam_orit[1] = u.extend(0.0);
            cam_light_layout_write.cam_orit[2] = b.extend(0.0);

            // Sync the light and update the final matrix
            cam_light_layout_write.sync_light_with_camera();
        }
    }
    // fn pan_scene(&mut self, dir2d: Vector2) {
    //     let (eye, pivot, cam_orit_initial) = {
    //         let light_cam_layout = self.ctx.read_cam_light_layout();
    //         (
    //             light_cam_layout.cam_pos(),
    //             light_cam_layout.pivot,
    //             light_cam_layout.cam_orit,
    //         )
    //     };
    //
    //     // Move both the camera eye and the pivot by the same vector.
    //     let dist = (eye - pivot).magnitude();
    //
    //     // Use fov_rad from projection method if possible, here using default PI/4
    //     let fov_rad = std::f64::consts::PI / 4.0;
    //     let window_height: f64 = self.read_config().window_height.try_into().unwrap();
    //
    //     let world_height_at_dist = 2.0 * dist * (fov_rad / 2.0).tan();
    //     let unit_per_px = world_height_at_dist / window_height;
    //
    //     let right = cam_orit_initial[0].truncate().normalize();
    //     let up = cam_orit_initial[1].truncate().normalize();
    //
    //     let translation_vec = right * (-dir2d[0] * unit_per_px) + up * (dir2d[1] * unit_per_px);
    //     let trans_mat = Matrix4::from_translation(translation_vec);
    //     let cam_orit_new = trans_mat * cam_orit_initial;
    //
    //     // Re-orthogonalize to prevent drift
    //     let r = cam_orit_new[0].truncate().normalize();
    //     let u = cam_orit_new[1].truncate().normalize();
    //     let b = r.cross(u).normalize();
    //
    //     // Open a separate write scope to avoid overlapping with any prior locks
    //     {
    //         let cam_light_layout_write = &mut self.ctx.write_cam_light_layout();
    //         cam_light_layout_write.cam_orit[0] = r.extend(0.0);
    //         cam_light_layout_write.cam_orit[1] = u.extend(0.0);
    //         cam_light_layout_write.cam_orit[2] = b.extend(0.0);
    //         println!(
    //             "new cam_orit = {:?}\n{:?}\n{:?}",
    //             r.extend(0.0),
    //             u.extend(0.0),
    //             b.extend(0.0)
    //         );
    //
    //         cam_light_layout_write.sync_light_with_camera();
    //         cam_light_layout_write.pivot += translation_vec;
    //     }
    // }

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
    //     let cam_light_config = self.read_config().cam_light_config;
    //     let cam_light_layout = self.ctx.read_cam_light_layout();
    //     match cam_light_config.cam_perspective {
    //         ProjectionMethod::Parallel {
    //             screen_size: _screen_size,
    //         } => {
    //             if let ProjectionMethod::Parallel { screen_size } =
    //                 self.ctx.read_cam_light_layout().perspective
    //             {
    //                 let zoom_amount = 1.0 + 0.05 * (delta as f64 / 35.0);
    //                 let new_screen_size = screen_size * zoom_amount;
    //                 self.ctx.write_cam_light_layout().perspective = ProjectionMethod::Parallel {
    //                     screen_size: new_screen_size,
    //                 };
    //             }
    //         }
    //         ProjectionMethod::Perspective { fov: _fov } => {
    //             let inv_mat = cam_light_layout.cam_orit;
    //             let eye = Point3::from_vec(inv_mat[3].truncate());
    //             let dist = (eye - cam_light_layout.pivot).magnitude();
    //
    //             let move_amount = dist * 0.05 * (delta as f64 / 35.0);
    //
    //             if dist - move_amount > 0.1 {
    //                 let trans = Matrix4::from_translation(camera.eye_direction() * move_amount);
    //                 self.ctx.write_cam_light_layout().cam_orit = trans * cam_light_layout.cam_orit;
    //             }
    //         }
    //     }
    //
    //     self.ctx.write_cam_light_layout().sync_light_with_camera();
    // }
}

impl CadTool for SelectionTool {
    fn handle_event(&mut self, event: AppMessage, user_io_utils: &mut UserIOUtils) -> ToolResult {
        match event {
            AppMessage::InputMsg { msg } => match msg {
                InputMessage::MouseMove { x, y, modifiers } => {
                    if user_io_utils.is_middle_mouse_down {
                        let (x, y) = user_io_utils.current_pos.unwrap_or((x as f64, y as f64));
                        let (start_x, start_y) = user_io_utils.prev_pos.unwrap_or((x, y));
                        let dir2d = Vector2::new(x - start_x, y - start_y);
                        self.pan_scene(dir2d);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        ToolResult::Continue(true)
    }
}
