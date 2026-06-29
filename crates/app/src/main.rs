// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms. #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod base_ops;
mod camera;
mod extrusion;
mod messages;
mod msg_macro;
mod scene;
mod selection;
mod sketch;
mod toolbar; 

use base_ops::{InputEventHandler, handle_baseops_controls, setup_baseops_controls};
use cad_core::{AppConfig, CONFIG, CadData, CamLightConfig, READ_CONFIG};
use cad_renderer::*;
use messages::AppMessage;
use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use toolbar::setup_toolbar_controls;
use truck_meshalgo::prelude::*;
use truck_modeling::{Edge, Face, Solid, Vertex, builder};
use truck_platform::wgpu::{self};
use truck_platform::*;

slint::include_modules!();

struct CameraLightLayout {
    cam_orit: Matrix4,
    light_pos: Point3,
    pivot: Point3,
}

impl CameraLightLayout {
    fn cam_pos(&self) -> Point3 {
        Point3::from_vec(self.cam_orit[3].truncate())
    }

    fn sync_light_with_camera(&mut self) {
        self.light_pos = self.cam_pos();
    }
}

struct CadScene {
    input_handler: InputEventHandler,
    window_size: (u32, u32),
    cam_light_layout: CameraLightLayout,
}

impl CadScene {
    fn new(input_handler: InputEventHandler, cam_light_layout: CameraLightLayout) -> Self {
        let glob_conf = READ_CONFIG();

        Self {
            input_handler,
            window_size: (glob_conf.window_width, glob_conf.window_height),
            cam_light_layout,
        }
    }

    fn pan_scene(&mut self, dir2d: Vector2) {
        self.input_handler.last_mouse_pos += dir2d;

        // Move both the camera eye and the pivot by the same vector.
        let eye = self.cam_light_layout.cam_pos();
        let dist = (eye - self.cam_light_layout.pivot).magnitude();

        // Use fov_rad from projection method if possible, here using default PI/4
        let fov_rad = std::f64::consts::PI / 4.0;
        let window_height: f64 = self.window_size.1.try_into().unwrap();

        let world_height_at_dist = 2.0 * dist * (fov_rad / 2.0).tan();
        let unit_per_px = world_height_at_dist / window_height;

        let right = self.cam_light_layout.cam_orit[0].truncate().normalize();
        let up = self.cam_light_layout.cam_orit[1].truncate().normalize();

        let translation_vec = right * (-dir2d[0] * unit_per_px) + up * (dir2d[1] * unit_per_px);
        let trans_mat = Matrix4::from_translation(translation_vec);
        self.cam_light_layout.cam_orit = trans_mat * self.cam_light_layout.cam_orit;

        // Re-orthogonalize to prevent drift
        let r = self.cam_light_layout.cam_orit[0].truncate().normalize();
        let u = self.cam_light_layout.cam_orit[1].truncate().normalize();
        let b = r.cross(u).normalize();
        self.cam_light_layout.cam_orit[0] = r.extend(0.0);
        self.cam_light_layout.cam_orit[1] = u.extend(0.0);
        self.cam_light_layout.cam_orit[2] = b.extend(0.0);

        self.cam_light_layout.sync_light_with_camera();
        self.cam_light_layout.pivot += translation_vec;
    }

    pub fn rot_camera(&mut self, dir2d: Vector2) {
        let config = READ_CONFIG();
        // Calculate sensitivity
        let sensitivity = config.cam_light_config.rot_sensitivity;
        let angle_x = -dir2d[0] * sensitivity; // Left/Right mouse move -> Rotate around world Y
        let angle_y = -dir2d[1] * sensitivity; // Up/Down mouse move -> Rotate around camera Right

        let inv_mat = self.cam_light_layout.cam_orit;

        let right = inv_mat[0].truncate().normalize();
        let rot_y = Matrix4::from_axis_angle(right, Rad(angle_y));
        let rot_x = Matrix4::from_axis_angle(Vector3::unit_y(), Rad(angle_x));

        // Rotate camera around the pivot point
        let transform = Matrix4::from_translation(self.cam_light_layout.pivot.to_vec())
            * rot_x
            * rot_y
            * Matrix4::from_translation(-self.cam_light_layout.pivot.to_vec());

        self.cam_light_layout.cam_orit = transform * self.cam_light_layout.cam_orit;

        // Re-orthogonalize to prevent drift
        let r = self.cam_light_layout.cam_orit[0].truncate().normalize();
        let u = self.cam_light_layout.cam_orit[1].truncate().normalize();
        let b = r.cross(u).normalize();
        self.cam_light_layout.cam_orit[0] = r.extend(0.0);
        self.cam_light_layout.cam_orit[1] = u.extend(0.0);
        self.cam_light_layout.cam_orit[2] = b.extend(0.0);

        self.cam_light_layout.sync_light_with_camera();
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

impl EventHandler<AppMessage> for CadScene {
    fn handle_event(&mut self, event: AppMessage) -> bool {
        match event {
            AppMessage::InputMsg(msg) => {
                self.input_handler.handle_baseops_controls(msg);
                true
            }
            _ => false,
        }
    }

    fn update_renderer(&self, _renderer: &mut dyn TruckRenderer) {}
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig {
        window_width: 640,
        window_height: 480,
        debug_mode: false,
        cam_light_config: CamLightConfig {
            init_cam_pos: Point3::new(4.5, 4.5, 4.5),
            base_axis: Vector3::unit_y(),
            cam_perspective: ProjectionMethod::Perspective { fov: Rad(PI / 4.0) },
            near_clip: 0.1,
            far_clip: 40.0,
            background_color: wgpu::Color {
                r: 230.0 / 256.0,
                g: 230.0 / 256.0,
                b: 230.0 / 256.0,
                a: 0.8,
            },
            pan_sensitivity: 0.005,
            rot_sensitivity: 0.005,
        },
    };

    // initialize the app config
    CONFIG
        .set(Arc::new(RwLock::new(config)))
        .ok()
        .expect("Failed to init");

    // Slint initializes first with wgpu — on linuxkms it creates its own wgpu instance
    // with DRM surface support. The wgpu resources are then passed to Bevy via the
    // rendering notifier callback.
    let mut wgpu_settings = slint::wgpu_27::WGPUSettings::default();
    wgpu_settings.device_required_limits = slint::wgpu_27::wgpu::Limits::default()
        .using_resolution(slint::wgpu_27::wgpu::Limits::downlevel_defaults());
    slint::BackendSelector::new()
        .require_wgpu_27(slint::wgpu_27::WGPUConfiguration::Automatic(wgpu_settings))
        .select()?;
    let ui = AppWindow::new().unwrap();
    // These will be filled once Truck is initialized from the rendering notifier.
    let truck_channels: Rc<
        RefCell<
            Option<(
                smol::channel::Receiver<slint::wgpu_27::wgpu::Texture>,
                smol::channel::Sender<ControlMessage<AppMessage>>,
            )>,
        >,
    > = Rc::new(RefCell::new(None));

    let app_weak = ui.as_weak();
    let truck_channels_setup = truck_channels.clone();

    ui.window()
        .set_rendering_notifier(move |state, graphics_api| {
            match state {
                slint::RenderingState::RenderingSetup => {
                    // Extract wgpu resources from Slint and initialize truck
                    let slint::GraphicsAPI::WGPU27 {
                        instance,
                        device,
                        queue,
                        ..
                    } = graphics_api
                    else {
                        return;
                    };

                    let adapter =
                        smol::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                            #[cfg(not(feature = "webgl"))]
                            power_preference: wgpu::PowerPreference::HighPerformance,
                            #[cfg(feature = "webgl")]
                            power_preference: PowerPreference::LowPower,
                            compatible_surface: None,
                            force_fallback_adapter: false,
                        }))
                        .expect("Failed to find an appropriate adapter");

                    let device_handler = DeviceHandler::new(adapter, device.clone(), queue.clone());

                    let vertex: Vertex = builder::vertex(Point3::new(-1.0, 0.0, -1.0));
                    let edge: Edge = builder::tsweep(&vertex, 2.0 * Vector3::unit_z());
                    let face: Face = builder::tsweep(&edge, 2.0 * Vector3::unit_x());
                    let cube: Solid = builder::tsweep(&face, 2.0 * Vector3::unit_y());

                    let cad_data = Arc::new(Mutex::new(CadData::new(cube)));

                    let renderer = SceneRenderer::new(device_handler.clone(), cad_data);
                    let cad_scene = CadScene::new(
                        InputEventHandler::default(),
                        CameraLightLayout {
                            cam_orit: Matrix4::identity(),
                            light_pos: renderer_conf.cam_pos,
                            pivot: Point3::new(0.0, 0.0, 0.0),
                        },
                    );

                    let glob_conf = READ_CONFIG();

                    let channels = run_truck_kernel_with_slint(
                        device_handler.clone(),
                        Box::new(renderer),
                        Box::new(cad_scene),
                        (glob_conf.window_width, glob_conf.window_height),
                    );

                    *truck_channels_setup.borrow_mut() = Some(channels);

                    if let Some((_, control_message_sender)) =
                        truck_channels_setup.borrow().as_ref()
                    {
                        setup_toolbar_controls(app_weak.clone(), control_message_sender.clone());
                        setup_baseops_controls(app_weak.clone(), control_message_sender.clone());
                    };
                }
                slint::RenderingState::BeforeRendering => {
                    let Some(app) = app_weak.upgrade() else {
                        return;
                    };

                    let channels = truck_channels_setup.borrow();
                    let Some((new_texture_receiver, control_message_sender)) = channels.as_ref()
                    else {
                        return;
                    };

                    if let Ok(new_texture) = new_texture_receiver.try_recv() {
                        if let Some(old_texture) = app.get_texture().to_wgpu_27_texture() {
                            let _ = control_message_sender.try_send(
                                ControlMessage::ReleaseFrontBufferTexture {
                                    texture: old_texture,
                                },
                            );
                        }
                        if let Ok(image) = new_texture.try_into() {
                            app.set_texture(image);
                        }

                        app.window().request_redraw();
                    } else {
                        app.window().request_redraw();
                        return;
                    }

                    let requested_width = app.get_requested_texture_width().round() as u32;
                    let requested_height = app.get_requested_texture_height().round() as u32;
                    if requested_width > 0 && requested_height > 0 {
                        let control_message_sender = control_message_sender.clone();
                        slint::spawn_local(async move {
                            control_message_sender
                                .send(ControlMessage::ResizeBuffers {
                                    width: requested_width,
                                    height: requested_height,
                                })
                                .await
                                .unwrap();
                        })
                        .unwrap();
                    }
                }
                _ => {}
            }
        })?;

    let _ = ui.run();

    Ok(())
}
