use crate::scene::SceneRenderer;
use truck_meshalgo::prelude::*;
use truck_modeling::{EdgeID, FaceID, VertexID};

pub fn handle_selection(renderer: &mut SceneRenderer, x: f64, y: f64) {
    // Basic screen dimensions (should ideally be queried dynamically)
    let window_width = 640.0;
    let window_height = 480.0;

    // Convert screen coordinates to Normalized Device Coordinates (NDC)
    let ndc_x = (2.0 * x / window_width) - 1.0;
    let ndc_y = 1.0 - (2.0 * y / window_height); // Y is flipped in NDC

    let (camera, _) = {
        let studio = renderer.scene.studio_config();
        (studio.camera.clone(), &studio.lights)
    };

    // Calculate the projection matrix
    let proj_mat = camera.projection(window_width / window_height);
    let inv_proj_mat = proj_mat.invert().unwrap();

    // The camera's matrix transforms from local to world space.
    let camera_to_world = camera.matrix;
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

    let mut cad_data = renderer.cad_data.lock().unwrap();

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
