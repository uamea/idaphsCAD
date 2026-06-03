use crate::*;
use renderer::SceneRenderer;
use truck_meshalgo::prelude::*;
use truck_platform::{Camera, Light, ProjectionMethod};
use truck_rendimpl::*;

impl<T> SceneRenderer<T>
where
    T: Send + Clone + 'static,
{
    fn pan_camera(&mut self, dir2d: Vector2) {
        // Move both the camera eye and the pivot by the same vector.

        let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
            let studio = self.scene.studio_config_mut();
            (&mut studio.camera, &mut studio.lights)
        };
        let eye = Point3::from_vec(camera.matrix[3].truncate());
        let dist = (eye - self.pivot).magnitude();

        let fov_rad = std::f64::consts::PI / 4.0;
        let window_height: f64 = self.conf.rendering_size.1.try_into().unwrap();

        let world_height_at_dist = 2.0 * dist * (fov_rad / 2.0).tan();
        let unit_per_px = world_height_at_dist / window_height;

        let right = camera.matrix[0].truncate().normalize();
        let up = camera.matrix[1].truncate().normalize();

        let translation_vec = right * (-dir2d[0] * unit_per_px) + up * (dir2d[1] * unit_per_px);

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
    }

    fn rot_camera(&mut self, dir2d: Vector2) {
        let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
            let studio = self.scene.studio_config_mut();
            (&mut studio.camera, &mut studio.lights)
        };
        let inv_mat = camera.matrix;

        // Calculate sensitivity
        let sensitivity = self.conf.rot_sensitivity;
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

    fn expand_camera(&mut self, delta: f32) {
        let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
            let studio = self.scene.studio_config_mut();
            (&mut studio.camera, &mut studio.lights)
        };

        match self.conf.cam_perspective {
            ProjectionMethod::Parallel {
                screen_size: _screen_size,
            } => {
                if let ProjectionMethod::Parallel { screen_size } = camera.method {
                    let zoom_amount = 1.0 + 0.05 * (delta as f64 / 35.0);
                    let new_screen_size = screen_size * zoom_amount;
                    camera.method = ProjectionMethod::Parallel {
                        screen_size: new_screen_size,
                    };
                }
            }
            ProjectionMethod::Perspective { fov: _fov } => {
                let inv_mat = camera.matrix;
                let eye = Point3::from_vec(inv_mat[3].truncate());
                let dist = (eye - self.pivot).magnitude();

                let move_amount = dist * 0.05 * (delta as f64 / 35.0);

                if dist - move_amount > 0.1 {
                    let trans = Matrix4::from_translation(camera.eye_direction() * move_amount);
                    camera.matrix = trans * camera.matrix;
                }
            }
        }

        if !lights.is_empty() {
            lights[0].position = camera.position();
        }
    }
}
