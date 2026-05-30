use crate::MyTruckRenderer;
use crate::*;
use messages::{ControlMessage, InputMessage};
use slint::Weak;

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
        on_mouse_moved(x, y) => AppMessage::InputMsg { msg: InputMessage::MouseMove { x, y } },
        on_wheel_scrolled(delta) => AppMessage::InputMsg { msg: InputMessage::Wheel { delta } },
        on_middle_click_down(x, y) => AppMessage::InputMsg { msg: InputMessage::MiddleClickDown { x, y } },
        on_middle_click_up => AppMessage::InputMsg { msg: InputMessage::MiddleClickUp },
        on_key_event_pressed_received(key_pressed, modifiers) => AppMessage::InputMsg { msg: InputMessage::KeyEventPressed { key: String::from(&key_pressed), modifiers } },
        on_key_event_released_received(key_released, modifiers) => AppMessage::InputMsg { msg: InputMessage::KeyEventReleased { key: String::from(&key_released), modifiers } },
    });
}

impl MyTruckRenderer<AppMessage> {
    pub fn handle_baseops_controls(&mut self, input_msg: InputMessage) {
        use InputMessage::*;
        match input_msg {
            Click { x, y } => {
                println!("Clicked at ({}, {})", x, y);
            }
            MouseMove { x, y } => {
                if self.is_middle_button_pressed && self.pressed_keys.contains("\u{0011}") {
                    // --- 並進移動（Panning） ---

                    let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
                        let studio = self.scene.studio_config_mut();

                        (&mut studio.camera, &mut studio.lights)
                    };

                    let position = Vector2::new(x as f64, y as f64);
                    let dir2d = position - self.last_mouse_pos;

                    // 1. 現在の距離を正確に把握
                    let eye = Point3::from_vec(camera.matrix[3].truncate());
                    let dist = (eye - self.pivot).magnitude();

                    // 2. 幾何学的に正確な感度(sensitivity)を計算
                    // FOVは camera.method から取得可能（Rad(PI / 4.5) 等）
                    let fov_rad = std::f64::consts::PI / 4.5; // 初期化時の値
                    let window_height = 640.0; // 本来はSlintから動的に取得するのがベスト

                    // 「その距離における画面の高さ（世界単位）」を計算
                    let world_height_at_dist = 2.0 * dist * (fov_rad / 2.0).tan();
                    let unit_per_px = world_height_at_dist / window_height;

                    // 3. 移動ベクトルの算出
                    let right = camera.matrix[0].truncate().normalize();
                    let up = camera.matrix[1].truncate().normalize();

                    // unit_per_px を掛けることで、マウスの1px移動が画面上の1px移動と一致する
                    let translation_vec = right * (-dir2d[0] * unit_per_px) * 0.15
                        + up * (dir2d[1] * unit_per_px) * 0.15;

                    // 4. 反映
                    let trans_mat = Matrix4::from_translation(translation_vec);
                    camera.matrix = trans_mat * camera.matrix;
                    // 回転処理の最後に入れて、行列の直交性を保つ（浮動小数点のゴミを掃除する）
                    let r = camera.matrix[0].truncate().normalize();
                    let u = camera.matrix[1].truncate().normalize();
                    let b = r.cross(u).normalize();
                    camera.matrix[0] = r.extend(0.0);
                    camera.matrix[1] = u.extend(0.0);
                    camera.matrix[2] = b.extend(0.0);

                    lights[0].position = camera.position();

                    self.pivot += translation_vec;
                }
                if self.is_middle_button_pressed && !self.pressed_keys.contains("\u{0011}") {
                    let position = Vector2::new(x as f64, y as f64);
                    let dir2d = position - self.last_mouse_pos;

                    if dir2d.so_small() {
                        return;
                    }

                    let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
                        let studio = self.scene.studio_config_mut();

                        (&mut studio.camera, &mut studio.lights)
                    };
                    // 2. 現在のカメラの位置を取得
                    let inv_mat = camera.matrix;
                    let eye = Point3::from_vec(inv_mat[3].truncate());

                    // 3. マウスの移動量から回転角を計算 (感度は適宜調整)
                    // 左右の動きは Y軸回転、上下の動きはカメラの横軸回転
                    let sensitivity = 0.005;
                    let angle_x = -dir2d[0] * sensitivity; // 左右
                    let angle_y = -dir2d[1] * sensitivity; // 上下

                    // 4. 回転行列の構築
                    // 垂直方向の回転（カメラの右方向ベクトル軸）
                    let right = inv_mat[0].truncate().normalize();
                    let rot_y = Matrix4::from_axis_angle(right, Rad(angle_y));

                    // 水平方向の回転（世界の垂直軸軸 - 通常は UnitY）
                    let rot_x = Matrix4::from_axis_angle(Vector3::unit_y(), Rad(angle_x));

                    // 5. 行列の更新：Pivotを中心に回転させる
                    // カメラ座標 = T(pivot) * R_world_y * R_camera_right * T(-pivot) * 現在の行列
                    let transform = Matrix4::from_translation(self.pivot.to_vec())
                        * rot_x
                        * rot_y
                        * Matrix4::from_translation(-self.pivot.to_vec());

                    camera.matrix = transform * camera.matrix;
                    lights[0].position = camera.position();

                    self.last_mouse_pos = position;
                }
            }
            Wheel { delta } => {
                // let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
                //     // StudioConfigのmutableな参照を受け取る
                //     let studio = self.scene.studio_config_mut();
                //     // カメラとライトのmutableな参照
                //     (&mut studio.camera, &mut studio.lights)
                // };
                let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
                    let studio = self.scene.studio_config_mut();

                    (&mut studio.camera, &mut studio.lights)
                };

                // 1. 回転の中心（Pivot）を設定。ティーポットが原点なら (0,0,0)
                let pivot = Point3::new(0.0, 1.5, 0.0);

                // 2. 現在のカメラの位置を取得
                let inv_mat = camera.matrix;
                let eye = Point3::from_vec(inv_mat[3].truncate());
                // Wheelの部分
                let dist = (eye - pivot).magnitude();
                let move_amount = 0.03 * delta as f64;
                // 近づきすぎ防止
                if dist - move_amount > 0.5 {
                    let trans = Matrix4::from_translation(camera.eye_direction() * move_amount);
                    camera.matrix = trans * camera.matrix;
                }

                lights[0].position = camera.position();
            }
            MiddleClickUp => {
                self.is_middle_button_pressed = false;
                println!("Middle button released");
            }
            MiddleClickDown { x, y } => {
                self.is_middle_button_pressed = true;
                self.last_mouse_pos = Vector2::new(x as f64, y as f64);
                println!("Middle button clicked at ({}, {})", x, y);
            }
            KeyEventPressed { key, modifiers } => {
                self.pressed_keys.insert(key.clone());
            }
            KeyEventReleased { key, modifiers } => {
                self.pressed_keys.remove(&key);
            }
        }

        // when both ctrl + 1 is pressed, reset the camera position to look at xy plane from (5, 0,
        // 0)
        if self.pressed_keys.contains("\u{0011}") && self.pressed_keys.contains("1") {
            self.look_at_origin(
                Point3::new(5.0, 0.0, 0.0),
                Point3::new(0.0, 0.0, 0.0),
                Vector3::unit_y(),
            );
        }
    }

    pub fn look_at_origin(&mut self, from: Point3, to: Point3, dir: Vector3) {
        let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
            let studio = self.scene.studio_config_mut();

            (&mut studio.camera, &mut studio.lights)
        };
        camera.matrix = Matrix4::look_at_rh(from, to, dir);
        lights[0].position = camera.position();
    }
}
