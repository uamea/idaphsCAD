use crate::UserIOUtils;
use crate::messages::{AppMessage, InputMessage};
use crate::tool::{CadTool, ToolResult};
use cad_core::*;
use std::sync::Arc;
use truck_meshalgo::prelude::*;
use truck_platform::{Camera, ProjectionMethod};

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

            let mut cam_orit_new = cam_orit_initial;
            cam_orit_new[3] += translation_vec.extend(0.0);

            // Re-orthogonalize just the rotation components to prevent drift
            let r = cam_orit_new[0].truncate().normalize();
            let u = cam_orit_new[1].truncate().normalize();
            let b = r.cross(u).normalize();

            let cam_orit_new_normalized =
                Matrix4::from_cols(r.extend(0.0), u.extend(0.0), b.extend(0.0), cam_orit_new[3]);

            cam_light_layout_write.cam_orit = cam_orit_new_normalized;

            // Sync the light and update the final matrix
            cam_light_layout_write.sync_light_with_camera();
        }
    }
    pub fn rot_camera(&mut self, dir2d: Vector2) {
        let (pivot, cam_orit_initial) = {
            let light_cam_layout = self.ctx.read_cam_light_layout();
            (light_cam_layout.pivot, light_cam_layout.cam_orit)
        };

        // Calculate sensitivity
        let sensitivity = self.read_config().cam_light_config.rot_sensitivity;
        let angle_x = -dir2d[0] * sensitivity; // Left/Right mouse move -> Rotate around world Y
        let angle_y = -dir2d[1] * sensitivity; // Up/Down mouse move -> Rotate around camera Right

        let right = cam_orit_initial[0].truncate().normalize();
        let rot_y = Matrix4::from_axis_angle(right, Rad(angle_y));
        let rot_x = Matrix4::from_axis_angle(Vector3::unit_y(), Rad(angle_x));

        // Rotate camera around the pivot point
        let transform = Matrix4::from_translation(pivot.to_vec())
            * rot_x
            * rot_y
            * Matrix4::from_translation(-pivot.to_vec());

        let cam_orit_new = transform * cam_orit_initial;

        let r = cam_orit_new[0].truncate().normalize();
        let u = cam_orit_new[1].truncate().normalize();
        let b = r.cross(u).normalize();

        let cam_orit_new_normalized =
            Matrix4::from_cols(r.extend(0.0), u.extend(0.0), b.extend(0.0), cam_orit_new[3]);

        {
            let cam_light_layout_write = &mut self.ctx.write_cam_light_layout();

            cam_light_layout_write.cam_orit = cam_orit_new_normalized;

            // Sync the light and update the final matrix
            cam_light_layout_write.sync_light_with_camera();
        }
    }

    pub fn expand_camera(&mut self, delta: f32) {
        let (perspective, cam_orit_initial, pivot) = {
            let cam_light_layout = self.ctx.read_cam_light_layout();
            (
                cam_light_layout.perspective,
                cam_light_layout.cam_orit,
                cam_light_layout.pivot,
            )
        };

        let mut next_perspective = perspective;
        let mut next_cam_orit = cam_orit_initial;

        let zoom_factor = 1.0 + 0.05 * (delta as f64 / 35.0);

        match perspective {
            ProjectionMethod::Parallel { screen_size } => {
                let new_screen_size = screen_size * zoom_factor;
                next_perspective = ProjectionMethod::Parallel {
                    screen_size: new_screen_size,
                };
            }
            ProjectionMethod::Perspective { fov: _fov } => {
                let eye = Point3::from_vec(cam_orit_initial[3].truncate());
                let forward = cam_orit_initial[2].truncate().normalize(); // 前方ベクトル
                let dist = (eye - pivot).magnitude();

                let move_amount = dist * 0.05 * (delta as f64 / 35.0);

                if dist - move_amount > 0.1 {
                    let translation_vec = forward * move_amount;

                    let mut cam_orit_new = cam_orit_initial;
                    cam_orit_new[3] += translation_vec.extend(0.0);

                    let r = cam_orit_new[0].truncate().normalize();
                    let u = cam_orit_new[1].truncate().normalize();
                    let b = r.cross(u).normalize();

                    next_cam_orit = Matrix4::from_cols(
                        r.extend(0.0),
                        u.extend(0.0),
                        b.extend(0.0),
                        cam_orit_new[3],
                    );
                }
            }
        }

        {
            let cam_light_layout_write = &mut self.ctx.write_cam_light_layout();

            cam_light_layout_write.perspective = next_perspective;
            cam_light_layout_write.cam_orit = next_cam_orit;

            // ライトの位置等をカメラに同期
            cam_light_layout_write.sync_light_with_camera();
        }
    }

    pub fn handle_selection(&mut self, x: f64, y: f64) {
        let conf = self.read_config();
        // Basic screen dimensions (should ideally be queried dynamically)
        let window_width = conf.window_width as f64;
        let window_height = conf.window_height as f64;

        // Convert screen coordinates to Normalized Device Coordinates (NDC)
        let ndc_x = (2.0 * x / window_width) - 1.0;
        let ndc_y = 1.0 - (2.0 * y / window_height); // Y is flipped in NDC

        let cam_light_layout = self.ctx.read_cam_light_layout();

        // Calculate the projection matrix
        let proj_mat = cam_light_layout.projection(window_width / window_height);
        let inv_proj_mat = proj_mat.invert().unwrap();

        // The camera's matrix transforms from local to world space.
        let camera_to_world = cam_light_layout.cam_orit;
        // 2. カメラの「位置」と「各向きのベクトル」を行列から直接抽出する
        // 列優先(Column-Major)の場合、x列がRight、y列がUp、z列がForward(の逆)、w列がPositionです
        let ray_origin = Point3::new(
            camera_to_world.w.x,
            camera_to_world.w.y,
            camera_to_world.w.z,
        );

        let cam_right = Vector3::new(
            camera_to_world.x.x,
            camera_to_world.x.y,
            camera_to_world.x.z,
        );
        let cam_up = Vector3::new(
            camera_to_world.y.x,
            camera_to_world.y.y,
            camera_to_world.y.z,
        );
        let cam_forward = Vector3::new(
            camera_to_world.z.x,
            camera_to_world.z.y,
            camera_to_world.z.z,
        );

        // 3. 視野角(FOV)を考慮したスケール係数を計算
        // カメラオブジェクトから fov (ラジアン) が取得できると想定しています。
        // もし直接取得できない場合は、通常使われる 45度(およそ 0.785) などを仮で入れてみてください。
        let fov_radians = 45.0f64.to_radians(); // もし camera.fov があればそれに置き換えてください
        let aspect_ratio = window_width / window_height;

        let tan_half_fov = (fov_radians * 0.5).tan();

        // 4. NDC座標とカメラの軸をブレンドして、ワールド空間の方向ベクトルを計算
        // ※ 多くのライブラリでカメラの正面は「-cam_forward」方向になります。
        let ray_dir = (cam_right * (ndc_x * aspect_ratio * tan_half_fov)
            + cam_up * (ndc_y * tan_half_fov)
            - cam_forward)
            .normalize();

        println!(
            "ray_origin: {}, {}, {}",
            ray_origin.x, ray_origin.y, ray_origin.z
        );
        println!("ray_dir: {}, {}, {}", ray_dir.x, ray_dir.y, ray_dir.z);
        // Check intersection with all meshes
        let mut closest_dist = f64::MAX;
        let mut selected_face = None;
        let mut selected_edge = None;

        let mut cad_data = self.ctx.write_model();

        // 1. Ray-Triangle intersection for Faces
        for (face_id, mesh) in &cad_data.face_meshes {
            let positions = mesh.positions();
            for tri in mesh.faces().triangle_iter() {
                let v0 = positions[tri[0].pos];
                let v1 = positions[tri[1].pos];
                let v2 = positions[tri[2].pos];

                if let Some(t) = ray_triangle_intersect(ray_origin, ray_dir, v0, v1, v2) {
                    println!("{}", t);
                    if t > 0.0 && t < closest_dist {
                        closest_dist = t;
                        selected_face = Some(*face_id);
                        selected_edge = None; // Face is closer or found
                    }
                }
            }
        }

        // 2. Ray-Line intersection for Edges
        let edge_threshold = 0.05; // 5% of unit or distance threshold
        for (edge_id, points) in &cad_data.edge_meshes {
            if points.len() >= 2 {
                let p0 = points[0];
                let p1 = points[1];

                // Distance between ray (line) and edge (line segment)
                if let Some((dist, t_ray)) = ray_segment_distance(ray_origin, ray_dir, p0, p1) {
                    if dist < edge_threshold && t_ray > 0.0 && t_ray < closest_dist {
                        closest_dist = t_ray;
                        selected_edge = Some(*edge_id);
                        selected_face = None; // Edge is closer
                    }
                }
            }
        }

        // Update selection state
        cad_data.selection.selected_faces.clear();
        cad_data.selection.selected_edges.clear();
        cad_data.selection.selected_vertices.clear();

        if let Some(edge_id) = selected_edge {
            println!("Selected Edge: {:?}", edge_id);
            cad_data.selection.selected_edges.push(edge_id);
        } else if let Some(face_id) = selected_face {
            println!("Selected Face: {:?}", face_id);
            cad_data.selection.selected_faces.push(face_id);
        } else {
            println!("Selected Nothing");
        }
    }
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
                        if user_io_utils.is_ctrl_pressed {
                            self.pan_scene(dir2d);
                        } else {
                            self.rot_camera(dir2d);
                        }
                    }
                }
                InputMessage::Wheel { delta } => {
                    self.expand_camera(delta);
                }
                InputMessage::Click { x, y } => {
                    self.handle_selection(x as f64, y as f64);
                }
                _ => {}
            },
            _ => {}
        }
        ToolResult::Continue(true)
    }
}

// Möller–Trumbore intersection algorithm
fn ray_triangle_intersect(
    orig: Point3,
    dir: Vector3,
    v0: Point3,
    v1: Point3,
    v2: Point3,
) -> Option<f64> {
    const EPSILON: f64 = 0.0000001;
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = dir.cross(edge2);
    let a = edge1.dot(h);

    if a > -EPSILON && a < EPSILON {
        return None; // This ray is parallel to this triangle.
    }

    let f = 1.0 / a;
    let s = orig - v0;
    let u = f * s.dot(h);

    if u < 0.0 || u > 1.0 {
        return None;
    }

    let q = s.cross(edge1);
    let v = f * dir.dot(q);

    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * edge2.dot(q);
    if t > EPSILON { Some(t) } else { None }
}

// Distance between a ray and a line segment
// Returns (distance, ray_t)
fn ray_segment_distance(
    ray_origin: Point3,
    ray_dir: Vector3,
    seg_p0: Point3,
    seg_p1: Point3,
) -> Option<(f64, f64)> {
    let u = ray_dir;
    let v = seg_p1 - seg_p0;
    let w = ray_origin - seg_p0;

    let a = u.dot(u); // Always 1.0 if ray_dir is normalized
    let b = u.dot(v);
    let c = v.dot(v);
    let d = u.dot(w);
    let e = v.dot(w);
    let denominator = a * c - b * b;

    let sc;
    let tc;

    if denominator < 1e-7 {
        // The lines are almost parallel
        sc = 0.0;
        tc = if b > c { d / b } else { e / c };
    } else {
        sc = (b * e - c * d) / denominator;
        tc = (a * e - b * d) / denominator;
    }

    // tc is the parameter for the segment. Clamp it to [0, 1]
    let tc_clamped = tc.clamp(0.0, 1.0);

    // Recompute sc for the clamped tc if it was clamped
    let sc_final = if tc == tc_clamped {
        sc
    } else {
        (tc_clamped * b - d) / a
    };

    let pt_on_ray = ray_origin + u * sc_final;
    let pt_on_seg = seg_p0 + v * tc_clamped;

    Some(((pt_on_ray - pt_on_seg).magnitude(), sc_final))
}
